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

/** Streaming Quality (ADR-0026 §1; the core's `StreamingQuality`): Auto is Lossless, or High on a
 *  metered connection. */
export type StreamingQuality = 'auto' | 'lossless' | 'high' | 'normal' | 'low';

/** Why the quality menu offers no levels (jf_web::quality::QualityNote). */
export type QualityNote = { kind: 'files_play_as_they_are' } | { kind: 'set_in_service'; service: string };

/** The quality menu for what plays (jf_web::quality::Menu). */
export interface QualityMenuInfo {
  /** Auto, then the source's levels best first; empty when there is no choice. */
  qualities: StreamingQuality[];
  note: QualityNote | null;
}

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

export type Capability =
  | 'Auth'
  | 'Search'
  | 'Browse'
  | 'ResolveStream'
  | 'Lyrics'
  | 'Artwork'
  | 'Recommendations'
  /** API 1.1: plays on the person's own devices (a manifest capability; Jellyfesh Web refuses such plugins). */
  | 'Remote'
  /** API 1.2 (manifest capabilities): making lyrics and the look (Pro), Discover, details, comments. */
  | 'MakeLyrics'
  | 'Appearance'
  | 'Discover'
  | 'Details'
  | 'Comments'
  /** API 1.3 (manifest capabilities): the person's likes and playlists, and downloads. */
  | 'Library'
  | 'Download';

/** API 1.2: a host import the manifest declares. */
export type Permission = 'Audio' | 'Speech';
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
  /** API 1.3: the streaming levels the source honours, best first (missing before). */
  qualities?: Quality[];
}

export type SignatureStatus =
  | 'Unsigned'
  | { Trusted: { key_id: string; label: string } }
  | { Community: { key_id: string } };

/** The manifest's `[links]` (API 1.1): https URL templates holding `{id}`, filled in by jf-web. */
export interface Links {
  track: string | null;
  album: string | null;
  artist: string | null;
  playlist: string | null;
}

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
  /** Missing from packages kept before API 1.1. */
  links?: Links;
  wasm_sha256: string;
  has_icon: boolean;
  /** API 1.2; missing from packages kept before it. */
  permissions?: Permission[];
  /** The schema translated by the package, by language tag (API 1.2). */
  localized_schemas?: Record<string, SettingsSchema>;
  requires_pro?: boolean;
  extension?: boolean;
}

// ---- API 1.3 answers (jf_plugin_model's types, as serde writes them) --------------------------

/** A playlist in the person's library on a service (`library.playlist-entry`). */
export interface SourcePlaylistEntry {
  item: Item;
  /** The person's own playlist, or one they collaborate on. */
  owned: boolean;
  /** The service lists its songs here (false: only in its own app). */
  songs_listed: boolean;
}

export interface PlaylistPage {
  entries: SourcePlaylistEntry[];
  next: string | null;
  total: number | null;
}

/** Where to fetch a song's file (`download.download-info`), checked by jf-web. */
export interface DownloadInfo {
  url: string;
  headers: { name: string; value: string }[];
  /** The file's extension, without the dot. */
  container: string;
  size: number | null;
  /** The file exactly as the server keeps it. */
  original: boolean;
  file_name: string | null;
}

// ---- the sources' likes and playlists (jf_web::source_library) --------------------------------

export type SourceLibraryPart = 'liked_songs' | 'liked_albums' | 'liked_artists' | 'playlists';

export interface SourceLibraryEntry {
  item: Item;
  owned: boolean;
  songs_listed: boolean;
}

export interface SourceLibraryList {
  part: SourceLibraryPart;
  entries: SourceLibraryEntry[];
  total: number | null;
  next: string | null;
  refreshed_at_ms: number | null;
  loading: boolean;
  error: string | null;
}

export interface SourceLibrary {
  plugin: string;
  name: string;
  attribution: string | null;
  /** In the order Liked Songs, Liked Albums, Liked Artists, Playlists; lists it does not keep left out. */
  lists: SourceLibraryList[];
}

// ---- API 1.2 answers (jf_plugin_model's types, as serde writes them) --------------------------

export type ShelfStyle = 'Row' | 'Grid' | 'Chart';

export interface Shelf {
  id: string;
  title: string;
  style: ShelfStyle;
  items: Item[];
  /** A node for See All (browse it). */
  more: string | null;
}

// ---- details (jf_web::details) --------------------------------------------------------------

export type QualityLabel = 'HiResLossless' | 'Lossless' | 'High' | 'Standard';

