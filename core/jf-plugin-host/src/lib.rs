//! The Jellyfesh plugin host: runs source plugins (WebAssembly components
//! for the `jellyfesh:plugin/source@1` world in `plugins/wit`) in a sandbox,
//! and gives the core an asynchronous API over them (ADR-0003, ADR-0004,
//! ADR-0012, ADR-0015; `docs/architecture.md` section 10).
//!
//! # Using it
//!
//! ```no_run
//! use std::sync::Arc;
//! use jf_plugin_host::*;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let host = PluginHost::new(HostConfig {
//!     dir: "/app-data/plugins".into(),
//!     cache_dir: "/app-cache/plugins".into(),
//!     secrets: Arc::new(MemorySecretStore::new()), // the OS keystore in the app
//!     allow_community: false,
//! })?;
//! let info = host.install("subsonic-1.0.0.jfpkg".as_ref())?;
//! host.set_settings(&info.id, r#"{"server_url": "http://192.168.1.20:4533"}"#)?;
//!
//! // Calls never block: each returns a channel that receives one answer.
//! let reply = host.search(&info.id, "blue", SearchFilters { kinds: vec![] },
//!                         PageRequest { cursor: None, limit: 50 });
//! match wait(&reply) {
//!     Ok(page) => println!("{} results", page.items.len()),
//!     Err(error) => eprintln!("search failed: {error}"),
//! }
//!
//! // Logins are host-driven state machines; the UI renders each step.
//! let flow = host.begin_auth(&info.id, AuthKind::PasswordExchange)?;
//! while let Ok(step) = flow.steps().recv() {
//!     match step {
//!         AuthStep::AskFields { .. } => flow.submit(vec![/* what the person typed */]),
//!         AuthStep::Done { account } => println!("logged in as {account}"),
//!         _ => {}
//!     }
//! }
//! # Ok(()) }
//! ```
//!
//! # The API
//!
//! Management (synchronous):
//!
//! * [`PluginHost::new`]`(HostConfig { dir, cache_dir, secrets, allow_community })`
//!   and [`PluginHost::with_options`] (trust list, resolver, limits, workers);
//! * [`PluginHost::install`]`(path) -> Result<PluginInfo, HostError>` and
//!   [`PluginHost::uninstall`]`(id)`;
//! * [`PluginHost::plugins`] / [`PluginHost::plugin`] → [`PluginInfo`] (id,
//!   name, version, author, `official` = signed by a trusted key, signature,
//!   capabilities, playback mode, login kinds, enabled, disabled reason,
//!   account, settings schema and values, content policy), and
//!   [`PluginHost::icon`];
//! * [`PluginHost::set_enabled`], [`PluginHost::set_allow_community`],
//!   [`PluginHost::set_settings`]`(id, json)`.
//!
//! Calls (asynchronous: each returns a [`Reply`], a channel that receives
//! exactly one `Result<T, CallError>`; [`wait`] blocks on one):
//! [`PluginHost::search`], [`PluginHost::browse`],
//! [`PluginHost::resolve_stream`], [`PluginHost::lyrics`],
//! [`PluginHost::artwork`], [`PluginHost::recommendations_for_item`],
//! [`PluginHost::recommendations_for_user`], [`PluginHost::capabilities`],
//! [`PluginHost::logout`]. Logins: [`PluginHost::begin_auth`] →
//! [`AuthFlow`] (steps on a channel, [`AuthFlow::submit`],
//! [`AuthFlow::cancel`]).
//!
//! Unified search: [`merge::merge`] merges per-source result lists (ISRC,
//! then normalised title + primary artist + duration within 2 s), keeping
//! each source's order. Fanning out and deadlines are the caller's.
//!
//! Packages: [`package`] reads, verifies and builds `.jfpkg` files and
//! defines the signature format; [`manifest`] and [`schema`] the strict
//! manifest and settings-schema profiles; [`trust`] the trust list.
//!
//! These modules, the data types, the merge, the answer checks, the login
//! state machine and the `crypto` primitives come from `jf-plugin-model`,
//! which needs no WebAssembly runtime: Jellyfesh Web (`core/jf-web`) runs
//! them in the browser. This crate adds wasmtime, the host imports, the
//! threads and the files.
//!
//! # The sandbox
//!
//! | Limit | Default | How |
//! | --- | --- | --- |
//! | CPU per call | 200 ms (2 s for `auth.begin` and start-up) | epoch interruption every 5 ms; guest time = wall time minus time in host imports |
//! | Wall clock per call | 10 s (30 s for login steps) | the same epoch callback; HTTP waits are bounded by it |
//! | Memory | 64 MiB linear memory, 10 000 table elements | `StoreLimits`, growth past them traps |
//! | Network | manifest allowlist, re-checked on every redirect hop; fixed hosts https only | `http` import |
//! | Addresses | private, loopback, link-local, ULA, CGNAT… only for the person's own server; one lookup per hop, connection to the checked address | resolver inside the HTTP client |
//! | HTTP | `max_rps` (≤ 20) per plugin, 16 MiB per response, 64 MiB per call, 30 s per request | token bucket, body limits |
//! | Storage | kv 1 MiB per plugin; secrets only in `jellyfesh/<id>/…`; cookie jar per plugin, AES-256-GCM on disk | `kv` import, [`SecretStore`] |
//! | Strikes | 3 violations in a row turn a plugin off with a reason | host |
//!
//! A trap or violation drops the plugin's instance; the next call starts a
//! fresh one. There are no WASI imports at all: a component that imports
//! anything but `jellyfesh:plugin/*@1` does not link. Plugin answers are
//! checked before they reach the caller: strings are cut, pages limited, and
//! URLs the engine will fetch (streams, artwork) must be on the allowlist.
//!
//! # Threads
//!
//! Two worker threads ([`WORKERS`]) run plugin calls. A plugin's calls run
//! one at a time, in order, on one worker at a time, and each plugin goes to
//! the back of the queue after a call, so a stuck plugin can hold at most one
//! worker and only until its deadline; the caller never waits. One extra
//! thread ticks the epoch while plugin code runs and parks otherwise, so an
//! idle host uses no CPU; each login flow runs on its own thread.
//!
//! # Files
//!
//! `<dir>/packages/<id>.jfpkg` (re-verified at every start),
//! `<dir>/data/<id>/{state,settings,kv}.json` and `cookies.jar`.
//! Components are compiled when installed and, after a restart, on first
//! use: loading a precompiled artifact (`Component::deserialize`) is `unsafe`
//! and the workspace denies `unsafe_code`, so `cache_dir` is reserved for
//! that cache and unused in v0.0.3.

mod auth;
pub mod bindings;
mod convert;
mod cookies;
mod host;
mod http;
mod imports;
pub mod net;
mod plugin;
mod plugin_log;
mod pool;
mod runtime;
pub mod secrets;
mod services;
mod storage;
pub mod types;

// Everything that needs no WebAssembly runtime lives in `jf-plugin-model`
// (shared with Jellyfesh Web); it is re-exported here under the paths the
// core and the tools have always used.
pub(crate) use jf_plugin_model::{allow, sanitize};
pub use jf_plugin_model::{manifest, merge, package, presets, schema, trust};

/// The host API's errors (defined in `jf-plugin-model`).
pub(crate) mod error {
    pub use jf_plugin_model::{CallError, HostError, plugin_error_text};
}

pub use auth::AuthFlow;
pub use error::{CallError, HostError, plugin_error_text};
pub use host::{HostConfig, HostOptions, PluginHost, wait};
pub use net::{Resolve, SystemResolver};
pub use pool::WORKERS;
pub use runtime::Limits;
pub use secrets::{MemorySecretStore, SecretError, SecretStore};
pub use types::*;
