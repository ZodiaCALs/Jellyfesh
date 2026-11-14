# The macOS app (v0.0.4)

SwiftUI on macOS 14.2 and later, over the Rust core through the UniFFI bindings
([`ffi.md`](ffi.md)). The app holds no business logic (ADR-0001): it shows the
core's slices and sends commands; sorting, filtering, matching, persistence and
networking stay in the core.

**Status: written without a Swift toolchain.** Nothing in `platforms/macos` has
been compiled yet; the first CI build on a `macos-15` runner is its first
compile. The names it assumes from the generated bindings are listed below so
they can be checked quickly.

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
   linker (`JF_RUST_LDFLAGS`);
3. the official plugins `dist/plugins/*.jfpkg` (or `$JF_PLUGINS_DIR`) copied to
   `Generated/Plugins/`, which the app carries as `Resources/Plugins/`; without
   them it says so and builds without bundled plugins;
4. `xcodegen generate` from `project.yml`;
5. `xcodebuild` of an unsigned (ad-hoc signed, ADR-0016) universal Release app;
6. `build/Jellyfesh-<version>.dmg` with a link to `/Applications`, and its SHA-256.

Output goes to `platforms/macos/build/` (`JF_MACOS_OUT` to change it).
`CARGO_TARGET_DIR` is honoured.

### In CI

A job for the release workflow (not added here; this change only touches
`platforms/macos`):

```yaml
macos-app:
  name: macOS app (DMG)
  needs: plugins
  runs-on: macos-15
  steps:
    - uses: actions/checkout@v7
    - uses: dtolnay/rust-toolchain@1.97.0
      with:
        targets: aarch64-apple-darwin, x86_64-apple-darwin
    - uses: actions/download-artifact@v8
      with:
        name: plugins
        path: dist/plugins
    - run: platforms/macos/scripts/build.sh --test
    - uses: actions/upload-artifact@v7
      with:
        name: release-macos
        path: platforms/macos/build/Jellyfesh-*.dmg*
```

### In Xcode

Run `build.sh` once (it makes `Generated/`), then `xcodegen generate` in
`platforms/macos` and open `Jellyfesh.xcodeproj`. The scheme `Jellyfesh` runs
the app and the unit tests.

### First launch

The app is ad-hoc signed and not notarised (ADR-0016): macOS blocks the first
launch; allow it in System Settings → Privacy & Security → Open Anyway. The app
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
│       │                   AppDelegate (launch, quit, open files, Dock menu, About), AppCommands (menus)
│       ├── CoreClient/     the only code that imports JellyfeshFFI: CoreClient protocol, the app's
│       │                   own models, FFI conversions, FFICoreClient, OfflineCoreClient, PlaybackClock
│       ├── Stores/         @Observable stores per slice: playback, queue, library, settings, theme,
│       │                   lyrics, search, sources (+ browse, sign-in), paged lists, router, toasts
│       ├── Design/         palette, type and motion from the tokens, glass and the
│       │                   transparency setting, Misty field, controls, TimelineView schedules
│       ├── Platform/       media session, Dock icon, device watcher, Low Power Mode and window
│       │                   visibility, Space to play
│       ├── Support/        artwork loader, JSON with ordered keys, schema forms, lyric layout,
│       │                   QR codes, file panels, design math (seek bar, motion, swatches)
│       └── Views/          root, sidebar, library pages, Now Playing, search, sources, settings
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
* **Window.** `NavigationSplitView` with Home, Albums (lazy grid), Artists,
  Songs, Playlists, Favourites, Search, Sources and the person's playlists;
  album, artist, playlist and browse pages. The mini player (columns under
  900 pt) or the player bar (900 pt and wider) floats over the detail column
  with an edge blur; Now Playing opens over the window (Esc or Library closes
  it).
* **Now Playing.** Artwork style (the core's baked backdrop, crossfading on
  dur-4, the scrubber thumb in Liquid Glass on macOS 26) or Misty style (the
  field at the artwork hue, the artwork on a Clear frame). Seek bar, transport,
  shuffle and repeat, volume, favourite, output device (Speakers chip), lyrics
  (word sweep, translation and romanization lines, 38 % anchor, manual scroll
  pauses follow mode for 4 s, tap a line to seek), queue (drag to reorder,
  Delete to remove, double-click to jump, pages past 50 entries), info.
