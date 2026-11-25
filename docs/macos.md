# The macOS app (v0.0.8)

SwiftUI on macOS 14.2 and later, over the Rust core through the UniFFI bindings
([`ffi.md`](ffi.md)). The app holds no business logic (ADR-0001): it shows the
core's slices and sends commands; sorting, filtering, matching, persistence and
networking stay in the core.

**Status: written without a Swift toolchain.** CI's "macOS app" job on a
`macos-15` runner is where it compiles and its tests run. The names it assumes
from the generated bindings are listed below so they can be checked quickly.

## v0.0.8: Jellyfesh Web's layout

ADR-0023: the app keeps SwiftUI, its native window frame, title-bar controls, menu bar, Dock
menu, file panels, Now Playing info and accessibility, and draws Jellyfesh Web's structure with
them. What changed, and why:

* **The shell** (`Views/RootView.swift`: `AppShell`, `ContentColumn`; the rules in
  `Support/ShellLayout.swift`). The `NavigationSplitView` and its grouped sidebar (Library,
  Discover, Playlists) are gone: the web has one floating panel, and the system sidebar could not
  float on macOS 14 and 15. The window's width picks the layout at the web's breakpoints:
  * **From 761 pt:** the floating sidebar panel (`Views/SidebarPanel.swift`), 236 pt of Clear
    glass with r-2xl corners and shadow-1, 12 pt from the window's edges (Liquid Glass on macOS
    26; solid Cloud when transparency is reduced): the wordmark with the app icon chosen in
    Settings, Home, Albums, Artists, Songs, Playlists, Favourites and Downloads, a line, Search
    and Sources, and Open Files… and Settings at its foot, as the web's 42 pt rows (the chosen one
    in accent text on accent-soft). The scan's progress shows above the foot.
  * **Up to 760 pt:** the web's phone tabs (`Views/PhoneTabBar.swift`), Home, Library, Search,
    Sources and Settings in a floating Veil capsule 68 pt high. The Library tab returns to the
    library place shown last, and its pages start with pill tabs between Albums, Artists, Songs,
    Playlists, Favourites and Downloads (`LibrarySwitcher` in `PageHeader`). The window can be as
    narrow as 600 pt.
  * A customized look orders the places (without the line, as the web) and the tabs; Downloads,
    which the look cannot place yet, follows its library places (`SidebarItem.panelEntries`,
    `PhoneTab.tabs`).
  * The field fills the window under a hidden title bar (`.windowStyle(.hiddenTitleBar)`); the
    traffic lights stay the system's, above the panel.
* **Pages.** A page opened from another starts with **Back** (accent text, as on the web),
  which goes back a page as Esc does; the system's toolbar back button is hidden. The Albums and
  Songs sort menus sit at the end of the page's title instead of the toolbar, so the window has
  no toolbar.
* **Settings is a page** (`Views/Settings/SettingsView.swift`, `SettingsPage`), with the web's
  pill tabs over the section in a column as wide as the web's (`layout.reading`, 704 pt); the
  Settings window is gone. Appearance, Playback, Library, Downloads and About are drawn as the
  web's cards (`Views/Settings/SettingsCards.swift`: Mist cards with the heading inside, rows of a
  label, a line of help and the control at the end); Sources and Extensions are still grouped
  forms. The app menu's Settings… (⌘,) opens
  it at the section shown last; **About Jellyfesh** opens Settings › About
  (`Views/Settings/AboutSettings.swift`), which starts with Keyboard Shortcuts (the shared keys
  with ⌘, `AboutInfo.shortcuts`) and holds what the About window held. Menu items that show a
  page open the main window first, as it may be closed while music plays. The Pro sheet and
  Enter Licence Key show in the main window.
