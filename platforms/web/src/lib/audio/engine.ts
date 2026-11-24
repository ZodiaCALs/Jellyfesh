// The audio engine: the browser's own decoders and output, arranged like jf-engine's pipeline
// (docs/architecture.md §9):
//
//   <audio> A → MediaElementSource → ReplayGain → fade ┐
//   <audio> B → MediaElementSource → ReplayGain → fade ┴→ preamp → 10 × BiquadFilter (EQ)
//        → limiter (DynamicsCompressor at -1 dBFS, only when engaged; bypassed otherwise) → volume → output
//
// Two elements make joins and crossfades possible: the next track is loaded into the idle element
// ahead of time and started by the TransitionScheduler (./scheduler.ts), either gaplessly as the
// current one ends or with equal-power gain curves (from jf-web) for a crossfade. Manual skips
// never crossfade. Every number that decides something (curves, EQ bands, ReplayGain scale,
// limiter, whether to crossfade) comes from the core.
//
// Position: a PlaybackAnchor from the playing element's clock, minus the output latency, stamped
// with performance.now(); re-anchored on play, pause, seek, track change and buffering, and when
// the clock drifts more than 15 ms from the extrapolation (checked every 100 ms for 2 s after a
// start, then every second), as the core does.

import type { EqSettings } from '../core/types';
import { type PlaybackAnchor, positionAt } from '../drawing/displayClock';
import { type DeckState, type Plan, TransitionScheduler } from './scheduler';

export interface MediaSource {
  /** What the element loads: a same-origin stream URL, a blob: URL or a direct URL. */
  url: string;
  /** The URL is on another origin: load it with CORS (Web Audio needs the samples). */
  crossOrigin: boolean;
}

export interface EngineEvents {
  /** `uid` started playing (its first sample is out). */
  started(uid: number, durationMs: number | null): void;
  /** The engine moved on to the prepared next track by itself (gapless or crossfade). */
  advanced(from: number, to: number): void;
  /** `uid` ended with nothing prepared after it. */
  ended(uid: number): void;
  anchor(anchor: PlaybackAnchor): void;
  buffering(uid: number, buffering: boolean): void;
  /** `uid` could not be played; `unsupported` when the browser cannot decode it. */
  error(uid: number, unsupported: boolean, message: string): void;
  /** The current track's length became known. */
  duration(uid: number, durationMs: number): void;
  /** Whether two tracks overlap right now (the limiter engages during overlaps). */
  overlapping(overlap: boolean): void;
  /** The browser refused to start `uid` without a click or key press (autoplay policy). */
  blocked(uid: number): void;
}

interface Deck {
  el: HTMLAudioElement;
  source: MediaElementAudioSourceNode | null;
  replaygain: GainNode | null;
  fade: GainNode | null;
  uid: number | null;
  /** Ready to start at once (enough data). */
  ready: boolean;
  /** play() was called and the clock has not moved yet: when. */
  startedAt: number | null;
  announced: boolean;
}

const DRIFT_US = 15_000n;
const EQ_TYPES: Record<string, BiquadFilterType> = { Peaking: 'peaking', LowShelf: 'lowshelf', HighShelf: 'highshelf' };

function dbToLinear(db: number): number {
  return 10 ** (db / 20);
}

/** A transition the engine made by itself, for diagnostics and the end-to-end tests. */
export interface TransitionRecord {
  from: number;
  to: number;
  /** 0 for a gapless join. */
  fadeMs: number;
  /** The start latency the scheduler allowed for, ms. */
  leadMs: number;
  atMs: number;
}

