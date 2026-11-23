//! Host bindings for the `jellyfesh:plugin` worlds, generated from
//! `plugins/wit` by wasmtime's `bindgen!`.
//!
//! The main bindings are the `source` world of API 1.0 (every plugin has
//! it). The exports added later live in worlds of their own, looked up only
//! for plugins whose manifest declares them, as ADR-0018 did for `remote`:
//! `remote-control` (1.1), `maker-part`, `appearance-part`, `discover-part`,
//! `details-part` and `comments-part` (1.2), `library-part` and
//! `download-part` (1.3). `maker-part` also imports
//! `audio` and `speech`, the imports 1.2 added; the runtime links them for
//! every plugin.
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

/// The `maker` export of 1.2 plugins and the `audio` and `speech` imports
/// (world `maker-part`, ADR-0019).
pub mod maker {
    wasmtime::component::bindgen!({
        world: "maker-part",
        path: "../../plugins/wit",
        with: {
            "jellyfesh:plugin/types": super::jellyfesh::plugin::types,
        },
    });
}

/// The `appearance` export of 1.2 plugins (world `appearance-part`, ADR-0020).
pub mod appearance {
    wasmtime::component::bindgen!({
        world: "appearance-part",
        path: "../../plugins/wit",
        with: {
            "jellyfesh:plugin/types": super::jellyfesh::plugin::types,
        },
    });
}

/// The `discover` export of 1.2 plugins (world `discover-part`).
pub mod discover {
    wasmtime::component::bindgen!({
        world: "discover-part",
        path: "../../plugins/wit",
        with: {
            "jellyfesh:plugin/types": super::jellyfesh::plugin::types,
        },
    });
}

/// The `details` export of 1.2 plugins (world `details-part`).
pub mod details {
    wasmtime::component::bindgen!({
        world: "details-part",
        path: "../../plugins/wit",
        with: {
            "jellyfesh:plugin/types": super::jellyfesh::plugin::types,
        },
    });
}

/// The `comments` export of 1.2 plugins (world `comments-part`).
pub mod comments {
    wasmtime::component::bindgen!({
        world: "comments-part",
        path: "../../plugins/wit",
        with: {
            "jellyfesh:plugin/types": super::jellyfesh::plugin::types,
        },
    });
}

/// The `library` export of 1.3 plugins (world `library-part`, ADR-0024).
pub mod library {
    wasmtime::component::bindgen!({
        world: "library-part",
        path: "../../plugins/wit",
        with: {
            "jellyfesh:plugin/types": super::jellyfesh::plugin::types,
        },
    });
}

/// The `download` export of 1.3 plugins (world `download-part`, ADR-0025).
pub mod download {
    wasmtime::component::bindgen!({
        world: "download-part",
        path: "../../plugins/wit",
        with: {
            "jellyfesh:plugin/types": super::jellyfesh::plugin::types,
        },
    });
}
