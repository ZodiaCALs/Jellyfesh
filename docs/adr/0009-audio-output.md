# ADR-0009: Audio output and device switching

* Status: Proposed
* Date: 2026-11-05

## Context

The brief names cpal with AAudio/Oboe on Android, plus output-device
switching. What cpal does today:

**Verified (cpal 0.18.2, 2026-08-16, Apache-2.0, MSRV 1.85; its CHANGELOG,
README and sources):**

* Android uses **AAudio through the `ndk` crate**. Oboe was removed in
  0.16.0 (2025-06-07), which raised the minimum to **API 26**. Device
  listing goes through JNI and `ndk-context`, which the app must initialise.
* Windows: **WASAPI shared mode only**; no exclusive mode.
* **No device-added, removed or default-changed events** (issue #373, open
  since 2020). Changes surface only on a live stream's error callback.
  CoreAudio streams on the default output follow the new default and report
  `DeviceChanged`; WASAPI default streams report `StreamInvalidated` or
  `DeviceNotAvailable` and must be rebuilt.
* The README states a **macOS 14.2 minimum**.
* Open Windows bug #1402 (2026-10-16): opening the default device can fail
  for the rest of the process with `RPC_E_CHANGED_MODE`; fix PR #1413 not
  merged.
* rubato 5.0.1 (2026-10-01) for resampling; four breaking majors in 2026.

## Decision

1. **cpal for streams** on all four platforms (CoreAudio, WASAPI, ALSA
   with PipeWire/PulseAudio behind it, AAudio).
2. **Our own `DeviceWatcher` per OS** instead of polling: CoreAudio
   property listeners (default output and device list), `IMMNotificationClient`
   on Windows, nothing on PipeWire systems when we play to `default` (the
   session manager moves the stream), and Android's `AudioDeviceCallback`
   forwarded by the shell as a command. Polling would eat the idle-CPU budget.
3. **Rebuild on change, keep position:** on device change the engine
   drains, reopens at the new device's rate, refills from the decoder
   position and re-anchors, so lyrics stay in sync.
4. **Engine audio runs on our own thread with its own COM apartment** on
   Windows, which sidesteps #1402's apartment clash with the WinUI thread;
   we track PR #1413.
5. **Wrap rubato behind a `Resampler` trait** so its API churn stays in one file.
6. **Android power:** spike S10 checks whether cpal's AAudio stream can use
   `AAUDIO_PERFORMANCE_MODE_POWER_SAVING` and large buffers. If not, we
   upstream the option or write a thin AAudio output on the same `ndk`
   crate. Low-latency mode would miss the battery budget.
7. **Bit-perfect and exclusive output are out of scope for v1** (cpal
   supports neither). It is offered as option E in the Step 0 index.

## Consequences

* macOS minimum is **14.2** (cpal's own minimum; decided 2026-11-05).
* Android minimum is at least **API 26** from cpal; ADR-0005 may raise it.
* Three small platform watchers to maintain, each with a contract test that
  unplugs a virtual device where the OS allows it.

## Verification sources

https://crates.io/api/v1/crates/cpal; the cpal 0.18.2 CHANGELOG and README;
https://github.com/RustAudio/cpal/issues/373; `/issues/1402`; `/pull/1413`;
https://crates.io/api/v1/crates/rubato.