export class AudioEngine {
  /** The last transitions (at most 20). */
  readonly transitions: TransitionRecord[] = [];
  private ctx: AudioContext | null = null;
  private decks: [Deck, Deck];
  private cur = 0;
  private generation = 0n;
  private plan: Plan | null = null;
  private scheduler = new TransitionScheduler();
  private timer: ReturnType<typeof setTimeout> | null = null;
  private driftTimer: ReturnType<typeof setTimeout> | null = null;
  private driftChecks = 0;
  private lastAnchor: PlaybackAnchor | null = null;
  private fadeEnd: ReturnType<typeof setTimeout> | null = null;
  private outgoing: Deck | null = null;
  /** An arm() that came while a crossfade still used the other element. */
  private pendingArm: { uid: number; media: MediaSource; gain: number; plan: Plan } | null = null;
  private volume = 1;
  private muted = false;
  private eq: EqSettings | null = null;
  private limiterOn = false;
  private nodes: {
    bus: GainNode;
    preamp: GainNode;
    filters: BiquadFilterNode[];
    limiter: DynamicsCompressorNode;
    wet: GainNode;
    dry: GainNode;
    master: GainNode;
  } | null = null;

  constructor(
    private readonly events: EngineEvents,
    private readonly curves: (points: number) => [Float32Array, Float32Array],
    private readonly limiter: { ceiling_db: number; attack_s: number; release_s: number },
  ) {
    this.decks = [this.makeDeck(), this.makeDeck()];
  }

  /** The output graph, for tests that measure what plays (gaps, levels). */
  get output(): { context: AudioContext; node: AudioNode } | null {
    return this.ctx && this.nodes ? { context: this.ctx, node: this.nodes.master } : null;
  }

  private makeDeck(): Deck {
    const el = new Audio();
    el.preload = 'auto';
    const deck: Deck = { el, source: null, replaygain: null, fade: null, uid: null, ready: false, startedAt: null, announced: false };
    el.addEventListener('canplaythrough', () => (deck.ready = true));
    el.addEventListener('canplay', () => (deck.ready = true));
    el.addEventListener('playing', () => this.onPlaying(deck));
    el.addEventListener('waiting', () => this.onWaiting(deck));
    el.addEventListener('pause', () => this.isCurrent(deck) && this.reanchor(false));
    el.addEventListener('seeked', () => this.isCurrent(deck) && this.reanchor(true));
    el.addEventListener('ratechange', () => this.isCurrent(deck) && this.reanchor(false));
    el.addEventListener('ended', () => this.onEnded(deck));
    el.addEventListener('error', () => this.onError(deck));
    el.addEventListener('durationchange', () => {
      if (deck.uid !== null && Number.isFinite(el.duration)) {
        this.events.duration(deck.uid, Math.round(el.duration * 1000));
        if (this.isCurrent(deck)) this.schedule();
      }
    });
    return deck;
  }

  private isCurrent(deck: Deck): boolean {
    return this.decks[this.cur] === deck;
  }

  /** Creates the AudioContext and graph (after a user gesture, for autoplay policies). */
  ensureContext(): AudioContext {
    if (this.ctx) {
      if (this.ctx.state === 'suspended') void this.ctx.resume();
      return this.ctx;
    }
    const ctx = new AudioContext({ latencyHint: 'playback' });
    this.ctx = ctx;
    const bus = ctx.createGain();
    const preamp = ctx.createGain();
    const filters = Array.from({ length: 10 }, () => ctx.createBiquadFilter());
    const limiter = ctx.createDynamicsCompressor();
    limiter.threshold.value = this.limiter.ceiling_db;
    limiter.knee.value = 0;
    limiter.ratio.value = 20;
    limiter.attack.value = this.limiter.attack_s;
    limiter.release.value = this.limiter.release_s;
    const wet = ctx.createGain();
    const dry = ctx.createGain();
    const master = ctx.createGain();
    bus.connect(preamp);
    let last: AudioNode = preamp;
    for (const filter of filters) {
      filter.type = 'peaking';
      filter.gain.value = 0;
      last.connect(filter);
      last = filter;
    }
    last.connect(limiter);
    limiter.connect(wet);
    last.connect(dry);
    wet.connect(master);
    dry.connect(master);
    master.connect(ctx.destination);
    wet.gain.value = 0;
    dry.gain.value = 1;
    this.nodes = { bus, preamp, filters, limiter, wet, dry, master };
    for (const deck of this.decks) {
      deck.source = ctx.createMediaElementSource(deck.el);
      deck.replaygain = ctx.createGain();
      deck.fade = ctx.createGain();
      deck.source.connect(deck.replaygain);
      deck.replaygain.connect(deck.fade);
      deck.fade.connect(bus);
    }
    this.applyVolume();
    if (this.eq) this.setEq(this.eq);
    this.setLimiter(this.limiterOn);
    return ctx;
  }

