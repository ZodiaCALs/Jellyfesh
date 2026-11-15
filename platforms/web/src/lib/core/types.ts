// The JSON shapes jf-web (core/jf-web, jf-plugin-model, jf-lyrics, jf-palette) reads and writes:
// serde's defaults, so field names are snake_case, unit enum variants are their Rust names
// ("Track") and payload variants are one-key objects ({"InvalidInput": "…"}).

import type { LyricTimeline } from '../drawing/lyricCursor';

export type ItemKind = 'Track' | 'Album' | 'Artist' | 'Playlist' | 'Folder';

export interface Header {
  name: string;
  value: string;
}

export interface ImageRef {
  url: string;
  headers: Header[];
  width: number | null;
  height: number | null;
}

export interface Unplayable {
  reason: 'Paywall' | 'Region' | 'Unavailable' | 'Other';
  message: string | null;
}

export interface Item {
  id: string;
  kind: ItemKind;
  title: string;
  artist: string | null;
  album: string | null;
  album_id: string | null;
  artist_id: string | null;
  duration_ms: number | null;
  track_number: number | null;
  disc_number: number | null;
  year: number | null;
  child_count: number | null;
  isrc: string | null;
  artwork: ImageRef | null;
  explicit: boolean;
  playable: boolean;
  unplayable: Unplayable | null;
}

export interface ItemPage {
  items: Item[];
  next: string | null;
  total: number | null;
}

export type Quality = 'Low' | 'Normal' | 'High' | 'Lossless';

export interface StreamInfo {
  url: string;
  headers: Header[];
  expires_at_ms: number | null;
  codec: string | null;
  bitrate_kbps: number | null;
  cacheable: boolean;
}

export type LyricFormat = 'Plain' | 'Lrc' | 'Elrc' | 'Ttml' | 'Vtt' | 'JsonSyllables';

export interface LyricPayload {
  format: LyricFormat;
  text: string;
  language: string | null;
  attribution: string | null;
}

export type Capability = 'Auth' | 'Search' | 'Browse' | 'ResolveStream' | 'Lyrics' | 'Artwork' | 'Recommendations';
export type AuthKind = 'Qr' | 'Oauth' | 'CookieImport' | 'PasswordExchange' | 'ApiKey';
export type FieldKind = 'Text' | 'Email' | 'Url' | 'Number' | 'CookieHeader';

export interface FieldSpec {
  name: string;
  label: string;
  kind: FieldKind;
  secret: boolean;
  required: boolean;
  placeholder: string | null;
  help: string | null;
}

export interface FieldValue {
  name: string;
  value: string;
}

export interface Flow {
  id: string;
  kind: AuthKind;
  state: string;
  oauth: null | {
    redirect_uri: string;
    code_challenge: string;
    code_challenge_method: string;
    state: string;
  };
}

export type Step =
  | { ShowQr: { data: string; hint: string | null; expires_at_ms: number | null } }
  | { ShowCode: { code: string; hint: string; expires_at_ms: number | null } }
  | { OpenUrl: { url: string; capture: 'None' | 'Loopback' } }
  | { AskFields: FieldSpec[] }
  | { Waiting: string }
  | { Done: string }
  | { Failed: string };

export interface FlowStep {
  step: Step;
  state: string;
  poll_after_ms: number | null;
}

/** One step for the sign-in sheet (jf_plugin_model::AuthStep). */
export type AuthStep =
  | { ShowQr: { data: string; hint: string | null; expires_at_ms: number | null } }
  | { ShowCode: { code: string; hint: string; expires_at_ms: number | null } }
  | { OpenUrl: { url: string } }
  | { AskFields: { fields: FieldSpec[] } }
  | { Waiting: { message: string } }
  | { Done: { account: string } }
  | { Failed: { message: string } };

/** What the page does next in a sign-in flow (jf_web::sources::FlowAction). */
export type FlowAction =
  | {
      next: 'call';
      call: 'begin' | 'poll' | 'submit';
      flow: Flow;
      fields: FieldValue[];
      protect: string[];
      keystore: boolean;
      restart: boolean;
    }
  | { next: 'poll'; step: AuthStep; after_ms: number }
  | { next: 'form'; step: AuthStep }
  | { next: 'redirect'; step: AuthStep }
  | { next: 'finish'; step: AuthStep };

