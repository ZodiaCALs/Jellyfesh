# The core API (v0.0.8)

Every UI (the Linux app in Rust, the macOS, Windows and Android apps through
the FFI bindings, and the `jf` CLI) talks to `jf-core` the same way, and
only this way (ADR-0001). Shells keep no business logic: no sorting,
filtering, matching, persistence or networking.

Source of truth: `core/jf-core/src/types.rs` (types), `core/jf-core/src/lib.rs`
(the `Core` facade), `core/jf-core/src/timing.rs` (drawing helpers).

## Lifecycle

```rust
let core = Core::start(Config::new(data_dir, cache_dir))?; // opens the library and session
let events = core.subscribe();                             // crossbeam Receiver<Event>
let snapshot = core.snapshot();                            // every slice, now
let id = core.send(Command::PlayCollection { collection: Collection::Album { id }, start: 0, shuffle: false });
// failures come back as Event::CommandFailed { id, error }
core.shutdown();                                           // saves the session
```

Subscribe before sending commands whose events you need. Events arrive on
any thread; UIs hop to their main thread. Commands are fire-and-forget.
`shutdown` lets plugin installs, uninstalls and logouts already sent finish
first (at most a minute) and still reports their results.

`Config` also carries `sources: SourcesConfig` (the plugin host on or off,
the app's `SecretStore`, the host's trust list and limits for tests, the
per-source search deadline, 8 s), `bundled_plugins: Vec<PathBuf>` (see
"Online sources"), `distribution: Distribution` (how this build reached the
person: `Direct`, `AppStore`, `MicrosoftStore`, `GooglePlay`, `Web` or
`Source`, the default; it decides which stores sell Pro and whether the core
checks for updates) and `speech: Option<Arc<dyn jf_speech::Recognizer>>`
(on-device recognition for AI Lyrics: `jf_speech::recognizer()` in builds with
Whisper; `None` makes the on-device mode unavailable).

## Slices (in `Snapshot`, and as events when they change)

| Slice | Event | Holds |
| --- | --- | --- |
| `playback: PlaybackSnapshot` | `Playback` | status, **anchor**, volume, eq, ReplayGain |
| `queue: QueueSnapshot` | `Queue` | length, position, current, shuffle, repeat, next 50 entries (`queue_window` pages the rest) |
| `now_playing` | `NowPlaying` | uid, track, title, artist, album, duration, artwork (`ArtworkRef::File` or `Url`), stream format, `link` (opens a source track in its service) |
| `lyrics` | `Lyrics` | `LyricDoc` + `LyricTimeline` for the current track, or none |
| `theme` | `Theme` | hue, light and dark Misty themes, baked backdrops (light, dark) |
| `settings: Settings` | `Settings` | all preferences (below) |
| `devices: DevicesSnapshot` | `Devices` | output devices and the selected one (`None` = system default) |
| `sources: SourcesSnapshot` | `Sources` | installed plugins (badge, signing key, capabilities, sign-in methods, account, settings schema and values, icon, why one is off, vendor or remote playback), the services offered in Add a Source (`presets`), community allowed, where sign-ins are kept, why sources are unavailable |
| `remote: RemoteSnapshot` | `Remote` | the remote session (ADR-0018): source and its name, status, the track and its album or playlist (as `SourceItem`s), **anchor**, shuffle, repeat, the playing device, every device, the last error; `source` is `None` without a session |
| `muted`, `scanning`, `library_revision` | `LibraryChanged`, `Scan` | `LibraryChanged` after any catalogue, playlist, favourite or play-count change: lists query again |
| `ai_lyrics: AiLyricsSnapshot` | `AiLyrics` | AI Lyrics (ADR-0019): whether to offer it, the plugin, its plan, what is needed first, the jobs, the speech models, the consents; see "AI Lyrics" |
| `look: LookSnapshot` | `Look` | the look, every value resolved (ADR-0020); see "The look" |
| `pro: ProSnapshot` | `Pro` | Jellyfesh Pro (ADR-0021); see "Jellyfesh Pro" |
| `update: UpdateSnapshot` | `Update` | update checks for direct downloads; see "Updates" |
| `source_library: SourceLibrarySnapshot` | `SourceLibrary` | each signed-in source's likes and playlists (ADR-0024); see "Sources' likes and playlists" |
| `downloads: DownloadsSnapshot` | `Downloads` | the downloads folder, downloads in progress and finished, why they wait (ADR-0025); see "Downloads" |

