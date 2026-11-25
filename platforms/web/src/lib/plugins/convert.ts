// Conversions between the values a jco-transpiled plugin takes and returns (WIT records as
// camelCase objects, enums as kebab-case strings, variants as {tag, val}, options as undefined,
// u64 as bigint) and the core's JSON shapes (src/lib/core/types.ts, serde's defaults). Pure data
// mapping, like jf-plugin-host's convert.rs; no decisions.

import type {
  AuthKind,
  Capability,
  CommentPage,
  DownloadInfo,
  FactKey,
  FieldValue,
  Flow,
  FlowStep,
  ImageRef,
  Item,
  ItemDetails,
  ItemKind,
  ItemPage,
  Look,
  LyricPayload,
  Made,
  MakerPlan,
  MakeSong,
  PlaylistPage,
  PluginError,
  Quality,
  Shelf,
  Step,
  StreamInfo,
} from '../core/types';

/* eslint-disable @typescript-eslint/no-explicit-any */
type Jco = any;

/** `resolve-stream` → `ResolveStream`. */
export function pascal(kebab: string): string {
  return kebab
    .split('-')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join('');
}

/** `ResolveStream` → `resolve-stream`. */
export function kebab(pascalName: string): string {
  return pascalName.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase();
}

const opt = <T>(value: T | undefined | null): T | null => (value === undefined ? null : value);
const und = <T>(value: T | null | undefined): T | undefined => (value === null ? undefined : value);
const num = (value: bigint | number | undefined | null): number | null =>
  value === undefined || value === null ? null : Number(value);

export function imageFromJco(i: Jco): ImageRef {
  return {
    url: i.url,
    headers: i.headers.map((h: Jco) => ({ name: h.name, value: h.value })),
    width: opt(i.width),
    height: opt(i.height),
  };
}

export function imageToJco(i: ImageRef): Jco {
  return { url: i.url, headers: i.headers, width: und(i.width), height: und(i.height) };
}

export function itemFromJco(i: Jco): Item {
  return {
    id: i.id,
    kind: pascal(i.kind) as ItemKind,
    title: i.title,
    artist: opt(i.artist),
    album: opt(i.album),
    album_id: opt(i.albumId),
    artist_id: opt(i.artistId),
    duration_ms: opt(i.durationMs),
    track_number: opt(i.trackNumber),
    disc_number: opt(i.discNumber),
    year: opt(i.year),
    child_count: opt(i.childCount),
    isrc: opt(i.isrc),
    artwork: i.artwork ? imageFromJco(i.artwork) : null,
    explicit: i.explicit,
    playable: i.playable,
    unplayable: i.unplayable ? { reason: pascal(i.unplayable.reason) as never, message: opt(i.unplayable.message) } : null,
  };
}

export function itemToJco(i: Item): Jco {
  return {
    id: i.id,
    kind: kebab(i.kind),
    title: i.title,
    artist: und(i.artist),
    album: und(i.album),
    albumId: und(i.album_id),
    artistId: und(i.artist_id),
    durationMs: und(i.duration_ms),
    trackNumber: und(i.track_number),
    discNumber: und(i.disc_number),
    year: und(i.year),
    childCount: und(i.child_count),
    isrc: und(i.isrc),
    artwork: i.artwork ? imageToJco(i.artwork) : undefined,
    explicit: i.explicit,
    playable: i.playable,
    unplayable: i.unplayable ? { reason: kebab(i.unplayable.reason), message: und(i.unplayable.message) } : undefined,
  };
}

export function pageFromJco(p: Jco): ItemPage {
  return { items: p.items.map(itemFromJco), next: opt(p.next), total: opt(p.total) };
}

/** `library.playlist-page` (API 1.3). */
export function playlistPageFromJco(p: Jco): PlaylistPage {
  return {
    entries: p.entries.map((e: Jco) => ({ item: itemFromJco(e.item), owned: e.owned, songs_listed: e.songsListed })),
    next: opt(p.next),
    total: opt(p.total),
  };
}

/** `download.download-info` (API 1.3). */
export function downloadFromJco(d: Jco): DownloadInfo {
  return {
    url: d.url,
    headers: d.headers.map((h: Jco) => ({ name: h.name, value: h.value })),
    container: d.container,
    size: num(d.size),
    original: d.original,
    file_name: opt(d.fileName),
  };
}

export function pageRequestToJco(cursor: string | null, limit: number): Jco {
  return { cursor: und(cursor), limit };
}

export function qualityToJco(q: Quality): Jco {
  return kebab(q);
}

export function streamFromJco(s: Jco): StreamInfo {
  return {
    url: s.url,
    headers: s.headers.map((h: Jco) => ({ name: h.name, value: h.value })),
    expires_at_ms: num(s.expiresAtMs),
    codec: opt(s.codec),
    bitrate_kbps: opt(s.bitrateKbps),
    cacheable: s.cacheable,
  };
}

export function lyricsFromJco(p: Jco): LyricPayload {
  return {
    format: pascal(p.format) as LyricPayload['format'],
    text: p.text,
    language: opt(p.language),
    attribution: opt(p.attribution),
  };
}

export function capabilityFromJco(c: string): Capability {
  return pascal(c) as Capability;
}

export function flowToJco(f: Flow): Jco {
  return {
    id: f.id,
    kind: kebab(f.kind),
    state: f.state,
    oauth: f.oauth
      ? {
          redirectUri: f.oauth.redirect_uri,
          codeChallenge: f.oauth.code_challenge,
          codeChallengeMethod: f.oauth.code_challenge_method,
          state: f.oauth.state,
        }
      : undefined,
  };
}

export function authKindToJco(k: AuthKind): string {
  return kebab(k);
}

