// AI Lyrics in Jellyfesh Web (ADR-0019; docs/ui.md "AI Lyrics, Customize and Pro"), in its online
// mode: the first enabled AI Lyrics plugin's plan says what making lyrics needs first, and jf-web
// decides (core/jf-web/src/ai_lyrics.rs, after the core's). One job at a time: the page decodes the
// song with the browser's decoders to 16 kHz mono 16-bit PCM for the plugin's `audio` import, once
// the person allowed sending it to the plan's origin ("Send This Song to {origin}?"), and keeps
// what the plugin made in this browser, shown when a song has no synced lyrics of its own.

import type { Jf } from '../core/wasm';
import type { Lyrics, MakeSong, MakerPlan, Row } from '../core/types';
import { SPEECH_RATE, type MakeAudio } from '../plugins/imports';
import type { Plugin, PluginHost } from '../plugins/host';
import type { Player } from './player.svelte';
import type { Pro } from './pro.svelte';

const CONSENTS = 'jellyfesh.audio-consents';
const MADE = 'jellyfesh.made-lyrics';
/** Made lyrics kept in this browser, newest first. */
const MADE_KEPT = 60;
/** The longest song decoded (as the core's MAX_SONG_MS). */
const MAX_SONG_MS = 20 * 60 * 1000;

export type AiLyricsNeed =
  | 'Nothing'
  | 'Pro'
  | 'SignIn'
  | { Consent: { origin: string } }
  | { Unavailable: { reason: string } };

export interface Consent {
  plugin: string;
  origin: string;
}

export type JobStage =
  | { stage: 'reading' }
  | { stage: 'sending'; origin: string }
  | { stage: 'failed'; message: string };

export interface Job {
  /** The song's key (`songKey`). */
  key: string;
  title: string;
  state: JobStage;
}

interface MadeEntry {
  key: string;
  engine: string;
  lyrics: Lyrics;
}

/** What identifies a song's made lyrics: the source and item, or the file. */
export function songKey(row: Row): string {
  return row.plugin ? `${row.plugin}\u0000${row.id}` : row.id;
}

function read<T>(key: string, fallback: T): T {
  try {
    const text = localStorage.getItem(key);
    return text ? (JSON.parse(text) as T) : fallback;
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Kept for this visit only.
  }
}

/** A decoded song as 16 kHz mono 16-bit samples. */
export async function decodeForSpeech(bytes: ArrayBuffer): Promise<{ pcm: Int16Array; durationMs: number }> {
  // Decoding into a 16 kHz context resamples; the channels are averaged into one.
  const context = new OfflineAudioContext(1, 1, SPEECH_RATE);
  const buffer = await context.decodeAudioData(bytes);
  const frames = Math.min(buffer.length, Math.floor((MAX_SONG_MS / 1000) * buffer.sampleRate));
  const pcm = new Int16Array(frames);
  const channels = Array.from({ length: buffer.numberOfChannels }, (_, c) => buffer.getChannelData(c));
  for (let i = 0; i < frames; i++) {
    let sum = 0;
    for (const channel of channels) sum += channel[i];
    const sample = Math.max(-1, Math.min(1, sum / channels.length));
    pcm[i] = sample < 0 ? sample * 0x8000 : sample * 0x7fff;
  }
  return { pcm, durationMs: Math.round((frames / buffer.sampleRate) * 1000) };
}

export class AiLyrics {
  plan = $state.raw<'asking' | { failed: string } | { ready: MakerPlan }>('asking');
  consents = $state.raw<Consent[]>(read(CONSENTS, []));
  job = $state.raw<Job | null>(null);
  /** Bumps when made lyrics change. */
  revision = $state(0);
  /** "Send This Song to {origin}?" is open, for this row. */
  asking = $state.raw<{ row: Row; origin: string } | null>(null);
  /** "Delete These Lyrics?" is open, for this row. */
  deleting = $state.raw<Row | null>(null);
  private key = '';
  private made: MadeEntry[] = read(MADE, []);
  private cancelled = false;

  constructor(
    private readonly jf: Jf,
    private readonly host: PluginHost,
    private readonly pro: Pro,
    private readonly player: Player,
  ) {
    host.onChange(() => void this.refresh());
    pro.onChange(() => void this.refresh());
    player.madeLyrics = (row) => {
      const found = this.made.find((m) => m.key === songKey(row));
      return found ? { lyrics: found.lyrics, engine: found.engine } : null;
    };
  }

