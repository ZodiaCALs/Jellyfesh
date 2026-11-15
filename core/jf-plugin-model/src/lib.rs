//! The parts of the Jellyfesh plugin host that need no WebAssembly runtime.
//!
//! `jf-plugin-host` runs plugins in wasmtime and re-exports everything here,
//! so the core and the tools keep using `jf_plugin_host::…` as before.
//! Jellyfesh Web (`core/jf-web`), which runs plugins on the browser's own
//! WebAssembly engine, uses these modules directly: they build for
//! `wasm32-unknown-unknown` and read no clock, file or socket of their own.
//!
//! * [`types`]: the WIT records, enums and variants of `plugins/wit` as plain
//!   Rust (mirrored, with the same `serde` shapes as wasmtime's generated
//!   types), plus installed-plugin and login-step types.
//! * [`package`], [`manifest`], [`schema`], [`trust`]: reading, verifying,
//!   signing and building `.jfpkg` files; the strict manifest and
//!   settings-schema profiles; the trust list with the release key.
//! * [`allow`]: a plugin's effective network allowlist.
//! * [`http`]: the `http` import's request rules and limits.
//! * [`sanitize`]: the checks every plugin answer passes before it is used.
//! * [`merge`]: unified search merging.
//! * [`auth`]: login flows as a state machine without I/O.
//! * [`install`]: which packages may be installed, and the words for refusals.
//! * [`links`]: links that open an item in its service (API 1.1).
//! * [`crypto`] (feature `crypto`): the `crypto` import's primitives.
//! * [`HostError`], [`CallError`]: the host API's errors.

pub mod allow;
pub mod auth;
#[cfg(feature = "crypto")]
pub mod crypto;
mod error;
pub mod http;
pub mod install;
pub mod links;
pub mod manifest;
pub mod merge;
pub mod package;
pub mod presets;
pub mod sanitize;
pub mod schema;
pub mod trust;
pub mod types;

pub use error::{CallError, HostError, plugin_error_text};
pub use types::*;

/// The WIT package major version hosts implement (`jellyfesh:plugin@1`).
pub const API_MAJOR: u64 = 1;
/// The WIT package minor version hosts implement: 1.1 added the `remote`
/// interface and manifest links (ADR-0018).
pub const API_MINOR: u64 = 1;