Other events: `PlaylistCreated { request, id }` (after `CreatePlaylist` and
`ImportPlaylist`), `Auth(AuthUpdate)` (sign-in steps),
`SearchResults(SearchUpdate)` (one per source per search, see below; `done`
on the last), `BrowsePage`, `Buffering`, `CommandFailed`, `Error`; and from
v0.0.7 `Discover(DiscoverPage)`, `Details(DetailsPage)`,
`Comments(CommentsPage)` (answers to the commands of those names, each with
the `request` id `send` returned) and `Artwork(ArtworkReady)` (a source
picture is cached, see "Covers"); and from v0.0.8 `SearchPage(SearchPage)`
(Show More, see "Search by source") and `SourceLibraryPage(SourceLibraryPage)`
(more of a source's list).

## Commands

* **Transport:** `Play`, `Pause`, `TogglePlayPause`, `Stop`, `Next`, `Previous`
  (restarts after 3 s), `Seek`, `SetVolume`, `SetMuted`.
* **Queue:** `PlayTracks`, `PlayCollection` (album, artist, playlist,
  favourites, every library song (`Collection::Songs { sort, descending }`,
  the Songs page's order with `TrackQuery`'s sort keys: Play and Shuffle for
  all songs, with no paging), a track list, or a source container; optional
  shuffle), `Enqueue`, `EnqueueCollection` (Play Next / Play Last),
  `RemoveFromQueue`, `MoveInQueue`, `JumpTo`, `ClearQueue`, `SetShuffle`,
  `SetRepeat`.
* **Settings and output:** `SetSettings` (send back a modified copy of the
  slice), `SetEq`, `SetReplayGain`, `SetCrossfade`, `SetOutputDevice`,
  `RefreshDevices` (send on OS device notifications; the core never polls).
* **Library:** `AddRoot` (adds and scans the folder), `RemoveRoot`, `Rescan`
  (checks every folder); playlists (`CreatePlaylist`, `RenamePlaylist`,
  `DeletePlaylist`, `AddToPlaylist`, `RemoveFromPlaylist`, `MovePlaylistEntry`,
  `ImportPlaylist`); `SetFavorite`. While `Settings::watch_folders` is on (and
  `Config::watch_folders`), changed folders are scanned by themselves.
* **Sources:** `InstallPlugin`, `UninstallPlugin`, `SetPluginEnabled`,
  `SetPluginSettings`, `AcknowledgeUnofficial` (community disclosure),
  `BeginLogin`, `ConnectSource` (a service from `presets`), `SubmitLogin`,
  `CancelLogin`, `Logout`, `Search` (library and
  sources, merged and de-duplicated), `Browse`; `PlayCollection` and
  `EnqueueCollection` with `Collection::Source`; `Remote { action }`
  (`Refresh`, `Transfer { device }`, `SetVolume { percent }`, `End`);
  `ResetPluginSettings { plugin }` (Reset to Defaults: the schema's
  defaults). See "Online sources".
* **AI Lyrics:** `MakeLyrics { song }`, `CancelLyrics { job }`,
  `DeleteMadeLyrics { song }`, `AllowAudioUpload { plugin, origin }`,
  `RevokeAudioUpload { plugin, origin }`, `DownloadSpeechModel { id }`,
  `CancelSpeechModelDownload { id }`, `DeleteSpeechModel { id }`. See "AI
  Lyrics".
* **Pro and updates:** `ActivateLicence { key }`, `RemoveLicence`,
  `CheckForUpdates`.
* **Discover, details, comments:** `Discover { source }`, `Details { item }`,
  `Comments { item, page }`.
* **v0.0.8:** `SetNetworkState { online, metered }` (see "Streaming quality"),
  `SearchMore { plugin, kind, cursor }`, `RefreshSourceLibrary { plugin }`,
  `SourceLibraryMore { plugin, part, cursor }`, `SavePlaylistCopy { plugin, id,
  name }`, `Download { target }`, `CancelDownload { id }`, `RetryDownload { id
  }`, `RemoveDownload { id }`, `RemoveAllDownloads`.

## Queries (synchronous, on the caller's thread)

`track`, `tracks(TrackQuery)`, `albums`, `albums_sorted`, `artists`,
`album_tracks`, `artist_albums`, `search` (library only, fast),
`recently_added`, `recently_played`, `most_played`, `playlists`, `playlist`,
`playlist_entries`, `favorites`, `is_favorite`, `artwork_thumbnail(hash, size)`,
`export_playlist(id, path)`, `roots`, `stats`, `queue_window`, `history`,
`tracks_for_query`, `localized_settings_schema(plugin, language)` and
`source_artwork(url, size_px)` (see "Covers"); free functions
`output_devices()`, `eq_presets()`, `lyrics_bar(timeline, t_ms)` (see
"Drawing time"), and from v0.0.8 `address_is_local(address)` (see "The local
network") and `download_preview(template)` (see "Downloads"). They are cheap enough for list views but are not for
per-frame use; call them off the UI thread. `set_store_entitlement(entitlement)`
reports a store purchase (see "Jellyfesh Pro").

Track lists (playlists, favourites, history) come as `TrackRow`s: what to
show, and the `TrackRef` to queue. Library rows carry `artwork_hash` for
`artwork_thumbnail`; source rows carry an artwork URL. A row is `missing`
when its file is gone; `PlayCollection` leaves such entries out, and its
`start` names the entry in the list as shown.

Play statistics: a play counts when the track plays to its end, or when it
is left after half of it (at most 4 minutes); earlier it counts as a skip.

## Settings (`Settings`)

appearance (system/light/dark), style (artwork/misty backdrop), app_icon
(`automatic` or one of the ten, ADR-0017), hue_source (artwork or a fixed
hue), reduce_effects, transparency (0 solid … 1 clearest, default 0.5;
the core keeps it within 0–1), lyrics_size (small/medium/large),
lyrics_align (start/center), show_translation, show_romanization, crossfade
(enabled, 1–12 s, album continuity), replaygain_mode, preamp_db, eq,
output_device, resume_on_launch (the core resumes the restored queue at its
saved position when it starts, when session restore is on and
`Config::resume_on_start` allows it: apps started without their window clear
it), watch_folders,
allow_community_plugins, check_for_updates (direct downloads: at most one
check a day; on by default), show_lyrics_bar (Show Lyrics Bar in Settings ›
Appearance › Lyrics: the one-line lyrics bar, on by default) and
desktop_lyrics (Desktop Lyrics: the current line in a window of its own
above other apps, off by default), and from v0.0.8 streaming_quality (`Auto`,
`Lossless`, `High`, `Normal`, `Low`; see "Streaming quality"), downloads
(`DownloadSettings`: `folder` (`None`: the default, which the `Downloads`
slice names), `quality` (`Original` by default, `High`, `Normal`, `Low`),
`name_template` (default `{album_artist}/{album}/{track} {title}`),
`add_missing_cover_and_lyrics` (off), `lyrics_in_tags` (on), `save_lrc_files`
(off), `wifi_only` (on), `at_once` (1–4, 2; the core keeps it within them))
and mini_player (`MiniPlayerSettings`: `keep_on_top` (on),
`open_when_window_closed` (off), `show_lyrics_line` (on), `android_pip`
(off); the apps apply them). The core applies playback, library, source and
download settings; shells apply the appearance ones, the lyrics bar, Desktop
Lyrics and the mini player among them. Settings saved before a field existed
read with its default (nested objects field by field).

Transparency scales the alpha of the glass grades: `jf_palette::glass_alpha`
(token alpha at 0.5, opaque at 0, half the token alpha at 1) or
`Theme::with_transparency` for a whole theme. Shells in other languages port
the function and must pass `tests/contract/glass-alpha.json`; shells whose
platform materials have no alpha map it to material thickness or tint
opacity (`docs/design-controls.md`, "v0.0.4").

## Drawing time

Never ask the core per frame. Feed every `PlaybackSnapshot.anchor` into a
`DisplayClock` and draw from `position_us(now)`. Redraw only when something
visible changes: `next_second_us`, `next_step_us` (progress bars),
`LyricTimeline::next_change` (lyrics) and `DisplayClock::wait_until` turn
those into sleeps. Shells in other languages port `DisplayClock`,
`LyricTimeline::cursor` and `next_change` and must pass
`tests/contract/lyrics-cursor.json`. Monotonic time: `monotonic_now_ns()`.

**The lyrics bar** (ADR-0022 §6): `lyrics_bar(timeline, t_ms) -> LyricsBar`
says what the one-line lyrics bar (player bar, mini-player) and Desktop
Lyrics show at a time: `line` (the active line, or in a gap between lines
the last one that started; `None` before the first line), `next` (the next
line by start; before the first line, the first one), `words` (each word of
`line` from 0, still to come, to 1, sung: the share of the word's time for
the word being sung) and `line_progress` (0–1 through `line`, for lines
without word timing; its end, or the next line's start). Draw the sung part
of each word by its value. Shells that draw every frame port it (it is a
binary search and one pass over a line's words) and must pass
`tests/contract/lyrics-bar.json`; the FFI also exports it.

## Playback output

`SetCrossfade` (or `Settings::crossfade`) crossfades automatic transitions
(1–12 s; with album continuity, tracks of one album join gaplessly);
manual skips never crossfade. `SetOutputDevice` moves playback to another
device where it is; when that device is gone, playback opens on the system
default and an `Error` says so. Shells send `RefreshDevices` on the OS's
device notifications, which also lets playback follow a new system default.
When the output fails and cannot be reopened, playback pauses where it was
with an `Error`; the next `Play` reopens it. `Buffering` marks a stream
waiting for data (anchors have rate 0 meanwhile). `NowPlaying.format`
carries codec, rate, channels, bit rate and whether joins are gapless.

## Online sources

Plugins (ADR-0003, ADR-0004) run inside the core: the plugin host keeps
packages in `<data>/plugins` and every call off the actor thread, so a slow
or hanging plugin never holds up a command or a snapshot.

* **Installing.** `InstallPlugin { path }` checks the package and its
  signature like the host always does. Packages signed by a trusted key are
  **Official** and start on. Anything else is **Community** (ADR-0012): it
  installs only while `Settings::allow_community_plugins` is on and starts
  off until the person accepts its disclosure (may violate the service's
  terms, may break at any time, personal use only):
  `AcknowledgeUnofficial { id }` remembers the consent in
  `<data>/plugins/consent.json`, turns community plugins on and turns the
  plugin on (a consent given before the install applies to it). Turning
  community plugins off keeps them installed but unusable, with
  `disabled_reason` saying why. `UninstallPlugin` removes a plugin with its
  settings, data, sign-in and secrets.
* **Bundled plugins.** `Config::bundled_plugins` names `.jfpkg` files the
  app ships. At start the core installs each that is missing and updates one
  whose bundled version is newer, checked like any package (failures arrive
  as `Error`). One the person uninstalls is remembered in the settings store
  and not installed again until they install it themselves. A bundled
  **community** package (not signed by a trusted key) waits, without an
  error, while community plugins are off and is installed when the person
  allows them; like every community plugin it then stays off until its
  disclosure is accepted (ADR-0012, amended for v0.0.6).
* **The slice.** `Sources` is published at start and after every change:
  install, uninstall, on/off, settings, consent, sign-in and sign-out, and
  when the host turns a plugin off after it broke its limits three times in
  a row. `PluginSummary.icon` is the package's `icon.png` copied to
  `<cache>/plugin-icons/` under a name taken from its content, so an update
  gets a new path; load it like any image file.
* **Settings.** `SetPluginSettings { id, settings_json }` takes JSON that
  matches `settings_schema_json` (a flat JSON Schema subset: strings, numbers,
  booleans, enums); the core validates it. `settings_schema_json` is the
  package's own text, so forms list the settings in the plugin's order.
* **Adding a service.** `SourcesSnapshot::presets` lists the services the
  apps offer in Sources → Add a Source (`jf_plugin_model::presets`: Jellyfin,
  Navidrome, Subsonic for now), each only while its plugin is installed and
  signed by Jellyfesh: id, name, plugin, sign-in methods, whether a server
  address is needed (with the one set now and an example), the account.
  `ConnectSource { preset, address, method }` sets the address, turns the
  plugin on and starts a sign-in exactly as `BeginLogin` does; a missing
  address or a service that is not offered fails the command.
* **Signing in.** `BeginLogin { plugin, method }` starts a flow; every step
  arrives as `Auth(AuthUpdate { plugin, flow, step })`, the first carrying the
  flow number `SubmitLogin` and `CancelLogin` take. Steps: `ShowQr { data,
  hint }` (the UI draws the QR code from `data`, usually a URL),
  `ShowCode { code, hint }` (show it large, to type on another device, as
  with Jellyfin Quick Connect), `OpenUrl { url }` (open the system browser;
  the core catches the redirect), `AskFields { fields }` (`kind`, `secret`,
  `required`, `placeholder`, `help` per field; answer with `SubmitLogin`),
  `Waiting { message }`, then `Done { account }` or `Failed { message }`.
  The core polls the plugin meanwhile; a new login to the same source
  cancels the one in progress. `Logout` deletes the source's secrets,
  cookies and account even when the plugin is off.
* **Secrets** (ADR-0015). Tokens, API keys and cookie-jar keys go to a
  `SecretStore`: the one the app passes in `SourcesConfig::secrets` (the FFI
  shells pass Keychain, Credential Manager or Android Keystore
  implementations), otherwise the system keystore through `keyring` (Keychain
  on macOS, Credential Manager on Windows, the Secret Service on Linux). The
  store is checked once at start, off the actor. When it cannot be reached,
  sign-ins stay in memory until the app quits: `SourcesSnapshot::secret_storage`
  is `SessionOnly` and an `Error` says so (once a plugin that signs in is
  installed). Passwords are never stored anywhere.
* **Search.** `Search { query, scope }` with `SearchScope::All` answers with
  the library at once, then asks every enabled source that can search in
  parallel, each within the search deadline (v0.0.8: sources whose terms
  forbid merging too, in sections of their own; see "Search by source").
  Each `SearchUpdate` is one source's section: its results less those an
  earlier source shows (same ISRC, else the same normalised title, primary
  artist and duration within 2 s), in its own order. Such an item appears
  once, in the section of its first playable copy (the library's when the
  library has it), with the other sources in `also_in`. When an answer
  changes an earlier section (a library track gains `also_in`), that
  section is sent again right before; show the latest update per `source`,
  library first. A source that fails or misses the deadline sends an empty
  section with `error`. `SearchScope::Plugin { id }` asks one source,
  `Library` only the library. A newer search supersedes older ones: their
  late answers are dropped.
* **Browsing.** `Browse { plugin, node, page }`: `node` `None` is the
  source's root, otherwise the `id` of an item the core listed for that
  plugin; pages count from 0 and `BrowseUpdate::next_page` names the next.
  `PlayCollection`/`EnqueueCollection` with `Collection::Source { plugin, id,
  item_kind }` (a `SourceItem`'s `source`, `id` and `kind`: the kind travels
  with the id, since an id alone is ambiguous) fetch the pages of an album or
  playlist (an artist plays its albums) and queue the playable tracks. To
  enqueue, the queue changes once every page is in; to play, it starts as
  soon as the page holding `start` is in, and later pages (an artist's later
  albums) join the queue as they arrive, shuffled in when shuffle is on. With
  `shuffle` and `start` 0 (a Shuffle button) any song can come first, also
  on a remote source's device.
* **Playing.** A `TrackRef::Source` resolves its stream URL on a worker; the
  entry shows as starting with `Buffering { uid, buffering: true }` until
  the audio flows. The entry after it is resolved ahead, so gapless joins
  and crossfades work between source tracks, and the engine asks the plugin
  for a fresh URL when one expires or is refused. `NowPlaying` shows the
  track's own metadata with `ArtworkRef::Url`; the core fetches the artwork
  through the plugin for the theme and backdrops, and lyrics through the
  plugin (`LyricsSource::Plugin(id)`; the attribution is in
  `LyricDoc::source.origin`; unsynced when the source's terms say so).
  Items the service will not play here (`playable: false`, region,
  paywall) never reach the queue: their `unplayable_reason` is shown, never
  bypassed.
* **Links** (API 1.1). `SourceItem::link` and `NowPlaying::link` are
  `https` links that open the item in its service, filled in from the
  plugin's `[links]` templates. Offer **Open in {Service}** (the plugin's
  name) wherever an item has one, as the main action for items that cannot
  play here.
* **Remote sources** (API 1.1, ADR-0018). A plugin with
  `PluginSummary::remote_playback` plays on the person's own devices (its
  items still carry `track`). `PlayTracks`, or `PlayCollection` with its
  tracks or containers, pauses the engine and starts a **remote session**:
  the core sends the service the command, then asks for its state every
  second while it plays, every five while paused, every fifteen while
  nothing plays (less often after errors), and publishes the `Remote`
  slice. While a session runs, `Play`, `Pause`, `TogglePlayPause`, `Next`,
  `Previous` (restarts after 3 s), `Seek`, `SetShuffle` and `SetRepeat`
  control the service's player, and `Stop` pauses it and ends the session;
  `Remote { Refresh }` asks for the devices again (open the device picker
  with it), `Transfer { device }` moves playback, `SetVolume { percent }`
  sets the playing device's volume, `End` stops controlling it (it keeps
  playing). Starting local playback ends the session too, and so do ten
  minutes without playing and a service that wants a new sign-in (`error`
  then says so). Remote sources' tracks never join the queue: `Enqueue`
  and `EnqueueCollection` with them fail.
* **Players an app provides** (ADR-0018 item 5). `Core::register_external_player(id, name,
  player)` registers an `ExternalPlayer` (devices, state, play, the
  transport, volume, shuffle, repeat, transfer) under a source id without
  dots (`apple-music`). `PlayTracks` with `TrackRef::Source` tracks whose
  `plugin` is that id, or `PlayCollection` with `Collection::Source { plugin:
  id, .. }`, starts a remote session on it, and everything above applies.
  The core calls it from the session's thread; its errors show as the
  slice's `error`. `unregister_external_player(id)` removes it.

## Playing and showing quality

`NowPlaying::quality: Option<AudioFormat>` is what is playing as decoded:
`codec`, `sample_rate_hz`, `bit_depth`, `channels`, `bitrate_kbps`,
`lossless` and `quality`, a `QualityLabel` the core decides (apps word it,
never compute it): `HiResLossless` (lossless above 16-bit or above 48 kHz),
`Lossless`, `High` (lossy at 256 kbit/s or more), `Standard`. It is `None`
until the engine has opened the track, and a new `NowPlaying` event carries
it then and on every track change. `NowPlaying::format` (`TrackInfo`) keeps
the engine's raw facts and gains `bits_per_sample` (PCM, FLAC, ALAC).

## Details and comments (ADR-0022)

`Details { item: ItemRef }` answers with `Event::Details(DetailsPage)`:
`item` and `request` (match them to what you asked), `title`, `subtitle`
(artist), `description` (album notes, an artist's biography), `image` (a
large cover or an artist photo: `ArtworkRef::File` to load, or
`ArtworkRef::Url` to fetch with `source_artwork`), `facts` (`Fact { key:
FactKey, value }`: the apps word the known keys `Version`, `Genre`,
`Language`, `Released`, `Label`, `Composer`, `Lyricist`, `Arranger`,
`Producer`, `Popularity`, `Quality`, `Isrc`, `Disc` ("1 of 2"), `Track` ("3
of 12"); `Other { label }` carries the service's own label), `technical`
(`TechnicalInfo`: `format` as above, `container`, `file_size_bytes`,
`duration_ms`, `replaygain`, `path`), `comment_count`, `attribution` and
`error`. `ItemRef` names a library `Track`, `Album` or `Artist` by id, a
`File` outside the library, or `Source { plugin, item_kind, id }` (an item a
source listed: its `SourceItem::source`, `kind` and `id`). Library items come
from the tags and the file; source items from the plugin's `details` (when
`PluginSummary::details`) and, for tracks that played, what the stream said.

`Comments { item, page }` (sources with `PluginSummary::comments`) answers
with `Event::Comments(CommentsPage)`: `hot` (first page only), `comments`,
`total`, `next` (send `Comments { item, page: next }` for more),
`attribution` (show it with the comments) and `error`. Comments are
read-only and never stored.

**Artist and album pages for source items.** `SourceItem::artist_id` and
`album_id` are the source's ids of a track's or album's artist and album.
Go to Artist: `Browse { plugin: source, node: Some(artist_id) }` lists the
artist's albums (or tracks) and `Details { item: ItemRef::Source { plugin,
item_kind: Artist, id: artist_id } }` gives the photo, biography and facts.
Go to Album: the same with `album_id` and `ItemKind::Album`; the album's
tracks come from `Browse`, its cover and facts from `Details`, and
`PlayCollection { collection: Collection::Source { plugin, id: album_id } }`
plays it. Library items keep using the library queries.

## Discover (ADR-0022)

`Discover { source }` answers with `Event::Discover(DiscoverPage { request,
source, shelves, error })`. Each `Shelf` has an `id`, a `title`, a `style`
(`Row`: one scrolling row of covers; `Grid`; `Chart`: a numbered list),
`items` (`SourceItem`s, at most 50) and `more` (a node for See All: `Browse
{ plugin: source, node: Some(more) }`). A plugin with `PluginSummary::discover`
makes its own shelves (at most 12, in its words); for other sources the core
makes them from the service's suggestions ("For You", id `for_you`, while
signed in) and the top level of `Browse`: each of the first six folders and
playlists there becomes a shelf of its first items (its id is the node's, and
`more` browses it), and the other items at the top level share one grid
(`browse`, titled with the source's name). Each page holds only its source's
items, as content policies ask, and its items are remembered, so playing them
and See All pass them back whole. `source: "library"` gives the library's
page: Recently Added (`recently_added`: the albums of the newest songs),
Most Played (`most_played`, a chart by plays) and Rediscover (`rediscover`:
songs played at least twice, but not in the last two months). Shelves the
core makes have stable ids; word their titles by id.

## Covers (ADR-0022 §5)

Source pictures (`ArtworkRef::Url` of source items, Now Playing, shelves,
details and comment avatars) are fetched by the core, never by the apps: the
plugin's allowlist decides, the picture's own headers go with the request
(some services need them), and `http` links move to `https` where only that
is allowed. For each one shown:

1. call `source_artwork(url, size_px)` (`size_px` about the size drawn: 64,
   128, 256, 512 or 1024);
2. a path means the picture is in the cache: load the file;
3. `None` means the core is fetching it: show the placeholder, and when
   `Event::Artwork(ArtworkReady { url, size_px, path })` arrives for that
   `url`, load `path` (or keep the placeholder when it is `None`: the
   picture cannot be had, so stop asking for it in this view).

Calling it for every list row as it appears is fine: requests for the same
picture are made once, a few run at a time, and the newest requests go first
while scrolling; the cache (`<cache>/source-artwork/`) is bounded and evicts
the least recently used files. Library rows keep `artwork_hash` and
`artwork_thumbnail`.

Items a source lists without a picture, from a plugin that answers
`artwork`, carry a reference of the core's own instead,
`jellyfesh-artwork://item?plugin=…&kind=…&id=…` (Subsonic's covers come only
this way: their URLs would hold the sign-in). Treat it like any other source
picture URL: `source_artwork` asks the plugin's `artwork` for the item and
fetches what it gives, with its headers, into the same cache. Never load
these URLs yourself.

