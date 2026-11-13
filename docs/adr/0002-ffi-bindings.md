# ADR-0002: FFI bindings — UniFFI for Swift and Kotlin; C# bindings

* Status: Accepted, updated 2026-11-07 (C# via the C ABI)
* Date: 2026-11-05 (crates.io, uniffi-rs CHANGELOG and manual sources,
  NordSecurity/uniffi-bindgen-cs tags, CHANGELOG and issues)

## Context

The brief: UniFFI for Swift and Kotlin; for C#, evaluate
uniffi-bindgen-cs and fall back to a small cbindgen C ABI if it is
unstable; verify maturity first. ADR-0001 keeps the exported surface tiny
(`Core::new`, `send`, `subscribe`, `query`, a few getters), so the
generated code is mostly data types: commands, events and state slices.

**Verified:**

* **UniFFI 0.32.2** (2026-09-23). Every minor release carries breaking
  changes (0.30.0 changed handles and vtables, 0.32.0 byte buffers); the
  README says it is "a long way from a 1.0 release".
  * Rust `async fn` maps to Swift `async` and Kotlin `suspend`, driven by
    the foreign side; no cancellation.
  * Foreign-implemented traits (`#[uniffi::export(foreign)]`) are the
    supported way to call back; callback interfaces are soft-deprecated.
  * Proc-macro-only (no UDL) works through library mode.
  * **Swift 6 support is partial**: generated async code is not `Sendable`
    (#2458); umbrella issue #2448 is open.
* **uniffi-bindgen-cs v0.11.1+v0.31.0** (2026-10-09) targets **UniFFI 0.31**.
  The 0.32 upgrade (PR #176) has been open since 2026-07-10. All bindings
  must come from the same UniFFI minor (checksums), so **C# holds Swift and
  Kotlin back to 0.31**.
  * Features: records, enums, objects, errors, async methods (`Task`),
    foreign traits, custom and external types; .NET 8+ with
    `LibraryImport`.
  * NativeAOT is not documented. Callbacks crash under Mono full-AOT since
    v0.11.0 (issue #175; fix PR #193 opened 2026-11-05). CoreCLR NativeAOT,
    which WinUI would use, is untested as far as I found.
  * Activity in 2026: 62 commits, about two thirds from one outside
    contributor; 25 open issues, 14 open PRs. Users include dojo.c and a
    Navigraph fork; matrix-rust-sdk's C# bindings wait on #176.
  * No other maintained UniFFI C# generator exists.
* Alternatives: **cbindgen 0.29.4** (C headers only, mature);
  **csbindgen 1.9.8** (generates C# P/Invoke from `extern "C"`, raw C ABI,
  one maintainer); **interoptopus 0.16.6** (idiomatic C# with services,
  callbacks and async; one maintainer; a second API surface beside UniFFI);
  **BoltFFI 0.32.1** (first release 2026-02; too young).

## Decision

1. **UniFFI, proc-macro only, for Swift and Kotlin.** Keep the surface
   synchronous plus one foreign-trait listener (`CoreListener`). No async
   across the boundary in v1: commands are fire-and-forget and results come
   back as events. That sidesteps the Swift 6 `Sendable` gaps and C#
   async differences.
2. **uniffi-bindgen-cs for C#, with UniFFI pinned to the minor it supports**
   (0.31.x today) for all three languages. We upgrade UniFFI only when the
   C# generator has a release for the new minor.
3. **Contract tests in all three languages from Milestone 2.** The same
   JSON scenarios (`tests/contract/`) run against Swift (macOS CI), Kotlin
   (JVM and emulator) and C# (.NET 10 on Linux and Windows CI), long before
   the WinUI shell exists, so a generator problem shows up early.
4. **Spike S8 (start of M4)** proves on Windows: NativeAOT publish of a
   WinUI 3 app with the generated bindings, the listener callback under
   NativeAOT, 10 000 events/s without leaks, and cold start inside budget.
5. **Fallback, defined now:** a `jf-ffi-c` crate exporting the same five
   facade functions over a C ABI (cbindgen header, csbindgen P/Invoke),
   passing commands and events as UTF-8 JSON (events are small and
   infrequent); C# message types generated from the Rust types' JSON Schema
   (`schemars`) and parsed with `System.Text.Json` source generation, which
   is NativeAOT-safe. Switch if S8 fails, or if the C# generator falls two
   UniFFI minors behind or stalls for six months.

## Update (2026-11-07, v0.0.3)

uniffi-bindgen-cs is not published on crates.io, and this project's build
environment does not install tools from Git repositories, so **C# uses the
fallback in point 5 from the start**: jf-ffi exports a small C ABI that
passes commands, events, snapshots and query results as UTF-8 JSON, and a
hand-written C# library (`Jellyfesh.Ffi`, System.Text.Json source
generation, `LibraryImport`) exposes the same names as the Swift and
Kotlin bindings. With C# off UniFFI, the 0.31 pin no longer applies:
Swift and Kotlin use the current UniFFI release. Spike S8 (NativeAOT,
listener callbacks, 10 000 events/s) still runs on Windows. Details:
[`../ffi.md`](../ffi.md).

## Consequences

* Swift and Kotlin forgo UniFFI 0.32 improvements until the C# generator
  catches up; acceptable for a surface this small.
* The fallback costs about a week and touches only the Windows shell's
  binding layer, because every shell talks to the same facade and slices.
* The Swift shell compiles the generated module in Swift 5 language mode
  (or with `@preconcurrency`) until UniFFI's Swift 6 work lands.

## Verification sources

https://crates.io/api/v1/crates/uniffi; uniffi-rs `CHANGELOG.md` and
`docs/manual`; github.com/mozilla/uniffi-rs/issues/2448 and `/2458`;
github.com/NordSecurity/uniffi-bindgen-cs (tags, CHANGELOG, README,
issues #175, PRs #176 and #193); crates.io for cbindgen, csbindgen,
interoptopus and boltffi.
