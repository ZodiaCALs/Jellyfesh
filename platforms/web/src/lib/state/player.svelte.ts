// The player: the core's queue and rules (jf-web Core) driving the browser's audio engine.
//
// The core decides what plays (next, previous, shuffle, repeat, what to prepare), how two tracks
// join (gapless, or a crossfade of a given length with album continuity), what counts as a play,
// the ReplayGain scale and whether the limiter engages. This file resolves streams through the
// plugins (stepping down in quality when the browser cannot play a format), hands them to the
// engine, and keeps the views, the Media Session and the saved queue up to date.

import type { Jf } from '../core/wasm';
import type {
  FileMeta,
  Item,
  Lyrics,
  NewEntry,
  PreviousAction,
  Quality,
  QueueSnapshot,
  ReplayGainInfo,
  RepeatMode,
  Row,
  Transition,
  WebSettings,
} from '../core/types';
import { AudioEngine, type MediaSource } from '../audio/engine';
import * as mediaSession from '../audio/mediaSession';
import type { Plan } from '../audio/scheduler';
import { prepareStream } from '../audio/streams';
import { monotonicNowNs, type PlaybackAnchor } from '../drawing/displayClock';
import { t } from '../i18n/index.svelte';
import type { Plugin } from '../plugins/host';
import type { Artworks } from './artwork';
import type { Files } from './files';
import { explain, type Sources } from './sources.svelte';
import type { ArtworkSource, Theme } from './theme.svelte';

type Core = InstanceType<Jf['Core']>;

interface Resolved {
  media: MediaSource;
  gain: number;
  replaygain: ReplayGainInfo | null;
  /** The quality asked for (null for a dropped file). */
  quality: Quality | null;
  /** What it is, for About This Song ("FLAC · 44.1 kHz"). */
  format: string | null;
  expiresAt: number | null;
  release(): void;
}

export type LyricsState =
  | { status: 'none' }
  | { status: 'loading' }
  | { status: 'missing' }
  | { status: 'ready'; lyrics: Lyrics };

const QUEUE_KEY = 'jf.queue';
const PLAYBACK_KEY = 'jf.playback';
const VOLUME_KEY = 'jf.volume';
/** Consecutive tracks that fail before playback stops instead of skipping on. */
const MAX_FAILURES = 3;
/** Re-resolve a stream this close to its expiry. */
const EXPIRY_MARGIN_MS = 30_000;
const NOW_PLAYING_ART = 1024;

const EMPTY: QueueSnapshot = { revision: 0, length: 0, position: null, current: null, shuffle: false, repeat: 'off', up_next: [] };

function stoppedAnchor(positionMs = 0, durationMs: number | null = null): PlaybackAnchor {
  return {
    trackGeneration: 0n,
    positionUs: BigInt(Math.round(positionMs * 1000)),
    anchorMonoNs: monotonicNowNs(),
    rate: 0,
    durationUs: durationMs === null ? null : BigInt(Math.round(durationMs * 1000)),
  };
}

function read<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw === null ? null : (JSON.parse(raw) as T);
  } catch {
    return null;
  }
}

function write(key: string, value: unknown) {
  try {
    localStorage.setItem(key, typeof value === 'string' ? value : JSON.stringify(value));
  } catch {
    // Storage full or blocked: this visit only.
  }
}

export class Player {
  queue = $state.raw<QueueSnapshot>(EMPTY);
  current = $state.raw<Row | null>(null);
  /** Playing, or about to (the person has not paused). */
  playing = $state(false);
  /** Resolving or buffering. */
  loading = $state(false);
  anchor = $state.raw<PlaybackAnchor>(stoppedAnchor());
  durationMs = $state<number | null>(null);
  volume = $state(1);
  muted = $state(false);
  lyrics = $state.raw<LyricsState>({ status: 'none' });
  /** The last problem, for the banner. */
  problem = $state<string | null>(null);
  /** The person must press play (autoplay policy, or a restored queue). */
  waitingForPlay = $state(false);
  /** The current track's format, once its stream is prepared. */
  format = $state<string | null>(null);