Now Playing for a source track starts with the picture it was listed with
(`ArtworkRef::Url`, or none); once the core has fetched the song's picture
through its plugin (its `artwork` answer, else the listed one), `NowPlaying`
comes again with `ArtworkRef::File` of a large JPEG in the cache.

Rows of lists the library keeps (Recently Played, playlists, favourites)
show a source song with the picture URL it was listed with, from any earlier
run, or the core's item reference when it had none and its plugin answers
`artwork`. `source_artwork` fetches a stored URL through the plugin (its
`artwork` answer for the song, else the URL itself while the plugin's
allowlist covers it), so these rows show covers after a restart too.

Library songs without a picture of their own get their folder's cover
(`cover`, `folder`, `front`, `album` or `artwork`, as JPEG, PNG, WebP or
GIF) when they are scanned, so rows, albums and Now Playing show the same
picture; songs catalogued before keep theirs and gain the folder's on the
next scan.

## AI Lyrics (ADR-0019)

The `AiLyrics` slice: `available` (an AI Lyrics plugin, the first enabled
plugin with `make_lyrics`, is installed and on: offer Create Synced Lyrics),
`plugin`, `plan` (`AiLyricsPlan { mode: OnDevice | Online, engine,
upload_to, automatic }`, `None` until the plugin answered or while Pro is
inactive), `need`, `jobs`, `models` and `consents`.

