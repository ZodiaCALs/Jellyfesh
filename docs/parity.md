# Parity of the five apps

An audit of feature and view parity between the macOS, Windows, Linux and
Android apps and Jellyfesh Web, against [ui.md](ui.md) as of commit `cac32aa`
(2026-11-17). It is the work list for the app agents who close the gaps.

**How this was checked.** By reading the code on `main`: views, view models,
FFI mappings and string tables, and by searching each app for the core
commands it sends (`SetCrossfade`, `ImportPlaylist`, `ConnectSource`,
`Browse`, `Remote`, …). Nothing was built or run for this audit (the macOS,
Windows and Android apps have never run, by their own docs), so behaviour that
only shows at run time is marked **unclear** with the reason. The v0.0.7 parts
of ui.md (AI Lyrics, Customize, Pro, Settings › Extensions, Discover, Get Info,
comments, the lyrics bar, the relay, About's legal links and updates, covers
fetched by the core) are listed in [section 5](#5-v007-not-built) and not
audited: none of them exists in any app yet.

**Paths** are relative to these roots:

| Short | Path |
|---|---|
| `mac/` | `platforms/macos/Jellyfesh/Sources/` |
| `win/` | `platforms/windows/src/` |
| `linux/` | `platforms/linux/src/` |
| `and/` | `platforms/android/app/src/main/kotlin/org/jellyfesh/android/` |
| `and-logic/` | `platforms/android/logic/src/main/kotlin/org/jellyfesh/logic/` |
| `and-ffi/` | `platforms/android/app/src/ffi/kotlin/org/jellyfesh/android/ffi/` |
| `web/` | `platforms/web/src/` |

English string tables: `platforms/macos/Jellyfesh/Resources/Localizable.xcstrings`,
`win/Jellyfesh.App/Strings/en-US/Resources.resw`, `platforms/linux/po/jellyfesh.pot`
(msgids from `linux/`), `platforms/android/app/src/main/res/values/strings.xml`,
and the web's English keys in `t('…')` calls (translations in `web/lib/i18n/*.json`).
A changed English string needs its three translations in every app.

## Summary

### Across apps

1. **Source covers** load only on Windows and the web. macOS, Linux and
   Android show a placeholder for every source item, for a source song in Now
   Playing and for a remote session's track ([1.13](#113-artwork-and-covers)).
   ADR-0022 item 5 (v0.0.7) moves cover fetching into the core; until it lands
   there is no shared way to do it, so the fix for macOS, Linux and Android
   (and for Windows, which fetches around the plugin host) is to show the
   core's cached file once that API exists [N29, N30].
2. **Resume Playback does nothing** on macOS, Windows and Android: the core
   restores the queue and position but never reads `resume_on_launch`
   (`core/jf-core/src/actor.rs`); only Linux sends `Play` at start [N26].
3. **Sign-in steps**: only the web draws every step and field as the core
   describes it. Windows fails on `ShowCode` (Jellyfin's "Sign In with QR
   Code" is Quick Connect, which shows a code); macOS and Android squeeze the
   code into a small waiting line; all four native apps drop the fields'
   `help`, `required` and `kind` (Linux also the placeholder) [N9–N12].
4. **Source albums and playlists** have the full page (artwork, Play, Shuffle,
   More) only on the web [N18].
5. **Songs › Play and Shuffle** cover the whole library only on macOS and the
   web [N2]. The core has no whole-library collection; one
   (`Collection::Library { sort, descending }` or similar) would let every app
   do the same.

### Per app, most important first

**macOS**
1. Source covers never load: `mac/Support/ArtworkLoader.swift:67` returns no path for `.url` [N29].
2. Resume Playback is ignored at launch: nothing in `mac/App/AppModel.swift` `start()` sends `Play` [N26].
3. Fixed: Jellyfin's Quick Connect code is a step of its own, and fields keep their help, required and kind [N9, N11].
4. Fixed: source album, artist and playlist pages have the library's header with artwork, Play, Shuffle and More [N18].
5. Fixed: About has the website and the third-party licences when the build knows the website [N28].
6. Fixed: the words of [3](#3-the-words) (swatches Blue … Violet, System Default, Install from File…, Word by word / Line by line / Unsynced).

**Windows**
1. Fixed: Jellyfin's Quick Connect code is a step of its own, and fields keep their help, required and kind [N10, N11].
2. Resume Playback is ignored at launch (`win/Jellyfesh.Shared/ViewModels/AppViewModel.cs` `Start()`) [N26].
3. Fixed: a source's albums, artists, playlists and folders open pages of their own (artwork, Play, Shuffle, More, the songs and their total), from its catalogue and from Search [N6, N17, N18].
4. Fixed: "Content from {Service}" on a source's pages and under its group in Search [N19].
5. Songs › Play and Shuffle take the first 2,000 songs; a row plays 500 [N2].
6. Fixed: About links to the website and its licences when the build knows them [N28].
7. Fixed: the words of [3](#3-the-words); also Add to Playlist… on tiles (N3), "Also in …" (N5), and the views of V1, V3, V5, V6, V10, V11, V14, V15, V16, V18, V19, V20 and V21 as the other apps have them.

**Linux**
1. Source covers never load: `linux/ui/convert.rs:36` maps `ArtworkRef::Url` to nothing [N29].
2. Fixed: About has the website and the third-party licences when the build knows the website (`JF_WEBSITE`, else GitHub Pages), else the notices file installed next to the app; the licence line is gone [N28].
3. Fixed: sign-in fields show their label, placeholder, help, Optional and an input for their kind; Server Address has the core's example as its placeholder; a QR code keeps the plugin's sentence [N7, N11].
4. Songs › Play queues the first 500 songs, a row the next 500, Shuffle 5,000 [N2].
5. Fixed: source album, artist and playlist pages have the library's header with artwork, Play, Shuffle and More, numbered songs and the footer; folders a large title [N18].
6. Fixed: merged search results say "Also in …" [N5].
7. Fixed: the words of [3](#3-the-words) (System Default, Rescan Library, Remove from Queue, Playlists (M3U8)) and V3–V7, V10, V15, V19–V21.

**Android**
1. Fixed: source covers come through the core (`source_artwork` and `Event::Artwork`; the media session gets the cached file) [N29].
2. Fixed: Resume Playback at launch: the core reads `resume_on_launch` since M3, and the app shows what it resumes [N26].
3. Fixed: Songs › Play, Shuffle and a row play the whole list (`Collection::Songs`) [N2].
4. Fixed: Jellyfin's Quick Connect code is a step of its own, and fields keep their help, required and kind [N9, N12].
5. Fixed: choosing a source in Your Sources opens its Discover page, with Browse in the header (its settings page while it is off) [N15].
6. Fixed: "Content from {Service}"; the media session follows remote sessions; Up Next lists every entry; plugin icons in every list; a source's album and playlist pages [N19, N21, N25, N31, N18].
7. Fixed: the words of [3](#3-the-words) (System Default, No Folders Yet, Remove from Queue, Unknown Title, Also in …).

**Web**
1. No remote playback (remote plugins are refused at install) and no Sign In in Browser or Paste Cookies. docs/web.md explains both, but ui.md does not list them as allowed differences: list them there, or build them (ADR-0022's relay covers part of it) [N13, N20].
2. Now Playing › Info: no Year, and the File row says "Dropped on this page; not saved" for library songs too (a bug) [N23]; Now Playing's More is hidden for library songs [N22].
3. Up Next reorders only through Move Up and Move Down; no drag [N24].
4. Artists is a grid of cards (ui.md: a list) [N1].
5. Settings › Sources has one Sign In button instead of one per method, and the connect step shows the plugin's whole settings form [N8, N16].
6. No attribution; About says "Third-party notices" and has no website link [N19, N28].

## 1. Function matrix

✓ works as ui.md says · ✗ missing · **partial:** what is missing ·
**n/a (allowed: …)** a difference ui.md allows. `[Nn]` points to the note
under the table: where the code is, or where the missing part belongs.

### 1.1 Navigation and pages

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Places in order: Home, Albums, Artists, Songs, Playlists, Favourites; Search, Sources; Settings | ✓ | ✓ | ✓ | ✓ | ✓ |
| Settings reached the platform's way (⌘, window; pane foot; main menu; gear on Home, Library, Sources; sidebar or tab) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Home: library size line | ✓ | ✓ | ✓ | ✓ | ✓ |
| Home: Recently Added (albums) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Home: Recently Played, Most Played | ✓ | ✓ | ✓ | ✓ | n/a (allowed: no history; each signed-in source's start page instead) |
| Home: welcome with Add Music Folder… and Add a Source | ✓ | ✓ | ✓ | ✓ | ✓ |
| Albums: grid, empty state | ✓ | ✓ | ✓ | ✓ | ✓ |
| Artists: list, empty state | ✓ | ✓ | ✓ | ✓ | partial: a grid of cards [N1] |
| Songs: Play, Shuffle and a row cover the whole list | partial: a row plays only the loaded rows [N2] | partial: 2,000 (a row: 500) [N2] | partial: Play 500, Shuffle 5,000 [N2] | ✓ (`Collection::Songs`) | ✓ |
| Songs: empty state | ✓ | ✓ | ✓ | ✓ | ✓ |
| Album page: artwork, facts, Play, Shuffle, More | ✓ | ✓ | ✓ | ✓ | ✓ |
| Artist page: picture, albums, songs, Play, Shuffle, More | ✓ | ✓ | ✓ | ✓ | ✓ |
| Playlists page: New Playlist…, Import Playlist…, empty state | ✓ | ✓ | ✓ | ✓ | n/a (allowed: no playlists) |
| Favourites page: Play, Shuffle, empty state | ✓ | ✓ | ✓ | ✓ | n/a (allowed: no favourites) |
| Sources page (see 1.4) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Settings: Appearance, Playback, Library, Sources (+ About where kept) | ✓ | ✓ | ✓ | ✓ | ✓ |

* **N1** Web Artists: `web/views/Artists.svelte` renders through `web/components/ItemList.svelte`, which puts every container (artist, album, playlist) in a `.grid` of `ItemCard`s. ui.md: "Artists. A list." Give `ItemList` a row style for artists (as `ItemRow`) or a `layout="list"` prop for this page.
* **N2** The core has no whole-library collection, so each app builds a track list:
  macOS `mac/Stores/LibraryPages.swift` `playAll`/`allTracks` pages through `songs()` up to `LibraryStore.trackLimit` (100,000) for Play and Shuffle, but a row (`mac/Views/Library/SongsView.swift`, `trackListActions`) plays only the rows loaded so far (pages of 200);
  Windows `win/Jellyfesh.Shared/ViewModels/LibraryViewModels.cs:572` `WholeList` = 2,000 for Play and Shuffle, `PlayFrom` 500 from the row;
  Linux `linux/ui/pages/library.rs:376` `play_songs_from`: 500 from the row (Play is the row 0 case), Shuffle 5,000;
  Android `and/ui/library/LibraryScreen.kt:304` plays `songs.loaded()` only;
  web `web/views/Songs.svelte` `playAll` takes every key of the sorted library.
  Do what macOS does for Play and Shuffle (page through `tracks()`), and use the same list from a row, or ask the core for a library collection.

### 1.2 Menus: song, album, artist, playlist; Play, Shuffle and sort

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Song menu: Play Next, Play Last | ✓ | ✓ | ✓ | ✓ | ✓ |
| Song menu: Add to Playlist… (with New Playlist…) | ✓ | ✓ | ✓ | ✓ | n/a (allowed: no playlists) |
| Song menu: Add to / Remove from Favourites | ✓ | ✓ | ✓ | ✓ | n/a (allowed: no favourites) |
| Song menu: Go to Album, Go to Artist | ✓ | ✓ | ✓ | ✓ | ✓ |
| Song menu: Open in {Service} after Go to Artist | ✓ | ✓ | ✓ | ✓ | ✓ |
| Song menu: Show in Finder / Explorer / Files | ✓ | ✓ | ✓ | n/a (allowed: desktops only) | n/a (a page cannot reveal files) |
| A remote source's songs: no Play Next, Play Last, Add to Playlist | ✓ | ✓ | ✓ | ✓ | n/a (follows from N20: no remote sources) |
| Album and artist page More: Play Next, Play Last, Add to Playlist… | ✓ | ✓ | ✓ | ✓ | ✓ (no Add to Playlist…: allowed) |
| Playlist page More: + Rename…, Export Playlist…, Delete Playlist… (asks) | ✓ | ✓ | ✓ (Export last, V7) | ✓ | n/a (allowed: no playlists) |
| Album, artist and playlist tiles and rows: Play, Shuffle, Play Next, Play Last, Add to Playlist… | ✓ | ✓ | ✓ | ✓ | partial: Play Next, Play Last, Go to Artist only [N4] |
| Play and Shuffle buttons on Songs, album, artist and playlist pages | ✓ | ✓ | ✓ | ✓ | ✓ |
| Albums sort: Title, Artist, Year, Recently Added, Descending | ✓ | ✓ | ✓ | ✓ | ✓ (a select, V9) |
| Songs sort: + Album | ✓ | ✓ | ✓ | ✓ | ✓ (a select, V9) |

* **N3** Windows tile menus: `win/Jellyfesh.App/Shell/CollectionMenu.cs` has Play, Shuffle, Play Next, Play Last (and Delete Playlist… on playlist tiles); add the Add to Playlist… submenu (`win/Jellyfesh.App/Shell/PlaylistMenu.cs` `FillAsync`, as `TrackMenu` does).
* **N4** Web cards and rows: `web/components/ItemCard.svelte` and `ItemRow.svelte` use `itemMenu` (`web/lib/items.ts`): Play Next, Play Last, Go to Artist, Open in {Service}. Play is the card's hover button; there is no Shuffle. Add Play and Shuffle to `itemMenu` for containers.

### 1.3 Search

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| One field, "Songs, albums and artists" | ✓ | ✓ | ✓ | ✓ | ✓ |
| Library first: Songs, Albums, Artists | ✓ | ✓ | ✓ | ✓ | ✓ |
| From Your Sources, one group per source | ✓ | ✓ | ✓ | ✓ | ✓ |
| "No Results" | ✓ | ✓ | ✓ | ✓ | ✓ |
| Merged items name the other sources ("Also in …") | ✓ | ✓ | ✓ | ✓ | ✓ |
| A source album, artist or playlist opens its page | ✓ | ✓ | ✓ | ✓ | ✓ |

* **N5** `also_in`: macOS `mac/Views/Search/SearchView.swift` ("Also in %@"), Android `search_also_in`, web `web/components/ItemRow.svelte` ("Also in {sources}"). Windows `win/Jellyfesh.Shared/ViewModels/SearchViewModel.cs:216` writes the source names as `Origin` without the words. Linux never reads `also_in`: add it to the second line in `linux/ui/pages/search.rs` `render_section` (and `linux/ui/convert.rs` `source_item`).
* **N6** Windows `win/Jellyfesh.Shared/ViewModels/SearchViewModel.cs:144` `PlayEntry` sends `PlayCollection(Collection.Source …)` for albums, artists and playlists. Open a browse page instead (see N17).

### 1.4 Sources

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Sources page: intro, Add a Source, Your Sources, Sources Settings… | ✓ | ✓ | ✓ | ✓ | ✓ |
| Add a Source from the core's presets, with the five descriptions | ✓ | ✓ | ✓ | ✓ | ✓ |
| Connect: Server Address (the core's example as placeholder), one button per method | ✓ | ✓ | ✓ | ✓ | partial: the whole settings form [N8] |
| Sign in with user name and password | ✓ | ✓ | ✓ | ✓ | ✓ |
| Sign in with an API key | ✓ | ✓ | ✓ | ✓ | ✓ |
| Sign in with a QR code (`ShowQr`) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Sign in with a code shown large (`ShowCode`: Jellyfin Quick Connect) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Sign in in the browser (OAuth, `OpenUrl`) | ✓ | ✓ | ✓ | ✓ | ✗ [N13] |
| Fields as the plugin asks (label, secret, placeholder, help, required, kind) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Paste Cookies (cookie import) | ✓ (untested, N14) | ✓ (untested, N14) | ✓ (untested, N14) | ✓ (untested, N14) | ✗ [N13] |
| Sign Out | ✓ | ✓ | ✓ | ✓ | ✓ |
| Your Sources: each with its badge and sign-in state, or Off | ✓ | ✓ | ✓ | ✓ | partial: no badge [N15] |
| Your Sources: choosing one browses it | ✓ | ✓ | ✓ | ✓ | ✓ |
| Settings › Sources › Installed: a button per sign-in method, or Sign Out | ✓ | ✓ | ✓ | ✓ | partial: one Sign In [N16] |
| Browse: pages, Load More, folders and artists drill in | ✓ | ✓ | ✓ | ✓ | ✓ |
| Browse: an album or playlist opens its songs | ✓ | ✓ | ✓ | ✓ | ✓ |
| Source album, artist, playlist page: artwork, Play, Shuffle, More | ✓ | ✓ (an artist page has no picture yet) | ✓ | partial: albums and playlists ✓; an artist has no header yet (Play and Shuffle in More) [N18] | ✓ |
| Open in {Service}: song menu, More menus, Now Playing More and Info | ✓ | ✓ | ✓ | ✓ | ✓ |
| An unplayable item with a link opens it as its main action | ✓ | ✓ | ✓ | ✓ | ✓ |
| "Content from {Service}" for sources with `attribution` | ✓ | ✓ | ✓ | ✓ | ✗ [N19] |
| Remote: "Playing on {Device}" and the error line in the player bar, mini-player, Now Playing | ✓ | ✓ | ✓ | ✓ | ✗ [N20] |
| Remote: Devices (sends `Refresh` on open; kinds; playing one checked; `Transfer`) | ✓ | ✓ | ✓ | ✓ | ✗ [N20] |
| Remote: the device's volume, hidden without one | ✓ | ✓ | ✓ | ✓ | ✗ [N20] |
| Remote: More holds Open in {Service} and Stop Controlling | ✓ | ✓ | ✓ | ✓ | ✗ [N20] |
| Remote: "Lyrics aren't shown while …", "Up Next is in …" | ✓ | ✓ | ✓ | ✓ | ✗ [N20] |
| Remote: an ending error shows once | ✓ | ✓ | ✓ | ✓ | ✗ [N20] |
| Remote: the system's media controls follow the session | ✓ | ✓ | ✓ | ✓ | ✗ [N20] |

* **N7** Linux `linux/ui/sources_ui.rs:352` `connect_dialog` shows the example as the group's description ("For example …") because `AdwEntryRow` has no placeholder. Acceptable for GTK; keep the words "For example …" in mind if the others ever show a note.
* **N8** Web `web/components/SignInSheet.svelte:91`: stage `settings` renders `SettingsForm` with the plugin's whole schema (Server Address, Streaming Quality, Format When the Server Converts, …) and **Continue**, then a list of methods. Show only the address field (with the preset's example) and the method buttons, as the apps do.
* **N9** macOS `mac/CoreClient/FFIConversions.swift:563` and Android `and-ffi/FfiMapping.kt:695` turn `ShowCode` into `Waiting("hint: code")`, drawn as small secondary text. Add a `showCode(code, hint, expiresAtMs)` step to `mac/CoreClient/CoreModels.swift` `SignInStep` and `and-logic/core/Model.kt` `AuthStep`, and draw it large and selectable (as Linux `linux/ui/sources_ui.rs:630` and web `SignInSheet.svelte` do) in `mac/Views/Sources/SignInSheet.swift` and `and/ui/sources/SignIn.kt`.
* **N10** Windows `win/Jellyfesh.Interop/FfiMapping.cs:249` maps any unknown step, `ShowCode` included, to `Failed("unknown sign-in step")`; the binding has `AuthStep.ShowCode` (`core/jf-ffi/csharp/Jellyfesh.Ffi/Types.cs`). Add `ShowCode` to `win/Jellyfesh.Shared/Core/Model.cs` `AuthStep`, a stage to `win/Jellyfesh.Shared/ViewModels/SignInViewModel.cs` `Apply`, and a panel to `win/Jellyfesh.App/Views/SignInDialog.xaml`.
* **N11** The core's `AuthField` has `kind`, `required`, `placeholder` and `help`; `ShowQr` has `hint`. macOS `SignInField` (`mac/CoreClient/CoreModels.swift:545`) and Windows `AuthField` (`win/Jellyfesh.Shared/Core/Model.cs:328`) keep only name, label, secret, placeholder; Linux `linux/ui/sources_ui.rs:605` `render_step` uses only label and secret. Carry all four and draw help under the field, mark optional ones, pick the keyboard from `kind` (a multi-line field for `CookieHeader`). The web (`SignInSheet.svelte`) is the model.
* **N12** Android `AuthField` (`and-logic/core/Model.kt:219`) has no `required`, so `AuthFlow.missing` (`and-logic/sources/AuthFlow.kt:72`) treats every field as required and Sign In stays off until optional ones are filled; no help, no kind (the keyboard is guessed from the field's name in `and/ui/sources/SignIn.kt`).
* **N13** Web `web/lib/state/sources.svelte.ts:226` drops `Oauth` and `CookieImport` from the methods (the redirect cannot reach a page; browsers cannot set `Cookie`). docs/web.md says so; ui.md's Allowed differences do not. Add it there, or offer them through the relay (ADR-0022, v0.0.7).
* **N14** No shipped plugin signs in with cookie import yet (`plugins/*/manifest.toml`), so the native "Paste Cookies" path (a button, then `AskFields`) has never met a real plugin.
* **N15** Android `and/ui/sources/SourcesScreens.kt:126` pushes `Screen.Source` (the source's settings page: badge, Enabled, Account, settings, Browse, Version, Uninstall…); browsing is its **Browse** row. ui.md: choosing one browses it. Push `Screen.SourceBrowse(plugin.id, null, plugin.name)` when the source is on and can browse, as macOS (`mac/Views/Sources/SourcesView.swift:75`) does. Web `web/views/SourcesView.svelte` `status()` shows "Signed in as …", "Not signed in", "Turned off", "Ready" without the Official or Community · unofficial badge.
* **N16** Web `web/views/SourcesSettings.svelte:159` shows one **Sign In** (the methods are chosen in the sheet). Show one button per method with the shared labels (Sign In with QR Code, Sign In in Browser, Paste Cookies, Sign In, Enter API Key).
* **N17** Windows `win/Jellyfesh.Shared/ViewModels/SourcesViewModel.cs:400` `OpenEntry`: folders and artists drill in; albums and playlists send `PlayCollection` at once, so their songs are never listed. Drill into them like folders (a new `BrowseLevel` and `Browse(entry.Item.Id …)`), with Play, Shuffle and More for the level.
* **N18** The web's `web/views/Browse.svelte` is the model: artwork, kind, title, the artist in accent text, year and count, **Play**, **Shuffle**, More, numbered songs and the "12 songs, 48 minutes" footer. macOS `mac/Views/Sources/SourcesView.swift:212` `BrowseView` has a title, the attribution or source name, rows and a More with only Open in {Service}; Linux `linux/ui/pages/sources.rs:185` `browse_page` has rows, Load More and the same More; Android `and/ui/sources/SourcesScreens.kt:446` `SourceBrowseScreen` has rows, Load More and a More with Play, Shuffle, Play Next, Play Last and Open in. v0.0.7's "Singer and album" makes these the library's album and artist pages; build toward that.
* **N19** Attribution: macOS (`BrowseView` subtitle, `SourcesStore.attribution(of:)`) and Linux (`linux/ui/pages/sources.rs` `attribution_text`) show it. Windows `PluginInfo` (`win/Jellyfesh.Shared/Core/Model.cs:290`, mapped in `win/Jellyfesh.Interop/FfiMapping.cs` `ToModel(F.PluginSummary)`) and Android `PluginSummary` (`and-logic/core/Model.kt:163`, `and-ffi/FfiMapping.kt`) do not carry `attribution`; the web has it in the manifest's content policy (`web/lib/core/types.ts` `ContentPolicy.attribution`) and never shows it.
* **N20** The web refuses remote plugins at install (`core/jf-web/src/lib.rs` `install_refusal`, called from `web/lib/plugins/host.ts:687`): "{Name} plays on your own devices, which Jellyfesh Web cannot do yet." ui.md's Allowed differences do not list it; add it there (no output device, no remote sessions) or plan it.
* **N21** Android's media session is built from the engine only: `SessionModels.from` (`and-logic/player/PlayerModel.kt:147`) reads `state.nowPlaying`, so the notification and lock screen show nothing (or the paused engine song) during a remote session (docs/android.md says so). macOS (`mac/Platform/NowPlayingCenter.swift`), Windows (`win/Jellyfesh.App/Platform/MediaTransport.cs`) and Linux (`linux/ui/mpris.rs`) follow the `Remote` slice.

### 1.5 Now Playing

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Artwork backdrop or the Misty field | ✓ | ✓ | ✓ | ✓ | ✓ |
| Shuffle, previous, play, next, repeat, scrubber with times | ✓ | ✓ | ✓ | ✓ | ✓ |
| Volume with mute | ✓ | ✓ | ✓ | ✓ | ✓ |
| Favourite heart | ✓ | ✓ | ✓ | ✓ | n/a (allowed: no favourites) |
| Device button (output device, or Devices) | ✓ | ✓ | ✓ | ✓ | n/a (allowed: a browser decides where sound goes) |
| More (the song's menu; Open in, Stop Controlling for a remote session) | ✓ | ✓ | ✓ | ✓ | partial: only for source songs [N22] |
| Panes: Lyrics, Queue, Info | ✓ | ✓ | ✓ | ✓ | ✓ |
| Closes with Library (desktops) or ⌄ (phones) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Lyrics: translation line | ✓ | ✓ | ✓ | ✓ | ✓ |
| Lyrics: romanization line | ✓ | ✓ | ✓ | ✓ | ✓ |
| Lyrics: Lyrics Size | ✓ | ✓ | ✓ | ✓ | ✓ |
| Lyrics: Lyrics Alignment | ✓ | ✓ | ✓ | ✓ | ✓ |
| Lyrics: word sweep | ✓ | ✓ | ✓ | ✓ | ✓ |
| Lyrics: tap a line to seek | ✓ | ✓ | ✓ | ✓ | ✓ |
| Info: About This Song: Title, Artist, Album, Year, Length, Format, Lyrics, File | ✓ | ✓ (Year for library songs) | ✓ | ✓ | partial [N23] |
| Info: Open in {Service} under About This Song | ✓ | ✓ | ✓ | ✓ | ✓ |

* **N22** Web `web/lib/items.ts:168` `nowPlayingMenu` returns nothing for library songs and dropped files, and `web/nowplaying/NowPlaying.svelte:166` then hides More. Library songs should get Go to Album and Go to Artist here, as their rows' menus already do (`itemMenu`).
* **N23** Web `web/nowplaying/InfoPane.svelte`: no **Year** row; the rows run Title, Artist, Album, Length, Format, File or Source, Lyrics (ui.md: … Year, Length, Format, Lyrics, File); and line 43 prints "Dropped on this page; not saved" for every `row.kind === 'file'`, which includes library songs (their ids start with `lib:`, `web/lib/state/library.svelte.ts:30`). Show the folder and file name for library songs.

### 1.6 Queue, mini-player and player bar

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Up Next; choosing an entry jumps to it | ✓ | ✓ | ✓ | ✓ | ✓ |
| Reorder Up Next | ✓ | ✓ | ✓ | ✓ | partial: Move Up and Move Down only [N24] |
| Remove an entry | ✓ | ✓ | ✓ | ✓ | ✓ |
| Entries past the slice's first 50 | ✓ | ✓ | ✓ | ✓ | ✓ |
| Clear Queue asks first ("Clear the Queue?") | ✓ | ✓ | ✓ | ✓ | ✓ |
| "Nothing Up Next" | ✓ | ✓ | ✓ | ✓ | ✓ |
| Mini-player (narrow windows, phones) | ✓ | ✓ | ✓ | ✓ | ✓ |
| Player bar (900 wide and wider): artwork, two lines, transport, seek bar, Lyrics, Queue, volume | ✓ | ✓ | ✓ | ✓ | ✓ |

* **N24** Web `web/nowplaying/QueuePane.svelte:61` offers Move Up, Move Down and Remove in each entry's menu; every app drags (Android by a handle). Add drag (a handle on touch screens) over the `VirtualList`, keeping the menu items for keyboards.
* **N25** Android `and/ui/player/Queue.kt` lists the slice's 50 entries and then "N more songs in the queue" (`queue_more`). Page the rest with `queue_window`, as macOS (`mac/Stores/QueueStore.swift`), Windows (`QueueViewModel`) and Linux (`linux/ui/nowplaying/queue.rs`) do.

### 1.7 Playback

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Gapless | ✓ | ✓ | ✓ | ✓ | partial: near-gapless [N27] |
| Crossfade, Length 1–12 s, Album Continuity | ✓ | ✓ | ✓ | ✓ | ✓ |
| ReplayGain (Off, Track, Album) and Preamp | ✓ | ✓ | ✓ | ✓ | partial: not for server streams [N27] |
| Equalizer: on/off, Preset from `eq_presets()`, Custom, Preamp, ten bands, Reset to Flat | ✓ | ✓ | ✓ | ✓ | ✓ |
| Output Device (System Default, then the devices) | ✓ | ✓ | ✓ | ✓ | n/a (allowed: the browser's) |
| Resume Playback at launch | ✗ [N26] | ✗ [N26] | ✓ | ✓ (the core, since M3) [N26] | partial: ready to play [N26] |
| Volume | ✓ | ✓ | ✓ | ✓ | ✓ |
| Mute | ✓ | ✓ | ✓ | ✓ | ✓ |
| Streaming Quality | n/a (allowed: web only) | n/a (allowed: web only) | n/a (allowed: web only) | n/a (allowed: web only) | ✓ |

* **N26** `core/jf-core/src/actor.rs` restores the queue and position (`resume_at`) but nothing in the core or the FFI reads `Settings::resume_on_launch`. Linux sends `Play` at start when it is on and the queue has a current entry (`linux/ui/mod.rs:172`). macOS (`mac/App/AppModel.swift:97` `start()`), Windows (`win/Jellyfesh.Shared/ViewModels/AppViewModel.cs:76` `Start()`) and Android (`and/AppGraph.kt:79` `start()`) only store the switch. Do what Linux does after the first snapshot (or move the rule into the core). The web restores the queue and position ready to play (`web/lib/state/player.svelte.ts:211`) because browsers start no sound before a click; docs/web.md says so, ui.md's Allowed differences do not.
* **N27** Web audio runs in the browser (docs/web.md, "Audio"): the next song starts by the element clock, measured with no silence at the join but not sample-exact; server streams carry no ReplayGain values through the plugins. Both are in docs/web.md's differences table, not in ui.md's Allowed differences.

### 1.8 Library

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Music Folders: the list and Add Folder… | ✓ | ✓ | ✓ | ✓ (+ Music on This Phone: allowed) | ✓ (Pick Folder… on each visit: allowed) |
| Show in Finder / Explorer / Files per folder | ✓ | ✓ | ✓ | n/a (allowed: no file manager by path) | n/a (allowed: browser) |
| Remove Folder… asks first | ✓ | ✓ | ✓ | ✓ | ✓ (its own message: allowed) |
| Watch Folders | ✓ | ✓ | ✓ | ✓ | n/a (allowed) |
| Rescan Library with progress and Last Scan | ✓ | ✓ | ✓ | ✓ | ✓ (per folder, V20) |
| Favourites: the heart, the page | ✓ | ✓ | ✓ | ✓ | n/a (allowed) |
| New Playlist… asks for the name first | ✓ | ✓ | ✓ | ✓ | n/a (allowed) |
| Rename… | ✓ | ✓ | ✓ | ✓ | n/a (allowed) |
| Delete Playlist… asks first | ✓ | ✓ | ✓ | ✓ | n/a (allowed) |
| Import Playlist… (M3U8) | ✓ | ✓ | ✓ | ✓ | n/a (allowed) |
| Export Playlist… (M3U8) | ✓ | ✓ | ✓ | ✓ | n/a (allowed) |
| Reorder a playlist's songs (drag; a handle on Android) | ✓ | ✓ | ✓ | ✓ | n/a (allowed) |
| Play counts and history (Recently Played, Most Played) | ✓ | ✓ | ✓ | ✓ | n/a (allowed) |
| Open Files… and dropped files, kept for one visit | n/a | n/a | n/a | n/a | ✓ (allowed: web only) |

### 1.9 Appearance

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Theme: System, Light, Dark with pictures | ✓ | ✓ | ✓ | ✓ | ✓ |
| Accent Colour: Follow the Artwork, ten swatches, Custom hue | ✓ | ✓ (unnamed swatches) | ✓ | ✓ | ✓ |
| Transparency (Solid … Clear) and Reduce Motion and Transparency | ✓ | ✓ | ✓ | ✓ | ✓ |
| Now Playing Backdrop: Artwork, Misty | ✓ | ✓ | ✓ | ✓ | ✓ |
| Lyrics in Appearance: Size, Alignment, Show Translations, Show Romanization | ✓ | ✓ | ✓ | ✓ | ✓ |
| App Icon | ✓ | ✓ | n/a (allowed: GNOME; the note is shown) | ✓ | n/a (ui.md: macOS, Windows, Android) |
| Language | n/a (allowed: web only) | n/a (allowed: web only) | n/a (allowed: web only) | n/a (allowed: web only) | ✓ |

### 1.10 About

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Where About lives (app menu; Settings; main menu; Settings; Settings) | ✓ | ✓ | ✓ | ✓ | ✓ |
| The version, from the build | ✓ | ✓ | ✓ | ✓ | ✓ |
| The website, where the build knows it | ✓ | ✓ | ✓ | ✓ | ✗ [N28] |
| Third-party licences | ✓ | ✓ | ✓ | ✓ | ✓ |

* **N28** macOS `mac/App/AppDelegate.swift` `showAbout()`: the standard panel with the tagline, the Misty credit and the core version. Windows `win/Jellyfesh.App/Views/SettingsPage.xaml` About: version, Third-Party Licences (the notices file next to the app), Open Log Folder. Linux `linux/ui/about.rs`: version, Design, Design Tokens and "Licence: No licence has been chosen yet: all rights reserved." (check against the licence work under way). Android `and/ui/settings/SettingsScreen.kt` `about()`: version, Website and Third-Party Licences when the build knows the website (`jellyfesh.website` or `GITHUB_REPOSITORY`). Web `web/views/Settings.svelte:548`: "Third-party notices" (`THIRD_PARTY_NOTICES.txt`), no website. Give macOS, Windows, Linux and the web the website and licences the way Android derives them.

### 1.11 Plugins

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Install from File… in Installed's heading | ✓ | ✓ | ✓ | ✓ | ✓ |
| Uninstall… asks first | ✓ | ✓ | ✓ | ✓ | ✓ |
| Enabled | ✓ | ✓ | ✓ | ✓ | ✓ |
| Settings forms from the plugin's schema (commit or Save, as allowed) | ✓ | ✓ | ✓ | ✓ (on the source's page) | ✓ (in a sheet) |
| "Version · Author" | ✓ | ✓ | ✓ | ✓ | ✓ |
| Community Plugins: Allow Community Plugins and turning one on show the disclosure | ✓ | ✓ | ✓ | ✓ | ✓ |
| A source that plays on the person's devices (remote) installs | ✓ | ✓ | ✓ | ✓ | ✗ [N20] |

### 1.12 Keyboard

| Keys (⌘ on macOS) | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Space play or pause | ✓ | ✓ | ✓ | n/a (ui.md: desktops and web) | ✓ |
| Ctrl+→ / Ctrl+← next, previous | ✓ | ✓ | ✓ | n/a | ✓ |
| Shift+→ / Shift+← skip 5 s | ✓ | ✓ | ✓ | n/a | ✓ |
| Ctrl+↑ / Ctrl+↓ volume | ✓ | ✓ | ✓ | n/a | ✓ |
| Ctrl+F search | ✓ | ✓ | ✓ | n/a | ✓ |
| Esc closes Now Playing, then goes back | ✓ | ✓ | ✓ | n/a | ✓ |

Where: macOS `mac/App/AppCommands.swift` and `mac/Platform/SystemState.swift` (`SharedKeyMonitor`); Windows `win/Jellyfesh.App/MainWindow.xaml` accelerators and `MainWindow.xaml.cs` (arrows); Linux `linux/ui/mod.rs` accelerators and `linux/ui/keys.rs`; web `web/App.svelte` `key()`. The extra keys disagree between apps (V24).

### 1.13 Artwork and covers

| Function | macOS | Windows | Linux | Android | Web |
|---|---|---|---|---|---|
| Library thumbnails | ✓ | ✓ | ✓ | ✓ | ✓ |
| A source item's cover (search, browse, Home shelves) | ✗ [N29] | ✓ (N30) | ✗ [N29] | ✓ (through the core) | ✓ |
| Now Playing artwork of a source song | ✗ [N29] | ✓ | ✗ [N29] | ✓ (through the core) | ✓ |
| A remote session's track artwork | ✗ [N29] | ✓ | ✗ [N29] | ✓ (through the core) | n/a (no remote sessions) |
| Plugin icons (Add a Source, Your Sources, Installed) | ✓ | ✓ | ✓ | ✓ | ✗ generic glyph [N31] |

The Now Playing backdrop of a source song works everywhere: the core fetches
the artwork through the plugin and bakes it, so only the artwork tile is
missing on macOS, Linux and Android.

**How each app loads a source item's cover URL** (`SourceItem.artwork`,
`NowPlaying.artwork` and `RemoteSnapshot.item` carry `ArtworkRef::Url`; the
core API has no call that fetches one for the app):

| App | What happens |
|---|---|
| macOS | Nothing is fetched. `ArtworkSource.url` reaches `ArtworkLoader.image(_:pixels:)` (`mac/Support/ArtworkLoader.swift:67`), which returns no path, so `ArtworkView` keeps the SF Symbol placeholder. Library art: `artworkThumbnail(hash:sizePx:)` (the core's JPEG at 64, 128, 256, 512 or 1024 px), decoded with ImageIO off the main actor, `NSCache` of 64 MB. |
| Windows | WinUI fetches the URL itself. `ArtworkItem.SetArtwork` (`win/Jellyfesh.Shared/ViewModels/Items.cs:86`) stores `ArtworkRef.ToUriString()`, which is the URL as given (`win/Jellyfesh.Shared/Core/Model.cs:121`); the `Artwork.Uri` attached property (`win/Jellyfesh.App/Controls/Primitives.cs:120`) makes `new BitmapImage { UriSource = url, DecodePixelWidth = size }`. Any absolute URI is accepted (http and https, no scheme check), no headers are sent, the plugin host's allowlist is not involved, and the only cache is WinUI's own image cache (**unclear** whether it persists to disk; never run). Now Playing and the remote track go the same way (`PlayerViewModel.ArtworkUri`, lines 396 and 667). Library art: `ThumbnailService` (`win/Jellyfesh.Shared/Services/ThumbnailService.cs`), the core's thumbnail path, two requests at a time, cancelled when a row is recycled. |
| Linux | Nothing is fetched. `convert::art_of` (`linux/ui/convert.rs:36`) maps `ArtworkRef::Url(_)` to `None`; `ArtKey` (`linux/ui/thumbs.rs`) knows only `Hash` and `File`. Library art: a worker thread asks `Core::artwork_thumbnail`, decodes to a `gdk::Texture`, newest request first, 64 MB LRU. |
| Android | Nothing is fetched by the app. `ArtKey.of` (`and/images/ArtworkLoader.kt:35`) returns null for `ArtworkRef.Url`. Library art: `artworkThumbnail` on the IO dispatcher (two at a time), `BitmapFactory` with `inSampleSize`, an `LruCache` sized by `ArtworkSizes.cacheBudget`. The media session does get the URL as `artworkUri` (`and-logic/player/PlayerModel.kt` `SessionModels`, `and/playback/CorePlayer.kt:56`), so the notification may show the cover through Media3's own loader (**unclear**: never run) while the app shows none. |
| Web | The page fetches it. `Art.svelte` waits until the row is within 200 px of the screen, then `Artworks.ref` (`web/lib/state/artwork.ts`) takes the item's own `ImageRef` when it is wide enough (or has no width), else asks the plugin's `artwork(item, size)` in its worker (when it declares `Artwork`), remembered per item and size (4,000). `Artworks.url`: without headers the URL goes straight into `<img loading="lazy">` (the browser's HTTP cache); with headers it is `fetch(url, { headers, mode: 'cors', credentials: 'omit' })` turned into an object URL, 300 kept. No scheme check in the page; on https the browser blocks or upgrades http images (mixed content); CSP `img-src * data: blob:` (`platforms/web/index.html`). Now Playing asks the same at 1,024 px (`web/lib/state/player.svelte.ts` `loadArtwork`). Library covers come from the library's own store (`web/lib/state/library.svelte.ts` `coverUrl`). |

* **N29** ADR-0022 item 5 (v0.0.7): "The core fetches a source item's picture through the plugin host (its allowlist and the picture's headers) into the artwork cache, and the apps show the cached file." When that API lands, macOS (`ArtworkLoader`), Linux (`convert::art_of`, `ArtKey`) and Android (`ArtKey.of`) should load the cached file it gives, and Windows should switch to it (N30). The web already goes through the plugin (`artwork` export, the picture's headers); it needs no change unless jf-web gains the same call.
* **N30** Windows loads covers, but around the plugin host: no headers (covers that need them fail), http allowed, outside the allowlist. Replace with the core's cached file (N29).
* **N31** Android shows plugin icons only for Add a Source (`PresetIcon`); Your Sources (`and/ui/sources/SourcesScreens.kt`) and Settings › Sources › Installed (`and/ui/settings/SettingsScreen.kt` `installed`) draw `InitialTile`. The web shows a globe (`web/components/AddSource.svelte`) or the sources glyph (`web/views/SourcesView.svelte`) and never the package's `icon.png`.

## 2. View differences

Where the same page looks or reads differently between apps beyond ui.md's
Allowed differences. "Platform mechanics" (sheets on Android, dialogs on
Windows, menus on macOS and GTK) are not listed.

**Home**
* **V1** Recently Played and Most Played are song rows on one surface on macOS (`mac/Views/Library/HomeView.swift`), Linux (`linux/ui/pages/home.rs` `Shelf`) and Android (`and/ui/home/HomeScreen.kt` `trackSection`), but sideways shelves of 148 px tiles on Windows (`win/Jellyfesh.App/Views/HomePage.xaml` `PlayedShelf`, `MostShelf` with `TrackTileTemplate`). Recently Added is a sideways shelf everywhere except the web, where it wraps as a grid (`web/views/Home.svelte` through `ItemList`).

**Library pages**
* **V2** Artists: a list everywhere, a grid of cards on the web (N1).
* **V3** Playlists: a list on macOS, Linux and Android, a grid of tiles on Windows (`win/Jellyfesh.App/Views/PlaylistsPage.xaml`). New Playlist… and Import Playlist… are labelled buttons under the title on macOS, Windows and Android, but icon buttons (+ and a folder, names only in tooltips) in the header bar on Linux (`linux/ui/pages/playlists.rs`).
* **V4** Favourites: Play and Shuffle are pill buttons under the title on macOS, Windows and Android, icon buttons in the header bar on Linux (`linux/ui/pages/favourites.rs`), while Linux's Songs page uses pills.
* **V5** An empty playlist: macOS "This Playlist Is Empty" and "Add songs from their menu with Add to Playlist."; Windows "This playlist is empty. Add songs from any list with a right-click." (`Playlist_Empty`); Android "This playlist is empty."; Linux shows nothing. ui.md has no words for it; agree on one title and sentence.
* **V6** Songs' default order: Artist on macOS (`mac/Stores/LibraryPages.swift`), Android (`and-logic/library/LibraryModel.kt` `DEFAULT_SONGS`) and the web (`web/views/Songs.svelte`); Title on Windows and Linux. Year and Recently Added start descending on Windows, Linux and Android, ascending on macOS and the web.
* **V7** A playlist's More: Rename…, Export Playlist…, Delete Playlist… on macOS, Windows and Android; Rename…, Delete Playlist…, then Export Playlist… on Linux (`linux/ui/pages/playlists.rs` `playlist_page`).
* **V8** Windows' album, artist and playlist tiles have a shorter menu than the page's More (N3); the web's cards have no Play or Shuffle in theirs (N4).
* **V9** The sort control is a menu (macOS toolbar, Windows flyout, GTK popover), a sheet on Android, and a `<select>` with a Descending toggle button on the web (`web/components/SortMenu.svelte`).

**Search**
* **V10** Before anything is typed: "Search Your Music" with a sentence on macOS, Linux, Android and the web (three different sentences), nothing on Windows. After nothing is found: "No Results" with "Try other words, or check the spelling." (macOS), "Try other words." (Linux, Android, web) or nothing (Windows).
* **V11** Library results: Albums and Artists are sideways tile shelves on Windows (`win/Jellyfesh.App/Views/SearchPage.xaml`), grids on the web, rows on macOS, Linux and Android. Windows alone adds **Play All** (`Search_PlayAll`).

**Sources**
* **V12** Android's Sources tab has **Install from File…** next to Sources Settings… (`and/ui/sources/SourcesScreens.kt`); no other app has it on the Sources page.
* **V13** Your Sources' second line: macOS, Windows and Linux "Official · Signed in as …" (and "Off"); Android "Official · Enabled" for a source without sign-in; the web "Ready", "Turned off" or "Community plugins are turned off." without the badge (N15).
* **V14** A source's catalogue: in place under a breadcrumb on Windows; pages on the others. Folders, albums and playlists are listed above the songs in a separate list on Windows, mixed in order elsewhere; folders as rows, containers as a card grid and songs as rows on the web (`web/components/ItemList.svelte`).
* **V15** Loading more of a catalogue: "Load More" (macOS, Linux, Android, web), "Show More" (Windows `Sources_More`). A failed page: "This Source Did Not Answer" with the error (macOS, Linux, web), the bare error (Windows in red, Android as a notice). An empty one: "Nothing Here" (macOS), "Nothing here." (Android), "Nothing here yet." (web), nothing (Windows, Linux).
* **V16** The sign-in sheet's words differ on Windows (section 3); its title is "Connect to {name}" on the web when started from Add a Source and "Sign In to {name}" in the apps (which show "Connect to {name}" in a separate step first). Android counts down a QR code's expiry; the others do not.
* **V17** Only macOS offers Apple Music (Add a Source and Your Sources, `mac/Views/Sources/AppleMusicView.swift`, ADR-0010); ui.md does not mention it.

**Now Playing**
* **V18** The Lyrics pane with no lyrics: "No Lyrics" with "Lyrics show here when the song has them: in its tags, or in an .lrc or .ttml file beside it." (macOS), "Jellyfesh shows lyrics from a .lrc, .ttml or .vtt file next to the song, or from its tags." (Linux, web), "Put an .lrc or .ttml file next to the song, or lyrics in its tags." (Android); "No Lyrics for This Song" alone on Windows (`Lyrics_None`, `win/Jellyfesh.App/Views/LyricsView.xaml`). Nothing playing: "Nothing Is Playing" (macOS) or "Nothing Playing" with "Pick a song, an album or a playlist." (Linux, Android, web).
* **V19** Up Next when empty: "Nothing Up Next" with "Use Play Next or Play Last on any song, album or playlist." everywhere but Windows, which has the title only (`Queue_Empty`). Removing an entry: "Remove from Queue" (macOS, Windows), "Remove" (Linux `linux/ui/rows.rs:290`, Android `action_remove`, web). Windows also has the compact mini player (Ctrl+M); no other app does.

**Settings**
* **V20** Library › Rescan: the button is "Rescan Library" on macOS, Windows and the web, "Rescan" on Linux (`linux/ui/prefs.rs:1017`); Android makes the whole row the button. Last Scan is one row on macOS, Windows, Linux and Android; the web puts each folder's last scan under its name ("Not read yet", "Added 3, updated 0, removed 1") and adds a **Size** row (`web/views/LibrarySettings.svelte`). With no folders: "No folders yet" (macOS, web), "No folders yet." (Android), "No Folders Yet" with "Add a folder with music." (Linux), nothing (Windows).
* **V21** Descriptions under the same rows differ (ui.md fixes only the theme sentence and those quoted in it):

  | Row | macOS | Windows | Linux | Android | Web |
  |---|---|---|---|---|---|
  | Accent Colour | The colour of buttons, selections and the Misty field. | same as macOS | The accent and the Misty field take the hue of the album cover. | same as macOS | The accent and the field take the hue of what plays. |
  | Transparency | How much of the background shows through the glass. | same as macOS | none | same as macOS | How much of what lies behind shows through glass. |
  | Reduce Motion and Transparency | Solid surfaces instead of glass, and short fades instead of movement. | a longer sentence naming the word sweep and Windows' settings | Solid surfaces instead of glass, and short fades instead of motion. | same as macOS | Solid surfaces instead of glass, and shorter motion. |
  | Now Playing Backdrop | one sentence per picture | The artwork, blurred, or the Misty field in its colour. | none | same as Windows | Artwork blurs the album cover behind Now Playing; Misty uses the Misty field in its colour. |
  | Show Translations | A translation under each line, when the lyrics have one. | none (a group note) | A line in another language under each lyric line, when the lyrics have one. | Under each line, when the lyrics have them. | same as Linux |
  | ReplayGain's Preamp | none | none | Decibels added to every song | none | same as Linux |
  | Output Device | Where the sound goes. | none | Where the sound goes. | Where the sound goes. | n/a |

* **V22** Settings › Sources: the web opens each source's settings in a sheet behind a **Settings** button and has one **Sign In** (N16); Android opens a page per source; macOS, Windows and Linux expand the row in place.
* **V23** About: Windows adds Open Log Folder; macOS adds the core version; the web adds Install Jellyfesh Web and lists the keyboard shortcuts (allowed).

**Keyboard**
* **V24** The extra keys, which ui.md leaves to each platform, give the same key different jobs: Ctrl+M is the compact player on Windows and Mute on Linux and the web; Ctrl+P is Play or Pause on Windows and Now Playing on Linux and the web; Ctrl+L is Now Playing on Windows (⌘L on macOS) and Lyrics on Linux and the web (⌥⌘L on macOS); shuffle and repeat are Ctrl+H and Ctrl+T on Windows, Ctrl+S and Ctrl+R on Linux and the web (where they also take the browser's Save and Reload). Agree on one set beyond ui.md's.

## 3. The words

English strings that differ from ui.md's Words (or from its Title Case rule),
per app, with the words the apps disagree on where ui.md does not fix them.
Where an app has them, it already uses ui.md's words for the confirmations
("Remove “Name” from the library?", "Uninstall Name?", "Delete “Name”?",
"Clear the Queue?", their messages and actions), the empty pages, the sign-in
methods, Add a Source and its five descriptions, Sources Settings…, Server
Address, From Your Sources, No Results, Reset to Flat, Volume Levelling and
ReplayGain, Open in {Service}, Playing on {Device}, Devices and Stop
Controlling; none of them says Preferences, Follow System, Background, Purple,
Scan again, Nothing found or Manage sources.

**macOS** (`platforms/macos/Jellyfesh/Resources/Localizable.xcstrings`): all fixed in the
parity pass (docs/macos.md), as listed here.
* Swatches are named "Hue 250" … "Hue 290" (`mac/Views/Settings/AppearanceSettings.swift:270`); ui.md's last accent is **Violet** and Linux, Android and the web name all ten (Blue, Cyan, Teal, Green, Olive, Orange, Red, Rose, Magenta, Violet).
* "Speakers" is the dock's output button value at System Default (`mac/Views/NowPlaying/QueuePanel.swift:400`); ui.md rules out "Speakers": say **System Default**.
* File › "Install Plugin…" (`mac/App/AppCommands.swift:62`; panel title "Install Plugin"): ui.md's word is **Install from File…**.
* "No folders yet" (`mac/Views/Settings/LibrarySettings.swift:16`): a title, so Title Case.
* Info's Lyrics row: "By word", "By line", "Not timed" (`mac/Views/NowPlaying/QueuePanel.swift:350`); the others say Word by word, Line by line, Unsynced.
* Tooltips "Sort albums", "Sort songs" and the label "Output device": Title Case (Sort Albums, Sort Songs, Output Device, as Windows, Linux and Android).

**Windows** (`win/Jellyfesh.App/Strings/en-US/Resources.resw`): all fixed in the parity pass, as
listed here.
* `Community_PluginTitle` "Turn on “{0}”?" (used in `win/Jellyfesh.Shared/ViewModels/SettingsViewModel.cs:906`): **Turn On “{0}”?** as in every other app.
* `Appearance_SwatchName` "Hue {0}" for the ten swatches (`SettingsViewModel.cs:253`): use the colour names, ending with Violet.
* `Lyrics_None` "No Lyrics for This Song": **No Lyrics** (the others; v0.0.7 uses it too).
* `SignIn_ScanQr` "Scan this code with the service's app on your phone." (others: "Scan the code with the {name} app on your phone."), `SignIn_Browser` "Finish signing in in your browser. Jellyfesh continues by itself when you are done." (others: "Continue in your browser. Come back here when you are done."), `SignIn_OpenAgain` "Open the Page Again" (others: "Open Browser Again"), `SignIn_Failed` "Signing in did not work: {0}" (Android: "Sign-in failed: %1$s"; macOS, Linux and the web show the message alone).
* `Sources_More` "Show More": **Load More** as the others.
* Unused or legacy: `Action_Clear` "Clear" (a word ui.md rules out; no code uses it), `Search_SourcesNotAvailable` and `Common_NotAvailable` ("Coming in a later build."), shown only when the core answers NotAvailable.

**Linux** (`platforms/linux/po/jellyfesh.pot`): all fixed in the parity pass, as
listed here.
* "Speakers" in "Output Device: {device}" at System Default (`linux/ui/outputs.rs:141`): **System Default**.
* The Rescan Library row's button is "Rescan" (`linux/ui/prefs.rs:1017`): **Rescan Library**.
* "Remove" for an Up Next entry (`linux/ui/rows.rs:290`): **Remove from Queue** (macOS, Windows).
* About's "Licence" row: "No licence has been chosen yet: all rights reserved." (`linux/ui/about.rs`).
* The import filter is "Playlists (M3U)" (`linux/ui/pages/playlists.rs:163`; it accepts .m3u and .m3u8); ui.md speaks of M3U8.

**Android** (`platforms/android/app/src/main/res/values/strings.xml`): all fixed in the parity
pass (docs/android.md), as listed here.
* `np_speakers` "Speakers" (the output chip at System Default, `and/ui/player/InfoAndDevices.kt:230`): **System Default** (`np_system_default` exists).
* `folders_empty` "No folders yet.": Title Case, no full stop.
* `action_remove` "Remove" for an Up Next entry: **Remove from Queue**.
* `unknown_title` "Unknown Song" (macOS "Unknown Title"); `search_also_in` "also in %1$s" (macOS and the web "Also in …").
* `source_enabled` "Enabled" doubles as a status line in Your Sources (`pluginStatus`); the others show nothing or "No sign-in needed" there.

**Web** (English keys in `web/`)
* "Third-party notices" (`web/views/Settings.svelte:548`): **Third-Party Licences** (Windows, Android; Title Case).
* "No folders yet" (`web/views/LibrarySettings.svelte:48`): Title Case.
* "Remove" for an Up Next entry (`web/nowplaying/QueuePane.svelte`): **Remove from Queue**.
* Your Sources: "Turned off" and "Ready" (`web/views/SourcesView.svelte`); the apps say "Off" and show nothing for a source that needs no sign-in.
* Settings › Sources: one "Sign In" for every method (N16).

## 4. Allowed differences, as built

Checked and kept as ui.md allows: the web's library (folders picked again each
visit, Pick Folder…, no Watch Folders, Remove Folder…'s shorter message, Open
Files…), no playlists, favourites or history on the web (Home shows each
signed-in source's start page), no Output Device on the web, Streaming Quality
and Language on the web, the web's keyboard shortcuts in Settings; Linux's App
Icon note; Android's Library tab, Allow Access to Music and Find Music on This
Phone; About in the macOS app menu and in Linux's main menu with Keyboard
Shortcuts; plugin settings that apply per field on macOS and Linux and with
Save on Windows, Android and the web.

Not in ui.md's Allowed differences but explained in docs/web.md: no remote
sources (N20), no Sign In in Browser or Paste Cookies (N13), near-gapless
playback and no ReplayGain for server streams (N27), Resume Playback that waits
for a click (N26). ui.md should list them, or the web should build them.

## 5. v0.0.7, not built

None of these exists in any app (no app has the words "Check for Updates",
"Licence Agreement", "Desktop Lyrics", "Get Info", "Create Synced Lyrics",
"Jellyfesh Pro", "Customize", "See All", "Relay Address" or "No Comments
Yet"). They are listed, not audited:

* **AI Lyrics** in the Lyrics pane (Create Synced Lyrics, Time These Lyrics, job stages, model download, API key, failures, "Made by AI Lyrics", Make Again, Delete Made Lyrics), the Sending Audio question, and Create Synced Lyrics in the song menu.
* **The Pro sheet**, Enter Licence Key and its errors.
* **The Look slice** (Customize): corners, glass, aurora, type, density, lyrics style, the order of the sidebar and Home, the Now Playing layout and the mini-player style; Settings › Appearance › **Customize…**.
* **Settings › Extensions**: Jellyfesh Pro, AI Lyrics (with On-Device Models and Sending Audio), Customize, other extensions, Install from File….
* **About**: Licence Agreement, Privacy Policy, Check for Updates, Check for Updates Automatically, "Jellyfesh {version} Is Available" and View Release.
* **Steps that ask first**: Delete Made Lyrics, Remove Licence…, Delete… (a speech model).
* **Discover** per source (shelves, See All, Browse, Try Again), and the library's Discover as Home.
* **Singer and album** pages for source items (Go to Album and Go to Artist for source songs). The web already opens source albums and artists from a song's menu (`web/lib/items.ts` `albumOf`, `artistOf`); the apps cannot yet, because the core's `SourceItem` has no album or artist id.
* **Get Info** (Version, Genre, Composer, Lyricist, Label; Audio: Quality, Format, Bitrate, Size, ReplayGain; File or Source) in the song menu, More menus and Now Playing's Info.
* **Comments** pane for sources that declare comments.
* **The lyrics bar** and **Desktop Lyrics**, with Show Lyrics Bar and Desktop Lyrics in Settings › Appearance › Lyrics.
* **Covers through the core** (ADR-0022 item 5; see N29).
* **Jellyfesh Web's relay** (Settings › Sources › Relay) and the "{Service} doesn't let browsers connect to it directly" message.
