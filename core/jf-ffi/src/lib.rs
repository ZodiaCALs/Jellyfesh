//! Foreign-language bindings for `jf-core` (ADR-0002, `docs/ffi.md`).
//!
//! * **Swift and Kotlin** use UniFFI (proc-macro, library mode, namespace `jellyfesh`):
//!   `cargo xtask bindings` generates them from the built library.
//! * **C#** uses the C ABI in [`c_api`], which exchanges the same types as UTF-8 JSON, and the
//!   hand-written `Jellyfesh.Ffi` library in `csharp/`.
//!
//! Both expose [`JellyfeshCore`], which wraps `jf_core::Core`, calls a [`CoreListener`] with
//! every event, keeps the sign-ins of online sources in the app's [`SecretStore`] when it passes
//! one, and converts between the core's types and the mirrors in [`types`]. On Android the app
//! also hands its `Context` to the audio output once (`android`).

// UniFFI's scaffolding (`setup_scaffolding!`, the derives and `#[uniffi::export]`) expands to
// `unsafe extern "C"` functions and `unsafe impl`s in this crate, so the workspace's
// `unsafe_code = "deny"` cannot hold here. Hand-written unsafe code stays in the boundary
// modules (`c_api`, `android`), and every block states its invariants.
#![allow(unsafe_code)]

uniffi::setup_scaffolding!("jellyfesh");

pub mod android;
mod api;
pub mod c_api;
mod convert;
mod error;
mod secrets;
pub mod types;

pub use api::{
    CoreListener, JellyfeshCore, default_cache_dir, default_data_dir, eq_presets, library_version,
    monotonic_now_ns, output_devices,
};
pub use error::CoreFfiError;
pub use secrets::{SecretStore, SecretStoreError};
pub use types::*;