`need` is what must happen before `MakeLyrics` works, re-read whenever the
plugin's settings, its sign-in, Pro, the consents or the models change:

| `need` | Show |
| --- | --- |
| `Nothing` | Create Synced Lyrics works |
| `Pro` | the Pro sheet |
| `SignIn` | "Add your API key to make lyrics online." and Open AI Lyrics Settings (its Sign In) |
| `Consent { origin }` | "Send This Song to {origin}?" (ui.md); Allow sends `AllowAudioUpload { plugin, origin }` |
| `Model { id, name, bytes }` | "Download the {name} model ({bytes}) to make lyrics on this device." and Download (`DownloadSpeechModel { id }`) |
| `Unavailable { reason }` | the reason |

The plugin's plan is asked for again whenever the plugin, its settings or its
sign-in change, Pro starts or stops, or a model arrives or goes. Recognition
on this device uses `Config::speech` (`jf_speech::recognizer()` in apps built
with Whisper; without one the on-device mode is `Unavailable`), with models
in `<data>/speech-models/` (`Config::speech_store` replaces the store). The
consents are kept in `<data>/ai-lyrics-consents.json`. When the plan says
`automatic`, each song that starts without synced lyrics of its own (and
without made ones) gets a job, once per queue entry.

`MakeLyrics { song: SongRef::Current | SongRef::Library { id } }` queues a job
(it fails with the reason while `need` is not `Nothing`, and for songs that
play elsewhere or only open in another app). `jobs` lists waiting and running
jobs and the newest finished ones, oldest first: `LyricsJob { id, title,
artist, uid (the queue entry, when made for the song playing), library_id,
stage }`. One runs at a time. `stage`: `Waiting`, `ReadingAudio`, `Sending {
origin }`, `Recognising { fraction }` (0–1), `TimingWords`, then `Done`,
`Failed { message }` ("Couldn't make lyrics: {message}", Try Again sends
`MakeLyrics` again) or `Cancelled`. `CancelLyrics { job }` stops one.