/** The format of what plays or of a file, with the core's quality label. */
export interface AudioFormat {
  codec: string | null;
  sample_rate_hz: number | null;
  bit_depth: number | null;
  channels: number | null;
  bitrate_kbps: number | null;
  lossless: boolean | null;
  quality: QualityLabel | null;
}

/** One row of Get Info, by key; the page words it. */
export type InfoRow =
  | { key: 'Title' | 'Artist' | 'Album' | 'Version' | 'Year' | 'Genre' | 'Composer' | 'Lyricist' | 'Label' | 'File' | 'Source'; value: string }
  | { key: 'Length' | 'Size'; value: number }
  | { key: 'Bitrate'; value: number }
  | { key: 'Quality'; value: QualityLabel }
  | { key: 'Format'; value: { codec: string | null; bit_depth: number | null; sample_rate_hz: number | null; channels: number | null } }
  | { key: 'ReplayGain'; value: ReplayGainInfo };

/** What the page knows of a song, for its rows (jf_web::details::Known). */
export interface KnownSong {
  title?: string | null;
  artist?: string | null;
  album?: string | null;
  year?: string | null;
  duration_ms?: number | null;
  size_bytes?: number | null;
  replaygain?: ReplayGainInfo | null;
  file?: string | null;
  source?: string | null;
}

export interface SongRows {
  about: InfoRow[];
  audio: InfoRow[];
}

/** A source's Discover page (jf_web::discover). */
export interface DiscoverPage {
  shelves: Shelf[];
  error: string | null;
  /** What See All browses, by shelf id. */
  see_all: Record<string, Item>;
  /** Made by Jellyfesh rather than the plugin: shelves are worded by id (`for_you`). */
  made: boolean;
}

export type FactKey =
  | 'Version'
  | 'Genre'
  | 'Language'
  | 'Released'
  | 'Label'
  | 'Composer'
  | 'Lyricist'
  | 'Arranger'
  | 'Producer'
  | 'Popularity'
  | 'Quality'
  | { Other: string };

export interface Fact {
  key: FactKey;
  value: string;
}

export interface ItemDetails {
  description: string | null;
  image: ImageRef | null;
  facts: Fact[];
  comment_count: number | null;
}

export interface Comment {
  id: string;
  author: string;
  avatar: ImageRef | null;
  text: string;
  likes: number | null;
  time_ms: number | null;
  replies: number | null;
}

export interface CommentPage {
  hot: Comment[];
  comments: Comment[];
  total: number | null;
  next: string | null;
}

export type MakerNeed = 'Nothing' | 'SignIn' | { Model: string } | { Unavailable: string };

export interface MakerPlan {
  mode: 'OnDevice' | 'Online';
  engine: string;
  upload_to: string | null;
  need: MakerNeed;
  automatic: boolean;
}

export interface MakeSong {
  title: string;
  artist: string | null;
  album: string | null;
  duration_ms: number;
  text: string | null;
  language: string | null;
}

export interface Made {
  lyrics: LyricPayload;
  engine: string;
}

export type Typeface = 'System' | 'Rounded' | 'Serif' | 'Mono';
export type Density = 'Compact' | 'Standard' | 'Roomy';
export type LyricsWeight = 'Regular' | 'Medium' | 'Semibold' | 'Bold' | 'Heavy';
export type Highlight = 'Artwork' | 'Accent' | { Custom: number };
export type Place = 'Home' | 'Albums' | 'Artists' | 'Songs' | 'Playlists' | 'Favourites' | 'Search' | 'Sources';
export type HomeSection = 'Summary' | 'RecentlyAdded' | 'RecentlyPlayed' | 'MostPlayed' | 'Favourites';
export type NowPlayingLayout = 'Stage' | 'Artwork' | 'Lyrics';
export type MiniPlayerStyle = 'Bar' | 'Compact' | 'Floating';

/** An appearance plugin's look (API 1.2); every field optional. */
export interface Look {
  corner_scale: number | null;
  glass: number | null;
  aurora: number | null;
  typeface: Typeface | null;
  text_scale: number | null;
  density: Density | null;
  lyrics_weight: LyricsWeight | null;
  lyrics_glow: number | null;
  lyrics_highlight: Highlight | null;
  sidebar: Place[] | null;
  home: HomeSection[] | null;
  now_playing: NowPlayingLayout | null;
  mini_player: MiniPlayerStyle | null;
}