  /** The AI Lyrics plugin: the first enabled one that runs. */
  plugin(): Plugin | undefined {
    return this.host.plugins.find((p) => p.state.enabled && !p.blocked && p.declares('MakeLyrics'));
  }

  /** What making lyrics needs now. */
  get need(): AiLyricsNeed {
    void this.revision;
    return JSON.parse(
      this.jf.ai_lyrics_need(this.plugin()?.id, this.pro.active, JSON.stringify(this.plan), JSON.stringify(this.consents)),
    );
  }

  /** Asks the plugin's plan again when it, its settings, its sign-in or Pro changed. */
  async refresh() {
    const plugin = this.plugin();
    const key = plugin
      ? `${plugin.id}\u0000${plugin.info?.version ?? plugin.entry.version}\u0000${JSON.stringify(plugin.state.settings ?? null)}\u0000${plugin.state.account ?? ''}\u0000${this.pro.active}`
      : '';
    if (key === this.key) return;
    this.key = key;
    if (!plugin || !this.pro.active) {
      this.plan = 'asking';
      return;
    }
    this.plan = 'asking';
    try {
      const plan = await plugin.plan();
      if (this.key === key) this.plan = { ready: plan };
    } catch (error) {
      if (this.key === key) this.plan = { failed: (error as Error).message };
    }
  }

  allow(origin: string) {
    const plugin = this.plugin();
    if (!plugin) return;
    if (!this.consents.some((c) => c.plugin === plugin.id && c.origin === origin)) {
      this.consents = [...this.consents, { plugin: plugin.id, origin }];
      write(CONSENTS, this.consents);
    }
  }

  /** Stop Allowing (Settings › Extensions › AI Lyrics › Sending Audio). */
  revoke(consent: Consent) {
    this.consents = this.consents.filter((c) => !(c.plugin === consent.plugin && c.origin === consent.origin));
    write(CONSENTS, this.consents);
  }

  /** Whether `row`'s song has lyrics AI Lyrics made. */
  hasMade(row: Row): boolean {
    void this.revision;
    return this.made.some((m) => m.key === songKey(row));
  }

  /** Delete Made Lyrics (asks first, in the view). */
  deleteMade(row: Row) {
    this.made = this.made.filter((m) => m.key !== songKey(row));
    write(MADE, this.made);
    this.revision += 1;
    if (this.player.current?.uid === row.uid) this.player.refreshLyrics();
  }

  /** Create Synced Lyrics (or Time These Lyrics, with `text`): asks what is needed first. */
  async make(row: Row, text: string | null = null) {
    const need = this.need;
    if (need === 'Pro') {
      this.pro.sheet = true;
      return;
    }
    if (typeof need === 'object' && 'Consent' in need) {
      this.asking = { row, origin: need.Consent.origin };
      return;
    }
    if (need !== 'Nothing') return;
    const plugin = this.plugin();
    const plan = typeof this.plan === 'object' && 'ready' in this.plan ? this.plan.ready : null;
    if (!plugin || !plan || this.job) return;
    const key = songKey(row);
    this.cancelled = false;
    this.job = { key, title: row.title, state: { stage: 'reading' } };
    try {
      const { pcm, durationMs } = await decodeForSpeech(await this.player.audioBytes(row));
      if (this.cancelled) return;
      const origin = plan.upload_to;
      const audio: MakeAudio = { track: 1n, durationMs, pcm, consented: origin ? [origin] : [] };
      const song: MakeSong = {
        title: row.title,
        artist: row.artist,
        album: row.album,
        duration_ms: durationMs,
        text,
        language: null,
      };
      const made = await plugin.make(song, audio, () => {
        if (origin && this.job?.key === key) this.job = { ...this.job, state: { stage: 'sending', origin } };
      });
      if (this.cancelled) return;
      const lyrics = plugin.parseLyrics(made.lyrics);
      this.made = [{ key, engine: made.engine, lyrics }, ...this.made.filter((m) => m.key !== key)].slice(0, MADE_KEPT);
      write(MADE, this.made);
      this.job = null;
      this.revision += 1;
      if (this.player.current && songKey(this.player.current) === key) this.player.refreshLyrics();
    } catch (error) {
      if (!this.cancelled) this.job = { key, title: row.title, state: { stage: 'failed', message: (error as Error).message } };
    }
  }

  /** Cancel: the plugin's worker stops with the call. */
  cancel() {
    this.cancelled = true;
    this.plugin()?.stop();
    this.job = null;
  }

  /** Clears a failed job (Try Again starts a new one). */
  clearFailed() {
    if (this.job?.state.stage === 'failed') this.job = null;
  }
}