  readonly engine: AudioEngine;
  /** What the player did lately (the last 100 events), for diagnostics and the end-to-end tests. */
  readonly trace: string[] = [];
  private resumeAt: number | null = null;
  private resolved = new Map<number, Resolved>();
  private resolving = new Map<number, Promise<Resolved>>();
  private generation = 0;
  private failures = 0;
  private overlapping = false;
  private saveTimer: ReturnType<typeof setTimeout> | null = null;
  private positionTimer: ReturnType<typeof setInterval> | null = null;
  private probe: HTMLAudioElement | null = null;

  constructor(
    private readonly jf: Jf,
    private readonly core: Core,
    private readonly sources: Sources,
    private readonly files: Files,
    private readonly theme: Theme,
    private readonly artworks: Artworks,
    private settings: WebSettings,
  ) {
    this.engine = new AudioEngine(
      {
        started: (uid, durationMs) => this.onStarted(uid, durationMs),
        advanced: (from, to) => this.onAdvanced(from, to),
        ended: (uid) => this.onEnded(uid),
        anchor: (anchor) => (this.anchor = anchor),
        buffering: (uid, buffering) => {
          if (uid === this.current?.uid) this.loading = buffering;
        },
        error: (uid, unsupported, message) => void this.onError(uid, unsupported, message),
        duration: (uid, durationMs) => {
          if (uid !== this.current?.uid) return;
          this.durationMs = durationMs;
          void this.armNext();
        },
        overlapping: (overlap) => {
          this.overlapping = overlap;
          this.updateLimiter();
        },
        blocked: (uid) => {
          this.note(`blocked ${uid}`);
          if (uid !== this.current?.uid) return;
          this.playing = false;
          this.loading = false;
          this.waitingForPlay = true;
        },
      },
      (points) => {
        const both = jf.fade_curves(points);
        return [both.slice(0, points), both.slice(points)];
      },
      JSON.parse(jf.limiter_params()),
    );
    const volume = read<{ volume: number; muted: boolean }>(VOLUME_KEY);
    if (volume) {
      this.volume = Math.min(1, Math.max(0, volume.volume));
      this.muted = volume.muted;
    }
    this.engine.setVolume(this.volume);
    this.engine.setMuted(this.muted);
    this.engine.setEq(settings.eq);
    mediaSession.bindMediaSession({
      play: () => this.play(),
      pause: () => this.pause(),
      next: () => this.next(),
      previous: () => this.previous(),
      seekTo: (ms) => this.seek(ms),
      seekBy: (ms) => this.seek(this.positionMs + ms),
      stop: () => this.stop(),
    });
    addEventListener('pagehide', () => this.savePosition());
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'hidden') this.savePosition();
    });
  }

  // ---- startup and settings --------------------------------------------------------------------

  /**
   * Restores the saved queue, ready to play (browsers do not start audio before a click).
   * `restored` runs right after the core restored it, to hand back the tags of library songs.
   */
  restore(restored?: () => void) {
    const saved = (() => {
      try {
        return localStorage.getItem(QUEUE_KEY);
      } catch {
        return null;
      }
    })();
    if (!saved || !this.core.restore(saved)) return;
    restored?.();
    this.queue = this.snapshot();
    const row = this.queue.current;
    if (!row) return;
    const playback = read<{ uid: number; position_ms: number }>(PLAYBACK_KEY);
    this.resumeAt = this.settings.resume_on_launch && playback?.uid === row.uid ? playback.position_ms : null;
    this.show(row, this.resumeAt ?? 0);
    this.waitingForPlay = this.resumeAt !== null;
  }

  applySettings(settings: WebSettings) {
    const before = this.settings;
    this.settings = settings;
    this.engine.setEq(settings.eq);
    for (const [uid, resolved] of this.resolved) {
      resolved.gain = this.core.replaygain_scale(JSON.stringify(resolved.replaygain));
      this.engine.setGain(uid, resolved.gain);
    }
    this.updateLimiter();
    if (JSON.stringify(before.crossfade) !== JSON.stringify(settings.crossfade)) void this.armNext();
    if (before.quality !== settings.quality) this.dropPrepared();
  }

  // ---- what the views call ----------------------------------------------------------------------

  /** Replaces the queue with `entries` and plays from `start`. */
  playEntries(entries: NewEntry[], start = 0, shuffle = false) {
    if (entries.length === 0) return;
    this.engine.ensureContext();
    try {
      this.core.play(JSON.stringify(entries), start, shuffle);
    } catch (error) {
      this.problem = (error as Error).message;
      return;
    }
    this.failures = 0;
    this.dropPrepared();
    const row = this.parse<Row | null>(this.core.current());
    if (row) void this.startRow(row, 0, false);
  }

  /** Play Next (`next`) or Play Last. */
  enqueue(entries: NewEntry[], next: boolean) {
    if (entries.length === 0) return;
    const wasEmpty = this.queue.length === 0;
    this.core.enqueue(JSON.stringify(entries), next);
    this.changed();
    if (wasEmpty) {
      const row = this.parse<Row | null>(this.core.current());
      if (row) this.show(row, 0);
    }
  }

  /** The engine plays the current entry (not still the one before it while a stream resolves). */
  get loaded(): boolean {
    return this.engine.current !== null && this.engine.current === this.current?.uid;
  }

  /** The current entry's position, ms (0 until its stream starts). */
  get positionMs(): number {
    return this.loaded ? this.engine.positionMs : (this.resumeAt ?? 0);
  }

  play() {
    this.engine.ensureContext();
    this.waitingForPlay = false;
    if (this.loaded) {
      this.playing = true;
      this.engine.resume();
      return;
    }
    const row = this.current ?? this.parse<Row | null>(this.core.current());
    if (!row) return;
    const at = this.resumeAt ?? 0;
    this.resumeAt = null;
    void this.startRow(row, at, false);
  }

  pause() {
    this.note('pause');
    this.playing = false;
    this.engine.pause();
    mediaSession.setPlaybackState('paused');
    this.savePosition();
  }

  toggle() {
    if (this.playing) this.pause();
    else this.play();
  }

  stop() {
    this.note('stop');
    this.endPlay(false);
    this.generation += 1;
    this.engine.stop();
    this.playing = false;
    this.loading = false;
    mediaSession.setPlaybackState('none');
    this.savePosition();
  }

  next() {
    this.engine.ensureContext();
    this.endPlay(false);
    const row = this.parse<Row | null>(this.core.next(true));
    if (row) void this.startRow(row, 0, !this.playing && this.loaded);
    else this.stopAtEnd();
  }

  previous() {
    this.engine.ensureContext();
    const position = this.positionMs;
    const action = this.parse<PreviousAction | null>(this.core.previous(position, this.loaded));
    this.note(`previous at ${Math.round(position)} ms: ${action ? (action.action === 'play' ? `play ${action.row.uid}` : 'restart') : 'none'}`);
    if (!action) return;
    if (action.action === 'restart') {
      if (this.loaded) this.seek(0);
      else if (this.current) void this.startRow(this.current, 0, !this.playing);
      return;
    }
    this.endPlay(false);
    void this.startRow(action.row, 0, !this.playing && this.loaded);
  }

  seek(positionMs: number) {
    if (!this.loaded) {
      if (this.current) this.resumeAt = Math.max(0, positionMs);
      this.anchor = stoppedAnchor(this.resumeAt ?? 0, this.durationMs);
      return;
    }
    const duration = this.engine.durationMs ?? this.durationMs;
    this.engine.seek(Math.max(0, duration ? Math.min(positionMs, duration - 250) : positionMs));
  }

  jumpTo(uid: number) {
    this.engine.ensureContext();
    this.endPlay(false);
    const row = this.parse<Row | null>(this.core.jump_to(uid));
    if (row) void this.startRow(row, 0, false);
  }

  remove(uid: number) {
    const wasCurrent = this.core.remove(uid);
    this.release(uid);
    if (wasCurrent) {
      this.endPlay(false);
      const row = this.parse<Row | null>(this.core.current());
      if (row) void this.startRow(row, 0, !this.playing);
      else this.clearPlayer();
    }
    this.changed();
  }

  move(uid: number, to: number) {
    this.core.move_item(uid, to);
    this.changed();
  }

  clear() {
    this.stop();
    this.core.clear();
    this.clearPlayer();
    this.changed();
  }

  setShuffle(on: boolean) {
    this.core.set_shuffle(on);
    this.changed();
  }

  cycleRepeat() {
    const order: RepeatMode[] = ['off', 'all', 'one'];
    const next = order[(order.indexOf(this.queue.repeat) + 1) % order.length];
    this.core.set_repeat(next);
    this.changed();
  }

  setVolume(volume: number) {
    this.volume = Math.min(1, Math.max(0, volume));
    if (this.volume > 0 && this.muted) this.muted = false;
    this.engine.setVolume(this.volume);
    this.engine.setMuted(this.muted);
    write(VOLUME_KEY, { volume: this.volume, muted: this.muted });
  }

  setMuted(muted: boolean) {
    this.muted = muted;
    this.engine.setMuted(muted);
    write(VOLUME_KEY, { volume: this.volume, muted: this.muted });
  }

  /** The rows of the playback order from `offset` (the queue view's lazy list). */
  window(offset: number, limit: number): Row[] {
    return this.parse<Row[]>(this.core.window(offset, limit));
  }

  /** The source and item behind a row. */
  sourceItem(row: Row): { plugin: Plugin; item: Item } | null {
    if (row.kind !== 'source') return null;
    const found = this.parse<{ plugin: string; item: Item } | null>(this.core.source_item(row.uid));
    const plugin = found ? this.sources.get(found.plugin) : undefined;
    return found && plugin ? { plugin, item: found.item } : null;
  }

  // ---- internals ---------------------------------------------------------------------------------

  private note(event: string) {
    this.trace.push(`${Math.round(performance.now())} ${event}`);
    if (this.trace.length > 100) this.trace.shift();
  }

  private parse<T>(json: string): T {
    return JSON.parse(json) as T;
  }

  private snapshot(): QueueSnapshot {
    return this.parse<QueueSnapshot>(this.core.snapshot());
  }

  /** The queue changed: refresh the views, prepare what follows, save. */
  private changed() {
    this.queue = this.snapshot();
    if (this.engine.current !== null) void this.armNext();
    this.scheduleSave();
  }

  private scheduleSave() {
    if (this.saveTimer) return;
    this.saveTimer = setTimeout(() => {
      this.saveTimer = null;
      write(QUEUE_KEY, this.core.save());
    }, 400);
  }

  private savePosition() {
    const uid = this.current?.uid;
    if (uid === undefined) return;
    write(PLAYBACK_KEY, { uid, position_ms: Math.round(this.positionMs) });
    if (this.saveTimer) {
      clearTimeout(this.saveTimer);
      this.saveTimer = null;
    }
    write(QUEUE_KEY, this.core.save());
  }

  /** Shows `row` as the current entry: views, lyrics, artwork, Media Session. */
  private show(row: Row, positionMs: number) {
    const changed = this.current?.uid !== row.uid;
    this.current = row;
    this.queue = this.snapshot();
    if (changed) {
      this.durationMs = row.duration_ms;
      this.anchor = stoppedAnchor(positionMs, row.duration_ms);
      void this.loadLyrics(row);
      void this.loadArtwork(row);
    }
    this.scheduleSave();
  }

  private clearPlayer() {
    this.current = null;
    this.format = null;
    this.durationMs = null;
    this.anchor = stoppedAnchor();
    this.lyrics = { status: 'none' };
    void this.theme.setArtwork(null);
    mediaSession.setMetadata(null);
    mediaSession.setPlaybackState('none');
  }

  /** The play in progress ends (a skip unless `natural`). */
  private endPlay(natural: boolean) {
    if (this.engine.current === null) return;
    const played = natural ? (this.engine.durationMs ?? this.durationMs ?? this.engine.positionMs) : this.engine.positionMs;
    this.core.finish(played, natural);
  }

  private async startRow(row: Row, positionMs: number, paused: boolean) {
    const generation = ++this.generation;
    this.note(`start ${row.uid} at ${Math.round(positionMs)}${paused ? ' paused' : ''} (playing ${this.playing})`);
    this.show(row, positionMs);
    this.playing = !paused;
    this.loading = !paused;
    this.problem = null;
    this.waitingForPlay = false;
    let resolved: Resolved;
    try {
      resolved = await this.resolve(row);
    } catch (error) {
      if (generation === this.generation) await this.failed(row, error);
      return;
    }
    if (generation !== this.generation) return;
    this.format = resolved.format;
    // Paused while the stream was resolving: load it paused.
    this.engine.play(row.uid, resolved.media, resolved.gain, positionMs, !this.playing);
    if (!this.playing) this.loading = false;
    this.prune();
    this.updateLimiter();
    this.startPositionTimer();
    void this.armNext();
  }

  /** Prepares the entry that follows on its own, and how it joins. */
  private async armNext() {
    const generation = this.generation;
    const current = this.current;
    const next = this.parse<Row | null>(this.core.upcoming());
    if (!current || !next) {
      this.engine.disarm();
      return;
    }
    let resolved: Resolved;
    try {
      resolved = await this.resolve(next);
    } catch {
      // It gets another try when it becomes current.
      if (generation === this.generation) this.engine.disarm();
      return;
    }
    if (generation !== this.generation || this.current?.uid !== current.uid) return;
    const still = this.parse<Row | null>(this.core.upcoming());
    if (still?.uid !== next.uid) return;
    const transition = this.parse<Transition>(this.core.transition(next.uid, this.engine.durationMs ?? undefined, undefined));
    const plan: Plan =
      transition.kind === 'crossfade' ? { kind: 'crossfade', overlapMs: transition.overlap_ms } : { kind: 'gapless' };
    this.engine.arm(next.uid, resolved.media, resolved.gain, plan);
  }

  /** A stream for `row` (prepared once; shared by concurrent callers). */
  private resolve(row: Row, quality?: Quality): Promise<Resolved> {
    const ready = this.resolved.get(row.uid);
    if (ready && quality === undefined && (ready.expiresAt === null || ready.expiresAt - Date.now() > EXPIRY_MARGIN_MS)) {
      return Promise.resolve(ready);
    }
    const busy = this.resolving.get(row.uid);
    if (busy && quality === undefined) return busy;
    if (ready) this.release(row.uid);
    const job = this.resolveNow(row, quality ?? this.settings.quality).then(
      (resolved) => {
        this.resolving.delete(row.uid);
        this.resolved.get(row.uid)?.release();
        this.resolved.set(row.uid, resolved);
        return resolved;
      },
      (error) => {
        this.resolving.delete(row.uid);
        throw error;
      },
    );
    this.resolving.set(row.uid, job);
    return job;
  }

  private async resolveNow(row: Row, start: Quality): Promise<Resolved> {
    if (row.kind === 'file') {
      const meta = this.parse<FileMeta | null>(this.core.file_of(row.uid));
      const dropped = meta ? await this.files.open(meta.id) : undefined;
      if (!meta || !dropped) {
        switch (this.files.whyClosed(meta?.id ?? row.id)) {
          case 'pick':
            throw new Error(t('Pick the folder of this song again in Settings › Library to play it.'));
          case 'gone':
            throw new Error(t('This song is no longer in its folder.'));
          default:
            throw new Error(t('This file is no longer open. Drop it here again to play it.'));
        }
      }
      const tags = dropped.tags;
      const format = [
        tags?.codec?.toUpperCase(),
        tags?.sample_rate ? `${(tags.sample_rate / 1000).toLocaleString()} kHz` : null,
        tags?.bitrate_kbps ? `${tags.bitrate_kbps} kbps` : null,
      ]
        .filter(Boolean)
        .join(' · ');
      return {
        media: { url: dropped.url, crossOrigin: false },
        gain: this.core.replaygain_scale(JSON.stringify(meta.replaygain)),
        replaygain: meta.replaygain,
        quality: null,
        format: format || null,
        expiresAt: null,
        release() {},
      };
    }
    const source = this.sourceItem(row);
    if (!source) throw new Error(t('The source of this song is not available.'));
    let quality = start;
    for (;;) {
      const info = await source.plugin.resolveStream(source.item, quality);
      const mime = info.codec ? this.jf.codec_mime(info.codec) : undefined;
      if (mime && !this.canPlay(mime)) {
        const lower = this.parse<Quality | null>(this.jf.next_quality(quality));
        if (lower) {
          quality = lower;
          continue;
        }
      }
      const prepared = await prepareStream(info, source.plugin.relay);
      const format = [info.codec?.toUpperCase(), info.bitrate_kbps ? `${info.bitrate_kbps} kbps` : null]
        .filter(Boolean)
        .join(' · ');
      return {
        media: prepared.media,
        gain: this.core.replaygain_scale('null'),
        replaygain: null,
        quality,
        format: format || null,
        expiresAt: info.expires_at_ms,
        release: prepared.release,
      };
    }
  }

  private canPlay(mime: string): boolean {
    this.probe ??= document.createElement('audio');
    return this.probe.canPlayType(mime) !== '';
  }

  private release(uid: number) {
    this.resolved.get(uid)?.release();
    this.resolved.delete(uid);
  }

  /** Lets go of streams nothing needs (keeps the current, the one after it and the one before). */
  private prune(keepAlso: number | null = null) {
    const upcoming = this.parse<Row | null>(this.core.upcoming());
    const keep = new Set([this.current?.uid, upcoming?.uid, this.engine.current, this.engine.armed, keepAlso]);
    for (const uid of [...this.resolved.keys()]) if (!keep.has(uid)) this.release(uid);
  }

  /** Everything prepared is stale (a new queue, or another quality). */
  private dropPrepared() {
    for (const uid of [...this.resolved.keys()]) if (uid !== this.engine.current) this.release(uid);
    this.engine.disarm();
  }

  private updateLimiter() {
    const scales: number[] = [];
    const current = this.engine.current;
    if (current !== null) scales.push(this.resolved.get(current)?.gain ?? 1);
    if (this.overlapping) {
      const armed = this.engine.armed;
      if (armed !== null) scales.push(this.resolved.get(armed)?.gain ?? 1);
    }
    this.engine.setLimiter(this.core.limiter_needed(Float32Array.from(scales), this.overlapping));
  }

  private startPositionTimer() {
    if (this.positionTimer) return;
    this.positionTimer = setInterval(() => {
      if (!this.playing) return;
      this.savePosition();
      const duration = this.engine.durationMs;
      if (duration) mediaSession.setPosition(this.engine.positionMs, duration, 1);
    }, 5000);
  }

  private stopAtEnd() {
    this.note('end of the queue');
    // The core rewound to the first entry: show it, ready to play.
    this.generation += 1;
    this.engine.stop();
    this.playing = false;
    this.loading = false;
    this.dropPrepared();
    const row = this.parse<Row | null>(this.core.current());
    if (row) this.show(row, 0);
    else this.clearPlayer();
    this.anchor = stoppedAnchor(0, this.durationMs);
    mediaSession.setPlaybackState('paused');
  }

  // ---- engine events -----------------------------------------------------------------------------

  private onStarted(uid: number, durationMs: number | null) {
    this.note(`started ${uid}`);
    if (uid !== this.current?.uid) return;
    this.core.started(uid, Date.now(), durationMs ?? undefined);
    this.failures = 0;
    this.loading = false;
    this.playing = true;
    if (durationMs) this.durationMs = durationMs;
    mediaSession.setPlaybackState('playing');
    if (durationMs) mediaSession.setPosition(this.engine.positionMs, durationMs, 1);
  }

  private onAdvanced(from: number, to: number) {
    this.note(`advanced ${from} -> ${to}`);
    // The finished track played to its end.
    this.core.finish(this.durationMs ?? 0, true);
    const row = this.parse<Row | null>(this.core.next(false));
    if (!row || row.uid !== to) {
      // The queue changed under the engine: play what the queue says now.
      if (row) void this.startRow(row, 0, false);
      else this.stopAtEnd();
      return;
    }
    this.show(row, 0);
    this.format = this.resolved.get(to)?.format ?? null;
    this.prune(from);
    void this.armNext();
  }

  private onEnded(uid: number) {
    this.note(`ended ${uid}`);
    if (uid !== this.current?.uid) return;
    this.endPlay(true);
    const row = this.parse<Row | null>(this.core.next(false));
    if (row) void this.startRow(row, 0, false);
    else this.stopAtEnd();
  }

  private async onError(uid: number, unsupported: boolean, message: string) {
    this.note(`error ${uid}${unsupported ? ' (unsupported)' : ''}: ${message}`);
    const resolved = this.resolved.get(uid);
    const isCurrent = uid === this.current?.uid && uid === this.engine.current;
    if (unsupported && resolved?.quality) {
      const lower = this.parse<Quality | null>(this.jf.next_quality(resolved.quality));
      const row = isCurrent ? this.current : this.parse<Row | null>(this.core.upcoming());
      if (lower && row && row.uid === uid) {
        if (isCurrent) {
          const position = this.engine.positionMs;
          const generation = ++this.generation;
          try {
            const again = await this.resolve(row, lower);
            if (generation !== this.generation) return;
            this.engine.play(uid, again.media, again.gain, position, !this.playing);
            void this.armNext();
          } catch (error) {
            if (generation === this.generation) await this.failed(row, error);
          }
        } else {
          this.release(uid);
          this.engine.disarm();
          try {
            const again = await this.resolve(row, lower);
            const transition = this.parse<Transition>(this.core.transition(uid, this.engine.durationMs ?? undefined, undefined));
            this.engine.arm(
              uid,
              again.media,
              again.gain,
              transition.kind === 'crossfade' ? { kind: 'crossfade', overlapMs: transition.overlap_ms } : { kind: 'gapless' },
            );
          } catch {
            // Tried again when it becomes current.
          }
        }
        return;
      }
    }
    if (!isCurrent) {
      // The prepared next track failed: it is tried again when it becomes current.
      if (uid === this.engine.armed) this.engine.disarm();
      this.release(uid);
      return;
    }
    const row = this.current;
    if (!row) return;
    await this.failed(row, new Error(unsupported ? t('This browser cannot play this song’s format.') : message));
  }

  /** `row` cannot be played: say why and skip on, as the apps do (up to a few in a row). */
  private async failed(row: Row, error: unknown) {
    this.note(`failed ${row.uid}: ${error instanceof Error ? error.message : String(error)}`);
    const source = row.kind === 'source' ? this.sourceItem(row) : null;
    const reason = await explain(error, source?.plugin);
    this.problem = t('Could not play “{title}”: {reason}', { title: row.title, reason });
    this.loading = false;
    this.release(row.uid);
    this.failures += 1;
    if (this.failures >= MAX_FAILURES || this.current?.uid !== row.uid) {
      this.playing = false;
      this.engine.stop();
      return;
    }
    const next = this.parse<Row | null>(this.core.next(true));
    if (next && next.uid !== row.uid) void this.startRow(next, 0, false);
    else {
      this.playing = false;
      this.engine.stop();
    }
  }

  private async loadLyrics(row: Row) {
    this.lyrics = { status: 'loading' };
    let lyrics: Lyrics | null = null;
    try {
      if (row.kind === 'file') {
        const meta = this.parse<FileMeta | null>(this.core.file_of(row.uid));
        lyrics = meta ? await this.files.lyrics(meta.id) : null;
      } else {
        const source = this.sourceItem(row);
        if (source?.plugin.declares('Lyrics')) lyrics = await source.plugin.lyrics(source.item);
      }
    } catch {
      lyrics = null;
    }
    if (this.current?.uid !== row.uid) return;
    this.lyrics = lyrics && lyrics.doc.lines.length > 0 ? { status: 'ready', lyrics } : { status: 'missing' };
  }

  private async loadArtwork(row: Row) {
    let source: ArtworkSource | null = null;
    if (row.kind === 'file') {
      const meta = this.parse<FileMeta | null>(this.core.file_of(row.uid));
      const dropped = meta ? await this.files.open(meta.id).catch(() => undefined) : undefined;
      if (dropped?.cover) source = { kind: 'bytes', key: `file:${dropped.id}`, bytes: dropped.cover.bytes, mime: dropped.cover.mime };
    } else {
      const found = this.sourceItem(row);
      if (found) {
        const image = await this.artworks.ref(found.plugin, found.item, NOW_PLAYING_ART);
        if (image) source = { kind: 'remote', key: `${found.plugin.id}:${image.url}`, image, relay: found.plugin.relay };
      }
    }
    if (this.current?.uid !== row.uid) return;
    await this.theme.setArtwork(source);
    if (this.current?.uid !== row.uid) return;
    mediaSession.setMetadata({
      title: row.title,
      artist: row.artist,
      album: row.album,
      artwork: this.theme.artwork,
    });
  }
}