export type PluginError =
  | 'Unsupported'
  | 'AuthRequired'
  | 'NotFound'
  | 'RateLimited'
  | { InvalidInput: string }
  | { Network: string }
  | { Service: string }
  | { Internal: string };

export type AllowRule = { Setting: string } | { Host: { host: string; wildcard: boolean; port: number } };

export interface ContentPolicy {
  unified_search: boolean;
  mixed_queue: boolean;
  library_storage: 'None' | 'Metadata' | 'Full';
  metadata_ttl_secs: number | null;
  lyrics_sync: boolean;
  attribution: string | null;
}

export type SignatureStatus =
  | 'Unsigned'
  | { Trusted: { key_id: string; label: string } }
  | { Community: { key_id: string } };

/** A package as a plugin worker read and verified it (jf_web_sandbox::PackageInfo). */
export interface PackageInfo {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string | null;
  official: boolean;
  signature: SignatureStatus;
  api_version: string;
  capabilities: Capability[];
  auth_kinds: AuthKind[];
  playback_mode: 'Stream' | 'Vendor' | 'Remote';
  allow: AllowRule[];
  max_rps: number;
  settings_schema: SettingsSchema;
  settings_defaults: Record<string, unknown>;
  content_policy: ContentPolicy;
  wasm_sha256: string;
  has_icon: boolean;
}

export interface SettingsSchema {
  title?: string;
  description?: string;
  type?: 'object';
  properties?: Record<
    string,
    {
      type: 'string' | 'integer' | 'number' | 'boolean';
      title?: string;
      description?: string;
      default?: unknown;
      enum?: string[];
      format?: 'uri' | 'email';
      minLength?: number;
      maxLength?: number;
      minimum?: number;
      maximum?: number;
    }
  >;
  required?: string[];
}

// ---- the session ------------------------------------------------------------------------------

export type RepeatMode = 'off' | 'all' | 'one';

/** A queue entry as the views show it (jf_web::session::Row). */
export interface Row {
  uid: number;
  kind: 'source' | 'file';
  plugin: string | null;
  id: string;
  title: string;
  artist: string | null;
  album: string | null;
  duration_ms: number | null;
  artwork_url: string | null;
  album_key: string | null;
}

export interface QueueSnapshot {
  revision: number;
  length: number;
  position: number | null;
  current: Row | null;
  shuffle: boolean;
  repeat: RepeatMode;
  up_next: Row[];
}

export interface HistoryRow {
  title: string;
  artist: string | null;
  plugin: string | null;
  played_at_ms: number;
}

export type NewEntry = { kind: 'source'; plugin: string; item: Item } | { kind: 'file'; id: string };

export type Transition = { kind: 'gapless' } | { kind: 'crossfade'; overlap_ms: number };

export type PreviousAction = { action: 'restart' } | { action: 'play'; row: Row };

export interface ReplayGainInfo {
  track_gain_db: number | null;
  track_peak: number | null;
  album_gain_db: number | null;
  album_peak: number | null;
}

export interface FileMeta {
  id: string;
  name: string;
  title: string;
  artist: string | null;
  album: string | null;
  duration_ms: number | null;
  replaygain: ReplayGainInfo | null;
}

/** A dragged-in file's tags (jf_web_media::tags::FileTags). */
export interface FileTags {
  title: string | null;
  artist: string | null;
  album: string | null;
  album_artist: string | null;
  /** Each ARTIST value and the ARTISTS list as tagged (the library splits them into artists). */
  artist_values: string[];
  explicit_artists: string[];
  year: number | null;
  track_no: number | null;
  disc_no: number | null;
  duration_ms: number | null;
  codec: string | null;
  sample_rate: number | null;
  channels: number | null;
  bitrate_kbps: number | null;
  replaygain: ReplayGainInfo | null;
  lyrics: string | null;
  cover_mime: string | null;
}

// ---- settings ---------------------------------------------------------------------------------

export type BandKind = 'Peaking' | 'LowShelf' | 'HighShelf';

export interface EqBand {
  kind: BandKind;
  freq_hz: number;
  gain_db: number;
  q: number;
  enabled: boolean;
}

export interface EqSettings {
  enabled: boolean;
  preamp_db: number;
  bands: EqBand[];
}

