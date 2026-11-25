# The Android app (v0.0.8)

Kotlin and Jetpack Compose over the Rust core through the UniFFI Kotlin bindings
(docs/ffi.md). minSdk 26 (cpal's AAudio output, ADR-0009), compileSdk and targetSdk 37.
Everything lives in `platforms/android/`. The shell holds no business logic (ADR-0001): it
draws the core's slices and sends commands.

**Status.** The Android SDK cannot be downloaded in the environment this was written in, so the
APK has never been built and the app has never run. The parts that need no Android are built
and tested here (`:logic`), the FFI adapter is compiled and run against the real core on the
JVM (`:ffi-check`), and the whole app except the Media3 session is compiled on the JVM against
stand-ins (`:ui-check`). The first CI build is the first real compile; see "Risks" below.

## v0.0.8: Jellyfesh Web's layout (ADR-0023)

The app draws Jellyfesh Web's structure with Compose, at the web's breakpoints; the rules are in
`:logic` (`nav/Navigator.kt`, `nav/Places.kt`, tested) and the views draw them.

* **Phones (up to 760 dp): the web's five tabs**, Home, Library, Search, Sources and Settings,
  in a floating Veil capsule. The Library tab switches between Albums, Artists, Songs, Playlists,
  Favourites and Downloads with the web's pills above each page's own title
  (`theme/PillTabs.kt`: the choices share the width when they fit and scroll sideways when they
  do not). A customized look orders the tabs and the Library pills (ADR-0020); Settings stays
  last and Downloads, newer than the look's places, comes after the look's library places.
* **Tablets and wide windows (from 761 dp): the web's sidebar panel** (`nav/SidebarPanel.kt`), a
  floating panel of Clear glass: the wordmark with the chosen app icon, Home, Albums, Artists,
  Songs, Playlists, Favourites and Downloads, a separator, Search and Sources, then Open Files…
  and Settings at its foot. Pages sit on the web's fluid gutter beside it (4 % of the window, 18
  to 32 dp). **Open Files…** plays picked audio files without adding them to the library, as on
  the web: in place when the core can read the path, else from a copy in the cache that the next
  Open Files… replaces (`library/OpenedFiles.kt`).