* **The player** floats over every page, Settings included, and is not drawn while Now Playing
  is open (it has its own dock); before, the bar stayed under Now Playing. The window's width
  picks the full bar (from 900 pt) or the capsule, as the web measures its viewport, not the
  content column. The full bar has the web's order: the artwork and lines, shuffle, previous,
  play, next and repeat over the seek bar, Lyrics, Queue, the device button (remote sources), the
  volume, and **More** (the song's menu and Desktop Lyrics). A bar narrower than 820 pt keeps the
  mute button and drops the volume line (`PlayerBarLayout`); so does Now Playing's dock in windows
  under 820 pt. The capsule holds the artwork, the lines, play and **next**.
* **The mini player** (ADR-0026 §4; `Stores/MiniPlayerStore.swift` behind `MiniPlayerHost`,
  `Platform/MiniPlayerWindow.swift`, `Views/Common/MiniPlayerView.swift`) is built as Desktop
  Lyrics is: a borderless non-activating `NSPanel` on every Space and beside full-screen apps, at
  the floating level while Keep on Top is on (normal otherwise), dragged by its background, its
  frame kept, first at the top right of the screen. It shows the cover, the title and the artist,
  the lyrics bar's line (Show Lyrics Line), previous, play and next, and the progress line, on the
  system's behind-window material with the Veil tint. The bar's mini player button and View ›
  Open Mini Player (⌥⌘M; ⌘M minimises, and the View menu comes before the Window menu's Minimize
  All) open it; its own button closes it. Closing the main window opens it while music plays when
  Open When the Window Is Closed is on. The in-window capsule, which the look calls the
  mini-player, is now `CapsulePlayer`.
* **Open Files…** (the panel's foot and File › Open Files…, ⌘O) plays audio files without adding
  them to the library. View › ⌘1–⌘8 follow the panel's order (⌘7 Downloads, ⌘8 Sources).
* **Covers that failed** (`Support/ArtworkLoader.swift`). A source picture the core could not get
  used to be refused for the rest of the run. It is now asked for again after a back-off (30
  seconds, doubling to 10 minutes), at once when the core later sends it, and whenever a source
  signs in or out; views showing a placeholder then load again (`ArtworkLoader.generation`).

## v0.0.8: the core's contract

The core's v0.0.8 contract (M1: da8e9f7, 7ee495f, 0565e2b) reached the app through the
hand-written adapters (`CoreClient/FFIConversions.swift`, `CoreModels.swift`; 948a2a6). What it
brought, and where:

* **The quality badge and menu** (ADR-0026 §3; `Support/QualityMenu.swift`,
  `Views/Common/QualityBadge.swift`, `PlaybackStore.qualityMenu(chosen:)`). A capsule in the
  player bar and in Now Playing's dock shows the core's label for what plays ("Hi-Res Lossless")
  or, before the format is known, the level chosen. It opens a native menu: the format line
  ("FLAC · 24-bit · 96 kHz", or "AAC · 256 kbit/s"), then **Streaming Quality** with Auto and the
  levels the source offers (`NowPlaying.qualities`), or, when the level isn't the app's to choose,
  the core's note ("Quality is set in Spotify.", "Files play as they are."; a remote session's
  `qualityNote`). The same setting is **Streaming** in Settings › Playback ("Auto plays lossless,
  or High on a metered connection.").
* **The network** (`Platform/NetworkMonitor.swift`). `NWPathMonitor` tells the core whether the
  Mac is online and whether the connection is metered (expensive or constrained), for Auto
  quality and downloads (`SetNetworkState`). It runs from launch to quit and costs nothing while
  nothing changes.
* **The mini player's settings** (Settings › Appearance › Mini Player): Keep on Top, Open When the
  Window Is Closed and Show Lyrics Line, kept by the core (`Settings.miniPlayer`).
* **Search by source** (ADR-0024 §4; `Stores/SearchStore.swift`, `Views/Search/SearchView.swift`).
  Tabs All, Library and one per source that answered. All shows the library's results, then
  **From Your Sources** (the sources that allow merging), Apple Music, and a link to each
  separate source's tab ("Results from Spotify"). A source's tab shows its Songs, Albums, Artists
  and Playlists, each with **Show More** (`SearchMore`, answered by `Event.searchPage`).
* **The sources' likes and playlists** (ADR-0024 §3; `Stores/SourceLibraryStore.swift`,
  `Views/Library/SourceLibraryViews.swift`). Favourites and Playlists add one section per
  signed-in source after the library's own: its name, its credit where its terms ask for one, when
  the cached lists were updated ("Updated 5 minutes ago"), then songs as rows that play, albums as
  cards and artists and playlists as rows that open their pages, each list with Show More. The
  lists refresh when either page opens, at most every ten minutes. A source playlist has **Save as
  Playlist…** in its More menu, which asks for the name and opens the new playlist when the core
  has made it; a playlist whose songs its service doesn't list says so and still plays there.
  Source collections carry their kind (`Collection.source(plugin, id, itemKind)`).
* **Downloads** (ADR-0025; `Stores/DownloadsStore.swift`):
  * **The Downloads page** (`Views/Library/DownloadsView.swift`): Play and Shuffle, the songs
    and the space used at the top, and why downloads wait ("Downloads resume when you're back
    online.", or off a metered connection); In Progress, each download with "Waiting", its
    progress and percentage, or why it failed with Try Again, and Cancel; then the downloaded songs
    by album. Each album and song has
    **Remove Download…**, which asks first. A downloaded song plays its library track, or the
    source's song, which plays the file, offline too. The rows sit in lazy stacks.
  * **Settings › Downloads** (`Views/Settings/DownloadsSettings.swift`), between Library and
    Sources: Folder with Show in Finder and Change…, Quality (Original, High, Normal, Low), File
    Names with its fields and a live preview from the core's rules (`downloadPreview`), the Tags
    switches, Network (**Pause on Metered Connections**, on, the core's `wifiOnly`: the app reports
    a hotspot or Low Data Mode as metered, so without the switch such downloads would never start;
    and Downloads at Once, 1 to 4), and Space Used with **Remove All Downloads…** (asked first).
  * **The folder.** The core is told `~/Music/Jellyfesh` (`CoreConfig.downloadsDir`; the real
    Music folder, not the sandbox container's). The App Store build may now write to Music
    (`com.apple.security.assets.music.read-write` instead of read-only). A folder chosen with
    Change… is kept there with a security-scoped bookmark under its own key
    (`FolderAccess.downloadsKey`), held from launch before the core downloads into it; one that
    moved is passed to the core at its new path.
  * **Download** is in the menus of the albums, playlists and songs of a source that offers
    downloads (`PluginSummary.download`): a source page's More, a card's or row's menu, and the
    song menu for the songs not downloaded yet. A downloaded source song shows the Downloaded
    mark in its row (`SourceItem.downloaded`, `TrackRow.downloaded`).
* **Shuffle** sends the collection with `start` 0 and `shuffle`, which the core reads as no song
  chosen, so any song can come first.

## v0.0.7: Pro, AI Lyrics, Customize, Discover and the lyrics bar

Built on the core's v0.0.7 contract ([`core-api.md`](core-api.md)); the app
decides nothing the core decides.

* **Distribution.** `JF_DISTRIBUTION` reaches the app as the Info.plist key
  `JFDistribution` and the core as `CoreConfig.distribution`
  (`Support/Distribution.swift`). Builds that may not install plugins (the Mac
  App Store's, `SourcesSnapshot.canInstallPlugins` false) show neither Install
  from File… (Settings and the File menu) nor Allow Community Plugins. The App
  Store build is sandboxed: a music folder chosen in the open panel gets a
  security-scoped bookmark, resolved and held open at launch before the core
  opens the library; a folder that moved is passed to the core at its new path
  (`Platform/FolderAccess.swift`, behind `FolderBookmarks`).
* **Covers through the core.** Every source picture goes through
  `source_artwork(url, size_px)`: a path is loaded, `nil` waits for
  `Event::Artwork` (at most a minute), a picture that cannot be had stops being
  asked for (`Support/ArtworkLoader.swift`).
* **Songs** plays and shuffles `Collection::Songs` in the page's order.
* **Lyrics, word by word.** `LyricsTimeline.bar(atMs:)` ports the core's timing
  helper and replays `tests/contract/lyrics-bar.json` within 1e-6. The player
  bar shows the line being sung under the title (Show Lyrics Bar), its words
  filling in the highlight colour, with its translation when Show Translations
  is on; Now Playing's sweep takes each word's progress from the same helper.
  **Desktop Lyrics** is a borderless non-activating `NSPanel` at the floating
  level, on every Space and beside full-screen apps: the line being sung and
  the next one, Lock and Close on hover; locked, it ignores mouse events and
  View › Unlock Desktop Lyrics (or Now Playing's More menu) unlocks it
  (`Platform/DesktopLyricsWindow.swift`, `Stores/DesktopLyricsStore.swift`
  behind `DesktopLyricsHost`). The panel keeps its frame; the lock is kept in
  the defaults.
* **The look (ADR-0020).** `Design/Look.swift` turns the `Look` slice into
  `\.look`: radii × the corner scale (artwork, floating glass, surfaces,
  rows), the glass scaling the Transparency setting (a look without glass is
  solid), the aurora's strength, the typeface (SF, SF Rounded, New York, SF
  Mono) and text scale for type roles and the main window's text, density for
  page margins, grids and rows, the lyrics' weight, glow and highlight, the
  sidebar's places and Home's sections in order (Home gains Favourites), Now
  Playing's Stage, Artwork and Lyrics layouts, and the mini-player as Bar,
  Compact or Floating. While the look is not customized every value is
  Luminous's, so the app draws as in v0.0.6.
* **Settings › Extensions.** Jellyfesh Pro (where it comes from; Buy Jellyfesh
  Pro – {price} and Restore Purchases in the App Store build; Enter Licence
  Key… where keys work; Remove Licence… asked first; the Licence Agreement,
  Privacy Policy and Terms of Sale), then each extension with its switch,
  account ("API Key Added"), settings and version. AI Lyrics adds its
  On-Device Models (Download, Cancel, Delete… asked first) and Sending Audio
  (Stop Allowing); appearance plugins add Reset to Defaults. Plugin settings
  come from the core's localised schema in the app's language and render
  schema profile 1.2: sliders, colour wells, ordered checklists (with Move Up
  and Move Down) and `x-jf-group` headings (`Support/SchemaForm.swift`,
  `Views/Settings/PluginSettingsForm.swift`). Appearance ends with Customize….
* **Jellyfesh Pro (ADR-0021).** Any Pro action while Pro is inactive opens the
  Pro sheet in the window that asked; Enter Licence Key shows the core's
  reason for a refused key. In the App Store build StoreKit 2 loads and buys
  `org.jellyfesh.pro`, restores with `AppStore.sync()`, and reports verified
  transactions from `Transaction.currentEntitlements` and
  `Transaction.updates` with `setStoreEntitlement`
  (`Platform/StoreKitPurchases.swift`, behind `ProPurchasing`).
* **AI Lyrics (ADR-0019).** The Lyrics pane offers Create Synced Lyrics (Time
  These Lyrics under plain lyrics) and what comes first: the model to
  download, the API key to add, or the reason. A job shows its stage, a
  progress bar and Cancel; a failed one says why with Try Again. Made lyrics
  end with "Made by AI Lyrics · {engine}"; Now Playing's More menu has Make
  Again and Delete Made Lyrics (asked first); the song menu has Create Synced
  Lyrics for library songs; audio leaves only after "Send This Song to
  {origin}?" is allowed (`Views/NowPlaying/LyricsMaking.swift`).
* **Discover.** Choosing a source opens its Discover page: the core's shelves
  as rows, grids or numbered charts, See All, Browse in the header, grey
  shelves while loading and Try Again after a failure
  (`Views/Sources/DiscoverView.swift`). The library's Discover is Home.
* **A source's albums and artists.** Go to Album and Go to Artist for a
  source's song open the source's pages (Browse with the item, Play and
  Shuffle through `Collection::Source`), with the source's picture and its
  notes or biography from `Details` where the plugin describes items.
* **Get Info** from the song menu and the album and artist More menus: About
  This Song and Audio (Quality, Format, Bitrate, Size, ReplayGain, File or
  Source), or an album's or artist's picture, notes and facts
  (`Support/DetailsRows.swift`, `Views/Common/GetInfoSheet.swift`). Now
  Playing's Info pane shows the same rows with the format being decoded.
* **Comments.** For sources that show comments, Now Playing has a Comments
  pane: "Comments from {Service}", Popular, then Latest a page at a time.
* **About** adds the Licence Agreement and Privacy Policy, and in direct
  downloads Check for Updates, Check for Updates Automatically and
  "Jellyfesh {version} Is Available" with View Release.

## The parity pass (v0.0.7, on the v0.0.6 core API)

What [`parity.md`](parity.md) listed for macOS that needed no new core API.
Written without a Swift toolchain, like everything here.

* **Sign-in steps** (N9, N11). `SignInStep.showCode` is a step of its own
  (Jellyfin's Quick Connect): the code large and selectable (VoiceOver spells
  it out), the plugin's words for where to enter it and "Waiting for
  approval…". A QR code keeps the plugin's own sentence (`showQR`'s `hint`),
  else "Scan the code with the {name} app on your phone.". `SignInField`
  carries `kind`, `required` and `help`: the help and "Optional" show under
  the label, Sign In waits for the required fields, a number field keeps its
  digits (full-width ones as ASCII), a pasted Cookie header gets a field of
  several lines, and each field names what the system may fill in (user names,
  passwords but not keys or tokens, email and web addresses, one-time codes).
  The rules are `Support/SignInForm.swift`.
* **About Jellyfesh** (N28) is a window of its own (`Views/AboutView.swift`, a
  `Window` scene without a Window menu item), opened from the app menu: the
  icon and name, "Version {version}" from `CFBundleShortVersionString` (the
  core's version after it only when it differs, as on Android), what Jellyfesh is,
  **Website** and **Third-Party Licences** when the build knows the website, and
  the design credits. The address is the build setting `JF_WEBSITE` (Info.plist
  key `JFWebsite`); `build.sh` takes `JF_WEBSITE`, else in GitHub Actions
  `https://OWNER.github.io/NAME/` from `GITHUB_REPOSITORY` (where web.yml
  publishes the website), and the licences link is its
  `THIRD_PARTY_NOTICES.html`. Without an address About shows the version only.
  The rules are `Support/AboutInfo.swift`.
* **A source's albums, playlists and artists** (N18) open a page like the
  web's and the library's: `BrowseView` draws the shared `CollectionHeader`
  (artwork, kind, title, artist, the song count once every page is in, a
  "Content from {Service}" credit), **Play** and **Shuffle** (the core plays
  the whole container, `Collection::Source`), and More (Play Next, Play Last
  and Add to Playlist… unless the source plays remotely, then Open in
  {Service}); a container nothing of which plays here offers Open in {Service}
  as its main button. An album's songs are numbered, the songs end with "12
  songs, 48 minutes", and loading, a failed page and an empty one show under
  the header. `Route.browse` carries the item it was opened from; a source's
  top level says its badge and account under its name. The library's album,
  playlist and artist pages use the same header (240 pt artwork; a round place
  for an artist's picture, which the library does not have).
* **Search** names the other sources that have a merged item ("Also in …") on
  song rows and album cards too, not only on artist and playlist rows. A blank
  `attribution` credits nothing.
* **Words** (parity.md section 3 and V6, V10, V18, V21): the accent swatches
  are Blue … Violet; the device button at the system default says System
  Default; File › Install from File…; Info's Lyrics row says Word by word, Line
  by line or Unsynced; No Folders Yet, Sort Albums, Sort Songs and Output
  Device in Title Case; "Try other words."; Info with nothing playing says
  Nothing Playing, "Pick a song, an album or a playlist."; the Lyrics pane
  without lyrics, Now Playing Backdrop and Show Translations use the words most
  apps use; choosing Year or Recently Added turns Descending on.

## v0.0.6: links, remote playback, Luminous and Apple Music

Written without a Swift toolchain, like everything here: the first CI build is
its first compile.

* **Links** (ADR-0018). `SourceResult.link` and `NowPlayingInfo.link` carry the
  core's `link`; Open in {Service} (`OpenInServiceButton`, `NSWorkspace.open`,
  `https` only) sits after Go to Artist in the song menu, in the browse page's
  More menu, in source cards' and rows' menus, in Now Playing's More and under
  About This Song. An item that cannot play here but has a link opens it as its
  main action (double-click, Return, VoiceOver), next to its reason.
* **Remote playback.** `RemoteSession` mirrors the `Remote` slice. While it has
  a source, `PlaybackStore.playing` is the session's track: the player bar, the
  mini player, Now Playing and the system's Now Playing show it with "Playing on
  {Device}" (its error in a quieter line), progress comes from the session's
  anchor (a second `PlaybackClock`), and shuffle, repeat, seek and the transport
  stay the usual commands. The device button opens Devices (it sends
  `Remote { Refresh }`; kinds as SF Symbols, the playing device checked;
  choosing sends `Transfer`); the volume sets the device's (`SetVolume`) and
  hides without one; More holds Open in {Service} and Stop Controlling (`End`).
  Lyrics say "Lyrics aren't shown while {Service} plays.", the queue "Up Next is
  in {Service}."; a session that ends with an error says so once (by slice
  version, so batching cannot hide it). Remote sources' songs offer no Play
  Next, Play Last, Add to Playlist or favourites.
* **Luminous** (docs/ui.md). The field has four larger glows at
  `field.aurora-strength` (`LuminousField`); grouped surfaces are the Mist tint
  without live blur plus shadow-1 (`surface(in:)`); the player bar is a floating
  Veil capsule (r-2xl) and the pages fade out just above the player
  (`PlayerFadeMask`, replacing the edge blur); Now Playing is a stage: the
  backdrop, the artwork up to 380 pt on shadow-3, lyrics up to 40 pt at line
  height 1.18 with inactive lines at 92 % and a 40 ms cascade, and one floating
  Veil dock with every control. Page titles are `text-h1` (48 pt); album covers
  have radius 16 on shadow-2 and lift 4 pt onto shadow-3 on hover or focus. The
  sidebar keeps the system sidebar material and Settings its native forms.
* **Apple Music** (ADR-0010, ADR-0018 item 5). `AppleMusicPlayer` is the app's
  `AppPlayer`, bridged to the core's `ExternalPlayer` by `FFIExternalPlayer` and
  registered as `apple-music` ("Apple Music"): one device, This Mac, without a
  volume; tracks, albums (numeric ids) and playlists (`pl.` ids) play through
  `ApplicationMusicPlayer`. The core calls it on its session thread; each call
  hops to the main actor and waits on a semaphore (never on the main thread,
  8 s, 15 s to start playing). Apple Music is offered in Sources › Add a Source
  only when the code signature has `com.apple.application-identifier` (read with
  `SecTaskCopyValueForEntitlement`, without MusicKit), turned on only after the
  person agrees, and shown only once MusicKit authorises the app, a developer
  token can be fetched and the subscription can play the catalogue; otherwise it
  is hidden (ad-hoc builds never offer it). Its page and its own group on the
  Search page search the catalogue (songs, albums); results stay in memory.
  `NSAppleMusicUsageDescription` is in `Info.plist`.

## v0.0.4 design pass

The controls and motion of [`design-controls.md`](design-controls.md),
"v0.0.4: the controls as built" (after Apple Music and Media Player, in
Misty's tokens):

* **Seek bar and volume** (`Views/Common/SeekBar.swift`, `VolumeSlider.swift`):
  a straight line with round caps instead of style D's wave, ink-90 over
  ink-30, elapsed and remaining (−2:28) times. Hovering or focusing thickens
  it from 4 to 6 pt and shows the 14 pt ringed thumb (ink-90, glass-edge ring,
  shadow-1; Liquid Glass on macOS 26 in the Artwork style); dragging thickens
  it to 8 pt and the times follow the thumb. Left/Right seek 5 s, Page Up and
  Page Down 30 s, Home and End go to the ends; VoiceOver reads "1:23 of 3:45".
  It still redraws only per pixel or per second, never while paused or hidden.
* **Mini player and player bar** (`MiniPlayer.swift`, `PlayerBar.swift`): the
  detail column measures its width. Under 900 pt the floating capsule
  (`player.mini-radius` corners, one play button) has a 2 pt progress line
  along its bottom edge, inset by its corners, instead of the ring; from
  900 pt the 80 pt player bar shows the artwork and two lines (they open Now
  Playing), shuffle, previous, play, next and repeat above the seek bar, and
  Lyrics, Queue and the volume. Both are Liquid Glass on macOS 26
  (`FloatingGlass`); under Now Playing they stop redrawing.
* **Now Playing:** the artwork scales to 0.88 while paused and springs back
  (dur-3 Spring); play and pause morph (an SF Symbol replace); Now Playing
  rises from the bottom of the window with a slide and fade (the split view's
  columns are separate hosting views, so the artwork cannot grow out of the
  mini player's) and sinks back on Exit. Lyrics take the Lyrics size and
  alignment settings; sung lines are ink-30, inactive lines 96 % from their
  start edge (their centre when centred), and the lines below the active one
  follow the scroll 30 ms apart: a `keyframeAnimator` per line plays the lag
  of `NowPlayingMotion.cascadeKeyframes`, with the scroll distance estimated
  from the line sizes. Tapping a line, or VoiceOver's default action, seeks
  to it.
* **Library:** large titles (`text-h2`, 30 pt). Album covers lift 2 pt onto
  shadow-2 with a 36 pt Mist play button in the bottom start corner and press
  to 97 %. The album page has 240 pt artwork, the title at 30 pt, the artist
  in accent text (it opens the library's artist of that name), genre, year
  and count, Play (accent) and Shuffle (glass) pills, track numbers that turn
  into play buttons on hover, a three-bar level glyph on the playing row (a
  `PhaseAnimator` that runs only while playing and visible) and a "12 songs,
  48 minutes" footer. Rows take the hover `fill`; the playing row is
  accent-soft with accent text.
* **Motion:** pages fade in and rise 8 pt (dur-2 Fluid), a page you come back
  to fades in (dur-1), theme colours crossfade (dur-4), Now Playing opens on
  `player.open-now-playing` and closes on Exit. Under reduced motion (the
  system's Reduce Motion or the app's Reduce Motion and Transparency) all of
  it becomes a short fade: the artwork keeps its size, the level glyph stands
  still and lyrics jump. Sheets and popovers keep the system's animations.
* **Preferences → Appearance:** Theme (System, Light and Dark as small window
  pictures), Follow the Artwork, ten swatches (the ring slides with
  `matchedGeometryEffect`) and a Custom hue on a hue-gradient track with a
  preview, Transparency from Solid to Clear with a live preview, Reduce
  Motion and Transparency, the Now Playing background (pictures), Lyrics size
  and alignment, translations and romanization (the Lyrics tab moved here),
  and the app icon. Every change is one `SetSettings`; the sliders send
  theirs when let go.
* **Transparency on materials:** SwiftUI materials have no alpha, so a grade
  moves one material thicker below 25 % and one thinner from 75 %
  (`GlassLook`) and its tint takes the alpha of the shared glass-alpha port
  (`GlassTransparency` over `platforms/shared/swift`); at 0 % the glass is
  solid Cloud.
* **The numbers** live in `Support/DesignMath.swift` (seek geometry, Now
  Playing motion, the swatches and an OKLCH port, the transparency look) and
  are tested in `Tests/DesignTests.swift`.

**Not compiled.** There is still no Swift toolchain where this was written:
the new views (macOS 14 APIs: `onKeyPress`, `keyframeAnimator`,
`PhaseAnimator`, `contentTransition(.symbolEffect(.replace))`,
`matchedGeometryEffect`), `DesignTests.swift` and the shared
`GlassContractTests.swift` have never been built or run, and the first macOS
CI build is their first compile. A Python mirror of the OKLCH port reproduces
all 16 OKLCH colours of `design/tokens.json`; the string catalog was checked
for a key behind every new `Text`.

## Build

Needs macOS with Xcode 16 or later, rustup (or both Rust targets), CMake (the
core builds libopus with it) and XcodeGen (installed with Homebrew when
missing).

```bash
plugins/release.sh                      # optional: the official plugins in dist/plugins/
platforms/macos/scripts/build.sh        # universal Release app and DMG
platforms/macos/scripts/build.sh --test # the same, after the unit tests
platforms/macos/scripts/build.sh --skip-core   # reuse Generated/ when only Swift changed
```

`build.sh` runs, in order:

1. `cargo xtask bindings --lang swift --release` and `cargo xtask tokens --target swift`
   (writes `platforms/macos/Generated/`, git-ignored);
2. `jf-ffi` as a static library for `aarch64-apple-darwin` and `x86_64-apple-darwin`,
   joined with `lipo` into `Generated/JellyfeshCore.xcframework`; the C half of the
   bindings goes to `Generated/include/` as a Clang module (`JellyfeshFFIFFI`);
   the system libraries `rustc --print native-static-libs` reports are passed to the
   linker (`JF_RUST_LDFLAGS`; the default in `project.yml`, for builds made in Xcode,
   includes Accelerate for whisper.cpp). whisper.cpp, which jf-ffi builds through
   whisper-rs-sys and CMake, is built with `GGML_NATIVE=OFF`, and the x86_64 slice with
   `GGML_AVX`, `GGML_AVX2`, `GGML_FMA` and `GGML_F16C` (every Intel Mac that runs macOS
   14.2 has them);
3. the official plugins `dist/plugins/*.jfpkg` (or `$JF_PLUGINS_DIR`) copied to
   `Generated/Plugins/`, which the app carries as `Resources/Plugins/`; without
   them it says so and builds without bundled plugins;
4. `xcodegen generate` from `project.yml`;
5. `xcodebuild` of an unsigned (ad-hoc signed, ADR-0016) universal Release app;
6. `build/Jellyfesh-<version>.dmg` with a link to `/Applications`, and its SHA-256.

Output goes to `platforms/macos/build/` (`JF_MACOS_OUT` to change it).
`CARGO_TARGET_DIR` is honoured. `JF_WEBSITE` names the website About links to
(in GitHub Actions it defaults to the repository's GitHub Pages address).

### In CI

`ci.yml` runs `build.sh --test`. The release workflow's `macos-app` job runs it
with `JF_EDITION=commercial` and `JF_DISTRIBUTION=direct`; with the Apple secrets
it signs the app and disk image with the Developer ID (`scripts/keychain.sh`),
notarizes and staples the disk image (`scripts/notarize.sh`) and builds the
sandboxed Mac App Store app and its installer package (`JF_DISTRIBUTION=app-store`,
`scripts/package-mas.sh`). The release carries `Jellyfesh-<version>-macOS.dmg`;
the package stays on the run's page (docs/github.md, docs/store-release.md).

### In Xcode

Run `build.sh` once (it makes `Generated/`), then `xcodegen generate` in
`platforms/macos` and open `Jellyfesh.xcodeproj`. The scheme `Jellyfesh` runs
the app and the unit tests.

### First launch

Without the Developer ID and notary secrets the app is ad-hoc signed and not
notarised (ADR-0016): macOS blocks the first launch; allow it in System Settings →
Privacy & Security → Open Anyway. The direct download's app
is not sandboxed and uses the hardened runtime with JIT allowed (the plugin
host compiles WebAssembly).

## Layout

```text
platforms/macos/
├── project.yml                 XcodeGen: JellyfeshFFI (static framework: bindings + shared ports),
│                               Jellyfesh (the app), JellyfeshTests
├── scripts/build.sh            the build above
├── scripts/make-icons.py       renders the app icon and the ten options from design/icons/round4
├── scripts/make-fallback-theme.py   the hue-250 theme for the first frames
├── Jellyfesh/
│   ├── Info.plist, Jellyfesh.entitlements
│   ├── Resources/Assets.xcassets   AppIcon (Ice), DockIcon-* (Steel and eight more), IconThumb-* (picker)
│   ├── Resources/Localizable.xcstrings   English, Simplified and Traditional Chinese, Japanese
│   └── Sources/
│       ├── App/            JellyfeshApp (scenes), AppModel (wiring, event dispatch),
│       │                   AppDelegate (launch, quit, open files, Dock menu), AppCommands (menus)
│       ├── CoreClient/     the only code that imports JellyfeshFFI: CoreClient protocol, the app's
│       │                   own models, FFI conversions, FFICoreClient, OfflineCoreClient, PlaybackClock
│       ├── Stores/         @Observable stores per slice: playback, queue, library, settings, theme,
│       │                   lyrics, search, sources (+ browse, sign-in), paged lists, router, toasts
│       ├── Design/         palette, type and motion from the tokens, glass and the
│       │                   transparency setting, Misty field, controls, TimelineView schedules
│       ├── Platform/       media session, Dock icon, device watcher, Low Power Mode and window
│       │                   visibility, the shared keys (Space, Shift-arrows, Esc)
│       ├── Support/        artwork loader, JSON with ordered keys, schema forms, sign-in forms,
│       │                   About's links, lyric layout, QR codes, file panels, design math
│       │                   (seek bar, motion, swatches)
│       └── Views/          root (the shell), sidebar panel, phone tabs, library pages, Now
│                           Playing, search, sources, the Settings page and its sections
├── Tests/                  XCTest on a fake CoreClient (+ ../shared/swift/Tests in the same target)
└── Generated/              git-ignored, made by build.sh
```

## What is where

* **Core access.** `CoreClient` is the seam: `FFICoreClient` runs queries on a
  serial background queue, converts events on the core's event thread and hands
  them to the main thread in batches, dropping slices a later one replaces.
  Every generated type is converted in `CoreClient/FFIConversions.swift`, so a
  renamed generated name is fixed in that folder alone. Source commands that
  answer "not available in this build yet" show a calm notice, not an error.
* **Drawing time.** Every anchor feeds `PlaybackClock` (the shared
  `DisplayClock` port). Time-driven views use `TimelineView` with schedules that
  wake only when what they show changes: the next second of a label, the next
  pixel of a bar (at most ten a second), the next lyric line or word
  (`LyricTimeline.nextChange`). Wake-ups take the later of the drawn position's
  and the anchor's estimate, so a slew never wakes a view early. The word sweep
  runs `TimelineView(.animation)` only while a word is being sung, on the active
  line only. Paused, or with the window minimised or covered, nothing is
  scheduled (Still mode).
* **Window.** Jellyfesh Web's layout (see v0.0.8 above): the floating sidebar panel from 761
  pt wide, the phone tabs up to 760 pt; Home, Albums (lazy grid), Artists, Songs, Playlists,
  Favourites, Downloads, Search, Sources and Settings; album, artist, playlist and browse pages
  pushed on them, each starting with Back. The capsule (windows under 900 pt) or the player bar
  (900 pt and wider) floats over every page, which fades out just above it; Now Playing opens over
  the window (Esc or Library closes it) and hides the player. Esc also goes back a page
  (`SharedKeyMonitor`, main window only, not while text is edited or a sheet is open).
* **Shared UI ([`ui.md`](ui.md)).** The pages, words, settings order and
  confirmations follow the shared spec in the Mac's own mechanics:
  * Home shows the library size, Recently Added (albums), Recently Played and
    Most Played; with an empty library and no source signed in, the welcome
    with Add Music Folder… and Add a Source (the Sources page).
  * Empty Albums, Artists and Songs pages offer Add Music Folder…, Playlists
    New Playlist….
  * Songs has Play and Shuffle: the core has no collection for the whole
    library, so `SongsModel.playAll` reads every song in the page's order (a
    thousand at a time) and plays them as a track list (the core shuffles).
  * Album, artist and playlist pages, the library's and a source's, share one
    header (`CollectionHeader`: artwork, kind, title, artist, facts, Play,
    Shuffle) and have a More menu (Play Next, Play Last, Add to Playlist…; a
    playlist adds Rename…, Export Playlist… and Delete Playlist…). Add to
    Playlist… is a submenu (New Playlist… and the playlists); for a page the
    store reads the collection's tracks first (`LibraryStore.tracks(of:)`), as
    the core adds tracks, not collections; a source's page adds the songs it
    has loaded, once every page is in.
  * New Playlist… (⌘N, the sidebar's +, Playlists, Add to Playlist…) asks for
    the name in an alert before anything is created (`LibraryStore.newPlaylist`).
  * The song menu is Play Next, Play Last, Add to Playlist…, Add to or Remove
    from Favourites, Download (a source's songs, where it offers downloads), Go
    to Album, Go to Artist and Show in Finder; rows that do not carry a library
    song's album or file look them up when chosen, and Go to Artist opens the
    library's artist of that name.
  * The Sources page: what sources are, Add a Source (the core's services, the
    same rows and Connect sheet as Settings), Your Sources (status; choosing one
    browses it) and Sources Settings…. A sign-in sheet opens in the window its
    flow started in (`SignInWindow`) and draws each step as the core and the
    plugin describe it (`SignInForm`).
  * About Jellyfesh (the app menu, ui.md's allowed difference) opens Settings › About: Keyboard
    Shortcuts, the version, the website and the third-party licences.
* **Now Playing.** Artwork style (the core's baked backdrop, crossfading on
  dur-4, the scrubber thumb in Liquid Glass on macOS 26) or Misty style (the
  field at the artwork hue, the artwork on a Clear frame). Seek bar, transport,
  shuffle and repeat, volume, favourite, output device (System Default or the device), lyrics
  (word sweep, translation and romanization lines, 38 % anchor, manual scroll
  pauses follow mode for 4 s, tap a line to seek), queue (Up Next: drag to
  reorder, Delete to remove, double-click to jump, pages past 50 entries;
  Clear Queue asks first; "Nothing Up Next" when empty), info (About This
  Song: title, artist, album, year, length, format, lyrics and the file).
* **Search.** One `Search` command for the library and every source; results
  appear per source as they arrive, under the tabs All, Library and one per
  source: All shows the library's first, then From Your Sources (one group per
  source that allows merging) and links to the separate sources' tabs; a
  source's tab has Show More for each kind. Library artwork comes from one
  `searchLibrary` query; if the command fails, the library query alone fills
  the page.
* **Settings.** A page of the main window, with pill tabs, in the order of `ui.md`.
  Appearance (theme, accent colour, transparency, Reduce Motion and Transparency, Artwork or
  Misty, lyrics size, alignment, translations and romanization, the Mini Player group, the app
  icon picker), Playback (output device; crossfade; Volume Levelling: ReplayGain and its preamp;
  equalizer with the Preset picker from the core's `eq_presets()`, showing
  Custom once a band moves, the preamp, the bands and Reset to Flat; Streaming; Startup:
  Resume Playback), Library (folders with Show in Finder, Remove Folder… and
  Add Folder…; Watch Folders; Rescan Library with the scan's progress and Last
  Scan), Downloads (folder, quality, file names, tags, Pause on Metered Connections, downloads at
  once, space used and Remove All Downloads…), Sources (Add a Source; Installed with Install from File… in its
  heading: enable, sign-in by QR code, browser or fields, settings forms from
  each plugin's JSON Schema, "Version · Author", Uninstall…; Community Plugins
  with the ADR-0012 disclosure), Extensions, and About (Keyboard Shortcuts, the version, the
  links, Check for Updates in direct downloads, the credits).
* **App icon (ADR-0017).** The bundle icon is Ice. Automatic shows Steel in the
  Dock while the system is dark; any other choice replaces the Dock icon while
  the app runs (`NSApplication.applicationIconImage`). Finder keeps Ice.
* **Media session.** `MPNowPlayingInfoCenter` is set on slice changes (the
  system extrapolates from the rate), with `playbackState` on every start and
  stop; `MPRemoteCommandCenter` handles play, pause, toggle, stop, next,
  previous, position, repeat and shuffle.
* **Menus.** The app menu (About Jellyfesh and Settings… ⌘,, which open those Settings
  pages), Controls (Space plays and pauses outside text fields; ⌘→ ⌘← next
  and previous; ⇧→ ⇧← skip 5 s outside text fields, and ⌥⌘→ ⌥⌘← from the menu;
  ⌘↑ ⌘↓ volume; ⌥⌘↓ mute; ⌘J current song), View (⌘1–⌘8 places, ⌘F search,
  ⌘L Now Playing, ⌥⌘L lyrics, ⌥⌘U queue), File (⌘O Open Files…, ⌘N New Playlist…, ⇧⌘O Add
  Music Folder…, Import Playlist…, Install from File…). These carry the shared keys of
  `ui.md`; Space, ⇧→ ⇧← and Esc (back) go through `SharedKeyMonitor` rather
  than key equivalents, which would take them from text fields.
* **Glass (ADR-0007).** Clear, Mist and Veil on the ultra-thin, thin and
  regular materials with the grade's tint, moved by the Transparency setting;
  solid Cloud under Reduce Transparency, Increase Contrast, Low Power Mode,
  Reduce Motion and Transparency or a Transparency of 0 %. macOS 26
  APIs are compiled only with an SDK that has them (`#if compiler(>=6.2)`) and
  used behind `#available(macOS 26.0, *)`.
* **Accessibility.** Every control has a label; lyric lines form a list with
  the current one selected; the seek bar (arrows, Page Up and Down, Home,
  End) and volume are adjustable from the keyboard and VoiceOver; Reduce
  Motion stops the sweep, the crossfade and animated scrolling, and reduced
  motion turns movement into short fades.
* **Bundled plugins.** The core installs the packages in `Resources/Plugins` on
  first start and updates them with the app (`CoreConfig.bundledPlugins`).
  That field arrives with the sources work in the core: `build.sh` defines
  `JF_BUNDLED_PLUGINS` once the generated bindings have it, so the app builds
  before and after.

## Tests

`build.sh --test` (or the scheme's Test action) runs the XCTest target:
playback, settings (with the equalizer presets), library (New Playlist… naming,
collections into playlists), the Songs page's Play and Shuffle, the router (Esc),
paging, search (From Your Sources), sources (which window shows a sign-in, the
code and QR steps, credits and "Also in …"), browse (song numbers, when every
page is in, when a container plays) and queue stores on `FakeCoreClient`; the
sort menus' starting direction; JSON, schema forms, sign-in forms, About's links
and version, lyric layout, schedules, event batching, the Dock icon and
formatting; the swatch names; v0.0.7's stores and rules (Pro and its App Store side,
licence keys, AI Lyrics' steps, updates, answers by request, Discover, comments,
covers through the core, Get Info's rows, schema profile 1.2, extensions and
localised schemas, the look and its layouts, Desktop Lyrics' store and window
frame, folder bookmarks, the build flags, and `tests/contract/lyrics-bar.json`);
v0.0.8's layout (`LayoutTests.swift`: the web's breakpoints, the panel's places and the phone
tabs for a look, the Library tab's memory, the player over every page but Now Playing, the narrow
bar, the wordmark's icon, Settings and About as pages, the shared keys), the mini player's store
and first frame (`MiniPlayerTests.swift`) and the covers' back-off; v0.0.8's contract
(`V008Tests.swift`: the network state, the quality badge and menu, the sources' likes and
playlists with their refresh and Save as Playlist…, the downloads store's lists, words and
commands, Download for albums and playlists, the Downloaded mark), the search tabs and Show More,
the downloads folder's bookmark (`FolderAccessTests.swift`) and v0.0.8's commands converted;
the v0.0.4 design math
(`DesignTests.swift`: seek geometry, Now Playing motion and the cascade, swatches
and the OKLCH port against `design/tokens.json`, the transparency look, the new
settings); and the shared contract tests (`platforms/shared/swift/Tests`, which replay
`tests/contract/lyrics-cursor.json`, `display-clock.json` and
`glass-alpha.json`).

## Names assumed from the generated bindings

All of these were checked against `Generated/JellyfeshFFI.swift` generated from
this revision (UniFFI 0.32.2) except `CoreConfig.bundledPlugins`.

* Object `JellyfeshCore(config:)` (throws), `send(command:) -> UInt64`, `snapshot()`,
  `setListener(listener: CoreListener?)`, `shutdown()`; queries `albumsSorted(sort:descending:offset:limit:)`,
  `artists(offset:limit:)`, `tracks(query:)`, `track(id:)`, `albumTracks(albumId:)`,
  `artistAlbums(artistId:)`, `searchLibrary(text:limit:)`, `recentlyAdded(limit:)`,
  `recentlyPlayed(limit:)`, `mostPlayed(limit:)`, `playlists()`, `playlist(id:)`,
  `playlistEntries(id:offset:limit:)`, `exportPlaylist(id:path:)`, `favorites(offset:limit:)`,
  `isFavorite(track:)`, `artworkThumbnail(hash:sizePx:) -> String?`, `queueWindow(offset:limit:)`,
  `roots()`, `stats()`. Free functions `outputDevices()`, `monotonicNowNs()`,
  `defaultDataDir() -> String?`, `defaultCacheDir() -> String?`, `libraryVersion()`, and
  **`eqPresets() -> [EqPreset]`** (record `EqPreset { id, settings: EqSettings }`; assumed from
  `core/jf-ffi`, not yet checked against a generated file).
* `protocol CoreListener: AnyObject, Sendable { func onEvent(event: Event) }`.
* `CoreConfig(dataDir:cacheDir:output:)` with defaults for the rest; `OutputKind.device(name:)`;
  **`CoreConfig.bundledPlugins: [String]`** (assumed name and type, behind `JF_BUNDLED_PLUGINS`).
* `CoreFfiError.Core(message:)`, `.NotAvailable(message:)`, `.ShutDown(message:)`, `.InvalidArgument(message:)`.
* Enums with data use the field names as labels: `TrackRef.library(id:)`, `.file(path:)`,
  `.source(track:)`; `ArtworkRef.file(path:)`, `.url(url:)`; `Collection.album(id:)` …
  `.tracks(tracks:)`, `.source(plugin:id:)`, `.favorites`; `ScanStatus.progress(progress:)`,
  `.finished(report:)`, `.failed(message:)`; `LyricsSource.sidecar(path:)`, `.embedded`;
  `HueSource.fixed(hue:)`; `SearchScope.plugin(id:)`; `AuthStep.showQr(data:hint:expiresAtMs:)`,
  `.showCode(code:hint:expiresAtMs:)`, `.openUrl(url:)`, `.askFields(fields:)`, `.waiting(message:)`,
  `.done(account:)`, `.failed(message:)`; `AuthField` (`name`, `label`, `kind`, `secret`, `required`,
  `placeholder`, `help`) and `AuthFieldKind` (`.text`, `.email`, `.url`, `.number`, `.cookieHeader`);
  `Event.playback(snapshot:)` (and the other slices), `.scan(status:)`, `.auth(update:)`,
  `.searchResults(update:)`, `.browsePage(update:)`, `.buffering(uid:buffering:)`,
  `.commandFailed(id:error:)`, `.error(message:)`, `.libraryChanged(revision:)`,
  `.playlistCreated(request:id:)`; every `Command` case with the Rust field names
  (`.setShuffle(on:)`, `.setRepeat(mode:)`, `.setVolume(volume:)`, `.setMuted(muted:)`,
  `.setPluginSettings(id:settingsJson:)`, `.search(query:scope:)`, …).
* Records with lowerCamelCase fields, including `QueueSnapshot.len` and `` `repeat` ``,
  `PlaybackSnapshot.replaygainMode`, `Track.trackNo`/`discNo`/`albumId`, `PlaylistEntry.entryId`,
  `PluginSummary.authMethods`/`settingsSchemaJson`/`settingsJson`/`icon`,
  `SourceTrack.artworkUrl`, `SourceItem.alsoIn`, `ThemeColor.rgba` (0xRRGGBBAA),
  `AuxTrack.lines: [UInt32: AuxLine]`, `Line.direction` (`.ltr`, `.rtl`, `.auto`), `Format.webVtt`.
* `Settings.transparency: Float`, `Settings.lyricsSize: LyricsSize` (`.small`, `.medium`,
  `.large`) and `Settings.lyricsAlign: LyricsAlign` (`.start`, `.center`).
* **Assumed, not yet checked against a generated file:** `SourcesSnapshot.presets`, the record
  `SourcePreset` (`id`, `name`, `plugin`, `authMethods`, `needsAddress`, `address`,
  `addressExample`, `account`; the app's own model has the same name, so the conversion writes
  `JellyfeshFFI.SourcePreset`) and `Command.connectSource(preset:address:method:)`.
* Shared ports (`platforms/shared/swift`): `DisplayClock()`, `update(_:nowNs:)`, `positionUs(nowNs:)`,
  `isRunning`, `waitUntilUs(targetUs:nowNs:)`, `anchor`; `PlaybackAnchor.positionAt(_:)`;
  `nextSecond(_:)`, `nextStep(_:step:)`; `LyricTimeline.cursor(_:)` (`LyricCursor`) and
  `nextChange(_:)`; `glassAlpha(_:transparency:)` and `defaultTransparency`.

Noticed in the generated code: the mirror `enum Collection` shadows
`Swift.Collection` inside the module and in any file that imports it (the app
writes `JellyfeshFFI.Collection`); `CoreFfiError.ShutDown` carries a message;
`CoreFfiError.errorDescription` is `String(reflecting:)`, so the app shows the
message instead; `import JellyfeshFFIFFI` is behind `#if canImport`, so a wrong
include path shows up as missing `RustBuffer` declarations rather than a
missing module.

**v0.0.8 additions** (assumed from `core/jf-ffi` at the M1 commits, not yet checked against a
generated file): `Settings.streamingQuality: StreamingQuality` (`.auto`, `.lossless`, `.high`,
`.normal`, `.low`), `Settings.downloads: DownloadSettings` (`folder`, `quality`, `nameTemplate`,
`addMissingCoverAndLyrics`, `lyricsInTags`, `saveLrcFiles`, `wifiOnly`, `atOnce`) and
`Settings.miniPlayer` (`keepOnTop`, `openWhenWindowClosed`, `showLyricsLine`, `androidPip`);
`CoreConfig.downloadsDir: String?`; `NowPlaying.qualities`/`qualityNote`/`qualityLevel`,
`RemoteSnapshot.qualityNote` and `QualityNote.setInService(service:)`, `.filesPlayAsTheyAre`;
`Collection.source(plugin:id:itemKind:)`; `Command.setNetworkState(online:metered:)`,
`.searchMore(plugin:kind:cursor:)`, `.refreshSourceLibrary(plugin:)`,
`.sourceLibraryMore(plugin:part:cursor:)`, `.savePlaylistCopy(plugin:id:name:)`,
`.download(target:)` with `DownloadTarget.track(track:)` and `.collection(plugin:id:itemKind:)`,
`.cancelDownload(id:)`, `.retryDownload(id:)`, `.removeDownload(id:)`, `.removeAllDownloads`;
`Event.searchPage`, `.sourceLibrary`, `.sourceLibraryPage`, `.downloads` and
`Snapshot.sourceLibrary`/`downloads`; `SearchPage`, `SourceLibrarySnapshot`, `SourceLibraryList`,
`SourceLibraryEntry` (`owned`, `songsListed`), `SourceLibraryPart`, `SourceLibraryPage`,
`DownloadsSnapshot` (`folder`, `items`, `bytesUsed`, `paused`), `DownloadItem` (`bytesDone`,
`bytesTotal`, `path`, `libraryId`, `original`, `state`), `DownloadState`, `DownloadError`
(`.notAllowed(service:)`, `.notOffered(service:)`, `.network`, `.storage`, `.other`),
`DownloadsPaused`; `PluginSummary.library`/`download`/`qualities`/`localNetwork`;
`SourceItem.downloaded`, `TrackRow.downloaded`; the free function `downloadPreview(template:)`.

**v0.0.7 additions** (checked against `Generated/JellyfeshFFI.swift` generated after the core's
M3, 432d68f): `CoreConfig(dataDir:cacheDir:output:distribution:)` and `Distribution`;
`JellyfeshCore.sourceArtwork(url:sizePx:)`, `localizedSettingsSchema(plugin:language:)`,
`setStoreEntitlement(entitlement:)`; the free function `lyricsBar(timeline:tMs:)` (ported,
not called); `Snapshot.aiLyrics`/`look`/`pro`/`update` and their events, `Event.discover`,
`.details`, `.comments`, `.artwork`; `ItemRef.source(plugin:itemKind:id:)`; `PluginKind.` `` `extension` ``;
`SourcesSnapshot.canInstallPlugins`, `ProSnapshot.licenceKeys`, `SourceItem.artistId`/`albumId`;
`Settings.checkForUpdates`/`showLyricsBar`/`desktopLyrics`; `NowPlaying.quality`.

**v0.0.6 additions** (checked against `Generated/JellyfeshFFI.swift` from this revision):
`SourceItem.link`, `NowPlaying.link`, `PluginSummary.remotePlayback`/`attribution`,
`Snapshot.remote`, `Event.remote(snapshot:)`, `Command.remote(action:)` with
`RemoteAction.refresh`, `.transfer(device:)`, `.setVolume(percent:)`, `.end`;
`RemoteSnapshot` (`version`, `source`, `sourceName`, `status`, `item`, `context`, `anchor`,
`shuffle`, `` `repeat` ``, `device`, `devices`, `error`), `RemoteDevice` (`volumePercent`),
`RemoteDeviceKind` (`.computer` … `.other`); `protocol ExternalPlayer` (`devices()`,
`state()`, `play(context:tracks:start:)`, `resume()`, `pause()`, `next()`, `previous()`,
`seek(positionMs:)`, `setVolume(percent:)`, `setShuffle(on:)`, `setRepeat(mode:)`,
`transfer(device:)`, all `throws`), `ExternalItem` (`artworkUrl`), `ExternalState`,
`ExternalPlayerError.Failed(reason:)`, and `JellyfeshCore.registerExternalPlayer(id:name:player:)`
(throws) / `unregisterExternalPlayer(id:)`.

## Known gaps

* v0.0.8: Settings › Sources and Extensions are still the system's grouped forms, not the web's
  Mist cards, and the title and section tabs stay put while the groups scroll; album and playlist
  songs are rows over the field rather than inside a card; the panel uses the app's Clear glass
  (SwiftUI materials) rather than AppKit's sidebar material; the mini player has no Liquid Glass
  on macOS 26 and does not open the main window from its cover. The Downloads page lists a
  download's song with the source's cover, which needs the network; an album's Remove Download…
  sends one command per song. The M1 core answers Download, Show More on the sources' lists and
  Save as Playlist… with "not available in this build yet", which the app shows as a notice. None
  of it has been compiled where it was written: CI's macOS job is its first build.
* Apple Music's results show placeholders: their pictures do not come
  through the core's cache.
* v0.0.7: StoreKit has not met App Store Connect yet (the product, the
  sandbox and Ask to Buy are untested); Desktop Lyrics is not resizable;
  ordered checklists reorder with buttons, not by dragging; the Settings
  window's own forms keep the system's type rather than the look's.
* Apple Music needs a paid team, an App ID with the MusicKit App Service and a
  provisioning profile; whether a Developer ID build can use MusicKit is open
  (ADR-0010). Until then it stays hidden.
* The macOS 26 layered icon (Icon Composer `.icon` with Ice and Steel) is not
  built: on every version the Dock switch happens at run time.
* `Info.plist` usage strings are English only.
* v0.0.4: the artwork does not grow out of the mini player's when Now Playing
  opens (a slide and fade instead, see above); page titles stay at `text-h2`
  on wide windows (the spec allows `text-h1` there); the lyrics cascade
  estimates the scroll distance from the line sizes instead of measuring it;
  in a gap between timed lines (no line active) every line shows as one to
  come, as on Windows and Android.