* **Search.** One `Search` command for the library and every source; results
  appear per source as they arrive, the library first. Library artwork comes
  from one `searchLibrary` query; if the command fails, the library query alone
  fills the page.
* **Settings.** Appearance (theme, accent colour, transparency, Reduce Motion
  and Transparency, Artwork or Misty, lyrics size, alignment, translations and
  romanization, the app icon picker), Playback (output device, crossfade,
  ReplayGain and preamp, equalizer), Library (folders, rescan, watch folders,
  last scan), Sources (community switch with the ADR-0012 disclosure, enable,
  sign-in by QR code, browser or fields, settings forms from each plugin's
  JSON Schema, install a `.jfpkg`, uninstall).
* **App icon (ADR-0017).** The bundle icon is Ice. Automatic shows Steel in the
  Dock while the system is dark; any other choice replaces the Dock icon while
  the app runs (`NSApplication.applicationIconImage`). Finder keeps Ice.
* **Media session.** `MPNowPlayingInfoCenter` is set on slice changes (the
  system extrapolates from the rate), with `playbackState` on every start and
  stop; `MPRemoteCommandCenter` handles play, pause, toggle, stop, next,
  previous, position, repeat and shuffle.
* **Menus.** Controls (Space plays and pauses outside text fields; ⌘→ ⌘← next
  and previous; ⌥⌘→ ⌥⌘← skip 5 s; ⌘↑ ⌘↓ volume; ⌥⌘↓ mute; ⌘J current song),
  View (⌘1–⌘7 sections, ⌘F search, ⌘L Now Playing, ⌥⌘L lyrics, ⌥⌘U queue),
  File (⌘N new playlist, ⇧⌘O add folder, import playlist, install plugin).
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
playback, settings, library, paging, search, sources, browse and queue stores on
`FakeCoreClient`; JSON, schema forms, lyric layout, schedules, event batching,
the Dock icon and formatting; the v0.0.4 design math (`DesignTests.swift`:
seek geometry, Now Playing motion and the cascade, swatches and the OKLCH port
against `design/tokens.json`, the transparency look, the new settings); and the
shared contract tests (`platforms/shared/swift/Tests`, which replay
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
  `defaultDataDir() -> String?`, `defaultCacheDir() -> String?`, `libraryVersion()`.
* `protocol CoreListener: AnyObject, Sendable { func onEvent(event: Event) }`.
* `CoreConfig(dataDir:cacheDir:output:)` with defaults for the rest; `OutputKind.device(name:)`;
  **`CoreConfig.bundledPlugins: [String]`** (assumed name and type, behind `JF_BUNDLED_PLUGINS`).
* `CoreFfiError.Core(message:)`, `.NotAvailable(message:)`, `.ShutDown(message:)`, `.InvalidArgument(message:)`.
* Enums with data use the field names as labels: `TrackRef.library(id:)`, `.file(path:)`,
  `.source(track:)`; `ArtworkRef.file(path:)`, `.url(url:)`; `Collection.album(id:)` …
  `.tracks(tracks:)`, `.source(plugin:id:)`, `.favorites`; `ScanStatus.progress(progress:)`,
  `.finished(report:)`, `.failed(message:)`; `LyricsSource.sidecar(path:)`, `.embedded`;
  `HueSource.fixed(hue:)`; `SearchScope.plugin(id:)`; `AuthStep.showQr(data:expiresAtMs:)`,
  `.openUrl(url:)`, `.askFields(fields:)`, `.waiting(message:)`, `.done(account:)`, `.failed(message:)`;
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

## Known gaps

* Equalizer presets live in jf-engine and are not in the FFI; the app edits the
  core's bands and offers Reset to Flat. An `eq_presets()` query would let it
  list them.
* Artwork URLs from sources are not fetched (networking stays in the core);
  those rows show a placeholder until the core hands out file paths.
* The macOS 26 layered icon (Icon Composer `.icon` with Ice and Steel) is not
  built: on every version the Dock switch happens at run time.
* `Info.plist` usage strings are English only.
* v0.0.4: the artwork does not grow out of the mini player's when Now Playing
  opens (a slide and fade instead, see above); page titles stay at `text-h2`
  on wide windows (the spec allows `text-h1` there); the lyrics cascade
  estimates the scroll distance from the line sizes instead of measuring it;
  in a gap between timed lines (no line active) every line shows as one to
  come, as on Windows and Android.