export function fieldsToJco(fields: FieldValue[]): Jco {
  return fields.map((f) => ({ name: f.name, value: f.value }));
}

export function flowStepFromJco(s: Jco): FlowStep {
  const v = s.step.val;
  let step: Step;
  switch (s.step.tag) {
    case 'show-qr':
      step = { ShowQr: { data: v.data, hint: opt(v.hint), expires_at_ms: num(v.expiresAtMs) } };
      break;
    case 'show-code':
      step = { ShowCode: { code: v.code, hint: v.hint, expires_at_ms: num(v.expiresAtMs) } };
      break;
    case 'open-url':
      step = { OpenUrl: { url: v.url, capture: pascal(v.capture) as 'None' | 'Loopback' } };
      break;
    case 'ask-fields':
      step = {
        AskFields: v.map((f: Jco) => ({
          name: f.name,
          label: f.label,
          kind: pascal(f.kind),
          secret: f.secret,
          required: f.required,
          placeholder: opt(f.placeholder),
          help: opt(f.help),
        })),
      };
      break;
    case 'waiting':
      step = { Waiting: v };
      break;
    case 'done':
      step = { Done: v };
      break;
    default:
      step = { Failed: String(v ?? 'The sign-in failed.') };
  }
  return { step, state: s.state, poll_after_ms: opt(s.pollAfterMs) };
}

/** A plugin error variant ({tag, val}) as the core's `PluginError`. */
export function pluginErrorFromJco(e: Jco): PluginError {
  const name = pascal(e.tag);
  return e.val === undefined ? (name as PluginError) : ({ [name]: String(e.val) } as PluginError);
}

/** Text for a plugin error, for people (as `jf_plugin_host::plugin_error_text`). */
export function pluginErrorText(e: PluginError): string {
  if (typeof e === 'string') {
    switch (e) {
      case 'Unsupported':
        return 'not supported by this plugin';
      case 'AuthRequired':
        return 'you need to log in';
      case 'NotFound':
        return 'not found';
      case 'RateLimited':
        return 'the service asked to slow down';
    }
  }
  if ('InvalidInput' in e) return `invalid input: ${e.InvalidInput}`;
  if ('Network' in e) return `network error: ${e.Network}`;
  if ('Service' in e) return e.Service;
  return `plugin error: ${e.Internal}`;
}

// ---- API 1.2 --------------------------------------------------------------------------------------

export function shelvesFromJco(shelves: Jco[]): Shelf[] {
  return shelves.map((s) => ({
    id: s.id,
    title: s.title,
    style: pascal(s.style) as Shelf['style'],
    items: s.items.map(itemFromJco),
    more: opt(s.more),
  }));
}

function factKey(k: Jco): FactKey {
  return k.tag === 'other' ? { Other: String(k.val) } : (pascal(k.tag) as FactKey);
}

export function detailsFromJco(d: Jco): ItemDetails {
  return {
    description: opt(d.description),
    image: d.image ? imageFromJco(d.image) : null,
    facts: d.facts.map((f: Jco) => ({ key: factKey(f.key), value: f.value })),
    comment_count: num(d.commentCount),
  };
}

export function commentsFromJco(p: Jco): CommentPage {
  const comment = (c: Jco) => ({
    id: c.id,
    author: c.author,
    avatar: c.avatar ? imageFromJco(c.avatar) : null,
    text: c.text,
    likes: num(c.likes),
    time_ms: num(c.timeMs),
    replies: num(c.replies),
  });
  return { hot: p.hot.map(comment), comments: p.comments.map(comment), total: num(p.total), next: opt(p.next) };
}

export function planFromJco(p: Jco): MakerPlan {
  const need = p.need.tag === 'model' ? { Model: String(p.need.val) } : p.need.tag === 'unavailable' ? { Unavailable: String(p.need.val) } : pascal(p.need.tag);
  return {
    mode: pascal(p.mode) as MakerPlan['mode'],
    engine: p.engine,
    upload_to: opt(p.uploadTo),
    need: need as MakerPlan['need'],
    automatic: p.automatic,
  };
}

export function songToJco(song: MakeSong, track: bigint): Jco {
  return {
    track,
    title: song.title,
    artist: und(song.artist),
    album: und(song.album),
    durationMs: BigInt(Math.max(0, Math.round(song.duration_ms))),
    text: und(song.text),
    language: und(song.language),
  };
}

export function madeFromJco(m: Jco): Made {
  return { lyrics: lyricsFromJco(m.lyrics), engine: m.engine };
}

export function lookFromJco(l: Jco): Look {
  const highlight = l.lyricsHighlight
    ? l.lyricsHighlight.tag === 'custom'
      ? { Custom: Number(l.lyricsHighlight.val) }
      : pascal(l.lyricsHighlight.tag)
    : null;
  const list = <T,>(values: string[] | undefined) => (values ? (values.map(pascal) as T[]) : null);
  return {
    corner_scale: opt(l.cornerScale),
    glass: opt(l.glass),
    aurora: opt(l.aurora),
    typeface: l.typeface ? (pascal(l.typeface) as Look['typeface']) : null,
    text_scale: opt(l.textScale),
    density: l.density ? (pascal(l.density) as Look['density']) : null,
    lyrics_weight: l.lyricsWeight ? (pascal(l.lyricsWeight) as Look['lyrics_weight']) : null,
    lyrics_glow: opt(l.lyricsGlow),
    lyrics_highlight: highlight as Look['lyrics_highlight'],
    sidebar: list(l.sidebar),
    home: list(l.home),
    now_playing: l.nowPlaying ? (pascal(l.nowPlaying) as Look['now_playing']) : null,
    mini_player: l.miniPlayer ? (pascal(l.miniPlayer) as Look['mini_player']) : null,
  };
}