* **The bar floats over every page**, Settings included, and is not drawn while Now Playing is
  open. Under 900 dp it is the capsule: the artwork, the title, play and next, with the lyrics
  bar's strip above it. From 900 dp it is the full bar with the web's measures: the artwork and
  two lines (the lyrics bar's line under the title when Show Lyrics Bar is on), the transport over
  the scrubber, the quality badge, Lyrics, Queue, the device button while a remote source plays,
  the volume, the mini player button and More (Now Playing's), in the web's three columns (the
  title at least 112 dp, the transport 232 to 340, the tools as wide as they need). A narrower bar
  drops the volume's line, then Lyrics and Queue, then the mute button, at the web's widths
  (`player/PlayerBarLayout.kt`).
* **The quality badge and menu** (ADR-0026 §3) are in the full bar and Now Playing's dock, beside
  More as in the web's: the core's label for what plays, else the chosen level. The menu is a sheet
  with the format line ("FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s") and the levels the core
  offers with Streaming Quality checked, or "Files play as they are." or "Quality is set in
  {Service}." (also while a remote session runs). Choosing a level is the same SetSettings as
  Settings › Playback › Streaming (`player/QualityMenu.kt`).
* **The mini player** is picture-in-picture (ADR-0026 §4): with Settings › Appearance › Mini
  Player in Picture-in-Picture on, leaving Jellyfesh while music plays opens it, and the full
  bar's mini player button opens it at once. Devices without picture-in-picture (low-RAM ones)
  show the switch off and say so.
* **The network reaches the core**: `SetNetworkState` at start and on every change of the default
  network (ConnectivityManager's callback, `system/NetworkWatcher.kt`): online when it reaches the
  internet, metered unless Android says it is not (Android 11's temporarily unmetered counts as
  unmetered). The core decides Auto quality (High while metered) and whether downloads wait
  (`state/NetworkReport.kt`).
* **Settings is one page**: its title, the sections as pills (Appearance, Playback, Library,
  Downloads, Sources, Extensions, About), then the section's groups as the web's cards, each a
  Mist card with its heading inside, kept to the web's reading width (704). Sources Settings… and
  Open AI Lyrics Settings open the Settings tab at their place. Settings › Sources lists each
  source inline, as the web does: icon, name and badge with Enabled, Version · Author, whether it
  is signed in, then Sign In (a choice when it offers several ways) or Sign Out, Settings (its
  form, with Save, in a sheet) and Uninstall…; the page each source had of its own is gone.
  Choosing a source on the Sources page opens its Discover page, the sign-in sheet while nobody
  has signed in, or Settings › Sources while it is off (`YourSources.opens`).
* **Search has the web's tabs**: All (the library, then From Your Sources), Library, and one per
  source that answered, in the order they did (`search/SearchModel.kt`). By source (ADR-0024 §4):
  All shows the sources that allow merging under From Your Sources and links to the tab of each
  source whose terms keep its results apart ("Results from Spotify"); a source's tab shows its
  credit and its Songs, Albums, Artists and Playlists, each with Show More while the core has a
  cursor for it. The next page joins its group once and keeps the new cursor; a failure shows
  under the group, which can be tried again.
* **Measures**: page titles are type.text-h2 (30.4) under 900 dp and type.text-h1 (48) from
  900 dp, as on the web; Back is the web's (the chevron and the word in accent text); Albums and
  Songs put their sort menu beside the title.
* **Not yet**: Downloads (the page and Settings › Downloads) say they are not in this build yet.

## v0.0.8: Android 17's Local Network permission

The app targets API 37, and on Android 17 such an app reaches servers on the home network only
with the runtime permission `ACCESS_LOCAL_NETWORK` (Nearby devices group): without it TCP
connections time out and UDP fails with EPERM, so Jellyfin and Navidrome at home failed
(developer.android.com/privacy-and-security/local-network-permission). Earlier Android versions
grant it with `INTERNET`.

* **Declared** in the manifest; asked only on Android 17 and later.
* **The core decides what is local**, by Android's definition: `PluginSummary.localNetwork` for
  saved sources, and `CoreClient.addressIsLocal` (the FFI's `address_is_local`) for the address
  typed in Add a Source before it is saved. The one gate, `LocalNetwork.needsPermission` in
  `:logic` (`sources/LocalNetwork.kt`, tested), is what they feed.
* **Asked before connecting**: Add a Source with a local address, Sign In on a local source's
  page, and turning a local source on show a sheet (why, Not Now, Continue), then Android's
  dialog. Jellyfesh also asks once by itself when it opens with enabled local sources and has
  never asked. The answer is advice: the action goes ahead whatever it is, because a server
  reached through a VPN needs no permission (`ui/sources/LocalNetworkUi.kt`).
* **Denied or revoked**: "Local Network Access Is Off" names the sources on Home, the Sources
  page, Settings › Sources and the source's own pages, with **Allow…**. After a denial the sheet
  gives the fuller reason and asks again, which Android's permission request reset counter
  allows; once Android stops showing its dialog (`shouldShowRequestPermissionRationale` false
  after an ask), the button is **Open Settings** (the app's page, Permissions › Nearby devices).
  The state is read again whenever the window comes back to the front.
* Whether Jellyfesh has asked, and whether it explained it once at start, are kept in the app's
  own preferences (`system/LocalNetworkPermission.kt`).

## v0.0.7: Pro, extensions, the look, AI Lyrics, Discover and details

Built from docs/ui.md and docs/core-api.md against the core's v0.0.7 contract; the rules read
from the slices are in `:logic` with tests, the views only draw them.

* **Covers through the core** (`images/ArtworkLoader.kt`, `logic/images/SourceCovers.kt`): every
  source picture (rows, shelves, details, comment avatars, Now Playing, a remote session's track,
  `jellyfesh-artwork://` items) goes to `source_artwork`; a path loads at once, otherwise the row
  waits for `Event::Artwork`, which reaches the loader directly (`CoreStore.artworkListener`), so a
  burst of covers cannot crowd the signal buffer. The media session shows the cached file only.
* **Play and Shuffle for every song** (`Collection::Songs` in the Songs sort order).
* **Resume Playback** (`logic/player/ResumeOnLaunch.kt`): the core starts with `resume_on_start`
  false, because the process may start without its window (a media button, the system restarting
  the media service) and no audio may begin before the media session exists. When the first
  window is created within 10 s of the process starting, the app sends Play itself if Resume
  Playback is on and the restored queue has a song.
* **The lyrics bar** above the mini-player (`ui/player/LyricsBarView.kt`): the core's `lyrics_bar`
  ported (`logic/lyrics/LyricsBar.kt`, replaying `tests/contract/lyrics-bar.json` within 1e-6),
  the line filling word by word in the highlight colour, its translation with Show Translations,
  and Show Lyrics Bar in Settings › Appearance › Lyrics. No Desktop Lyrics (an allowed difference).
* **Settings › Extensions** (`ui/settings/ExtensionsSettings.kt`): Jellyfesh Pro (Preview,
  Purchased, a licence with Remove Licence…, or the offer with Buy and Restore Purchases where the
  core lists this build's store and Enter Licence Key… where keys work; the Licence Agreement, the
  Privacy Policy and the Terms of Sale), then each extension on a page of its own: Enabled, its
  account ("API Key Added" for AI Lyrics' key), its settings, Reset to Defaults for an appearance
  plugin, AI Lyrics' On-Device Models and Sending Audio, its version. "Part of Jellyfesh Pro." with
  Learn More… and the Pro badge while Pro is off. Appearance ends with Customize….
* **Plugin settings in the app's language and schema profile 1.2** (`ui/sources/PluginSettings.kt`,
  `logic/sources/SettingsSchema.kt`, `FormControls.kt`): `localized_settings_schema` for every
  language but English, `x-jf-group` headings, colour wells, sliders on their steps, ordered
  checklists with drag handles (and Move up, Move down for TalkBack), long choices from a list.
* **The Pro sheet and licence keys** (`ui/settings/ProSheets.kt`): the sheet any Pro action opens
  while Pro is off; Enter Licence Key waits for the Pro slice and words `licence_error`.
* **Google Play Billing** in the Google Play build only (`billing/PlayBilling.kt`, Play Billing
  9.1.0): the one-time product `pro`, `queryPurchasesAsync` at start and on every return to the
  app, each purchase's signature checked with `BuildConfig.PLAY_LICENSE_KEY` (a build without the
  key grants nothing), the result reported with `set_store_entitlement`, then acknowledged;
  pending payments grant nothing. Other builds have no store (`AppGraph.purchases` is null).
* **About**: the Licence Agreement and the Privacy Policy; direct downloads (the Update slice's
  `supported`) add Check for Updates, its setting and View Release. Store builds hide Install
  from File… and Community Plugins (`can_install_plugins`).
* **AI Lyrics** (`ui/player/LyricsMakingView.kt`, `logic/lyrics/LyricsMaking.kt`): Create Synced
  Lyrics and Time These Lyrics in the Lyrics pane, the job's stage with Cancel, the model or the
  API key that comes first, Try Again, "Send This Song to {origin}?" before the first online job,
  "Made by AI Lyrics · {engine}", Make Again and Delete Made Lyrics in Now Playing's More, and
  Create Synced Lyrics in the song menu.
* **The look applied live** (`ui/theme/Look.kt`, `logic/theme/LookStyle.kt`): corner radii, glass
  (tint and blur), the aurora, the face (System, Serif or Mono; Rounded is the system's), text size
  through the density's font scale, spacing, the lyrics' weight, glow and highlight, the Library
  switcher's places and Home's sections in the look's order, Now Playing's Stage, Artwork and
  Lyrics layouts and the Bar, Compact and Floating mini-players (`logic/player/NowPlayingLayouts.kt`).
* **Discover, details and comments**: choosing a source opens its Discover page
  (`ui/sources/DiscoverScreen.kt`); Get Info in the song, album and artist menus and Now Playing's
  Info pane with the decoded format and the core's quality label (`ui/player/DetailsViews.kt`);
  Go to Album and Go to Artist for a source's songs open its pages with the picture and the notes
  or biography; a Comments pane for sources that show comments.

Checked: `:logic` 240 tests pass, `:ffi-check` 6 pass against the M3 core (a v0.0.7 scenario among
them: the slices' first values, a refused key, a song's details, the library's Discover page and
the decoded format), and `:ui-check` compiles. `:ui-check` compiles the billing glue against
stand-ins written from the developer.android.com reference; only CI builds it against Play
Billing, and nothing here ran on a device.

## v0.0.7: the parity pass

What docs/parity.md listed for Android and needed no new core API, with the rules in `:logic` and
tested (`sources/AuthFlow.kt`, `sources/YourSources.kt`, `sources/Browse.kt`, `sources/Links.kt`,
`player/PlayerModel.kt`, `queue/QueueModel.kt`).

* **Sign-in steps as the plugin describes them.** Jellyfin's Quick Connect code (`ShowCode`) is a
  step of its own: the code large and selectable, spelled out for TalkBack, the plugin's sentence
  and "Waiting for approval…". A QR code shows the plugin's own sentence when it gives one (no
  countdown, as in the other apps). Fields keep the core's kind, required and help: only required
  fields hold Sign In back, an optional one says "Optional" beside its label, the help shows under
  the input, the keyboard follows the kind and a pasted cookie header gets several lines. A failed
  sign-in shows the plugin's message alone.
* **Your Sources.** Choosing a source opens its catalogue when it is on and can browse
  (`YourSources.opens`), else its page in Settings › Sources. Its line is the badge, then "Off",
  "Signed in as …" or "Not signed in" (the badge alone for a source that needs no sign-in), and
  every list of sources shows the plugin's own icon. A source that asks for attribution says
  "Content from {Service}" under its pages' titles and its Search section.
* **A source's album or playlist** gets the full page, after Jellyfesh Web's: the artwork, what it
  is, the title, the artist in accent text, the song count, Play and Shuffle (the whole collection,
  through the core) with More, the songs (numbered on an album) and "12 songs, 48 minutes"; when
  none of its songs plays here, Open in {Service} is its main action. A page the source did not
  answer says "This Source Did Not Answer" with the error; an empty one "Nothing Here". Other
  levels (the root, artists, folders) keep their title, rows and the top bar's More.
* **The media session follows a remote session** (`SessionModels`): the notification, lock screen
  and headset keys show the service's song with its artwork, length, position, play state,
  shuffle and repeat. The song stands alone (the service keeps its Up Next); Next and Previous
  still reach the service, because `SimpleBasePlayer` hands a skip it would ignore to
  `handleSeek` with its command.
* **Up Next lists every entry** after the current one: the slice's 50, then pages read with
  `queue_window` as they scroll into view (read again when the queue changes), with the same tap,
  drag, Remove from Queue and Move up and Move down on every row.
* **Words:** System Default (never "Speakers"), No Folders Yet, Unknown Title, Also in …, Remove
  from Queue, "This Playlist Is Empty" with "Add songs from their menu with Add to Playlist.", the
  no-lyrics and Show Translations sentences of the Linux app and the web, and Rescan Library as a
  button beside its sentence. The Sources tab no longer has Install from File… (it is in Settings
  › Sources › Installed, as in every app), and About This Song lists Lyrics before File.

Checked for this pass: `:logic` 181 tests pass, `:ffi-check` 5 pass against the core, and
`:ui-check` compiles with no new warnings. The app itself was not built here (no Android SDK).

## v0.0.6: links, remote playback and Luminous

What docs/ui.md's "Links and remote playback" and "Luminous" ask of phones, with the rules in
`:logic` and tested (`player/RemoteModel.kt`, `player/PlayerModel.kt`, `sources/Links.kt`,
`library/LibraryModel.kt`, `theme/Luminous.kt`).

* **Links.** `SourceItem.link` and `NowPlaying.link` map into the models; only `https` links are
  ever handed to the system (`Links.openable`, scheme lower-cased). Open in {Service} (the plugin's
  name) is in the song menu after Go to Artist, in the More menu of a source's album, artist or
  playlist (search rows and the browse page), in Now Playing's More and under About This Song.
  It starts `ACTION_VIEW` (category browsable), so the service's own app opens the link when it
  handles it, else the browser; nothing installed to open it gives a short message. A source row
  that cannot play here but has a link opens it as its main action, beside its reason, with the
  external glyph at its end (`SourceItems.mainAction`).
* **Remote playback.** `CoreState.remote` mirrors the Remote slice (versioned like the others).
  While it has a source, the mini-player, the player bar and Now Playing show the service's song
  (`PlayerModel.track`) with "Playing on {Device}" and the last error in a quieter line; progress
  comes from the session's anchor through the one ticker; the transport sends the usual commands.
  The device button opens Devices (it sends `Remote { Refresh }`; the list follows the slice; kind
  glyphs; the playing device checked; choosing another sends `Transfer`). The volume sets the
  playing device's volume when the finger lifts and is hidden when the device has none; More holds
  Open in {Service} and Stop Controlling. Lyrics and Queue say "Lyrics aren't shown while
  {Service} plays." and "Up Next is in {Service}."; the Now Playing backdrop is the Misty field
  (the core bakes none for the service's song). The store forwards the slice that ends a session
  on its signal stream, so an ending error shows once as a message, even when the session ends
  right after it starts. A remote source's songs and lists never offer Play Next, Play Last or Add
  to Playlist. Since the v0.0.7 parity pass the system media session follows the remote session
  too (below).
* **Luminous.** The field draws four larger glows at `field.aurora-strength` (75 %) on every
  screen. The tab bar and the mini-player are floating Veil capsules (68 dp, and a 64 dp pill with
  round artwork) and the screens fade out just above them (`fadeOutAbove`, an offscreen layer
  masked with `DstIn`). Settings groups, source panels and Home's lists are Mist-tinted surfaces
  with the edge and shadow-1, without live blur (`GlassSurface(blur = false)`). Now Playing: the
  Artwork backdrop, artwork with shadow-3, a title from 24 sp on phones to 32 sp, lyrics on the
  token's fluid range (28 to 40 sp) with inactive lines at 92 % and the cascade 40 ms apart, round
  glass top controls (tint only) and every control in one floating Veil dock: a rounded card on
  phones, an 80 dp bar with radius 40 on wide windows. Page titles are `type.text-h1` (48) where
  the window allows and its fluid minimum (38.4) on phones; grid covers have radius 16 and
  shadow-2 and lift 4 dp on hover or focus. Blur runs only on Android 12 and later; before that,
  and with battery saver, low RAM, Reduce Motion and Transparency, the system's blur turned off
  (`WindowManager.isCrossWindowBlurEnabled`) or more contrast asked for (`UiModeManager.getContrast`,
  Android 14+), glass is solid Cloud. These and "Remove animations" are read again whenever the
  app returns to the foreground; Compose scales every animation by the system's animation scale.
  Live blur layers: the tab bar and the mini-player on library screens, the dock on Now Playing.

**Checked for this pass:** `:logic` 168 tests pass, `:ffi-check` 5 tests pass against the core
built from this release (bindings and tokens regenerated), and `:ui-check` compiles with no
errors. The app itself was not built here (no Android SDK), and nothing ran on a device.

## v0.0.4 design pass

The controls and motion of [`design-controls.md`](design-controls.md),
"v0.0.4: the controls as built" (after Apple Music and Media Player, in
Misty's tokens). The numbers are in `:logic` and tested (`seek/SeekGeometry.kt`,
`player/NowPlayingMotion.kt`, `theme/AccentChoices.kt` with an OKLCH port,
`settings/SettingsModel.kt`; `DesignTest.kt` reads the sizes from
`design/tokens.json`); the Compose code draws them.

* **Seek bar and volume** (`ui/theme/Controls.kt`): a straight line with round
  caps instead of style D's wave, ink-90 over ink-30, elapsed and remaining
  (−1:23) times. On touch it is 6 dp and 12 dp while dragged, with no thumb;
  with a mouse (desktop mode) 4 dp, 6 dp and the 14 dp ringed thumb on hover
  or focus, 8 dp dragged. TalkBack reads "1:23 of 3:45"; with keyboard focus
  Left and Right step 5 s (mirrored right to left), Page Up and Page Down
  30 s, Home and End go to the ends. The fill redraws per device pixel. The
  volume is the same line (Left and Right 5 %).
* **Mini player and player bar** (`ui/nav/Navigation.kt`): under 900 dp the
  floating Veil capsule (`player.mini-radius` corners, one play button) with
  a 2 dp progress line along its bottom edge, inset by its corners, instead
  of the ring; from 900 dp (tablets, desktop mode) the 80 dp player bar:
  artwork and two lines, the transport capsule above the seek bar, Lyrics,
  Queue and the volume.
* **Now Playing:** the artwork rests at 88 % while paused and springs back
  (dur-3 Spring); play and pause morph; it opens with a slide and fade (no
  shared-element artwork). Lyrics take the Lyrics size and alignment
  settings; sung lines are ink-30, inactive lines 96 % from their start edge
  (their centre when centred), and the lines below the active one settle
  30 ms apart (keyframes over the scroll). Tapping a line seeks to it.
* **Library:** titles at `text-h2`; album cards lift with a 36 dp glass play
  button when a mouse hovers them and press to 97 %; the album page has
  full-width artwork on phones and 240 dp on wide screens, the artist in
  accent text (it opens the artist), genre, year and count, Play and
  Shuffle, and a "12 songs, 48 minutes" footer; the playing row is
  accent-soft with a three-bar level glyph that moves only while playing.
  Track numbers stay numbers (no hover on touch).
* **Motion:** pages fade in and rise 8 dp (dur-2 Fluid), going back fades
  (dur-1); dialogs scale from 96 % and fade; theme colours crossfade. Under
  reduced motion (the system's "remove animations", or Reduce motion and
  transparency) nothing moves and transitions are a short fade (dur-1); the
  artwork keeps its size, the level glyph stands still and lyrics jump. Bottom sheets keep their slide
  (now with a fade) rather than scaling from 96 %.
* **Settings → Appearance:** Theme (System, Light and Dark with previews), Accent
  Colour (Follow the Artwork, ten named swatches and a Custom hue on a
  hue-gradient track with a preview), Transparency from Solid to Clear in
  whole percent with a live preview and Reduce Motion and Transparency, the Now
  Playing Backdrop with previews, Lyrics (size, alignment, translations and
  romanization) and the App Icon, each group under its heading (see "Aligned
  with docs/ui.md"). Each change is one `SetSettings`.
* **Transparency on Cloudy:** `GlassSurface` keeps Cloudy's blur and sets the
  Misty tint's alpha from the shared glass-alpha port
  (`platforms/shared/kotlin`, `glassAlpha`): the tokens' alphas at 50 %,
  opaque toward Solid, halved toward Clear; at 0 % the glass is solid Cloud.

**Checked here, not built for Android:** `:logic` 114 tests pass, `:ffi-check`
3 tests pass against the core, and `:ui-check` compiles the app (with these
screens) against Compose Multiplatform 1.8.2 and the stand-ins with no errors
or warnings. Nothing ran on a device; the APIs Compose 1.12 has beyond 1.8 are
not used.

## Aligned with docs/ui.md

The app follows the shared UI spec ([`ui.md`](ui.md)) with Android's own mechanics: bottom tabs,
sheets for menus and pickers, dialogs for questions, the system's document picker. Order and
structure live in `:logic` and are tested; the Compose code draws them.

* **Settings** (`settings/SettingsModel.kt`, `SettingsLayout`): the sections Appearance, Playback,
  Library, Sources and About, each with its groups in the spec's order under headings. There is
  no Lyrics section any more: Lyrics Size, Lyrics Alignment, Show Translations and Show
  Romanization are Appearance › Lyrics. Playback: Output (Output Device), Crossfade, Volume
  Levelling (ReplayGain and its preamp), Equalizer, Startup (Resume Playback). Library: Music on
  This Phone (access to music and Find Music on This Phone, Android's allowed difference), Music
  Folders (Remove Folder… asks "Remove “Name” from the library?"; Add Folder… after the folders),
  Watch Folders, Rescan Library (its progress, then Last Scan). Sources: Add a Source (the connect
  sheet of the Sources tab), Installed (Install from File… in its heading; each source opens its
  page: badge and sign-in state, Enabled, the sign-in buttons or Sign Out, its settings with Save,
  Browse, "Version · Author", Uninstall…, which asks first), Community Plugins.
* **Equalizer:** Preset lists the core's `eq_presets()` (`CoreClient.eqPresets()`) translated by id
  and shows Custom once a band moves (`EqPresets`: a preset is its bands, so the switch and the
  equalizer's own preamp stay the person's when one is chosen). Preamp ±12 dB, the ten bands, and
  Reset to Flat, which applies the core's flat preset with a 0 dB preamp.
* **About:** the version is `BuildConfig.VERSION_NAME`, with the core's `library_version()` after
  it only when the two differ; nothing is typed in. The website and its third-party notices
  (`THIRD_PARTY_NOTICES.html`, which the website build publishes) show when the build knows the
  website: the Gradle property `jellyfesh.website`, else in GitHub Actions
  `https://OWNER.github.io/NAME/` from `GITHUB_REPOSITORY` (docs/github.md). Other builds show the
  version only; the APK carries no licence texts of its own.
* **Sources tab:** the intro line, Add a Source, Your Sources (each with its icon and status;
  choosing one browses it, or opens its page while it is off or has nothing to browse), then
  Sources Settings…. Install from File… is in Settings › Sources › Installed, as in every app.
* **Home:** "1,204 songs · 96 albums · 41 artists" (counts with the language's thousands
  separator), Recently Added (albums), Recently Played and Most Played. With nothing in the
  library and no source signed in (`HomeModel`), the welcome instead: "Add your music folders, or
  connect a server in Sources.", Add Music Folder… and Add a Source, after Allow Access to Music
  while that access is missing.
* **Library:** Albums and Songs sort from a menu (a sheet): Title, Artist, Album (songs), Year,
  Recently Added, then Descending (`SortMenus`; Year and Recently Added start newest first). The
  empty pages say "Add a music folder to see your albums here." with Add Music Folder….
* **Playlists:** New Playlist… asks for the name first. Import Playlist… and Export Playlist…
  (M3U8) use the document picker. Import hands the core the picked file where it is when its
  provider names a path the core can read, so the playlist's relative lines resolve against its
  folder; otherwise a copy in the cache under the file's own name (the core names the playlist
  after the file; relative lines in such a copy do not resolve). Export lets the core write the
  M3U8 to the cache (`export_playlist`; songs get full paths) and copies it into the document
  created as `audio/x-mpegurl`, the type Android maps to `.m3u8`. Songs reorder by dragging the
  handle (`PlaylistReorder`; TalkBack offers Move up and Move down), and the drop shows until the
  list read after the core's change arrives.
* **Menus:** a song's menu is Play Next, Play Last, Add to Playlist…, Add to or Remove from
  Favourites, Go to Album and Go to Artist (`TrackMenu`; a favourite state, album or artist a row
  does not carry is read from the library first). The More menu of an album, artist or playlist
  page: Play Next, Play Last, Add to Playlist… (the collection's songs, `CollectionTracks`), and on a
  playlist Rename…, Export Playlist… and Delete Playlist…, which asks first. Clear Queue asks
  first; Now Playing › Info adds the Year.
* **Words:** the spec's words in all four languages (System, Reset to Flat, Server Address,
  Sources Settings…, Uninstall…, Remove Folder…, From Your Sources, swatch names ending with
  Violet, and so on). `StringsTest` checks that every translation has every string with the same
  placeholders, and that the English avoids the words the spec rules out.
* **Not here:** Show in Files for a music folder (Android cannot reliably open a folder by path in
  a file manager), and the spec's keyboard keys, which are for the desktop apps and the web.

Checked for this pass: `:logic` 149 tests pass, `:ffi-check` 4 tests pass against the core (with
bindings generated from the host build of `jf-ffi`), and `:ui-check` compiles with no errors or
warnings. The Android SDK is not available here, so `:app` itself was not built.

## Modules

| Module | What | Builds here |
| --- | --- | --- |
| `:logic` | Pure Kotlin/JVM: mirrors of the core's types, the `CoreClient` interface, the state store, drawing-time scheduling (`PlaybackTicker`, `FrameScheduler`), lyric rows and the word sweep, navigation, queue drag, seek, search, sign-in flows, plugin settings from JSON Schema, app icons, library roots, paging, artwork sizes, theme fallback, the media-session mapping | yes, with tests |
| `:core-bindings` | Android library: the generated UniFFI bindings and tokens (git-ignored), the shared drawing-time ports (`platforms/shared/kotlin`), JNA, and `libjf_ffi.so` per ABI from a cargo-ndk task | needs the SDK |
| `:app` | The Compose app. `src/ffi/` (with `src/ffi-config/`) holds the only code that names generated types (`UniffiCoreClient`, `FfiMapping`, `SharedTime`, `CoreConfigs`) | needs the SDK |
| `:ffi-check` | JVM only, never shipped: compiles `app/src/ffi` with the bindings and runs it against the host-built core | yes, after `cargo xtask bindings` |
| `:ui-check` | JVM only, never shipped: compiles `app/src/main` (all but `playback/` and `MainActivity`) and `app/src/ffi` against Compose Multiplatform 1.8.2 for desktop, Robolectric's android-all and stubs | yes, after bindings and tokens |

The Android modules join the build only when an SDK is found (`ANDROID_HOME`,
`ANDROID_SDK_ROOT` or `local.properties`), so a plain JDK never resolves the Android Gradle
plugin. Versions are in `gradle/libs.versions.toml`: AGP 9.5.0, Kotlin 2.4.21, Compose BOM
2026.09.00, Media3 1.11.1, Cloudy 0.7.1 (pinned by ADR-0005), JNA 5.19.1. AGP 9 needs Gradle 9.6
or later: the wrapper (`./gradlew`) pins 9.8.2.

## Build and test

From `platforms/android/`:

```bash
# :logic, with the installed Gradle 8.14.3 (the exact command used for this release)
gradle :logic:test
# or with the wrapper (Gradle 9.8.2)
./gradlew :logic:test

# Behind a rate-limited proxy (Maven Central answering 429), add:
#   --max-workers=1 -Dorg.gradle.internal.repository.max.retries=12 -Dorg.gradle.internal.repository.initial.backoff=3000

# The JVM checks need the bindings and tokens first (from the repository root):
cargo xtask bindings --lang kotlin && cargo xtask tokens --target kotlin
gradle :ffi-check:test          # runs the adapter against target/debug/libjf_ffi.so
                                # (or $CARGO_TARGET_DIR/debug, or JF_FFI_LIBRARY)
gradle :ui-check:compileKotlin  # compiles the app against the stand-ins
```

Results for v0.0.4: `:logic` 114 tests pass; `:ffi-check` 3 tests pass against the core;
`:ui-check` compiles with no errors or warnings (v0.0.3: 103, 3 and the same).

### The APK

```bash
platforms/android/scripts/build.sh            # what CI runs
platforms/android/scripts/build.sh --skip-core   # reuse core-bindings/src/main/jniLibs
```

It needs `ANDROID_HOME` with platform 37 and the build tools, an NDK (`ANDROID_NDK_HOME`, or
the newest under `$ANDROID_HOME/ndk/`), JDK 17 or later, rustup, cmake (the core builds libopus
and whisper.cpp) and, on Linux, the ALSA headers (`libasound2-dev`) for the host build behind
`cargo xtask bindings`. The script adds the `aarch64-linux-android` and
`x86_64-linux-android` targets, installs cargo-ndk 4.1, builds `jf-ffi` for arm64-v8a and
x86_64 (`--release`), generates the bindings and tokens, and runs
`./gradlew -Pjellyfesh.skipCargo=true :app:assembleRelease`.

whisper.cpp (on-device AI Lyrics, built by whisper-rs-sys) links the C++ runtime as
`c++_shared`, so cargo-ndk runs with `--link-libcxx-shared`: it links it and copies the NDK's
`libc++_shared.so` next to `libjf_ffi.so` in each ABI's `jniLibs` folder, and Gradle packages
both (without it `libjf_ffi.so` fails to load in the app). `GGML_NATIVE=OFF` builds ggml for each
ABI's baseline CPU, never the build machine's. The script and `:core-bindings:cargoNdkBuild` both
check that every ABI's folder holds the two libraries, and the script checks the APK
(`lib/<abi>/`) and the App Bundle (`base/lib/<abi>/`) for them. The unsigned APK is copied to
`dist/android/jellyfesh-<version>-android-unsigned.apk`. The release workflow signs it with the
Jellyfesh release keystore from the repository secrets `ANDROID_KEYSTORE_BASE64`,
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD` (certificate SHA-256
`71:8D:02:2E:77:C3:09:7F:FD:F4:E0:B6:E4:FB:DB:62:FF:B3:EF:92:A8:9C:2B:3E:E3:57:F2:58:B7:13:56:46`)
and publishes it as `Jellyfesh-<version>-Android.apk`; without those secrets a release has no APK
and its text says the APK is coming (it is never signed with another key). With `--play-bundle`
the script also builds the App Bundle for Google Play, which the workflow signs with the same
keystore as `Jellyfesh-<version>-Google-Play.aab`, a run artifact for Play Console
(docs/store-release.md), never a release file. Without `-Pjellyfesh.skipCargo=true` Gradle
builds the core itself through `:core-bindings:cargoNdkBuild`.

Install on a device or emulator (arm64-v8a or x86_64): `adb install -r <apk>`.

### Bundled plugins

The official plugins ship inside every APK (releases no longer carry them separately). The
release workflow builds the commercial edition, which leaves out the unofficial community plugins
(`JF_EDITION=commercial`; they are the release's `Jellyfesh-<version>-Community-Plugins.zip`).
The app's build copies `dist/plugins/*.jfpkg` (made by the plugin packaging script;
the release workflow puts them there) into the APK's `assets/plugins` (task `bundlePlugins`,
stored uncompressed); without them it builds anyway, with a warning. On start the app copies the
packages into its files once per install or update and passes their paths to the core as
`CoreConfig.bundledPlugins` (`app/src/ffi-config/.../CoreConfigs.kt`, the one place that names
the field, which the core and FFI are gaining in this release). Until the generated bindings have
the field, `settings.gradle.kts` switches `:app` and the JVM checks to
`app/src/ffi-config-compat`, which starts the core without the bundled plugins, and the build says
so with a warning, as the macOS and Windows builds do.

### Icons

`python3 platforms/android/scripts/gen_icons.py` (Pillow in a virtual environment) writes the ten
adaptive icons with monochrome layers from `design/icons/round4/`: PNG foregrounds for Ice and
Steel, vectors for the other eight. The results are committed.

## How the app is put together

* **Start.** `JellyfeshApp` creates `AppGraph`, which starts the core off the main thread:
  `AndroidContext.initialize(applicationContext)` first (ndk-context for the audio output and the
  keystore), then `JellyfeshCore`, then the store (`CoreStore`: listener first, then the
  snapshot). Views show the Misty field until the core is ready, and a message if it failed.
* **One adapter.** `CoreClient` (`:logic`) is the shell's view of the core;
  `UniffiCoreClient` and `FfiMapping` (`app/src/ffi`) implement it over the generated types, so a
  renamed field after a core change is fixed there. Queries run on the IO dispatcher and come
  back as `QueryResult` (`NotAvailable` for features still being integrated, shown as "not
  available in this build yet", not as errors).
* **Drawing time.** One `PlaybackTicker` over the ported `DisplayClock` runs while the activity
  is started (nothing ticks with the screen off). Views register what they draw (`TimeDemand`):
  clock labels wake once a second, the seek bar and the mini-player's line per pixel step, lyrics
  at the timeline's next change, and every frame only while a word sweep moves. Positions are read
  in draw phases or through `derivedStateOf`, so a moving position redraws pixels without
  recomposing.
* **Theme.** Colours come from the core's Theme slice (hue-250 fallback before the first one) and
  crossfade on hue changes; type, spacing, radii and motion from the generated `Tokens`.
  `ui/theme/Controls.kt` holds every bar and button (the v0.0.4 straight seek bar and volume
  line, the transport capsule, toggles and fields) in one file.
* **Glass.** `GlassSurface` is the only entry to Cloudy (ADR-0005): backdrop blur with the Misty
  tint on API 33+, uniform blur on 31-32 with progressive edges as scrims, and below 31 the
  translucent fallback (Misty Cloud at the 86 % dialog glass alpha, no blur); solid Cloud on
  low-RAM devices, in battery saver, with Reduce motion and transparency (MAT-08) or at a
  Transparency of 0 %. The Transparency setting sets the tint's alpha through the glass-alpha
  port. At most two live layers per screen: the tab bar and mini-player on library screens, the
  top bar and transport capsule on Now Playing. Both Now Playing styles are built: the core's
  baked artwork backdrop (Artwork) and the Misty field with the artwork on a Clear frame (Misty).
* **Library access: option C of the Step 0 index**, implemented here while the choice is open.
  With the music permission, MediaStore lists the folders that hold music at once and they become
  library roots (the core reads the tags itself); the system folder picker (SAF) adds any other
  folder. Both reach the core as paths. In this version the core reads files by path, so it
  needs the music permission even for SAF folders, and folders on providers without paths (cloud
  drives) are refused with a message; a SAF-backed `FileSystem` in the core is the follow-up.
* **Media session.** `PlaybackService` (Media3 `MediaSessionService`) hosts `CorePlayer`, a
  `SimpleBasePlayer` facade: notification, lock screen, headset and Bluetooth buttons become core
  commands, and the session state is the core's slices (the engine's song and Up Next, or a
  remote session's song while one runs). `AudioFocusController` requests focus
  while playing, pauses on loss (resumes after a transient one) and when audio becomes noisy.
  `DeviceWatcher` sends `RefreshDevices` on Android's device callbacks.
* **App icon.** One `activity-alias` per icon (ADR-0017), Ice enabled by default; Automatic is
  Ice. The picker confirms before switching and states the launchers' limits.
* **Navigation.** Five places (Home, Library, Search, Sources, Settings): the phone layout's tabs,
  or the sidebar panel from 761 dp, where the library's sections are places of their own. Each
  keeps its own back stack in `AppGraph` (so it survives the activity being recreated, and a
  rotation between the layouts), with Now Playing over everything. Back closes sheets, then Now
  Playing, then pops the stack.
* **Accessibility.** Every control has a TalkBack label and state; lyrics are a list with the
  current line selected; the queue offers Move up and Move down; text is in sp; the seek bar and
  volume take keyboard focus and steps; reduced motion (the system's "remove animations", or
  Reduce motion and transparency) turns movement into short fades and the word sweep into
  whole-line highlighting.
* **Languages.** English, Simplified Chinese, Traditional Chinese and Japanese
  (`res/values*/strings.xml`, terms shared with the Linux app), with per-app language support on
  Android 13+ (`res/xml/locales_config.xml`).

## Names assumed from the generated bindings

These were checked against bindings generated from the core of this release (`:ffi-check`
compiles and runs them), except where marked:

* package `org.jellyfesh.ffi`; `JellyfeshCore(config: CoreConfig)` with `send`, `snapshot`,
  `setListener(CoreListener?)`, the queries of docs/ffi.md and `shutdown`; top-level
  `outputDevices()`, `eqPresets()` (`EqPreset(id, settings)`), `libraryVersion()` and
  `monotonicNowNs()`, which reach the views through `CoreClient` like the queries, so `:logic`
  tests use a fake; `CoreFfiException.Core/NotAvailable/ShutDown/InvalidArgument`;
  `AndroidContext.initialize(Any)`;
* `CoreConfig(dataDir, cacheDir, output = OutputKind.Device(name), watchFolders, ...)`;
* **not yet in the bindings:** `CoreConfig.bundledPlugins: List<String>` (paths), named only in
  `app/src/ffi-config/.../CoreConfigs.kt` (while the bindings lack it, the builds compile
  `app/src/ffi-config-compat`, which starts the core without its bundled plugins);
* `ThemeColor(name, rgba: UInt)` with `rgba` = `0xRRGGBBAA`; `LyricTimeline` arrays read by the
  shared ports (`cursor`, `nextChange`); unsigned values as `UInt`/`ULong`;
* the tokens object `org.jellyfesh.tokens.Tokens` with `ThemeColorKey` names such as
  `ink.ink-90` (from `cargo xtask tokens --target kotlin`).

## Risks in code that has never been compiled for Android

* **AGP 9 build scripts.** Built-in Kotlin (no `org.jetbrains.kotlin.android` plugin), the
  Kotlin and Compose compiler Gradle plugins on the root classpath next to AGP, and the variant
  API calls (`sources.kotlin`, `sources.assets`, `sources.jniLibs`) were written without an Android
  build; versions of AGP, the Compose BOM, Media3, activity, lifecycle and browser could not be
  resolved here (Google's Maven is unreachable). The JVM modules configure and build with Gradle
  8.14.3 and the 9.8.2 wrapper.
* **Compose 1.12 versus the check.** `:ui-check` compiles against Compose Multiplatform 1.8.2
  and hand-written stubs (activity-compose, core-ktx, browser, lifecycle, Compose's Android
  resources, Cloudy's backdrop API); an API that differs in 1.12 or a stub that drifted from the
  real library shows up only in CI.
* **Media3.** `CorePlayer`, `PlaybackService` and `MainActivity` were never compiled (no
  stand-in for Media3 1.11's `SimpleBasePlayer`); notification behaviour, foreground-service
  rules on Android 14+ and lock-screen artwork (a `file://` URI) are untested.
* **Cloudy at run time.** The backdrop blur's cost, its re-capture after artwork loads and
  progressive edges on real devices are unmeasured (spike S9); `GlassSurface` falls back to solid
  Cloud when in doubt.
* **Native build.** cargo-ndk with CMake for libopus and the NDK's toolchain file is untested
  here. HTTPS in the engine uses rustls-native-certs, which may find no root certificates on
  Android, so online sources may fail TLS until the core uses Android's trust store or bundled
  roots.
* **Storage access.** On Android 13+ the music permission lets native code read audio files by
  path, but FUSE may hide other files from it: `.lrc`/`.ttml` sidecar lyrics and cover images
  next to the songs may not be readable (embedded lyrics and artwork are). SAF-only use (music
  permission declined) cannot read anything until the core gets a SAF `FileSystem`.
* **Library-wide playback.** Songs plays and shuffles the songs loaded so far (pages of 100):
  the core has no "whole library" collection yet.
* **Source commands** answer "not available in this build yet" until the sources work lands;
  search, sign-in and browsing show that, and their rendering of real steps is untested.