export interface CrossfadeSettings {
  enabled: boolean;
  duration_ms: number;
  album_continuity: boolean;
}

export type LyricsSize = 'small' | 'medium' | 'large';
export type LyricsAlign = 'start' | 'center';

export interface WebSettings {
  appearance: 'system' | 'light' | 'dark';
  style: 'artwork' | 'misty';
  hue_source: { kind: 'artwork' } | { kind: 'fixed'; hue: number };
  reduce_effects: boolean;
  /** How see-through glass is: 0 solid … 1 clearest, 0.5 the tokens' own alphas (clamped by the core). */
  transparency: number;
  lyrics_size: LyricsSize;
  lyrics_align: LyricsAlign;
  show_translation: boolean;
  show_romanization: boolean;
  crossfade: CrossfadeSettings;
  replaygain_mode: 'Off' | 'Track' | 'Album';
  preamp_db: number;
  eq: EqSettings;
  resume_on_launch: boolean;
  quality: Quality;
  language: string | null;
}

// ---- lyrics and themes ------------------------------------------------------------------------

export interface Word {
  start_ms: number;
  end_ms: number;
  text: string;
  joins_next: boolean;
}

export interface Line {
  id: number;
  start_ms: number | null;
  end_ms: number | null;
  agent: string | null;
  text: string;
  words: Word[];
  background: Word[];
  direction: 'ltr' | 'rtl' | 'auto';
}

export interface AuxTrack {
  kind: 'translation' | 'romanization';
  language: string | null;
  /** By line id (`Line.id`, as a string key). */
  lines: Record<string, { text: string; words: Word[] }>;
}

export interface LyricDoc {
  language: string | null;
  timing: 'unsynced' | 'line' | 'word';
  offset_ms: number;
  lines: Line[];
  tracks: AuxTrack[];
  source: { format: string; origin: string | null };
  metadata: Record<string, string>;
}

export interface Lyrics {
  doc: LyricDoc;
  timeline: LyricTimeline;
  synced: boolean;
}

/** A Misty theme: every colour by token name, as RGBA bytes. */
export interface Theme {
  hue: number;
  appearance: 'light' | 'dark';
  colors: Record<string, [number, number, number, number]>;
}

export interface Themes {
  hue: number;
  from_artwork: boolean;
  light: Theme;
  dark: Theme;
}

// ---- the music library (jf_web::library) ----------------------------------------------------------

/** A song in the library as the page keeps it (IndexedDB) and hands it to the catalogue. */
export interface LibTrack {
  /** `<folder>/<path>`. */
  key: string;
  folder: number;
  /** Where the file is inside the folder, `/`-separated. */
  path: string;
  title: string | null;
  artist: string | null;
  artist_values: string[];
  explicit_artists: string[];
  album: string | null;
  album_artist: string | null;
  year: number | null;
  track_no: number | null;
  disc_no: number | null;
  duration_ms: number | null;
  replaygain: ReplayGainInfo | null;
  /** The cover's id in the artwork store. */
  artwork: string | null;
  /** The lyrics file beside it, by its path in the folder. */
  lyrics_path: string | null;
  size: number;
  modified_ms: number;
  added_ms: number;
  /** For Now Playing › Info (the catalogue ignores them). */
  codec: string | null;
  sample_rate: number | null;
  bitrate_kbps: number | null;
}

export interface LibTrackView {
  key: string;
  title: string;
  artist: string | null;
  artist_id: string | null;
  album: string | null;
  album_id: string | null;
  year: number | null;
  track_no: number | null;
  disc_no: number | null;
  duration_ms: number | null;
  artwork: string | null;
}

export interface LibAlbumView {
  id: string;
  title: string;
  artist: string | null;
  artist_id: string | null;
  year: number | null;
  track_count: number;
  duration_ms: number;
  artwork: string | null;
}

export interface LibArtistView {
  id: string;
  name: string;
  album_count: number;
  track_count: number;
  artwork: string | null;
}

export interface LibStats {
  tracks: number;
  albums: number;
  artists: number;
  duration_ms: number;
}

export type AlbumSort = 'title' | 'artist' | 'year' | 'added';
export type TrackSort = 'title' | 'artist' | 'album' | 'year' | 'duration' | 'path' | 'added';
