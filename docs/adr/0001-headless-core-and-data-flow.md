# ADR-0001: Headless core, unidirectional data flow, playback anchor

* Status: Proposed
* Date: 2026-11-05

## Context

Four native shells (SwiftUI, WinUI 3, GTK4, Compose) must behave the same
and stay free of business logic. FFI calls are cheap but not free, and
UniFFI, C# interop and JNI each add their own overhead and failure modes.
Time-driven views (progress, karaoke lyrics) redraw every frame.

## Decision

1. **Actor core.** `jf-core` runs one actor thread that owns all state.
   Shells send `Command`s; the actor applies them in order and publishes
   state *slices* (`Playback`, `Queue`, `Theme`, `Lyrics`, `Sources`,
   `Settings`, `Devices`, `Scan`), each with a version number, plus discrete
   `Event`s (errors, auth prompts, toasts).
2. **Small FFI surface.** The exported API is `Core::new(config)`,
   `send(Command) -> CommandId`, `subscribe(listener)`, `query(view,
   window)` and a handful of synchronous getters for startup. Everything
   else is data. A small surface keeps three binding generators in step and
   makes contract tests exhaustive.
3. **Paging, not pushing.** Collections are queried by window and revision.
   The core never pushes a 50 000-row list across FFI.
4. **Images by path.** Artwork, thumbnails and baked backdrops are written
   to a content-addressed cache and referenced by path.
5. **Playback anchor.** Position, monotonic timestamp and rate are
   published on change; shells extrapolate per frame. Each platform reads the
   same monotonic clock the core stamps with (table in
   [`architecture.md`](../architecture.md#the-playback-anchor)); a contract
   test bounds the skew at 1 ms. The core re-anchors on state changes and
   when the audio clock drifts more than 15 ms.
6. **Lyrics cursor in the shell, specified in the core.** Shells binary-search
   precomputed timeline arrays; shared JSON test vectors pin the behaviour.
7. **Linux shell calls the crates directly**, but through the same facade
   and slices as the FFI shells, so it cannot grow private shortcuts.

## Consequences

* Shells are view code plus adapters; logic bugs are fixed once.
* A slice-level snapshot is simpler than fine-grained diffs. If a slice
  grows hot (for example `Queue` during drag-reordering), it can gain a
  diff event later without changing the model.
* The clock contract is a new source of bugs on each platform; the contract
  test is mandatory in CI.

## Alternatives considered

* **Serialized message bus** (protobuf or postcard over one C ABI for all
  four shells): uniform, but loses UniFFI's typed bindings and async support
  for Swift and Kotlin, and adds a schema toolchain. Kept as the fallback for
  C# in ADR-0002.
* **Per-frame position polling**: simplest for shells, but violates the
  brief and costs CPU in Still mode.