  // ---- loading and transport ------------------------------------------------------------------

  /** Plays `uid` now from `positionMs` (or loads it paused): a manual start, never a crossfade. */
  play(uid: number, media: MediaSource, gain: number, positionMs = 0, paused = false) {
    this.ensureContext();
    this.pendingArm = null;
    this.cancelFade();
    this.clearTimers();
    for (const deck of this.decks) this.reset(deck);
    const deck = this.decks[this.cur];
    this.load(deck, uid, media, gain);
    this.generation += 1n;
    if (positionMs > 0) deck.el.currentTime = positionMs / 1000;
    if (paused) {
      this.reanchor(false);
    } else {
      this.start(deck);
    }
  }

  /** Prepares `uid` to follow the current track (`plan`: how it joins). */
  arm(uid: number, media: MediaSource, gain: number, plan: Plan) {
    this.ensureContext();
    if (this.outgoing) {
      this.pendingArm = { uid, media, gain, plan };
      return;
    }
    const next = this.decks[1 - this.cur];
    if (next.uid !== uid || next.el.src !== new URL(media.url, document.baseURI).href) {
      this.reset(next);
      this.load(next, uid, media, gain);
    } else if (next.replaygain) {
      next.replaygain.gain.value = gain;
    }
    this.plan = plan;
    this.schedule();
  }

  /** Nothing follows the current track any more. */
  disarm() {
    this.plan = null;
    this.pendingArm = null;
    if (!this.outgoing) this.reset(this.decks[1 - this.cur]);
  }

  /** The uid prepared to follow, if any. */
  get armed(): number | null {
    return this.decks[1 - this.cur].uid;
  }

  get current(): number | null {
    return this.decks[this.cur].uid;
  }

  get playing(): boolean {
    const el = this.decks[this.cur].el;
    return !el.paused && !el.ended;
  }

  /** The current track's position, ms (for rules such as Previous's restart). */
  get positionMs(): number {
    return this.decks[this.cur].el.currentTime * 1000;
  }

  get durationMs(): number | null {
    const d = this.decks[this.cur].el.duration;
    return Number.isFinite(d) ? d * 1000 : null;
  }

  /** Whether the current track can be opened again at a position (its stream answers ranges). */
  get canSeek(): boolean {
    const el = this.decks[this.cur].el;
    const ranges = el.seekable;
    return Number.isFinite(el.duration) && ranges.length > 0 && ranges.end(ranges.length - 1) > 0;
  }

  resume() {
    const deck = this.decks[this.cur];
    if (deck.uid === null) return;
    this.ensureContext();
    this.start(deck);
    this.outgoing?.el.play().catch(() => undefined);
  }

  pause() {
    this.decks[this.cur].el.pause();
    // A crossfade does not survive a pause: the outgoing track stops.
    if (this.outgoing) this.finishFade();
    this.clearTimers();
  }

  seek(positionMs: number) {
    const deck = this.decks[this.cur];
    if (deck.uid === null) return;
    this.cancelFade();
    this.generation += 1n;
    deck.el.currentTime = Math.max(0, positionMs / 1000);
    this.reanchor(true);
    this.schedule();
  }

