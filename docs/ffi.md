# The FFI layer (v0.0.4)

How the macOS (Swift), Android (Kotlin) and Windows (C#) shells reach
`jf-core`: one crate, `core/jf-ffi`, exposes the core API of
[`core-api.md`](core-api.md) to all three. Decision record:
[ADR-0002](adr/0002-ffi-bindings.md); what changed against it is at the end.

* **Swift and Kotlin:** UniFFI 0.32.2 (proc-macro, library mode, namespace
  `jellyfesh`), generated from the built library.
* **C#:** a C ABI in jf-ffi that exchanges the same types as UTF-8 JSON, and a
  hand-written C# library, `Jellyfesh.Ffi`, with the same API names.

Both sides wrap the same Rust object (`JellyfeshCore`) and the same mirror
types (`core/jf-ffi/src/types.rs`), so the three bindings cannot drift apart
in behaviour; contract files in `tests/contract/` keep their shapes and the
ported drawing helpers in step.

## Build and generate

```bash
cargo xtask bindings                      # all three; or --lang swift|kotlin|csharp
cargo xtask bindings --release            # release build of the native library
cargo xtask tokens --target all           # Rust module + Swift, Kotlin and C# tokens
cargo xtask tokens --target kotlin        # one platform (swift|kotlin|csharp|rust)
```

`bindings` builds `jf-ffi` for the host (`libjf_ffi.so`, `libjf_ffi.dylib` or
`jf_ffi.dll` in `target/<profile>/`) and writes:

| Shell | Folder (git-ignored) | Files |
| --- | --- | --- |
| macOS | `platforms/macos/Generated/` | `JellyfeshFFI.swift`, `JellyfeshFFIFFI.h`, `JellyfeshFFIFFI.modulemap`, `Tokens.swift` |
| Android | `platforms/android/core-bindings/src/main/kotlin/` | `org/jellyfesh/ffi/jellyfesh.kt`, `org/jellyfesh/ffi/AndroidContext.kt`, `org/jellyfesh/tokens/Tokens.kt` |
| Windows | `platforms/windows/Generated/` | `Jellyfesh.Ffi/` (the C# project), `native/<library>`, `Tokens.cs` |

UniFFI runs through `jf-ffi-bindgen` (`core/jf-ffi/bindgen`), the documented
`uniffi-bindgen` binary built from the crates.io `uniffi` release the
scaffolding uses, with `--no-format` so the output does not depend on ktlint
or swiftformat. Library mode reads `core/jf-ffi/uniffi.toml` (Kotlin package,
Swift module). Never edit generated files; regenerate them.

`jf-ffi` is a `cdylib` (Android, Windows, desktop tests), a `staticlib` (the
XCFramework for macOS and iOS) and an `rlib` (Rust tests). Building it for a
device target needs that target's Rust standard library and a C toolchain:
the core compiles libopus with CMake (`opusic-sys`), SQLite and ring with
`cc`; Android needs the NDK (for example through `cargo-ndk`, from crates.io).
Packaging (XCFramework, AAR with `jniLibs/`, MSIX) is not part of xtask yet.

## Names

The shells code against these names; they follow the Rust ones.

| Rust | Swift | Kotlin | C# |
| --- | --- | --- | --- |
| crate `jf-ffi`, namespace `jellyfesh` | module `JellyfeshFFI` | package `org.jellyfesh.ffi` | namespace `Jellyfesh.Ffi` |
| `JellyfeshCore::new(config)` | `try JellyfeshCore(config:)` | `JellyfeshCore(config)` | `new JellyfeshCore(config)` |
| `JellyfeshCore::with_secret_store(config, secrets)` | `try JellyfeshCore.withSecretStore(config:secrets:)` | `JellyfeshCore.withSecretStore(config, secrets)` | `new JellyfeshCore(config, secrets)` |
| `SecretStore` (foreign trait: `get`, `set`, `delete`) | protocol `SecretStore` | interface `SecretStore` | interface `SecretStore` (C ABI callback) |
| `SecretStoreError::Unavailable { reason }` | `SecretStoreError.Unavailable(reason:)` | `SecretStoreException.Unavailable(reason)` | `SecretStoreException` (any exception) |
| `send(command) -> u64` | `send(command:) -> UInt64` | `send(command): ULong` | `Send(Command) -> ulong` |
| `snapshot()` | `snapshot()` | `snapshot()` | `Snapshot()` |
| `set_listener(Option<CoreListener>)` | `setListener(listener:)` | `setListener(listener)` | `SetListener(CoreListener?)` |
| `CoreListener::on_event(event)` | protocol `CoreListener`, `onEvent(event:)` | interface `CoreListener`, `onEvent(event)` | interface `CoreListener`, `OnEvent(Event)` |
| queries (below) | `throws` | `@Throws(CoreFfiException::class)` | throw `CoreFfiException` |
| `CoreFfiError` | `enum CoreFfiError: Error` | `sealed class CoreFfiException` | `CoreFfiException` + `CoreFfiErrorKind` |
| free functions | global functions | top-level functions | static class `JellyfeshMethods` |

**Queries** (all return `Result<_, CoreFfiError>`): `albums(offset, limit)`,
`albums_sorted(sort, descending, offset, limit)`, `artists(offset, limit)`,
`tracks(query: TrackQuery)`, `track(id)`, `album_tracks(album_id)`,
`artist_albums(artist_id)`, `search_library(text, limit) -> SearchResults`,
`tracks_for_query(text, limit)`, `recently_added(limit)`,
`recently_played(limit)`, `most_played(limit)`, `playlists()`,
`playlist(id)`, `playlist_entries(id, offset, limit)`,
`export_playlist(id, path)`, `favorites(offset, limit)`, `is_favorite(track)`,
`artwork_thumbnail(hash, size_px) -> Option<String>`,
`queue_window(offset, limit)`, `history()`, `roots()`, `stats()`; and
`shutdown()`. Swift and Kotlin use lowerCamelCase (`searchLibrary`,
`artworkThumbnail(hash:sizePx:)`), C# PascalCase (`SearchLibrary`).

**Free functions:** `output_devices()` (throws), `monotonic_now_ns()`,
`default_data_dir()`, `default_cache_dir()`, `library_version()`.

**Config:** `CoreConfig { data_dir, cache_dir, output: OutputKind, buffer_ms
(200), restore_session (true), persist_session (true), watch_folders (true),
bundled_plugins ([]) }` (defaults in Swift, Kotlin and C# constructors; in
JSON `bundled_plugins` may be left out). `bundled_plugins` lists the `.jfpkg`
files the app ships (`core-api.md`, "Online sources"). The core's other
source options keep their defaults; the secret store goes to
`with_secret_store`.

**Types.** Every type of `core-api.md` and `jf-core`'s `types.rs` is mirrored
with the same type and field names (`Command`, `Event`, `Snapshot`,
`PlaybackSnapshot`, `TrackRef`, `Collection`, `Settings`, `LyricDoc`,
`LyricTimeline`, …), with these rules:

| In jf-core | Across the FFI |
| --- | --- |
| `PathBuf` | `String` (non-UTF-8 paths are converted lossily; the library skips such files) |
| `Arc<T>` | `T` |
| `CommandId` | `u64` (the value `send` returned) |
| `usize` (counts, indices, limits) | `u32` (larger values saturate) |
| tuple variants | named fields: `TrackRef::Library { id }`, `File { path }`, `Source { track }`; `ArtworkRef::File { path }`, `Url { url }`; `LyricsSource::Sidecar { path }`, `Plugin { id }`; `ScanStatus::Progress { progress }`, `Finished { report }`, `Failed { message }` |
| `Event` tuple variants | slice events carry `snapshot` (`Playback`, `Queue`, `NowPlaying`, `Lyrics`, `Theme`, `Settings`, `Devices`, `Sources`); `Scan { status }`; `Auth`, `SearchResults`, `BrowsePage` carry `update` |
| `ScanReport.errors: Vec<(PathBuf, String)>` | `Vec<ScanError { path, message }>` |
| `SubmitLogin.fields: Vec<(String, String)>` | `HashMap<String, String>` |
| `Theme.colors: BTreeMap<String, [u8; 4]>` | `Vec<ThemeColor { name, rgba: u32 }>`, sorted by name, `rgba` = `0xRRGGBBAA` |
| `BTreeMap` (`LyricDoc.metadata`, `AuxTrack.lines`) | `HashMap` |

Language mappings: `u32`/`u64` are `UInt32`/`UInt64` (Swift), `UInt`/`ULong`
(Kotlin), `uint`/`ulong` (C#). Records are Swift structs with `var` fields,
Kotlin data classes and C# positional records. Enums without data are Swift
enums (`.playing`), Kotlin enum classes (`Status.PLAYING`) and C# enums
(`Status.Playing`). Enums with data are Swift enums with associated values
(`.playback(snapshot:)`), Kotlin sealed classes (`Event.Playback`) and C#
abstract records with one nested record per variant (`TrackRef.Library`,
`Command.PlayCollection`, `Event.Playback`), with PascalCase members.

**C# exceptions to the names**, because C# forbids a member named like its
type: `Collection.Tracks.Value`, `ArtworkRef.Url.Value`,
`ScanStatus.Progress.Value`, `Event.Buffering.Value`; and
`Event.CommandFailed.Message` (Rust `error`), because `Event.Error` is a member
of every `Event` record. The JSON keeps the Rust names. In the C# tokens,
`Tokens.Accent.Accent` and `Tokens.Line.Line` are `AccentColor` and
`LineColor`.

## Behaviour

* **Listener.** One core thread (`jf-ffi-events`) calls the listener, one
  event at a time, in order. Hop to the UI thread to update views. When
  `set_listener` returns, the previous listener is no longer called (unless
  `set_listener` was called from inside its own `on_event`), so it can be
  released; with `None`/`null` the events are dropped. Never block inside
  `on_event` waiting for a thread that is calling `set_listener`, `shutdown`
  or (C#) `Dispose`: they wait for a call in progress. A listener that throws
  or panics loses that one event; the stream continues (C# reports it
  through `JellyfeshCore.ListenerFailed`). Set the listener, then read
  `snapshot()` for the state so far.
* **Shutdown.** `shutdown()` saves the session, lets plugin installs,
  uninstalls and logouts already sent finish (at most a minute), stops
  playback and the event thread, and releases the listener; no event
  arrives after it returns.
  Afterwards queries fail with `ShutDown`, `send` returns 0 and `snapshot()`
  returns the last state. Dropping the last reference (Swift `deinit`,
  Kotlin `close()`, C# `Dispose()`) shuts the core down too. A listener that
  holds the core keeps both alive: call `shutdown()` (or `close()`/`Dispose()`).
* **Errors.** `CoreFfiError` kinds: `Core` (library, session, audio or file;
  the message says which), `NotAvailable` (a feature not in this build; no
  v0.0.3 feature uses it any more), `ShutDown`, `InvalidArgument` (unreadable
  JSON over the C ABI). Command failures arrive as
  `Event::CommandFailed { id, error }` with the id `send` returned.
* **Secret store** (ADR-0015). Pass the platform keystore to
  `with_secret_store` (C#: `new JellyfeshCore(config, secrets)`): Keychain on
  macOS, Android Keystore on Android, Credential Manager on Windows. The core
  keeps the sign-in tokens, API keys and cookie-jar keys of online sources
  there under keys `jellyfesh/<plugin-id>/<name>`; passwords never reach it.
  It calls the store from its own threads, possibly several at once, and
  reads one entry no plugin can name at start: if that call throws, sign-ins
  stay in memory for this run and `SourcesSnapshot.secret_storage` is
  `SessionOnly`. Without a store, `new` uses the system keystore the core
  reaches itself (macOS, Windows, Linux; none on Android and iOS, so pass
  one there). After `shutdown` returns the store is never called again
  (C# frees its handle with the core).
* **Time.** `monotonic_now_ns()` is the clock anchors are stamped with, in the
  same process. Draw with the ported `DisplayClock`, never by asking the core
  per frame (`core-api.md`, "Drawing time").

## Drawing-time ports

Reference ports of `LyricTimeline::cursor`, `LyricTimeline::next_change`,
`DisplayClock` (same behaviour as `core/jf-core/src/timing.rs`, including
`PlaybackAnchor::position_at`), `next_second_us` and `next_step_us`, on the
generated types:

| Language | Files | API |
| --- | --- | --- |
| Swift | `platforms/shared/swift/Sources/` | `timeline.cursor(t)`, `timeline.nextChange(t)`, `LyricCursor`, `DisplayClock` (struct), `anchor.positionAt(now)`, `nextSecond(_:)`, `nextStep(_:step:)`, `glassAlpha(_:transparency:)` |
| Kotlin | `platforms/shared/kotlin/src/main/kotlin/org/jellyfesh/shared/` | `timeline.cursor(t)`, `timeline.nextChange(t)`, `LyricCursor`, `DisplayClock`, `anchor.positionAt(now)`, `nextSecond`, `nextStep`, `glassAlpha` |
| C# | `platforms/shared/csharp/Jellyfesh.Shared/` (namespace `Jellyfesh.Shared`) | `timeline.Cursor(t)`, `timeline.NextChange(t)`, `LyricCursor`, `DisplayClock`, `anchor.PositionAt(now)`, `DrawingTime.NextSecond`, `DrawingTime.NextStep`, `Glass.Alpha` |

`waitUntil` returns a `Duration` (Kotlin; Swift on macOS 13, plus
`waitUntilUs` in microseconds) or a `TimeSpan` (C#). Each port replays
`tests/contract/lyrics-cursor.json` and `tests/contract/display-clock.json`;
the second is recorded from the Rust `DisplayClock` itself
(`core/jf-ffi/tests/drawing_contract.rs`). Next to them, each language ports
`jf_palette::glass_alpha` (the transparency setting's glass alpha, which needs
no generated type) and replays `tests/contract/glass-alpha.json`.

## Tokens

`cargo xtask tokens --target swift|kotlin|csharp` writes the Misty tokens
from `design/tokens.json` as constants: `Tokens.swift` (`enum Tokens`),
Kotlin `org.jellyfesh.tokens.Tokens` and C# `Jellyfesh.Design.Tokens`. One
nested type per group (`type` is `TypeScale`), dimensions as density-
independent units (pt, dp, epx) from the token file's px, durations in
milliseconds (`dur0Ms`), curves as `CubicBezier`, type roles as `Typography`,
shadows and glass grades as small records. Colours are not baked (Misty
PLT-03): a colour token is a `ThemeColorKey`, the name of that colour in the
core's theme snapshot (`ThemeSnapshot.light.colors` and `.dark.colors`), which
resolves every colour from one hue at run time.

## Per-language notes

### Swift (macOS)

* Compile `JellyfeshFFI.swift` as the module `JellyfeshFFI` in **Swift 5
  language mode** (or with `@preconcurrency import`): UniFFI's Swift 6
  support is partial (ADR-0002). The C declarations are the module
  `JellyfeshFFIFFI` (`JellyfeshFFIFFI.h` and `.modulemap`, UniFFI's default
  name); link the static library or wrap both in an XCFramework.
* `CoreListener` is `AnyObject & Sendable`: implement it with a `final
  class` whose state is immutable or synchronised (or mark it
  `@unchecked Sendable`), and hop to the main actor in `onEvent`.
  `SecretStore` likewise (over the Keychain, `SecItemCopyMatching` and
  friends); throw `SecretStoreError.Unavailable(reason:)` when it cannot
  answer.
* The mirror `Collection` shadows `Swift.Collection` in files that import
  `JellyfeshFFI`; write `Swift.Collection` for the protocol.
* The ports in `platforms/shared/swift/` use `#if canImport(JellyfeshFFI)`, so
  they build in the same target as the generated file or beside it.
* **Unverified:** no Swift toolchain was available; the generated bindings,
  the ports and `Tests/DrawingContractTests.swift` have not been compiled.
  The contract files and the Kotlin and C# ports, which pass, are the
  reference.

### Kotlin (Android and JVM)

* The bindings use **JNA** (direct mapping). Android apps depend on
  `net.java.dev.jna:jna:5.19.1@aar` and ship `libjf_ffi.so` per ABI in
  `jniLibs/`; JNA loads `jf_ffi` (the system property
  `uniffi.component.jellyfesh.libraryOverride` overrides the name or path).
* **Initialise `ndk-context` before the first playback:**
  `AndroidContext.initialize(applicationContext)`, for example in
  `Application.onCreate`. The audio output (cpal's AAudio host) reads the
  Java VM and a `Context` from `ndk-context` when it opens a stream and
  panics without them; JNA loads the library without telling Rust about the
  VM. `AndroidContext` (copied next to the bindings) loads the same library
  through `System.loadLibrary` and calls the JNI function
  `Java_org_jellyfesh_ffi_AndroidContext_nativeInitialize`, which stores the
  VM and a global reference to the context once. The JVM tests run this path.
* The generated file uses the JVM cleaner variant (`java.lang.ref.Cleaner`,
  falling back to JNA's cleaner), which works on Android too; objects are
  `AutoCloseable` (`close()` frees the Rust side).
* JNA attaches the core's event thread to the VM afresh for each callback,
  so `Thread.currentThread()` differs between calls; post to the main
  thread (`Handler`, coroutines) rather than relying on thread identity.
* Unsigned types are Kotlin's (`UInt`, `ULong`); `CoreFfiException` subclasses
  are `Core`, `NotAvailable`, `ShutDown`, `InvalidArgument`.
* Implement `SecretStore` over the Android Keystore (for example values
  encrypted with a Keystore key in the app's private storage) and throw
  `SecretStoreException.Unavailable(reason)` when it cannot answer; Android
  has no keystore the core reaches itself, so without one sign-ins last
  only while the app runs.
* Importing `org.jellyfesh.ffi.Collection` shadows
  `kotlin.collections.Collection` in that file.

### C# (Windows)

* `Jellyfesh.Ffi` (net10.0) calls the C ABI with `LibraryImport` (explicit
  cdecl), receives events through an `[UnmanagedCallersOnly]` callback whose
  exceptions never cross into Rust, and (de)serialises with System.Text.Json
  source generation (`JellyfeshJsonContext`, public; snake_case names, enum
  tags written first). `IsAotCompatible` turns on the trimming and NativeAOT
  analyzers, and the library builds with them as errors. A `SafeHandle` owns
  the core and frees the listener's GC handle only after the core has stopped
  calling it.
* **NativeAOT is spike S8** (ADR-0002): publish the WinUI 3 app with
  NativeAOT on Windows, run the listener callback under it, push 10 000
  events/s without leaks and check cold start. Nothing here has been
  published with NativeAOT yet.
* The app references `Generated/Jellyfesh.Ffi/Jellyfesh.Ffi.csproj`; the
  native library in `Generated/native/` is copied to the app's output by
  that project. The ports: compile `platforms/shared/csharp/Jellyfesh.Shared/*.cs`
  into the app, or reference `Jellyfesh.Shared.csproj` with `JfFfiProject`
  pointing at the generated `Jellyfesh.Ffi.csproj`.
* Records compare lists by reference (C# record semantics): compare JSON or
  elements when testing equality of snapshots.
* The listener interface is `CoreListener` (the shared name, without an `I`);
  `DelegateListener` wraps a lambda. The secret store interface is
  `SecretStore` likewise; whatever it throws becomes a failed call, and
  `CoreConfig.BundledPlugins` is left out of the JSON when null.

## The C ABI (`core/jf-ffi/src/c_api.rs`)

```c
typedef struct JellyfeshCore JellyfeshCore;                  /* opaque */
typedef void (*JfEventCallback)(void *user_data, const char *event_json);

JellyfeshCore *jf_core_new(const char *config_json, char **error_json);  /* NULL on failure */
typedef void (*JfSecretReply)(void *reply_context, const char *reply_json);
typedef void (*JfSecretCallback)(void *user_data, const char *request_json,
                                 JfSecretReply reply, void *reply_context);
JellyfeshCore *jf_core_new_with_secret_store(const char *config_json, JfSecretCallback secrets,
                                             void *secrets_user_data, char **error_json);
uint64_t jf_core_send(const JellyfeshCore *core, const char *command_json); /* 0: not sent */
char *jf_core_snapshot(const JellyfeshCore *core);
void jf_core_set_listener(const JellyfeshCore *core, JfEventCallback callback, void *user_data);
char *jf_core_<query>(const JellyfeshCore *core, const char *args_json);    /* one per query */
void jf_core_shutdown(const JellyfeshCore *core);
void jf_core_free(JellyfeshCore *core);                      /* shuts down if needed */
char *jf_output_devices(void);                               /* {"ok"} or {"error"} */
uint64_t jf_monotonic_now_ns(void);
char *jf_default_data_dir(void);  char *jf_default_cache_dir(void);  char *jf_library_version(void);
void jf_string_free(char *s);
```

* Every `char *` returned belongs to the caller: free it with
  `jf_string_free`. `event_json` is valid only during the callback.
* Query arguments are one JSON object named like the UniFFI parameters
  (`{"offset": 0, "limit": 50}`, `{"query": {...}}`, `{"id": 5, "path": "..."}`;
  NULL or `{}` when there are none). Results are `{"ok": <value>}` or
  `{"error": {"kind": "...", "message": "..."}}`; `jf_core_new` writes the
  error object to `*error_json`.
* The handle may be used from any thread at once, but `jf_core_free` must not
  race with another call on it. The callback runs on the core's event thread
  and must be callable from any thread with `user_data` until it is replaced
  or the core is shut down; the replacement rules are those of the listener.
* The secret store callback receives `{"op": "get", "key": "..."}`,
  `{"op": "set", "key": "...", "value": "..."}` or `{"op": "delete", "key": "..."}`
  and answers before it returns by calling `reply(reply_context, json)` once:
  `{"ok": null}` (done, or no value), `{"ok": "<value>"}` or
  `{"error": "<message>"}`. Both strings are valid only during the call. It is
  called from core threads, possibly several at once, until
  `jf_core_shutdown` or `jf_core_free` returns, never afterwards; those wait
  for a call in progress, so never block in the callback waiting for the
  thread that calls them.
* A panic in the core never unwinds into the caller: it becomes an error
  result, 0 or NULL.

**JSON.** Each value is a mirror type serialised with serde: field names as in
Rust; enums without data as their variant name (`"Playing"`); enums with data
as objects tagged by `"type"` with the variant name, **always as the first
key** (System.Text.Json's polymorphic reader requires it), for example
`{"type":"Library","id":42}` and `{"type":"PlayCollection","collection":{"type":"Playlist","id":5},"start":0,"shuffle":false}`.
Maps with integer keys (`AuxTrack.lines`) use string keys; map entries are
sorted. `tests/contract/ffi-types.json` holds one sample of every root type.

## Tests

| What | Command | Here |
| --- | --- | --- |
| Rust: conversions both ways, JSON round trips, the JSON contract, the listener's guarantees, the C ABI (JSON, callback, freeing), the app's secret store (UniFFI and C callback, working and locked), the shared scenario twice | `cargo test -p jf-ffi` | 19 pass |
| Kotlin on the JVM: the ports against the three contracts, the UniFFI bindings against the shared scenario, a Kotlin `SecretStore`, the Android-context JNI path, the generated tokens | `cargo xtask bindings --lang kotlin && cargo xtask tokens --target kotlin && gradle -p platforms/shared/kotlin test` | 8 pass |
| C# on .NET 10: the ports against the three contracts, every `ffi-types.json` sample, the binding against the shared scenario, listener errors, a C# `SecretStore` (working and locked), the generated tokens | `cargo build -p jf-ffi && cargo xtask tokens --target csharp && dotnet test --project platforms/shared/csharp/Jellyfesh.Tests` | 11 pass |
| Swift: the ports against the three contracts | add `platforms/shared/swift/Tests` to an Xcode test target | not run |
| Token generators | `cargo test -p xtask` | pass |

Contract files in `tests/contract/`:

* `lyrics-cursor.json` (from jf-lyrics) and `display-clock.json` (recorded from
  `jf_core::DisplayClock`; `UPDATE_GOLDEN=1 cargo test -p jf-ffi --test
  drawing_contract`): the drawing-time ports.
* `glass-alpha.json` (recorded from `jf_palette::glass_alpha` by
  `core/jf-palette/tests/glass_contract.rs`): the glass-alpha ports.
* `ffi-types.json`: one JSON sample per mirror root type, read back by C#
  (`UPDATE_GOLDEN=1 cargo test -p jf-ffi --test types`).
* `core-scenario.json`: the end-to-end run every binding replays — generated
  WAV tones, temporary folders, `OutputKind::Null { realtime: false }`, scan,
  queries, a playlist (`PlaylistCreated`), a favourite, export, playing the
  playlist with `NowPlaying`, `Playback` and `Queue` events matching
  `snapshot()`, play statistics, shutdown. (Before v0.0.3's release an output
  faster than real time could make the core announce one entry twice; the
  scenario still counts consecutive repeats once.)

Gradle needs Maven Central and the plugin portal; behind a rate-limited proxy
add `-Dorg.gradle.internal.repository.max.retries=12
-Dorg.gradle.internal.repository.initial.backoff=3000 --max-workers=1`. The
repository's `global.json` opts `dotnet test` into the Microsoft.Testing.Platform
runner that xunit v3 needs on the .NET 10 SDK (`dotnet test --project <dir>`
or `--solution <file>`).

## Versions (checked 2026-11-07)

UniFFI 0.32.2 (crates.io), jni-sys 0.4.1 and ndk-context 0.1.1 (the versions
cpal's Android backend uses); Kotlin 2.4.21, JNA 5.19.1, JUnit 6.1.3,
kotlinx-serialization-json 1.12.0 (tests), Gradle 8.14.3; .NET 10 SDK (10.0.114), xunit.v3
4.0.2, xunit.runner.visualstudio 4.0.1, Microsoft.NET.Test.Sdk 18.11.0.

## Changes against ADR-0002

* **UniFFI 0.32.2**, not the 0.31 pin: the pin only served uniffi-bindgen-cs,
  which is not on crates.io and cannot be installed here.
* **C# goes through the fallback from the start** (ADR-0002's update): the C
  ABI lives in `jf-ffi` itself rather than a separate `jf-ffi-c` crate, and
  covers the whole API (one function per query) rather than five facade
  functions. The C# side is hand-written (`LibraryImport`, source-generated
  System.Text.Json) instead of cbindgen + csbindgen with types generated from
  JSON Schema; `tests/contract/ffi-types.json` checks it against the Rust
  types instead. No C header is generated; the signatures are above.
* **One set of mirror types** serves UniFFI and the JSON ABI (UniFFI and serde
  derives on the same types), so the two cannot drift.
* The listener is a UniFFI 0.32 **foreign trait** (`#[uniffi::export(foreign)]`),
  which replaces `with_foreign`/callback interfaces.
* **Contract tests** run Kotlin on the JVM (not yet on an emulator) and C# on
  .NET 10 on Linux and Windows; Swift runs in the macOS app's CI job. They replay shared JSON scenarios as planned.
* **Added beyond the plan:** the Android `ndk-context` entry point, queries the
  facade gained in v0.0.3 (`albums_sorted`, `playlist`, `export_playlist`,
  `track`, `history`, `tracks_for_query`), token generation for the three
  shells, and the app's secret store as a foreign trait (UniFFI) and a JSON
  callback (C ABI).
