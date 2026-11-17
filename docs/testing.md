# Testing Jellyfesh

Three ways to test, from quickest to most complete. Everything the
automated checks play or show is generated during the run (sine tones,
solid-colour images, invented lyric lines), never real recordings.

## 1. Automated tests (any machine, including a cloud session)

```bash
cargo test --workspace
```

Unit, golden, property and end-to-end tests for every crate. Audio goes to
WAV files or a paced null output, so no sound card is needed. CI runs the
same on Linux, macOS and Windows, plus `fmt`, `clippy`, `cargo deny` and
`cargo xtask tokens --check`.

The apps and bindings have their own tests (CI runs all of them):

```bash
cargo xtask bindings --lang kotlin && gradle -p platforms/shared/kotlin test      # Kotlin bindings (JVM)
cargo build -p jf-ffi && dotnet test --project platforms/shared/csharp/Jellyfesh.Tests  # C# bindings (.NET 10)
dotnet test --solution platforms/windows/Jellyfesh.Portable.slnf                   # Windows view models
(cd platforms/android && ./gradlew :logic:test)                                    # Android logic (JVM)
platforms/macos/scripts/build.sh --test                                            # macOS app (needs Xcode)
(cd platforms/web && npm ci && npm run build && npm test && npm run test:e2e)     # Jellyfesh Web (Chromium)
```

Jellyfesh Web's build needs `wasm-bindgen-cli` and the plugin packages from
`plugins/release.sh` first; its end-to-end tests run the built app against
mock Subsonic and Jellyfin servers (`docs/web.md`, "Building and testing").

The official plugins' tests (`cargo test -p jf-subsonic -p jf-jellyfin
-p jf-qqmusic -p jf-spotify -p jf-netease`) run each plugin in the real host
against a mock server; the online services' mocks are written from their
documentation (or, for NetEase, from what its official client sends) with
invented data, and stand in for services these tests never reach.

The community plugins' tests (`cargo test -p jf-community-netease
-p jf-community-qqmusic -p jf-community-common -p jf-community-testsupport`)
run their `mock-service` builds the same way, installed unsigned as
community plugins, against mocks of the services' web interfaces written
from the author's understanding of them, with invented data
(`plugins/community/README.md`).

## 2. The real audio-device path without a sound card (cloud, CI)

```bash
cargo build --release -p jf-cli
tools/cloud-audio-check/run.sh
```

The script starts a private PulseAudio server with a virtual 48 kHz
speaker, points ALSA at it for `jf` only, plays generated albums through
`jf play` (cpal → ALSA → PulseAudio, the same path as on a desktop), records
the speaker and checks the recording:

* **gapless:** a continuous sine split into three FLAC, MP3 and Vorbis
  tracks must come out as one unbroken sine;
* **dropouts and pitch:** no silence inside the tone; 1000 Hz after
  resampling from 44.1 kHz;
* **start and end:** how much audio is lost when playback starts and when
  the queue ends;
* **lyric sync:** `jf play --lyrics` runs in a pseudo-terminal while the
  speaker is read on the same clock; every line must appear within 30 ms of
  its own tone being heard.

It needs Linux with `pulseaudio pulseaudio-utils libasound2-plugins flac
lame vorbis-tools python3`, changes nothing outside a temporary folder, and
takes about a minute. CI runs it as the `audio` job.

To listen yourself in a cloud session, render to a file and download it:
`jf play <files or folder> --out render.wav`.

## 3. On your own computer, by ear

What a machine with real speakers or headphones adds: device latency, pause
and resume, Bluetooth and USB devices, and your own music. The checklist is
in the v0.0.1 notes: [`releases/v0.0.1.md`](releases/v0.0.1.md) ("Try it").
When something is wrong, send the OS, the output device and a log:
`jf -vv play "<album>" 2> jf-log.txt`.
