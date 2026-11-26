// Words Get Info's rows (jf-web's details rules pick them and their order; docs/ui.md
// "Details") as the apps word them: "Hi-Res Lossless", "FLAC · 24-bit · 96 kHz · Stereo",
// "320 kbit/s", "8.4 MB", "Track −6.2 dB · Album −5.8 dB".

import type { App } from './app';
import type { AudioFormat, InfoRow, Item, KnownSong, QualityLabel } from './core/types';
import { formatTime, t, tn } from './i18n/index.svelte';
import { LIBRARY, LIBRARY_ID } from './state/library.svelte';

/** What the page knows of a song, for Get Info: the item, the library's file, the source's name. */
export function knownOf(app: App, plugin: string, item: Item): KnownSong {
  const known: KnownSong = {
    title: item.title,
    artist: item.artist,
    album: item.album,
    year: item.year === null ? null : String(item.year),
    duration_ms: item.duration_ms,
  };
  if (plugin === LIBRARY) {
    const track = app.library.track(item.id);
    if (track) {
      const folder = app.library.folders.find((f) => f.id === track.folder);
      known.file = folder ? `${folder.name}/${track.path}` : track.path;
      known.size_bytes = track.size;
      known.replaygain = track.replaygain;
      known.duration_ms ??= track.duration_ms;
    }
  } else {
    known.source = app.sources.get(plugin)?.name ?? plugin;
  }
  return known;
}

/** The format of a library song from its tags, or of the song playing; null when unknown. */
export function formatOf(app: App, plugin: string, item: Item): AudioFormat | null {
  const current = app.player.current;
  const playing =
    current !== null &&
    (current.kind === 'source' ? app.player.sourceItem(current)?.item.id === item.id : current.id === `${LIBRARY_ID}${item.id}`);
  if (playing && app.player.format) return app.player.format;
  if (plugin !== LIBRARY) return null;
  const track = app.library.track(item.id);
  if (!track) return null;
  const format = JSON.parse(
    app.jf.audio_format(track.codec ?? undefined, track.sample_rate ?? undefined, undefined, undefined, track.bitrate_kbps ?? undefined),
  ) as AudioFormat;
  return format.codec ? format : null;
}

export function qualityText(label: QualityLabel): string {
  switch (label) {
    case 'HiResLossless':
      return t('Hi-Res Lossless');
    case 'Lossless':
      return t('Lossless');
    case 'High':
      return t('High');
    case 'Standard':
      return t('Standard');
  }
}

/**
 * The quality menu's format line (ADR-0026 §3): "FLAC · 24-bit · 96 kHz" for lossless audio,
 * "AAC · 256 kbit/s" otherwise, with what is known of it; `codecName` writes the codec as people do
 * (jf-web's `codec_name`).
 */
export function formatLine(format: AudioFormat, codecName: (codec: string) => string = (codec) => codec): string {
  const lossless = format.lossless ?? (format.quality === 'Lossless' || format.quality === 'HiResLossless');
  const codec = format.codec ? codecName(format.codec) : null;
  const parts = lossless
    ? [codec, format.bit_depth ? t('{n}-bit', { n: format.bit_depth }) : null, format.sample_rate_hz ? sampleRateText(format.sample_rate_hz) : null]
    : [codec, format.bitrate_kbps ? t('{n} kbit/s', { n: format.bitrate_kbps.toLocaleString() }) : null];
  return parts.filter(Boolean).join(' · ');
}

/** A sample rate for people: 44.1 kHz, 48 kHz, 192 kHz. */
export function sampleRateText(hz: number): string {
  const khz = hz / 1000;
  return Math.abs(khz - Math.round(khz)) < 0.05 ? `${Math.round(khz)} kHz` : `${khz.toFixed(1)} kHz`;
}

/** +3.5 dB, −6.2 dB (a real minus), 0 dB. */
export function gainText(db: number): string {
  const rounded = Math.round(db * 10) / 10;
  const body = Math.abs(rounded % 1) < 0.05 ? String(Math.round(Math.abs(rounded))) : Math.abs(rounded).toFixed(1);
  return rounded > 0 ? `+${body} dB` : rounded < 0 ? `−${body} dB` : '0 dB';
}

function sizeText(bytes: number): string {
  const mb = bytes / 1_000_000;
  return mb >= 1000 ? t('{n} GB', { n: (mb / 1000).toFixed(1) }) : t('{n} MB', { n: mb.toFixed(1) });
}

export function rowLabel(row: InfoRow): string {
  switch (row.key) {
    case 'Title':
      return t('Title');
    case 'Artist':
      return t('Artist');
    case 'Album':
      return t('Album');
    case 'Version':
      return t('Version');
    case 'Year':
      return t('Year');
    case 'Genre':
      return t('Genre');
    case 'Composer':
      return t('Composer');
    case 'Lyricist':
      return t('Lyricist');
    case 'Label':
      return t('Label');
    case 'Length':
      return t('Length');
    case 'Quality':
      return t('Quality');
    case 'Format':
      return t('Format');
    case 'Bitrate':
      return t('Bitrate');
    case 'Size':
      return t('Size');
    case 'ReplayGain':
      return t('ReplayGain');
    case 'File':
      return t('File');
    case 'Source':
      return t('Source');
  }
}

export function rowValue(row: InfoRow): string {
  switch (row.key) {
    case 'Length':
      return formatTime(row.value);
    case 'Size':
      return sizeText(row.value);
    case 'Bitrate':
      return t('{n} kbit/s', { n: row.value.toLocaleString() });
    case 'Quality':
      return qualityText(row.value);
    case 'Format': {
      const f = row.value;
      const channels =
        f.channels === null ? null : f.channels === 1 ? t('Mono') : f.channels === 2 ? t('Stereo') : tn('{n} channel', '{n} channels', f.channels);
      return [f.codec, f.bit_depth ? t('{n}-bit', { n: f.bit_depth }) : null, f.sample_rate_hz ? sampleRateText(f.sample_rate_hz) : null, channels]
        .filter(Boolean)
        .join(' · ');
    }
    case 'ReplayGain': {
      const g = row.value;
      return [
        g.track_gain_db === null ? null : t('Track {gain}', { gain: gainText(g.track_gain_db) }),
        g.album_gain_db === null ? null : t('Album {gain}', { gain: gainText(g.album_gain_db) }),
      ]
        .filter(Boolean)
        .join(' · ');
    }
    default:
      return row.value;
  }
}
