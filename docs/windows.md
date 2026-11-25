# The Windows app (v0.0.6)

WinUI 3 (Windows App SDK 2.6.5) in C# on .NET 10 (LTS), over the Rust core through
the hand-written `Jellyfesh.Ffi` binding ([`ffi.md`](ffi.md)). Windows 10 1809
or later; Mica on Windows 11. Everything except XAML and WinUI builds and is
tested on any OS.

## v0.0.6: links, remote playback and Luminous

* **Links** ([`ui.md`](ui.md), "Links and remote playback"): an item with a `link` offers **Open in
  {Service}** (the source's name): in the song menu after Go to Artist, in a menu on the sources'
  albums, artists, playlists and folders (search and browse), in More for an artist being browsed,
  in the player's More and in Now Playing › Info under About This Song. A row that cannot play here
  shows why and opens its link when chosen. Links open with `Launcher.LaunchUriAsync`, web
  addresses only (`ItemLinks`, `ShellPlatform.OpenUrl`).
* **Remote playback** (ADR-0018): while the `Remote` slice has a source, `PlayerViewModel` shows its
  track (title, artist, artwork, progress from its anchor through the same `DisplayClock`),
  **Playing on {Device}** under the title and the session's error in a quieter line (the mini and
  compact players show the device line instead of the artist). The transport sends the usual
  commands; the volume sets the device's volume (`Remote { SetVolume }`) and hides when it has none;
  the device button opens **Devices** (`Remote { Refresh }` as it opens, each device with its
  kind's icon, the playing one checked, choosing one sends `Transfer`); More holds Open in {Service}
  and **Stop Controlling** (`End`). Lyrics say "Lyrics aren't shown while {Service} plays.", Queue
  "Up Next is in {Service}."; a session that ends with an error says so once in the notice bar. A
  remote source's songs have no Play Next, Play Last or Add to Playlist….
* **Luminous** (the owner's direction C): the field with four larger glows at
  `field.aurora-strength` (`Drawing/AuroraField.cs`) behind every page and the navigation pane,
  which is Clear glass (in-app acrylic) under a Mica Alt title bar; grouped surfaces in the Mist
  tint without blur (`JfMistSurface`); the player bar floats above content that fades out just
  above it; Now Playing is a stage: the Artwork backdrop, 380 epx artwork on shadow-3, lyrics
  40 epx bold (line height 1.18) from a 640 epx pane, inactive lines at 92 %, a 40 ms cascade, and
  every control in one floating Veil dock (one row; two on narrow windows, without the volume);
  grid covers at radius 16 on shadow-2 lift 4 epx on pointer-over or keyboard focus; page titles
  48 epx. Shadows are `ThemeShadow`s shared per layer and cast onto the field or the stage's
  backdrop (`Controls/Elevation.cs`; outside popups they need receivers). Translucent glass casts
  none, since a rectangular `ThemeShadow` would show through it. Without transparency (or with
  Reduce Motion and Transparency) the glass and the Mist surfaces are solid Cloud and the lyrics
  do not cascade.

**Checked here, not built:** the portable tests pass (118); the app's C# compiles against the
Windows App SDK 2.6.5 reference assemblies with generated stand-ins for the XAML compiler's members,
every XAML event handler against its event, every `x:Bind` (474) as a typed assignment, and every
literal XAML attribute (1,578) as an assignment to the WinUI property it names; every resource key
the XAML uses is defined and every `x:Uid` has resources in the four languages. The XAML compiler,
acrylic, `ThemeShadow` and everything at run time first run on Windows in CI.

## v0.0.4 design pass

The controls and motion of [`design-controls.md`](design-controls.md),
"v0.0.4: the controls as built" (after Media Player and Apple Music, in
Misty's tokens). The numbers live in `Jellyfesh.Shared` and are tested
(`Drawing/SeekGeometry.cs`, `Drawing/NowPlayingMotion.cs`, `Drawing/Easing.cs`,
`Appearance/AccentChoices.cs`, the view models; `tests/.../DesignTests.cs`);
the app only draws them.

* **Seek bar and volume** (`Controls/SeekBar.cs`, `Styles/Controls.xaml`): a
  straight line with round caps instead of style D's wave, ink-90 over
  ink-30, elapsed and remaining (−1:23) times, no thumb at rest. Hover or
  keyboard focus thickens it from 4 to 6 px and shows the 14 px ringed thumb
  (ink-90, glass-edge ring, shadow-1); dragging thickens it to 8 px and the
  times follow the thumb; touch gets 6 and 12 px and no thumb. Changes run
  over dur-1 Fluid on the compositor. Its automation peer reads
  "1:23 of 3:45" (Value pattern); Left and Right seek 5 s, Page Up and Page
  Down 30 s, Home and End go to the ends. It still redraws only when the
  fill moves by a device pixel. The volume slider is the same line.
* **Mini player and player bar** (`Views/MiniPlayer.xaml`, `Views/PlayerBar.xaml`):
  under 900 px the floating Veil capsule (`player.mini-radius` corners, one
  play button) has a 2 px progress line along its bottom edge, inset by its
  corners, instead of the ring; from 900 px of content width a floating 80 px
  player bar with `r-xl` corners: artwork and two lines (they open Now
  Playing), shuffle, previous, play, next and repeat above the seek bar, and
  the quality badge (its menu: the format line and Streaming Quality's
  levels, ADR-0026 §3), Lyrics, Queue and the volume. The mini player
  (Ctrl+M, `Views/CompactPlayer.xaml`, ADR-0026 §4) turns the window into a
  small player, Windows' compact overlay while Keep on Top is on, with the
  lyrics bar's line when Show Lyrics Line is on and the same straight line
  along the top of its glass; with Open When the Window Is Closed, closing the
  window while music plays leaves the mini player. The smoke test opens and
  closes it.
* **Now Playing:** the artwork scales to 0.88 while paused and springs back
  (dur-3 Spring); play and pause morph; the artwork grows from the mini
  player's or the bar's through a connected animation where the toolkit
  allows (else a slide and fade) and Now Playing closes on Exit. Lyrics take
  the Lyrics size and alignment settings; sung lines are ink-30, inactive
  lines 96 % from their start edge (their centre when centred), and the lines
  below the active one settle 30 ms apart (compositor keyframes over the
  scroll). Tapping a line seeks to it.
* **Library:** large titles (`text-h2`) that scroll with the content; album
  tiles lift 2 px onto shadow-2 with a 36 px glass play button in their bottom
  start corner and press to 97 %; the album page has 240 px artwork, the
  artist in accent text (it opens the artist), genre, year and count, Play
  (accent) and Shuffle (glass), track numbers that turn into a play glyph on
  hover, a three-bar level glyph on the playing row that moves only while
  playing, and a "12 songs, 48 minutes" footer.
* **Motion:** pages fade in and rise 8 px (dur-2 Fluid) and going back fades
  (dur-1), on the compositor; theme colours crossfade (dur-4). Under reduced
  motion (Windows' animation setting off, or Reduce motion and transparency)
  pages, Now Playing and theme colours only fade, over dur-1: the artwork
  keeps its size, the level glyph stands still and lyrics jump. Flyouts and
  dialogs keep WinUI's own transitions.
* **Settings → Appearance:** Theme (System, Light and Dark with previews),
  Accent Colour (Follow the Artwork, ten swatches and a Custom hue on a
  hue-gradient track with a preview), Transparency from Solid to Clear with a
  live preview and Reduce Motion and Transparency, Now Playing Backdrop
  (Artwork, Misty, with previews), Lyrics (Lyrics Size, Lyrics Alignment, Show
  Translations, Show Romanization, Show Lyrics Bar, Desktop Lyrics: the only
  place they live), App Icon and **Customize…** ("Corners, glass, type, lyrics
  style and layout.", with a Pro badge while Pro is inactive), which opens
  Settings › Extensions. Each change is one `SetSettings` and applies at once.
* **Transparency on Acrylic:** the glass brushes' `TintOpacity` comes from
  the shared glass-alpha port (`platforms/shared/csharp/.../Glass.cs`): the
  tokens' alphas at 50 %, opaque toward Solid, halved toward Clear; at 0 %,
  or with Windows' transparency effects off, the glass is solid Cloud.

**Checked here, not built:** the portable tests pass (86, including the
design tests); the app's C# compiles against the Windows App SDK 2.6.5 and
Windows SDK projection reference assemblies with stand-ins for the XAML
compiler's classes, and every XAML file passes the metadata check (0 errors,
0 warnings, 214 `x:Uid` entries). The XAML compiler, the composition and
connected animations, Acrylic's look at each transparency and everything
else at run time first run on Windows in CI.

## One app with the others (docs/ui.md)

The Windows app follows the shared specification in [`ui.md`](ui.md): its
words in Title Case (titles, buttons, menu items, switches, tabs and group
headings; descriptions and messages stay sentences), its pages, its Settings
order and the steps that ask first. Where Windows keeps its own mechanics, it
is the navigation pane, cards in Settings, ContentDialogs for questions and
plugin settings applied with **Save**.

* **Home:** the library's size ("1,204 songs · 96 albums · 41 artists"), then
  Recently Added (albums, newest first from the core's `AlbumsSorted`),
  Recently Played and Most Played (songs). With nothing in the library and no
  source signed in, "Welcome to Jellyfesh" with **Add Music Folder…** (it adds
  the folder directly) and **Add a Source** (the Sources page).
* **Library pages:** Albums is a grid and Artists a list; Songs has **Play**
  and **Shuffle**, which play every song in the page's order
  (`Collection::Songs { sort, descending }`, nothing paged in first), and a
  song chosen plays them all from it. Albums and Songs sort from a menu (Title, Artist, Year,
  Recently Added; Songs also Album) with **Descending**; each order starts in
  its own direction (newest first for Year and Recently Added). Empty pages
  say "No Albums Yet", "No Artists Yet" or "No Songs Yet" with **Add Music
  Folder…**; Playlists says "No Playlists Yet" with **New Playlist…**,
  Favourites "No Favourites Yet".
* **Album, artist and playlist pages:** Play, Shuffle and a More menu: Play
  Next, Play Last, Add to Playlist… (New Playlist… or a playlist); a playlist
  also has Rename…, Export Playlist… and Delete Playlist…, which asks first and
  then goes back.
* **Song menu:** Play Next, Play Last, Add to Playlist…, Add to Favourites or
  Remove from Favourites, Go to Album, Go to Artist and Show in Explorer
  (library songs; the core's search finds the album and artist).
* **Search:** "Search Your Music" and a sentence before anything is typed;
  library results first (Songs, Albums, Artists, as rows), then **From Your
  Sources**, one group per source ("Content from {Service}" under one that asks
  for credit), an item the core merged saying "Also in …"; a source's album,
  artist, playlist or folder opens its page; "No Results" and "Try other
  words." when nothing is found.
* **Sources:** the intro line, **Add a Source** (the core's presets; a service
  opens "Connect to …" for the Server Address and how to sign in, then the
  sign-in dialog), **Your Sources** (every installed source with its badge
  and "Signed in as …", "Not signed in" or "Off"; choosing one that is on
  opens its catalogue) and **Sources Settings…**, which opens Settings ›
  Sources.
* **A source's pages** (`SourceBrowsePage`, after the web's): the top level or
  a folder lists what it holds in the source's order, folders, albums and
  songs mixed; an album or a playlist has the large header (artwork, kind,
  title, the artist in accent text, the song count, **Play**, **Shuffle**,
  More with Play Next, Play Last and Open in {Service}), its songs (numbered on
  an album) and the "12 songs, 48 minutes" footer; an artist plays its albums.
  Choosing an album, artist, playlist or folder opens its page (Back returns);
  a song plays with the rest of its album or playlist after it. **Load More**
  while the source has more; "This Source Did Not Answer" with its message;
  "Nothing Here"; Open in {Service} as the main action when nothing plays here;
  "Content from {Service}" for a source that asks for credit.
* **Now Playing:** Queue has "Up Next", **Clear Queue** (it asks first) and
  "Nothing Up Next"; Info is "About This Song" with Title, Artist, Album, Year
  (library songs), Length, Quality (the core's label for what is decoded:
  "Hi-Res Lossless", "Lossless", "High", "Standard"), Format (what is decoded,
  "FLAC · 24-bit · 96 kHz · Stereo · 3100 kbps", once the engine opened the
  track), Lyrics (timing and file, the plugin, or "Made by AI Lyrics ·
  {engine}"; "None found") and File, and **Get Info**.
* **Get Info** (the song menu, the album, artist and source pages' More menus,
  the Info pane): a dialog that opens with what the row knows and fills in
  when the core's `Details` answer comes (matched by request id): About This
  Song (Title, Artist, Album, Version, Year, Genre, Composer, Lyricist, Label,
  Length) and Audio (Quality, Format, Bitrate, Size, ReplayGain, File or
  Source), rows without a value left out; for albums and artists the notes and
  the facts in the core's order, the service's own labels for others, and
  "Content from {Service}" when it asks for credit (`DetailsRows`,
  `DetailsViewModel`, `Shell/InfoDialog.cs`).
* **Settings:** Appearance, Playback, Library, Sources, Extensions and About; no separate
  Lyrics section. Playback: Output (Output Device), Crossfade (Crossfade,
  Length, Album Continuity), Volume Levelling (ReplayGain, Preamp), Equalizer
  (Equalizer, Preset from the core's `EqPresets()` with Custom once a band
  moves, Preamp, the bands as the core's settings have them, **Reset to
  Flat**) and Startup (Resume Playback). Library: Music Folders (each with
  Show in Explorer and **Remove Folder…**, which asks first; then **Add
  Folder…**), Watch Folders and Rescan Library with the scan's progress and
  Last Scan. Sources: Add a Source, Installed (**Install from File…** in its
  heading; each source's badge, status, Enabled, a sign-in button for each
  method it offers or Sign Out, its settings, "Version · Author" and
  **Uninstall…**, which asks first; "No Sources Installed" when empty) and
  Community Plugins (Allow Community Plugins shows the disclosure first).
  Store builds have no Install from File… and no Community Plugins
  (`SourcesState.CanInstallPlugins`). About: the version from the build,
  Third-Party Licences (the notices file next to the app), the log folder,
  Licence Agreement and Privacy Policy (`<website>/legal/…`), and where the
  `Update` slice is supported (direct downloads) **Check for Updates**, **Check
  for Updates Automatically** and "Jellyfesh {version} Is Available" with
  **View Release**.
* **Settings › Extensions** (v0.0.7): Jellyfesh Pro (the Pro slice in words:
  "Pro features are free during the public beta.", "Jellyfesh Pro ·
  Purchased", "Jellyfesh Pro · Licence …ABCD" with **Remove Licence…**, which
  asks first, or "AI Lyrics and Customize are part of Jellyfesh Pro."; **Buy
  Jellyfesh Pro – {price}** and **Restore Purchases** where the build's store
  sells Pro; **Enter Licence Key…** where `licence_keys`; Licence Agreement,
  Privacy Policy, Terms of Sale), then each extension (plugins of kind
  `Extension`) as a card: its Pro badge while Pro is inactive, Enabled, AI
  Lyrics' API key ("API Key Added", Sign In or Sign Out), its settings in the
  app's language (`LocalizedSettingsSchema` with the strings' own language
  tag) by group with **Save**, Customize's **Reset to Defaults**, AI Lyrics'
  **On-Device Models** ("Name · Size", Download, "Downloading… 35%" with a
  bar and Cancel, "Ready" with **Delete…**, which asks first) and **Sending
  Audio** (each allowed origin with **Stop Allowing**), "Version · Author"
  and Uninstall…; **Install from File…** in the heading. Settings forms read
  schema profile 1.2: sliders for numbers and integers with both bounds
  (`multipleOf` the step, 1 for integers; ADR-0020 §3), colour wells (`format: color`, a colour picker), ordered checklists
  (ticked items in order, Move Up and Move Down), `oneOf` choices with their
  titles and `x-jf-group` headings.
* **The Pro sheet** opens from any Pro action while Pro is inactive: "Jellyfesh
  Pro", what Pro brings, Buy and Restore Purchases (store builds), Enter
  Licence Key…, Not Now, and the legal links. **Enter Licence Key** stays open
  until the core accepted the key and words its refusal ("That doesn't look
  like a Jellyfesh licence key.", "This licence key isn't valid.", "This key
  is for another product.", "This licence key has been withdrawn."), or shows
  the core's own message. The Microsoft Store build's glue
  (`Platform/StorePurchases.cs`, `JF_DISTRIBUTION_MICROSOFT_STORE` only) uses
  `Windows.Services.Store.StoreContext` with the window (`InitializeWithWindow`),
  finds the durable add-on whose product ID is `org.jellyfesh.pro`
  (`GetAssociatedStoreProductsAsync`, its formatted price), buys it with
  `RequestPurchaseAsync`, and reports its licence (`AddOnLicenses`) to the core
  with `SetStoreEntitlement` at start, after a purchase or Restore, and on
  `OfflineLicensesChanged`.

**Checked here, not built (this alignment):** the portable tests pass (105);
the view models carry the behaviour above and are tested with the fake core
(`tests/.../UiSpecTests.cs`). The app's C# compiles against the Windows App SDK
2.6.5 reference assemblies with generated stand-ins for the XAML compiler's
members, every XAML event handler against its event's delegate and every
`x:Bind` (436) as a typed assignment; a script checked that every `x:Uid` has
resources in the four languages, every resource an `x:Uid`, and every resource
key a definition. The XAML compiler itself first runs on Windows in CI.

## Layout (`platforms/windows/`)

| Path | What it is |
| --- | --- |
| `Jellyfesh.sln` | The four projects below; the app in x64 and ARM64 configurations |
| `Jellyfesh.Portable.slnf` | The same without the app, for `dotnet build` and `dotnet test` on Linux and macOS |
| `src/Jellyfesh.Shared/` | `net10.0`, no WinUI types: the core client seam (`ICoreClient`) and the shell's mirror of the core (`CoreHub`), view models (CommunityToolkit.Mvvm), formatting, drawing-time scheduling (`PlaybackTicker`, progress and lyrics presenters), settings forms from JSON Schema, the QR encoder, the app icon set |
| `src/Jellyfesh.Interop/` | `net10.0`: the only project that names `Jellyfesh.Ffi` types. `FfiCoreClient` implements `ICoreClient`; `PortDrawingTime` wraps the shared `DisplayClock` and lyrics-cursor ports (`platforms/shared/csharp/`) |
| `src/Jellyfesh.App/` | WinUI 3, `net10.0-windows10.0.19041.0`: views and platform adapters only (ADR-0001) |
| `tests/Jellyfesh.Shared.Tests/` | xUnit v3 with a fake core: view models, the hub, drawing time, the FFI mapping against `tests/contract/`, formatting, the QR encoder, the string tables |
| `scripts/build.ps1` | The CI build: native libraries, bindings, tokens, publish, zips |
| `scripts/make_icons.py` | Renders the icons from `design/icons/round4/` (committed output) |
| `scripts/fallback_theme.py` | Writes the hue-250 fallback theme from the core's colour tables |
| `Generated/` (git-ignored) | `Jellyfesh.Ffi/`, `native/jf_ffi.dll`, `Tokens.cs`, written by `cargo xtask` |

Inside the app: `App.xaml.cs` and `Program.cs` (single instance, start-up),
`MainWindow` (title bar, navigation, the Misty field, the mini player capsule
or the player bar, Now Playing, the compact mini player, notices, keys),
`Views/` (pages, the player bar, Now Playing, lyrics, queue, settings,
sign-in), `Controls/` (seek bar, lyric line, play/pause and level glyphs,
album tile, track number cell, theme previews, composition motion helpers,
QR code),
`Theming/` (theme brushes from the core's Theme slice, `{th:Token}`, Mica and
the glass fallbacks), `Platform/` (System Media Transport Controls, taskbar
buttons, window and shortcut icons, pickers, device notifications),
`Styles/Controls.xaml` (control style D, the only place control shapes live),
`Styles/Templates.xaml` (row and tile templates), `Strings/<lang>/Resources.resw`.

## Build and run

**Everything but the app (any OS):**

```bash
dotnet test --solution platforms/windows/Jellyfesh.Portable.slnf
```

Before `cargo xtask bindings --lang csharp` has run, `Jellyfesh.Interop`
compiles the binding from `core/jf-ffi/csharp/` directly, so this needs no
Rust build.

**The app (Windows 10 1809+ with the .NET 10 SDK, Rust with the MSVC
build tools, and CMake for libopus):**

```powershell
pwsh platforms/windows/scripts/build.ps1                  # both architectures, Release
pwsh platforms/windows/scripts/build.ps1 -Architectures x64 -Configuration Debug
pwsh platforms/windows/scripts/build.ps1 -SkipNative       # app only, Generated/ as it is
pwsh platforms/windows/scripts/build.ps1 -Msix             # also an unsigned MSIX (for later)
```

The script runs `cargo xtask bindings --lang csharp --release` and
`cargo xtask tokens --target csharp`, cross-builds `jf-ffi` for the other
architecture (`x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`), publishes
`Jellyfesh.App` unpackaged and self-contained for `win-x64` and `win-arm64`
(the Windows App SDK runtime and .NET are in the folder, so nothing needs
installing, ADR-0016), and writes
`platforms/windows/artifacts/jellyfesh-<version>-windows-<arch>.zip` with a
`.sha256`. The version comes from the `v*` tag in `GITHUB_REF_NAME`, else from
`Directory.Build.props`. `CARGO_TARGET_DIR` is respected.

**The installer** that releases carry, `Jellyfesh-<version>-Windows-Setup.exe`, is
made from the two published folders (`platforms/windows/publish/win-<arch>/Jellyfesh`)
by `scripts/installer.ps1` with Inno Setup 6.3 or later (`installer/Jellyfesh.iss`;
the `windows-2025` runner image has Inno Setup 6.7):

```powershell
pwsh platforms/windows/scripts/build.ps1                  # both architectures first
pwsh platforms/windows/scripts/installer.ps1              # artifacts/Jellyfesh-<version>-Windows-Setup.exe
```

One Setup serves x64 and Arm64 PCs and installs the build for the PC's own
processor, for the current user without administrator rights, in
`%LOCALAPPDATA%\Programs\Jellyfesh`, with a Start menu shortcut and an
uninstaller in Settings › Apps. It installs over any earlier version (the
folder is emptied first; the library and settings live elsewhere and stay).
With `-SigningCertificate`, Setup and its uninstaller are signed;
`build.ps1 -SigningCertificate` signs Jellyfesh's own files inside first.

To run from Visual Studio 2022 (17.14+ with the WinUI workload) or
`dotnet build`, generate first, then pick x64 or ARM64:

```powershell
cargo xtask bindings --lang csharp
cargo xtask tokens --target csharp
dotnet build platforms/windows/src/Jellyfesh.App -p:Platform=x64
```

**Official plugins.** The Subsonic and Jellyfin plugins ship inside every
app (releases no longer carry them separately; the commercial edition of the
release workflow leaves out the unofficial community plugins). The project copies `dist/plugins/*.jfpkg`
(made by `plugins/release.sh`, or the release workflow's `plugins` artifact)
into the app's `Plugins` folder; `-p:JfBundledPluginsDir=<folder>` points
elsewhere. Without them the script warns and the app starts with no sources.
At start the app passes their paths to the core (`CoreConfig.BundledPlugins`),
which installs them on first start and updates them with the app. Until the
binding has that field, the build leaves it out (`Jellyfesh.Interop.csproj`
reads the binding's `Types.cs` to decide).

**CI.** A job on `windows-2025` runs `scripts/build.ps1` after the `plugins`
job and runs `dotnet test
platforms/windows/Jellyfesh.Portable.slnf`; another, "Windows app starts",
builds for x64 and runs the smoke test below. The library and session live in
`%APPDATA%\Jellyfesh`, artwork and backdrops in `%LOCALAPPDATA%\Jellyfesh\cache`
(the core's defaults; `JF_DATA_DIR` and `JF_CACHE_DIR` override them); the
app's own log is `%LOCALAPPDATA%\Jellyfesh\logs\app.log` (`JF_LOG_DIR` moves it).
`build.ps1` stops when a published folder lacks a file the app cannot start
without (`Jellyfesh.exe`, `Jellyfesh.dll`, `jf_ffi.dll`, the Windows App SDK
runtime, WinUI, `Microsoft.UI.Xaml.Controls.pri` and the app's own resource index)
and lists the folder's `.pri` files. An unpackaged build names its index after the
executable, `Jellyfesh.pri` (its compiled XAML and strings); MRT Core looks for
`resources.pri`, then `<executable>.pri`, next to the executable, so either serves.

## When Jellyfesh does not start

**The log.** Every start writes `%LOCALAPPDATA%\Jellyfesh\logs\app.log` (or the
temporary folder's `Jellyfesh\logs` when that cannot be written): the version,
edition and distribution, the app's path, Windows version, architecture and
language, the data and cache folders, then each stage of start-up (loading the
Windows App SDK runtime, the single-instance check, XAML, creating the
application, WinUI loading App.xaml, the colours and styles, the core, the main
window). The colours and styles go into `Application.Resources` in
`OnLaunched`: WinUI makes the `App` object in `Application.Start`'s callback,
before it creates the thread's application resources and loads App.xaml, so
`Resources` fails there with E_UNEXPECTED (0x8000FFFF). Until the window is open it also records every
first-chance exception, so the first error of a failed start is there even when
something later catches it. Lines carry the time and the process id. **Open Log
Folder** is in Settings › About; a person reporting a problem sends `app.log`.

**A failed start is shown, never silent.** A failure before the main window is
open is logged in full and shown: a small window ("Jellyfesh Could Not Start",
the stage and the error, the log file's path and **Open Log Folder**), or a
message box with the same when XAML itself could not start. Jellyfesh then
exits. It never stays running without a window, which would also swallow every
later start through the single-instance redirect. Other safety nets:

* The Windows App SDK's self-contained start (what its UndockedRegFreeWinRT
  auto-initializer does in a module initializer) runs at the top of `Main`
  instead (`WindowsAppSdkUndockedRegFreeWinRTInitialize` is off), so a runtime
  that cannot load is reported rather than ending the process before any of
  Jellyfesh's code runs.
* Optional Windows parts (the title bar, Mica, the window and taskbar icons,
  the taskbar buttons, the system media controls, audio device notifications,
  the caption buttons' colours) are each guarded: one that fails on a PC is
  logged once and the window opens without it.
* A second start waits at most 20 s for the running Jellyfesh to take it, then
  says that Jellyfesh is running but not answering. Closing the window releases
  the single-instance key first (as the Windows App SDK recommends), so a new
  start opens a new window instead of handing itself to a process that is
  shutting down; and a shutdown that takes longer than 2 minutes (the core waits
  up to one for plugin installs under way) ends the process.
* Exceptions on other threads and unobserved task exceptions are logged.
  Native crashes (outside .NET) appear in Windows' Event Viewer, Windows Logs ›
  Application, as Application Error and .NET Runtime entries.

**`Jellyfesh.exe --smoke-test`** starts the core and the main window, waits for
its first frames, opens every section (Home, Albums, Artists, Songs, Playlists,
Favourites, Search, Sources, Settings), each loaded and drawn, closes the
window (which shuts the core down) and exits. It registers its own
single-instance key, so a running Jellyfesh never takes it, and shows nothing:

| Exit code | Meaning |
| --- | --- |
| 0 | Every page opened |
| 1 | Start-up failed, or anything failed on the way (the log says where) |
| 2 | It timed out (3 minutes to open every page) |
| 3 | Another Jellyfesh took the start, or did not answer |
| 4 | Shutting down did not end in 2 minutes |

`scripts/smoke-test.ps1` runs it twice: from the publish folder (its own data,
cache and log folders, or with `-DefaultFolders` the user's, as in CI), and from
a copy in a folder whose path has spaces and non-ASCII characters, with its
folders on such paths too, as for a Windows user name that is not ASCII. It
prints the log after each run and the event log's entries about Jellyfesh after
a failed one, and keeps the logs in `<WorkDir>\logs`; `-InPlaceOnly` runs only
the first (the release workflow uses it for the app Setup installed):

```powershell
pwsh platforms/windows/scripts/build.ps1 -Architectures x64
pwsh platforms/windows/scripts/smoke-test.ps1
```

## How it works

* **One seam.** Only `Jellyfesh.Interop` names binding types; everything else
  uses the model in `Jellyfesh.Shared/Core/`, so a renamed binding member is
  fixed in `FfiMapping.cs` alone. Queries run off the UI thread; a feature the
  core answers with `NotAvailable` shows "coming in a later build", not an
  error.
* **Events.** `CoreHub` keeps the newest copy of each slice from the core's
  event thread and applies them on the UI thread once per dispatcher turn, so a
  scan reporting thousands of files costs one update per frame at most.
* **Drawing time.** Anchors feed the ported `DisplayClock`; `PlaybackTicker`
  redraws only at the next visible change (the next second, the next pixel of
  the seek bar or a progress line, the next lyric line or word) and sleeps
  between. While paused, minimised or with Now Playing closed nothing is
  scheduled (Still mode). The word being sung is swept by a compositor
  animation over its remaining time, so no app code runs per frame even then.
  The window's monotonic clock is `Stopwatch` with an offset measured once
  against `MonotonicNowNs()` (both count QueryPerformanceCounter ticks).
* **Lists.** Albums and playlists are `GridView`s with fixed tile sizes,
  artists, songs and favourites `ListView`s; every row is a placeholder until its
  page of the core's query arrives, and thumbnails are requested at the drawn
  size, two at a time, off the UI thread, and cancelled when a row scrolls away.
* **The lyrics bar** (Show Lyrics Bar, on by default): in the player bar the
  line being sung takes the artist's place under the title, with its
  translation when Show Translations is on. `LyricsBarPresenter` follows the
  core's `lyrics_bar` rule through the ported `LyricsBars.At` (the active
  line, or in a gap the last one that started; the next line for Desktop
  Lyrics): a word-timed line fills word by word in the accent (the artwork's
  highlight) with the same compositor sweep as Now Playing (`SweptWord`), told
  again only when the word, the line or the anchor changes.
* **Desktop Lyrics** (Settings › Appearance › Lyrics, or the player's More
  menu): a window of its own on acrylic at the bottom of the screen, always on
  top and out of Alt+Tab (`OverlappedPresenter.CreateForToolWindow`,
  `IsAlwaysOnTop`, `SetBorderAndTitleBar(false, false)`,
  `AppWindow.IsShownInSwitchers`), with the current line (30 px) and the next
  (dimmed). It is dragged by its glass; its tools (shown under the pointer)
  are Lock, which lets clicks pass through it (a layered window with
  `WS_EX_TRANSPARENT`, fully opaque through `SetLayeredWindowAttributes`), and
  Close, which turns the setting off. More offers **Unlock Desktop Lyrics**
  while it is locked.
* **Discover** (`DiscoverViewModel`, `SourceDiscoverPage`): choosing a source on
  the Sources page opens its Discover page: the source's name and status with
  **Browse** (its own folders) in the header, then the shelves of the core's
  `Discover` answer (matched by request id): row shelves scroll sideways as
  covers, grid shelves wrap, chart shelves are numbered lists, each with **See
  All** when it has more (browsing its node). Shelves the core makes are worded
  by their id (For You, Recently Added, Most Played, Rediscover). While
  loading, placeholders the shape of the shelves; on failure, the core's words
  and **Try Again**. An album, artist or playlist opens its page; a song plays
  with its shelf's songs.
* **Artist and album pages for source songs:** a source's song with the
  source's artist and album ids (`SourceItem.artist_id`, `album_id`) offers Go
  to Artist and Go to Album in its menu; they open the source page of that
  artist or album (`Browse` of the id), which asks for its `Details` when the
  source offers them: the picture for the header and the notes or biography
  (three lines, **More** shows it whole), and Get Info in More.
* **The look** (ADR-0020, the `Look` slice, applied live by `LookRules`): the
  navigation pane shows the slice's places in its order (a separator before
  Search and Sources; Settings always); Home shows the sections it lists, in
  its order; text uses the platform's face for the typeface (Segoe UI
  Variable, Georgia for serif, Cascadia Mono for mono; Windows has no rounded
  face, so Rounded keeps the system's); the aurora field fades with a weaker
  aurora (never brighter than the default); glass is drawn at the
  Transparency setting times the look's glass, so a look without glass is
  solid Cloud and Reduce Transparency still wins (WinUI's acrylic has no blur
  amount to scale). Lyrics: the lines take the look's weight (Regular 400 to
  Heavy 800; Bold by default); the current line and its sung words light up in
  the highlight (ink-90 for Artwork, as before; the accent text colour; or the
  plugin's colour), and the lyrics bar and Desktop Lyrics, which light words in
  the accent, take a colour of the look's own; in a customized look the
  current line glows in that colour (`TextGlow`: a composition drop shadow
  shaped by the text's alpha mask, behind the text; for word-timed lines it
  sits inside each word's sweep clip, so it follows the sweep). Now Playing on
  windows of 760 epx and more: Stage is the v0.0.6 layout; Artwork shows a
  large cover (520 epx, 440 on medium windows) with the line being sung under
  it in place of the Lyrics pane (Queue, Info and Comments keep the stage);
  Lyrics lays the chosen pane across the window under a row with a 72 epx
  cover and the titles; narrower windows keep one view at a time. The
  mini-player: Bar is the player bar from 900 epx and the capsule below it
  (v0.0.6); Compact is the capsule (cover, title, play) stretched into a bar
  as wide as the player bar; Floating is the capsule at every width. Not
  applied on Windows: corner scale, text scale and density, because WinUI
  reads the Misty tokens (`{th:Token}`) once, when a style or view loads, and
  most spacing in the app's XAML is not a token; applying them live needs the
  styles to take theme resources that can be swapped, which is left for later.
* **Comments** (`CommentsViewModel`): for songs from sources whose
  `PluginSummary.comments` is true, Now Playing has a fourth pane: "Comments
  from {Service}", "Popular" (the first page's hot comments) then "Latest",
  each with the author's picture (through the core), name, age ("3 days ago")
  and likes, and the text; more load near the end of the list; "No Comments
  Yet" without any. Read-only, asked by request id when the pane opens.
* **AI Lyrics** (`AiLyricsViewModel`, from the `AiLyrics` slice): the Lyrics
  pane offers **Create Synced Lyrics** under "No Lyrics" and **Time These
  Lyrics** under plain lyrics while AI Lyrics is offered (or Pro is inactive,
  when it opens the Pro sheet); the song's job shows its stage ("Reading
  Audio…", "Sending to {host}…", "Recognising… 42%", "Timing Words…") with a
  bar and **Cancel**; what is needed first shows in words: a model to
  download (**Download**, then "Downloading {Model}… 35%" with **Cancel**), an
  API key (**Open AI Lyrics Settings**, Settings › Extensions), or why it
  cannot work; a failed job says "Couldn't make lyrics: {message}" with **Try
  Again**. The first upload to an origin asks "Send This Song to {origin}?"
  (**Allow** sends `AllowAudioUpload`, then `MakeLyrics`). Lyrics AI Lyrics
  made say "Made by AI Lyrics · {engine}" under the last line, and More adds
  **Make Again** and **Delete Made Lyrics** (asks first). The song menu offers
  **Create Synced Lyrics** after Go to Artist for library songs.
* **Covers from sources** ([`core-api.md`](core-api.md), "Covers"): the app
  never fetches a picture itself. Rows, page headers, Now Playing (at 1024 px)
  and the system media controls ask `SourceArtwork(url, size)` through
  `ThumbnailService.GetSourceAsync`: a cached file loads at once; otherwise the
  placeholder stays until the core's `Artwork` event brings the file (at most a
  minute). A picture the core cannot have is not asked for again for ten
  minutes. Items a source lists without a picture carry a
  `jellyfesh-artwork://` reference, which goes the same way (the core asks the
  plugin); no URL is ever loaded by the app itself.
* **Look.** Colours come from the core's Theme slice into shared brushes
  (Misty PLT-03), crossfaded over 600 ms on a new hue; sizes from the
  generated tokens through `{th:Token}`. Mica on Windows 11, the solid field
  elsewhere; glass (in-app acrylic tinted Clear, Mist or Veil, its tint
  opacity set by the Transparency setting) turns into solid Cloud when
  transparency is off, in Battery Saver, high contrast, with Reduce motion and
  transparency or at a Transparency of 0 %. Now Playing has both styles: the
  core's baked backdrop (Artwork) or the Misty field with the artwork on a
  Clear frame (Misty).
* **Windows parts.** System Media Transport Controls through
  `SystemMediaTransportControlsInterop.GetForWindow`; three taskbar thumbnail
  buttons (`ITaskbarList3`, clicks through a subclassed window procedure); the
  window and taskbar icon by `WM_SETICON`, and the Start menu and desktop
  shortcuts that point at this executable get the chosen icon (ADR-0017;
  icons other than Ice and Steel are packed into an `.ico` from their PNGs on
  first use, in `%LOCALAPPDATA%\Jellyfesh\Icons`); `RefreshDevices` on audio
  device notifications (ADR-0009); `SetNetworkState` at start and on each
  network change (`NetworkInformation`: online is internet access, metered is
  a fixed or variable cost, roaming or a data limit near or over;
  ADR-0026 §1); one instance per user.
* **Strings.** English, Simplified Chinese, Traditional Chinese and Japanese
  in `.resw` (x:Uid for XAML text, `ILocalizer` keys for messages); a test
  keeps the four tables and the code's keys in step.

## Keys

The keys every Jellyfesh app shares ([`ui.md`](ui.md), "Keyboard"):

| Key | Action |
| --- | --- |
| Space | Play or pause |
| Ctrl+Right / Ctrl+Left | Next / previous song |
| Shift+Right / Shift+Left | Skip 5 seconds forward / back |
| Ctrl+Up / Ctrl+Down | Volume up / down |
| Ctrl+F | Search |
| Esc | Back (closes Now Playing first) |

None of them acts while a text box has focus, in the window or in a dialog.
The arrow keys are read before the focused control (`PreviewKeyDown` on the
window's root), so a list or slider with focus does not take them; Space
leaves a focused button or list its own Space. The Groove and Windows Media
Player keys that reuse none of these stay:

| Key | Action |
| --- | --- |
| Ctrl+P | Play or pause |
| Ctrl+B | Previous |
| Ctrl+E | Search |
| Ctrl+H / Ctrl+T | Shuffle / repeat |
| F8 / F9 / F10 | Mute / volume down / volume up |
| Ctrl+L | Open or close Now Playing |
| Ctrl+M | The mini player (above other windows while Keep on Top is on) and back |
| Alt+Left, mouse back | Back |
| Alt+Up / Alt+Down, Delete | Move or remove the selected queue or playlist row |
| Left / Right on the seek bar | Seek 5 s (Page Up / Page Down: 30 s; Home / End: start / end) |

Media keys reach the app through the media transport controls.

## Verified here, and not

The `net10.0` projects build with warnings as errors and their tests pass on
Linux. WinUI cannot build on Linux, so the app's C# was compiled against the
Windows App SDK 2.6.5 and Windows SDK projection reference assemblies with
stand-ins for the XAML compiler's partial classes, and every XAML file was
checked against the WinUI metadata and the compiled view models (element
types, properties, attached properties, events and their handlers, enum
values, resource keys, `x:Bind` paths and two-way types, content models).
The XAML compiler itself, MRT Core resource loading, and everything at run
time (Mica, SMTC, taskbar buttons, icons, NativeAOT spike S8) first run on
Windows in CI.

Every project targets .NET 10 (LTS, ADR-0007); .NET 8 reached end of support
on 2026-11-10.
