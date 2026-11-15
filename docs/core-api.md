# The core API (v0.0.5)

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
per-source search deadline, 8 s) and `bundled_plugins: Vec<PathBuf>` (see
"Online sources").

## Slices (in `Snapshot`, and as events when they change)

| Slice | Event | Holds |
| --- | --- | --- |
| `playback: PlaybackSnapshot` | `Playback` | status, **anchor**, volume, eq, ReplayGain |
| `queue: QueueSnapshot` | `Queue` | length, position, current, shuffle, repeat, next 50 entries (`queue_window` pages the rest) |
| `now_playing` | `NowPlaying` | uid, track, title, artist, album, duration, artwork (`ArtworkRef::File` or `Url`), stream format |
| `lyrics` | `Lyrics` | `LyricDoc` + `LyricTimeline` for the current track, or none |
| `theme` | `Theme` | hue, light and dark Misty themes, baked backdrops (light, dark) |
| `settings: Settings` | `Settings` | all preferences (below) |
| `devices: DevicesSnapshot` | `Devices` | output devices and the selected one (`None` = system default) |
| `sources: SourcesSnapshot` | `Sources` | installed plugins (badge, signing key, capabilities, sign-in methods, account, settings schema and values, icon, why one is off), the services offered in Add a Source (`presets`), community allowed, where sign-ins are kept, why sources are unavailable |
| `muted`, `scanning`, `library_revision` | `LibraryChanged`, `Scan` | `LibraryChanged` after any catalogue, playlist, favourite or play-count change: lists query again |

Other events: `PlaylistCreated { request, id }` (after `CreatePlaylist` and
`ImportPlaylist`), `Auth(AuthUpdate)` (sign-in steps),
`SearchResults(SearchUpdate)` (one per source per search, see below; `done`
on the last), `BrowsePage`, `Buffering`, `CommandFailed`, `Error`.

## Commands

* **Transport:** `Play`, `Pause`, `TogglePlayPause`, `Stop`, `Next`, `Previous`
  (restarts after 3 s), `Seek`, `SetVolume`, `SetMuted`.
* **Queue:** `PlayTracks`, `PlayCollection` (album, artist, playlist,
  favourites, a track list, or a source container; optional shuffle),
  `Enqueue`, `EnqueueCollection` (Play Next / Play Last), `RemoveFromQueue`,
  `MoveInQueue`, `JumpTo`, `ClearQueue`, `SetShuffle`, `SetRepeat`.
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
  `EnqueueCollection` with `Collection::Source`. See "Online sources".

## Queries (synchronous, on the caller's thread)

`track`, `tracks(TrackQuery)`, `albums`, `albums_sorted`, `artists`,
`album_tracks`, `artist_albums`, `search` (library only, fast),
`recently_added`, `recently_played`, `most_played`, `playlists`, `playlist`,
`playlist_entries`, `favorites`, `is_favorite`, `artwork_thumbnail(hash, size)`,
`export_playlist(id, path)`, `roots`, `stats`, `queue_window`, `history`,
`tracks_for_query`; free functions `output_devices()` and `eq_presets()`. They are cheap enough
for list views but are not for per-frame use; call them off the UI thread.

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
output_device, resume_on_launch, watch_folders, allow_community_plugins.
The core applies playback, library and source settings; shells apply the
appearance ones.

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
  and not installed again until they install it themselves.
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
  the library at once, then asks every enabled source that can search (and
  whose terms allow merging) in parallel, each within the search deadline.
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
  `PlayCollection`/`EnqueueCollection` with `Collection::Source { plugin, id }`
  fetch every page of an album or playlist (an artist plays its albums) and
  queue the playable tracks; the queue changes once the plugin answered.
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

## Status in this release's development

Available now: everything from v0.0.2, `PlayCollection`/`EnqueueCollection`
for albums, artists, playlists, favourites, track lists and source
containers, playlists, favourites, play statistics, artist pages,
thumbnails, folder watching, crossfade, output-device switching,
`SetSettings`, `SetMuted`, `RefreshDevices`, and every source command
(plugins, sign-ins, unified search, browsing, playing source tracks).
Code UIs against the behaviour described here.