  stop() {
    this.cancelFade();
    this.clearTimers();
    for (const deck of this.decks) this.reset(deck);
    this.generation += 1n;
    this.plan = null;
    this.lastAnchor = null;
    this.events.anchor({ trackGeneration: this.generation, positionUs: 0n, anchorMonoNs: nowNs(), rate: 0, durationUs: null });
  }

  setVolume(volume: number) {
    this.volume = Math.min(1, Math.max(0, volume));
    this.applyVolume();
  }

  setMuted(muted: boolean) {
    this.muted = muted;
    this.applyVolume();
  }

  private applyVolume() {
    if (!this.ctx || !this.nodes) return;
    // About 5 ms of smoothing, so a change does not click (as the engine's one-pole ramp).
    this.nodes.master.gain.setTargetAtTime(this.muted ? 0 : this.volume, this.ctx.currentTime, 0.005);
  }

  /** The EQ bands and preamp, as the core's settings describe them. */
  setEq(eq: EqSettings) {
    this.eq = eq;
    if (!this.nodes || !this.ctx) return;
    const now = this.ctx.currentTime;
    this.nodes.preamp.gain.setTargetAtTime(eq.enabled ? dbToLinear(eq.preamp_db) : 1, now, 0.01);
    this.nodes.filters.forEach((filter, i) => {
      const band = eq.bands[i];
      if (!band) {
        filter.gain.setTargetAtTime(0, now, 0.01);
        return;
      }
      filter.type = EQ_TYPES[band.kind] ?? 'peaking';
      filter.frequency.value = band.freq_hz;
      filter.Q.value = band.q;
      filter.gain.setTargetAtTime(eq.enabled && band.enabled ? band.gain_db : 0, now, 0.01);
    });
  }

  /** Engages or bypasses the limiter (the core decides when). */
  setLimiter(engaged: boolean) {
    this.limiterOn = engaged;
    if (!this.nodes || !this.ctx) return;
    const now = this.ctx.currentTime;
    this.nodes.wet.gain.setTargetAtTime(engaged ? 1 : 0, now, 0.005);
    this.nodes.dry.gain.setTargetAtTime(engaged ? 0 : 1, now, 0.005);
  }

  /** The ReplayGain scale of the playing track. */
  setGain(uid: number, gain: number) {
    for (const deck of this.decks) if (deck.uid === uid && deck.replaygain) deck.replaygain.gain.value = gain;
  }

  // ---- internals ------------------------------------------------------------------------------

  private load(deck: Deck, uid: number, media: MediaSource, gain: number) {
    deck.uid = uid;
    deck.ready = false;
    deck.announced = false;
    deck.startedAt = null;
    deck.el.crossOrigin = media.crossOrigin ? 'anonymous' : null;
    deck.el.src = media.url;
    deck.el.load();
    if (deck.replaygain) deck.replaygain.gain.value = gain;
    if (deck.fade && this.ctx) {
      deck.fade.gain.cancelScheduledValues(this.ctx.currentTime);
      deck.fade.gain.setValueAtTime(1, this.ctx.currentTime);
    }
  }

  private reset(deck: Deck) {
    deck.el.pause();
    if (deck.el.getAttribute('src')) {
      deck.el.removeAttribute('src');
      deck.el.load();
    }
    deck.uid = null;
    deck.ready = false;
    deck.announced = false;
    deck.startedAt = null;
  }

  private start(deck: Deck) {
    const from = deck.el.currentTime;
    const at = performance.now();
    deck.startedAt = at;
    // Measure how long play() takes until the clock moves (for gapless starts): look a few times
    // right after the call, then stop.
    const probe = (tries: number) => {
      if (deck.startedAt !== at) return;
      const moved = deck.el.currentTime - from;
      if (moved > 0) {
        this.scheduler.calibrate(performance.now() - at - moved * 1000);
        deck.startedAt = null;
      } else if (tries > 0 && !deck.el.paused) {
        setTimeout(() => probe(tries - 1), 4);
      }
    };
    setTimeout(() => probe(150), 4);
    deck.el.play().catch((error: DOMException) => {
      if (error.name === 'AbortError' || deck.uid === null) return; // replaced by another load
      if (error.name === 'NotAllowedError') this.events.blocked(deck.uid);
      else this.events.error(deck.uid, error.name === 'NotSupportedError', error.message);
    });
  }

