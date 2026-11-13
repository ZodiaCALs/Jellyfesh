# ADR-0003: Plugin runtime — wasmtime, not wasmi

* Status: Proposed
* Date: 2026-11-05 (wasmtime release notes and docs at v50.0.1, wasmi
  CHANGELOG and issues, wasmi-benchmarks raw data, local experiments with
  Rust 1.97, wit-bindgen 0.63 and wasm-tools 1.263)

## Context

The brief: plugins are WASM components with a WIT interface, run
in-process with wasmtime, or wasmi if it starts faster and is lighter on
Android; benchmark both. Plugins mostly wait on HTTP and parse JSON; heavy
crypto runs on the host side through imports. So start-up time, binary
size, memory and isolation matter more than raw execution speed.

**Verified:**

* **wasmi 2.0.0** (2026-09-01): a new interpreter core, lazy translation by
  default, fuel metering, `StoreLimits`. **No Component Model support**, and
  the maintainer said in 2025 there are no plans for it yet (issue #1665;
  tracking issue #897 open). The only bridge, `wasm_component_layer` 0.1.18
  (2024), **does not compile against the runtime layer wasmi 2.0 needs**
  (97 errors in a local build); it works only on a 2024 wasmi 0.36. The
  maintainer's sponsorship ended in October 2026.
* **wasmtime 50.0.1** (2026-10-29); a new major every month; every 12th
  major is an LTS supported for 24 months, so **48.x is the current LTS**.
  Component Model is Tier 1. Pulley (interpreter) is Tier 2. **Android
  targets are Tier 3** ("we don't test Android"); recent Android fixes:
  pointer-authentication crash (fixed in 45.0), 16 KB pages (42.0), AArch64
  icache coherence (50.0), a Cranelift memory blow-up on a large module
  (register allocator fix, Feb 2026).
* Start-up for a 1.64 MB module (compile + instantiate, wasmi-benchmarks
  raw data, 2026-09): Cranelift 114 ms (M2 Pro) / 221 ms (EPYC); Pulley
  133 / 239 ms (Cranelift still compiles it); **Winch 17 / 37 ms**; wasmi
  0.8 / 1.5 ms. Execution relative to Cranelift: Winch 2–2.5× slower, wasmi
  6–9× slower, Pulley about 20× slower.
* Size: a stripped x86_64 host with wasmtime runtime, component model,
  Cranelift and Pulley is **7.25 MB** (local build, opt-level s, LTO); the
  published compiler-less C API is 1.2 MB with LTO.
* `Component::deserialize` is `unsafe`: precompiled artifacts must be
  trusted output of the same wasmtime version and configuration.
* Epoch interruption costs about 10% and is not deterministic; fuel is
  deterministic but slower. Neither interrupts a blocked host call, and
  resource limits do not count host-side allocations.
* About 60 security advisories appear in wasmtime's 2026 release notes,
  mostly in WASI and async-component code, a few in core paths.
* Guest side: a Rust `wasm32-wasip2` std build imports **14 WASI
  interfaces** even for a trivial plugin; building for
  `wasm32-unknown-unknown` and wrapping with `wasm-tools component new`
  yields **zero WASI imports** (local experiment). `cargo-component` is
  being deprecated; wit-bindgen 0.63.0 is current. On that target
  `getrandom` needs a custom backend and `SystemTime::now()` panics.

## Decision

1. **wasmtime, pinned to the LTS line (48.x)**, taking its patch releases.
   wasmi cannot host WIT components today, and replacing WIT with a
   hand-rolled ABI would give up the typed interface the brief asks for.
2. **Smallest feature set:** `component-model`, `runtime`, the chosen
   compiler, `pooling-allocator` off on mobile. **No WASI** (our world has no
   WASI imports) and **no component-model-async** in v1: host imports are
   synchronous from the plugin's view and each call runs on a plugin worker
   thread. This removes most of the code the 2026 advisories touched.
3. **Compile once, then load.** A plugin is compiled when installed or
   updated, and the artifact is cached in app-private storage with an HMAC
   (key in the OS keystore); later launches `deserialize` it only if the
   HMAC, wasmtime version and config hash match, otherwise recompile.
4. **Which compiler is decided by spike S6** on desktop and two Android
   devices (low-end and mid-range), with these candidates:

   | Option | Why | Watch |
   | --- | --- | --- |
   | Winch on device | Fast compile, 2–2.5× Cranelift speed | Component trampolines and features Winch may not cover |
   | Cranelift on device | Fastest code | 100–200 ms compile per plugin; binary size; memory |
   | Pulley in store builds | No JIT on device, small runtime; store builds load only our signed, CI-precompiled artifacts (ADR-0012) | 20× slower execution; artifacts must be trusted |

   S6 also runs wasmi on equivalent core modules as a reference point,
   so you can see what the component model costs us.
5. **Limits:** epoch interruption for CPU budgets (cheaper), a per-call
   wall-clock deadline, `StoreLimits` for memory and tables, plus our own
   quotas on host-side buffers (HTTP bodies, kv), since the runtime does not
   count those (ADR-0004).
6. **Guest toolchain:** `wasm32-unknown-unknown` + wit-bindgen 0.63 +
   `wasm-tools component new`, wrapped by `jf-plugin build`. The SDK
   provides a `getrandom` backend over `crypto.random-bytes` and time over the
   `clock` import, so plugins never touch WASI.
7. **Android CI:** because wasmtime's Android targets are Tier 3, our CI runs
   the plugin-host test suite (including the hostile-plugin suite) on an
   Android emulator for every wasmtime update.

## Consequences

* The brief's "wasmi if it starts faster" cannot be honoured with WIT
  components; S6 still measures it so the trade-off is visible. You
  confirmed on 2026-11-05 that WIT components stay a requirement.
* S6 weighs the results in your trade-off order: execution speed and
  smoothness first, then memory and binary size.
* Expect roughly 2–7 MB of added binary size depending on the compiler
  choice; S6 reports the exact number per platform.
* Monthly wasmtime majors are ignored in favour of LTS patch releases, with a
  planned move every LTS (12 months).

## Verification sources

wasmtime `RELEASES.md` per release branch, `docs/stability-tiers.md`,
`docs/examples-minimal.md`, issues #10757, #11295, #12229, #12691;
github.com/wasmi-labs/wasmi CHANGELOG and issues #897, #1665;
wasmi-benchmarks raw criterion data (2026-09-01); crates.io for
wasm_component_layer, wasm_runtime_layer, wit-bindgen; the Rust platform
support pages for `wasm32-wasip2`, `wasm32-wasip3`, `wasm32-none-component`;
local experiment logs (kept outside the repository).
