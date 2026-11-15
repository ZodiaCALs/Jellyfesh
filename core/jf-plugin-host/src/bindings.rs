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

/// The `remote` export of 1.1 plugins (world `remote-control`), looked up on
/// its own for plugins whose manifest declares `remote` (ADR-0018). It shares
/// the types of the main bindings.
pub mod remote {
    wasmtime::component::bindgen!({
        world: "remote-control",
        path: "../../plugins/wit",
        with: {
            "jellyfesh:plugin/types": super::jellyfesh::plugin::types,
        },
    });
}