  private onPlaying(deck: Deck) {
    if (deck.uid === null) return;
    this.events.buffering(deck.uid, false);
    if (!deck.announced) {
      deck.announced = true;
      if (this.isCurrent(deck)) {
        const d = deck.el.duration;
        this.events.started(deck.uid, Number.isFinite(d) ? Math.round(d * 1000) : null);
      }
    }
    if (this.isCurrent(deck)) {
      this.reanchor(false);
      this.schedule();
    }
  }

  private onWaiting(deck: Deck) {
    if (deck.uid === null || !this.isCurrent(deck)) return;
    this.events.buffering(deck.uid, true);
    this.reanchor(false, true);
  }

  private onEnded(deck: Deck) {
    if (deck === this.outgoing) {
      this.finishFade();
      return;
    }
    if (!this.isCurrent(deck) || deck.uid === null) return;
    const ended = deck.uid;
    const next = this.decks[1 - this.cur];
    if (this.plan && next.uid !== null) {
      // The join was not started in time (the next track was still loading): start it now.
      this.advance(0);
    } else {
      this.clearTimers();
      this.events.ended(ended);
    }
  }

  private onError(deck: Deck) {
    if (deck.uid === null) return;
    const error = deck.el.error;
    const unsupported = error?.code === MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED || error?.code === MediaError.MEDIA_ERR_DECODE;
    this.events.error(deck.uid, unsupported, error?.message || 'the audio could not be played');
  }

