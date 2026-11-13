//! Host bindings for the `jellyfesh:plugin/source` world, generated from
//! `plugins/wit` by wasmtime's `bindgen!`.
//!
//! The generated types are used only at the runtime boundary: the host's
//! public data types are their plain mirrors in `jf_plugin_model::types`
//! (re-exported from [`crate::types`]), converted in `convert.rs`.

wasmtime::component::bindgen!({
    world: "source",
    path: "../../plugins/wit",
});

/// The WIT package version this host implements.
pub use jf_plugin_model::{API_MAJOR, API_MINOR};