Made lyrics are kept in `<data>/made-lyrics/` and used whenever the song's
own lyrics are missing or unsynced: the `Lyrics` slice then has `source:
LyricsSource::Made { plugin, engine }` ("Made by AI Lyrics · {engine}").
`DeleteMadeLyrics { song }` deletes them (ask first, ui.md).

`models` lists the on-device models (`SpeechModelState { id, name, bytes,
state }`, `state`: `NotDownloaded`, `Downloading { fraction }`, `Ready`,
`Failed { message }`); `DownloadSpeechModel { id }`,
`CancelSpeechModelDownload { id }` (keeps what came, to resume) and
`DeleteSpeechModel { id }`. `consents` lists `AudioConsent { plugin, origin
}`; `RevokeAudioUpload { plugin, origin }` is Stop Allowing.

## The look (ADR-0020)

The `Look` slice holds resolved values, so apps apply one thing:
`corner_scale` (radii ×, 0–2), `glass` (glass alpha and blur ×, 0–1;
Reduce Transparency and the Transparency setting still win), `aurora` (the
field's strength, 0–1), `typeface` (`System`, `Rounded`, `Serif`, `Mono`: the
platform's own faces), `text_scale` (every type size ×, 0.85–1.30),
`density` (spacing × 0.8, 1.0, 1.2), `lyrics_weight`, `lyrics_glow` (0–1),
`lyrics_highlight` (`Artwork`, `Accent`, `Custom { rgb }` as `0xRRGGBB`),
`sidebar` (the places shown, in order; Settings is never listed and always
shown), `home` (Home's sections, in order), `now_playing` (`Stage`,
`Artwork`, `Lyrics`), `mini_player` (`Bar`, `Compact`, `Floating`),
`customized` and `plugin`. With `customized` false every value is the
Luminous default (`LookSnapshot::defaults`): 1, 1, the token
`field.aurora-strength` (0.75), System, 1, Standard, Bold, 0.6, Artwork, all
eight places in the ui.md order, Summary, Recently Added, Recently Played,
Most Played, Stage, Bar. The core asks the active appearance plugin (the
first enabled one) when it loads and after its settings change, checks the
answer, and returns to the defaults while Pro is off.

**Plugin settings in the app's language** (API 1.2):
`localized_settings_schema(plugin, language)` returns the plugin's schema
text with its translations for a BCP 47 tag (`zh-Hans`, `zh-TW`, `ja-JP`;
English where a word is missing), the settings in the plugin's order. Render
forms from it rather than from `settings_schema_json` once the app has a
language other than English. Schema profile 1.2 adds `"format": "color"`
(`#RRGGBB`, a colour well), sliders (`minimum` and `maximum`, `multipleOf`
the step), ordered checklists (`"type": "array"` with `items` and
`uniqueItems`; the value lists the shown items in order), `oneOf` choices
with titles, and `x-jf-group` headings (consecutive properties with the same
group together); `docs/plugins/authoring.md` has the details.

## Jellyfesh Pro (ADR-0021)

The `Pro` slice: `active`, `source` (`None`; `Preview`: "Pro features are
free during the public beta."; `Store { store }`: "Jellyfesh Pro ·
Purchased"; `Licence { id_suffix, issued_at_ms }`: "Jellyfesh Pro · Licence
…{id_suffix}"), `preview` (a Pro Preview build), `licence_error` (why the last
key was refused: `Malformed`, `UnknownKey`, `BadSignature`, `WrongProduct`,
`Revoked`; the apps word them, ui.md), `stores` (where this build can sell
Pro: show Buy and Restore Purchases only for these) and `licence_keys`
(show Enter Licence Key… only when true: App Store and Google Play builds
sell Pro only through their store, and `ActivateLicence` fails there).
Sources, strongest first: a Preview build, an active store entitlement, a
valid licence key.

**Store builds** (`Distribution::AppStore`, `MicrosoftStore`, `GooglePlay`;
ADR-0021 §4): `SourcesSnapshot::can_install_plugins` is false, so hide
Install from File… and Allow Community Plugins; the core refuses
`InstallPlugin` and `AcknowledgeUnofficial` and keeps
`Settings::allow_community_plugins` off (bundled plugins still install and
update). They have no update check (`UpdateSnapshot::supported` is false).

* `ActivateLicence { key }` checks a key offline (`JFP1-…`; dashes, spaces and
  case do not matter) and keeps it with the settings, where the next start
  checks it again (a key withdrawn in a later release, or signed by a key the
  build does not know, then shows its `licence_error`); a bad one leaves
  `licence_error`. `RemoveLicence` removes it. `Config::pro` (Rust) holds
  Preview and the public keys; `ProConfig::for_this_build()` is what the
  release was built with (`JF_PRO_PREVIEW=1`, `JF_LICENCE_PUBLIC_KEYS`).
* Store builds report what their store says with
  `Core::set_store_entitlement(StoreEntitlement { store, product, active,
  transaction })` (product `org.jellyfesh.pro`, `pro` on Google Play) at
  start and on every change.
* While Pro is inactive, Pro plugins (`PluginSummary::requires_pro`) stay
  installed, their settings and sign-in work, and their Pro calls answer
  "needs Jellyfesh Pro": AI Lyrics' `need` is `Pro` and the look is the
  default. Turning Pro off stops lyrics jobs; nothing is deleted.

`PluginSummary::kind` is `Source` or `Extension` (plugins that make lyrics or
change the look and offer no music: Settings › Extensions, not Sources);
`permissions` lists the host imports the manifest declares (`audio`,
`speech`) for the install screen; `discover`, `details` and `comments` say
which Discover, details and comments the source offers.

## Updates (ADR-0021 §5)

The `Update` slice: `supported` (direct downloads: show Check for Updates
and its setting), `checking`, `available` (`UpdateInfo { version, url }`:
"Jellyfesh {version} Is Available" and View Release opening `url`),
`last_checked_ms`, `auto` (checks run by themselves: supported and
`Settings::check_for_updates` on) and `error`. The core checks GitHub's
releases at most once a day while `auto` (the time of the last check is kept
in `<data>/update.json`, so restarts do not check again within the day; a
failed check counts too), and on `CheckForUpdates`, which other builds refuse.
Pre-releases count while Jellyfesh is in beta; drafts never. Nothing is
downloaded or installed, and the request carries nothing about the person.

## Streaming quality (ADR-0026)

`Settings::streaming_quality` is the level asked of sources: `Auto` (Lossless,
or High while the network is metered), `Lossless`, `High`, `Normal`, `Low`.
Apps report the network with `SetNetworkState { online, metered }` at start
and on every change (Android always; Windows when it says a connection is
metered; the others may send `online` alone); until then the core takes the
network as online and not metered.

The core passes the level to the plugin's `resolve-stream`, fitted to the
levels the plugin offers (`content_policy.qualities`: the level itself, else
the best one below it, else the lowest offered). Changing the setting
reopens the playing source song where it is, at the new level (the listen
goes on: no new play or history entry), and the next song is resolved again;
a network change under `Auto` applies from the next song. A URL refreshed
while a song plays keeps the level it started with.

The plugins' own Streaming Quality settings went in v0.0.8 (Jellyfin,
Subsonic, NetEase and the community NetEase and QQ Music plugins). On the
first start of v0.0.8 the core reads them before the new plugin versions drop
them: when every plugin that set a fixed level agrees, that level becomes
`streaming_quality`; otherwise it stays `Auto`. This happens once.