  /** Arms a timer for the next transition check. */
  private schedule() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    const deck = this.decks[this.cur];
    const next = this.decks[1 - this.cur];
    if (deck.uid === null || this.outgoing) return;
    const state: DeckState = {
      currentTime: deck.el.currentTime,
      duration: deck.el.duration,
      paused: deck.el.paused,
      ended: deck.el.ended,
      rate: deck.el.playbackRate,
    };
    const decision = this.scheduler.decide(state, next.uid === null ? null : this.plan, next.ready);
    if (decision.kind === 'wait') {
      this.timer = setTimeout(() => this.schedule(), decision.ms);
    } else if (decision.kind === 'start') {
      this.advance(decision.fadeMs);
    } else if (this.plan && next.uid !== null && !next.ready && !deck.el.paused) {
      // Not ready yet: look again shortly.
      this.timer = setTimeout(() => this.schedule(), 250);
    }
  }

  /** Starts the prepared next track: gaplessly (`fadeMs` 0) or crossfading over `fadeMs`. */
  private advance(fadeMs: number) {
    const from = this.decks[this.cur];
    const to = this.decks[1 - this.cur];
    if (to.uid === null || from.uid === null || !this.ctx) return;
    const fromUid = from.uid;
    this.clearTimers();
    this.generation += 1n;
    this.cur = 1 - this.cur;
    this.plan = null;
    if (fadeMs > 0 && to.fade && from.fade) {
      const now = this.ctx.currentTime;
      const seconds = fadeMs / 1000;
      const points = Math.max(2, Math.min(256, Math.round(seconds * 32)));
      const [outCurve, inCurve] = this.curves(points);
      to.fade.gain.cancelScheduledValues(now);
      from.fade.gain.cancelScheduledValues(now);
      to.fade.gain.setValueCurveAtTime(inCurve, now, seconds);
      from.fade.gain.setValueCurveAtTime(outCurve, now, seconds);
      this.outgoing = from;
      this.events.overlapping(true);
      this.fadeEnd = setTimeout(() => this.finishFade(), fadeMs + 50);
    } else if (!from.el.paused && !from.el.ended && Number.isFinite(from.el.duration)) {
      // Gapless: the next track starts its start latency early, so the last milliseconds of this
      // one play out under it instead of being cut.
      const tailMs = Math.max(0, (from.el.duration - from.el.currentTime) * 1000);
      this.outgoing = from;
      this.fadeEnd = setTimeout(() => this.finishFade(), tailMs + 50);
    } else {
      this.reset(from);
    }
    to.announced = false;
    this.start(to);
    this.transitions.push({ from: fromUid, to: to.uid, fadeMs, leadMs: this.scheduler.leadMs, atMs: performance.now() });
    if (this.transitions.length > 20) this.transitions.shift();
    this.events.advanced(fromUid, to.uid);
  }

  private finishFade() {
    if (this.fadeEnd) clearTimeout(this.fadeEnd);
    this.fadeEnd = null;
    const old = this.outgoing;
    this.outgoing = null;
    if (old) {
      this.reset(old);
      if (old.fade && this.ctx) {
        old.fade.gain.cancelScheduledValues(this.ctx.currentTime);
        old.fade.gain.setValueAtTime(1, this.ctx.currentTime);
      }
      this.events.overlapping(false);
    }
    const pending = this.pendingArm;
    this.pendingArm = null;
    if (pending) this.arm(pending.uid, pending.media, pending.gain, pending.plan);
    else this.schedule();
  }

  private cancelFade() {
    if (!this.outgoing) return;
    const incoming = this.decks[this.cur];
    if (incoming.fade && this.ctx) {
      incoming.fade.gain.cancelScheduledValues(this.ctx.currentTime);
      incoming.fade.gain.setValueAtTime(1, this.ctx.currentTime);
    }
    this.finishFade();
  }

  private clearTimers() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
  }

  /** The anchor of the playing element now. */
  private anchorNow(buffering = false): PlaybackAnchor {
    const el = this.decks[this.cur].el;
    const latency = this.ctx ? this.ctx.outputLatency || this.ctx.baseLatency || 0 : 0;
    const running = !el.paused && !el.ended && !buffering && el.readyState >= 3;
    const position = Math.max(0, el.currentTime - (running ? latency : 0));
    return {
      trackGeneration: this.generation,
      positionUs: BigInt(Math.round(position * 1e6)),
      anchorMonoNs: nowNs(),
      rate: running ? el.playbackRate : 0,
      durationUs: Number.isFinite(el.duration) ? BigInt(Math.round(el.duration * 1e6)) : null,
    };
  }

  /** Publishes an anchor now (`jump`: a seek or a new track) and restarts the drift checks. */
  private reanchor(_jump: boolean, buffering = false) {
    const anchor = this.anchorNow(buffering);
    this.lastAnchor = anchor;
    this.events.anchor(anchor);
    this.driftChecks = 0;
    if (this.driftTimer) clearTimeout(this.driftTimer);
    this.driftTimer = anchor.rate > 0 ? setTimeout(() => this.checkDrift(), 100) : null;
  }

  private checkDrift() {
    this.driftTimer = null;
    const last = this.lastAnchor;
    if (!last || !(last.rate > 0)) return;
    const actual = this.anchorNow();
    if (actual.rate === 0) {
      this.reanchor(false);
      return;
    }
    const extrapolated = positionAt(last, actual.anchorMonoNs);
    const drift = extrapolated - actual.positionUs;
    if (drift > DRIFT_US || drift < -DRIFT_US) {
      this.lastAnchor = actual;
      this.events.anchor(actual);
    }
    this.driftChecks += 1;
    this.driftTimer = setTimeout(() => this.checkDrift(), this.driftChecks < 20 ? 100 : 1000);
  }
}

function nowNs(): bigint {
  return BigInt(Math.round(performance.now() * 1e6));
}