/** One setting of a plugin's schema (profiles 1.1 and 1.2). */
export interface SettingSpec {
  type: 'string' | 'integer' | 'number' | 'boolean' | 'array';
  title?: string;
  description?: string;
  default?: unknown;
  enum?: string[];
  /** Profile 1.2: `color` is `#RRGGBB` (a colour well). */
  format?: 'uri' | 'email' | 'color';
  minLength?: number;
  maxLength?: number;
  minimum?: number;
  maximum?: number;
  /** Profile 1.2: a slider's step. */
  multipleOf?: number;
  /** Profile 1.2: choices with titles. */
  oneOf?: { const: string | number; title?: string }[];
  /** Profile 1.2: an ordered checklist's items (`type: "array"`, `uniqueItems`). */
  items?: { type?: string; enum?: string[]; oneOf?: { const: string; title?: string }[] };
  uniqueItems?: boolean;
  /** Profile 1.2: consecutive settings with the same group share a heading. */
  'x-jf-group'?: string;
}

export interface SettingsSchema {
  title?: string;
  description?: string;
  type?: 'object';
  properties?: Record<string, SettingSpec>;
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

/** A remote session's devices (the core's `Remote` slice, ADR-0018), for the device button. */
export interface RemoteDevices {
  devices: { id: string; name: string; active: boolean }[];
}

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
  /** Streaming Quality; `auto` by default. */
  streaming_quality: StreamingQuality;
  language: string | null;
  /** Show Lyrics Bar (Settings › Appearance › Lyrics); on by default. */
  show_lyrics_bar: boolean;
  /** Desktop Lyrics: a Document Picture-in-Picture window, where the browser offers one; off by default. */
  desktop_lyrics: boolean;
  /** Settings › Appearance › Mini Player (the core's; the web shows Show Lyrics Line only). */
  mini_player: MiniPlayerSettings;
}

export interface MiniPlayerSettings {
  keep_on_top: boolean;
  open_when_window_closed: boolean;
  /** The lyrics bar's current line in the mini player; on by default. */
  show_lyrics_line: boolean;
  android_pip: boolean;
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

// ---- hearts, playlists and plays (jf_web::collections) --------------------------------------

/** A song as hearts, playlists and plays keep it: a source's item with its plugin's id, or a
 *  library song under `library` with its key as id. */
export interface Saved {
  plugin: string;
  item: Item;
}

/** Why the core refused a change to hearts or playlists (the error's message). */
export type CollectionError = 'empty-name' | 'no-playlist' | 'no-entry' | 'not-a-song';

export interface PlaylistSummary {
  id: number;
  name: string;
  entry_count: number;
  duration_ms: number;
  created_at_ms: number;
  updated_at_ms: number;
  /** The first entry with artwork. */
  cover: Saved | null;
}

export interface PlaylistEntry {
  entry_id: number;
  position: number;
  plugin: string;
  item: Item;
}

export interface ItemStats {
  plugin: string;
  item: Item;
  favorite: boolean;
  favorited_at_ms: number | null;
  play_count: number;
  skip_count: number;
  last_played_ms: number | null;
}

/** A play that ended (Core.finish). */
export interface FinishedPlay {
  uid: number;
  counted: boolean;
  started_ms: number;
  /** What the entry is for hearts and plays; null for a dropped file. */
  saved: Saved | null;
}

// ---- search by source (jf_web::search) ------------------------------------------------------------

export interface SearchPlan {
  kinds: ItemKind[];
  limit: number;
  library_limit: number;
}

export interface SectionRow {
  item: Item;
  also_in: string[];
}

export type SearchStatus = 'searching' | 'done' | 'failed';

/** A merging source's section on the All tab (From Your Sources). */
export interface SearchSection {
  source: string;
  name: string;
  status: SearchStatus;
  error: string | null;
  rows: SectionRow[];
}

export interface SearchTabLink {
  source: string;
  name: string;
  merged: boolean;
}

export interface SearchView {
  library: SectionRow[];
  sections: SearchSection[];
  tabs: SearchTabLink[];
  done: boolean;
  empty: boolean;
}

/** One kind of one source's results (the core's `SearchGroup`). */
export interface SearchGroup {
  kind: ItemKind;
  items: Item[];
  /** Show More goes on from here; null when there is no more. */
  next: string | null;
  total: number | null;
}

export interface SearchTab {
  source: string;
  name: string;
  merged: boolean;
  groups: SearchGroup[];
}