`NowPlaying` carries the quality menu:

* `qualities`: the levels to offer for what plays, in the menu's order:
  `Auto`, then the source's levels best first (the plugin's
  `content_policy.qualities`). Check `Settings::streaming_quality`.
* `quality_note` when `qualities` is empty: `FilesPlayAsTheyAre` ("Files play
  as they are.": library songs, files, downloads) or `SetInService { service
  }` ("Quality is set in {service}.": a source that offers no levels).
  `RemoteSnapshot::quality_note` is `SetInService` while a remote session
  runs (Spotify).
* `quality_level`: the level the playing stream was asked for (`Auto`
  resolved and fitted to the source's levels); `None` for files.
* `quality` (`AudioFormat`) has the format line's facts ("FLAC · 24-bit · 96
  kHz", "AAC · 256 kbit/s"): what the decoder found, with the plugin's codec
  and bitrate hints filling in what it does not say.

`PluginSummary::qualities` lists a source's levels for Settings.

## Search by source (ADR-0024 §4)

Every `SearchUpdate` is one source's section: `items` (the All tab's rows),
`merged` and `groups`.

* `merged` true: the library and the sources whose terms allow merging, as
  before (From Your Sources): `items` less what an earlier section shows, with
  `also_in`.
* `merged` false: a source whose terms forbid merging (Spotify, QQ Music),
  searched with `SearchScope::All` too: `items` are all its results, never
  de-duplicated and never shown beside other rows. The All tab links to its
  own tab ("Results from Spotify").
* `groups`: the source's own results kind by kind (`SearchGroup { kind,
  items, next, total }`: Songs, Albums, Artists, Playlists), for its tab.
  Each kind is asked of the source on its own, 20 at first, so every group
  has its own `next` and `total`; the section comes once all its kinds have
  answered (or with what came by the deadline), and a kind that fails leaves
  the others.
  `next` is Show More's cursor: `SearchMore { plugin: source, kind, cursor }`
  answers with `Event::SearchPage(SearchPage { request, query, source, kind,
  items, next, error })`; add `items` to the group and keep `next`. Show More
  continues the newest search.

## Sources' likes and playlists (ADR-0024 §2)

The `SourceLibrary` slice lists every enabled, signed-in source with the
`library` capability (`PluginSummary::library`): `SourceLibrary { plugin,
name, attribution, lists }`, each `SourceLibraryList { part, entries, total,
next, refreshed_at_ms, loading, error }` in the order `LikedSongs`,
`LikedAlbums`, `LikedArtists`, `Playlists` (lists a source does not keep are
left out). An entry is `SourceLibraryEntry { item, owned, songs_listed }`:
play and browse `item` like any source item; a playlist with `songs_listed`
false shows "{Service} lists this playlist's songs only in {Service}." and
plays as a whole.

* `RefreshSourceLibrary { plugin }` (`None`: every source) when Favourites or
  Playlists opens, or is returned to after ten minutes; the core never polls.
  The lists come from a cache on disk at once, with `refreshed_at_ms` ("Updated
  5 minutes ago").
* `SourceLibraryMore { plugin, part, cursor: next }` answers with
  `Event::SourceLibraryPage { request, plugin, part, entries, next, error }`.
* `SavePlaylistCopy { plugin, id, name }` makes a Jellyfesh playlist of the
  source playlist's songs (as the source's items), answered by
  `PlaylistCreated`.

## Downloads (ADR-0025)

Sources with `PluginSummary::download` (Jellyfin, Subsonic/Navidrome) offer
Download. `Download { target }` takes `DownloadTarget::Track { track }` (a
`TrackRef::Source`) or `DownloadTarget::Collection { plugin, id, item_kind }`
(an album or playlist). The `Downloads` slice: `folder` (the person's folder,
else `Config::downloads_dir`, the app's default `Jellyfesh` folder in Music,
else one the core works out), `items` (in progress in queue order, then the
finished ones by album), `bytes_used` and `paused` (`Offline`, or `Metered`
with Wi-Fi Only on). A `DownloadItem` has `id`, `track` (`SourceTrack`),
`state` (`Queued`, `Downloading`, `Done`, `Failed { error }`), `bytes_done`,
`bytes_total`, `path`, `library_id` and `original`. `DownloadError`:
`NotAllowed { service }` ("Your {service} server doesn't allow downloads for
this account."), `NotOffered { service }`, `Network`, `Storage`, `Other`
(with a `message`). `CancelDownload`, `RetryDownload`, `RemoveDownload`
(deletes the file; ask first) and `RemoveAllDownloads` (ask first) take an
item's `id`. `SourceItem::downloaded` and `TrackRow::downloaded` mark source
songs with a finished download (they play the file, offline too).

File names follow `DownloadSettings::name_template`; `download_preview(template)`
gives the live preview ("Album Artist/Album/01 Title.flac"). The template and
the safe-name rules are `jf_plugin_model::filenames` (`render`, `safe_part`,
`file_name`, `unique_file_name`), shared with Jellyfesh Web.

## The local network (Android SDK 37)

`PluginSummary::local_network` says a source's server (the address the person
typed) is on the local network, so Android apps targeting SDK 37 ask for
`ACCESS_LOCAL_NETWORK` before using it; `address_is_local(address)` answers
the same for an address not saved yet (Add a Source). The rule is Android's:
IPv4 10/8, 172.16/12, 192.168/16, 100.64/10 and link-local 169.254/16; IPv6
link-local fe80::/10 and unique local fc00::/7; multicast and broadcast;
`.local` names. Loopback is not local; other host names count as not local
(they would need a DNS lookup).

## Status in this release's development

v0.0.8 (in progress): the types and commands of the sections above are the
contract. Working now: the settings, `SetNetworkState`, `Collection::Source`
with its kind, search sections (`merged`, `groups` with
per-kind cursors from 20-per-kind queries), `SearchMore`, the quality menu's fields and the
level asked of sources, the local-network flag and the downloads folder.
`RefreshSourceLibrary` lists the sources without their lists yet;
`SourceLibraryMore`, `SavePlaylistCopy` and the download commands answer
"not available in this build yet" until their implementation lands.

New in v0.0.7: AI Lyrics, the look, Jellyfesh Pro and store builds, the
update check, Discover, details and comments, source pictures through the
core, the quality label and resuming playback on launch, as described above.

Available since v0.0.6: everything from v0.0.2, `PlayCollection`/`EnqueueCollection`
for albums, artists, playlists, favourites, track lists and source
containers, playlists, favourites, play statistics, artist pages,
thumbnails, folder watching, crossfade, output-device switching,
`SetSettings`, `SetMuted`, `RefreshDevices`, and every source command
(plugins, sign-ins, unified search, browsing, playing source tracks).
Code UIs against the behaviour described here.
