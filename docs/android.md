# The Android app (v0.0.4)

Kotlin and Jetpack Compose over the Rust core through the UniFFI Kotlin bindings
(docs/ffi.md). minSdk 26 (cpal's AAudio output, ADR-0009), compileSdk and targetSdk 37.
Everything lives in `platforms/android/`. The shell holds no business logic (ADR-0001): it
draws the core's slices and sends commands.

**Status.** The Android SDK cannot be downloaded in the environment this was written in, so the
APK has never been built and the app has never run. The parts that need no Android are built
and tested here (`:logic`), the FFI adapter is compiled and run against the real core on the
JVM (`:ffi-check`), and the whole app except the Media3 session is compiled on the JVM against
stand-ins (`:ui-check`). The first CI build is the first real compile; see "Risks" below.

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
* **Settings → Appearance:** Theme (System, Light and Dark with previews), the
  accent colour (Follow the artwork, ten swatches and a Custom hue on a
  hue-gradient track with a preview), Transparency from Solid to Clear in
  whole percent with a live preview, Reduce motion and transparency, the Now
  Playing background with previews, Lyrics size and alignment, translation
  and romanization, and the app icon. Each change is one `SetSettings`.
* **Transparency on Cloudy:** `GlassSurface` keeps Cloudy's blur and sets the
  Misty tint's alpha from the shared glass-alpha port
  (`platforms/shared/kotlin`, `glassAlpha`): the tokens' alphas at 50 %,
  opaque toward Solid, halved toward Clear; at 0 % the glass is solid Cloud.

**Checked here, not built for Android:** `:logic` 114 tests pass, `:ffi-check`
3 tests pass against the core, and `:ui-check` compiles the app (with these
screens) against Compose Multiplatform 1.8.2 and the stand-ins with no errors
or warnings. Nothing ran on a device; the APIs Compose 1.12 has beyond 1.8 are
not used.

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
the newest under `$ANDROID_HOME/ndk/`), JDK 17 or later, rustup, cmake (the core builds libopus)
and, on Linux, the ALSA headers (`libasound2-dev`) for the host build behind
`cargo xtask bindings`. The script adds the `aarch64-linux-android` and
`x86_64-linux-android` targets, installs cargo-ndk 4.1, builds `jf-ffi` for arm64-v8a and
x86_64 (`--release`), generates the bindings and tokens, and runs
`./gradlew -Pjellyfesh.skipCargo=true :app:assembleRelease`. The unsigned APK is copied to
`dist/android/jellyfesh-<version>-android-unsigned.apk`. The release workflow signs it with the
Jellyfesh release keystore from the repository secrets `ANDROID_KEYSTORE_BASE64`,
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD` (certificate SHA-256
`71:8D:02:2E:77:C3:09:7F:FD:F4:E0:B6:E4:FB:DB:62:FF:B3:EF:92:A8:9C:2B:3E:E3:57:F2:58:B7:13:56:46`). Without `-Pjellyfesh.skipCargo=true` Gradle builds the core itself through
`:core-bindings:cargoNdkBuild`.

Install on a device or emulator (arm64-v8a or x86_64): `adb install -r <apk>`.

### Bundled plugins

The official Subsonic and Jellyfin plugins ship inside every APK and as separate `.jfpkg`
downloads. The app's build copies `dist/plugins/*.jfpkg` (made by the plugin packaging script;
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
  commands, and the session state is the core's slices. `AudioFocusController` requests focus
  while playing, pauses on loss (resumes after a transient one) and when audio becomes noisy.
  `DeviceWatcher` sends `RefreshDevices` on Android's device callbacks.
* **App icon.** One `activity-alias` per icon (ADR-0017), Ice enabled by default; Automatic is
  Ice. The picker confirms before switching and states the launchers' limits.
* **Navigation.** Four tabs (Home, Library, Search, Sources), each with its own back stack kept
  in `AppGraph` (so it survives the activity being recreated), Now Playing over everything, and
  Settings from Home and Library. Back closes sheets, then Now Playing, then pops the stack.
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
  `outputDevices()` and `monotonicNowNs()`; `CoreFfiException.Core/NotAvailable/ShutDown/
  InvalidArgument`; `AndroidContext.initialize(Any)`;
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
