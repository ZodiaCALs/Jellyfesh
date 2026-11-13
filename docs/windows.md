# The Windows app (v0.0.4)

WinUI 3 (Windows App SDK 2.6.5) in C# on .NET 10 (LTS), over the Rust core through
the hand-written `Jellyfesh.Ffi` binding ([`ffi.md`](ffi.md)). Windows 10 1809
or later; Mica on Windows 11. Everything except XAML and WinUI builds and is
tested on any OS.

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
  Lyrics, Queue and the volume. The compact mini player (Ctrl+M) has the same
  straight line along the top of its glass.
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
* **Settings → Appearance:** Theme (System, Light and Dark with previews), the
  accent colour (Follow the artwork, ten swatches and a Custom hue on a
  hue-gradient track with a preview), Transparency from Solid to Clear with a
  live preview, Reduce motion and transparency, the Now Playing background
  with previews, Lyrics size and alignment, translation and romanization,
  and the app icon. Each change is one `SetSettings` and applies at once.
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

To run from Visual Studio 2022 (17.14+ with the WinUI workload) or
`dotnet build`, generate first, then pick x64 or ARM64:

```powershell
cargo xtask bindings --lang csharp
cargo xtask tokens --target csharp
dotnet build platforms/windows/src/Jellyfesh.App -p:Platform=x64
```

**Official plugins.** The Subsonic and Jellyfin plugins ship inside every
app and as separate downloads. The project copies `dist/plugins/*.jfpkg`
(made by `plugins/release.sh`, or the release workflow's `plugins` artifact)
into the app's `Plugins` folder; `-p:JfBundledPluginsDir=<folder>` points
elsewhere. Without them the script warns and the app starts with no sources.
At start the app passes their paths to the core (`CoreConfig.BundledPlugins`),
which installs them on first start and updates them with the app. Until the
binding has that field, the build leaves it out (`Jellyfesh.Interop.csproj`
reads the binding's `Types.cs` to decide).

**CI.** A job on `windows-2025` runs `scripts/build.ps1` after the `plugins`
job, uploads the two zips, and runs `dotnet test
platforms/windows/Jellyfesh.Portable.slnf`. The library and session live in
`%APPDATA%\Jellyfesh`, artwork and backdrops in `%LOCALAPPDATA%\Jellyfesh\cache`
(the core's defaults; `JF_DATA_DIR` and `JF_CACHE_DIR` override them); the
app's own log is `%LOCALAPPDATA%\Jellyfesh\logs\app.log`.

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
* **Lists.** Albums and artists are `GridView`s with fixed tile sizes, songs,
  playlists and favourites `ListView`s; every row is a placeholder until its
  page of the core's query arrives, and thumbnails are requested at the drawn
  size, two at a time, off the UI thread, and cancelled when a row scrolls away.
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
  device notifications (ADR-0009); one instance per user.
* **Strings.** English, Simplified Chinese, Traditional Chinese and Japanese
  in `.resw` (x:Uid for XAML text, `ILocalizer` keys for messages); a test
  keeps the four tables and the code's keys in step.

## Keys

| Key | Action |
| --- | --- |
| Ctrl+P | Play or pause |
| Ctrl+F / Ctrl+B | Next / previous |
| Ctrl+H / Ctrl+T | Shuffle / repeat |
| F8 / F9 / F10 | Mute / volume down / volume up |
| Ctrl+L | Open or close Now Playing |
| Ctrl+M | The compact mini player (always on top) and back |
| Ctrl+E | Search |
| Esc, Alt+Left, mouse back | Back (closes Now Playing first) |
| Alt+Up / Alt+Down, Delete | Move or remove the selected queue or playlist row |
| Left / Right on the seek bar | Seek 5 s (Page Up / Page Down: 30 s; Home / End: start / end) |

They follow Groove and Windows Media Player, so none of them is taken from a
text box. Media keys reach the app through the media transport controls.

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
