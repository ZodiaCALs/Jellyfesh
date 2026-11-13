//! `jf-plugin`, the tool for Jellyfesh plugin authors (the guide is
//! `docs/plugins/authoring.md`):
//!
//! | Command | Does |
//! | --- | --- |
//! | `new <name>` | a plugin crate from the template ([`scaffold`]) |
//! | `build [--release]` | the crate → a WebAssembly component (`jf-plugin-build`, shared with `cargo xtask plugin build` and the plugin tests) |
//! | `pack` | manifest + component + settings schema + icon → `.jfpkg` ([`pack`]) |
//! | `keygen <file>` | a new ed25519 signing key in a file of mode 0600, never printed ([`keygen`]) |
//! | `sign <package>` | signs with `--key <file>` or `JF_PLUGIN_SIGNING_KEY` ([`sign`]) |
//! | `verify <package>` | the host's install checks plus the component's imports ([`verify`]) |
//! | `inspect <package>` | manifest, capabilities, signature, key id, entries, imports ([`inspect`]) |
//!
//! Packages are read and checked with the plugin host's own code
//! (`jf_plugin_host::package`), so what this tool accepts the app accepts.

mod key;
mod package;
mod scaffold;

pub use key::{KEY_ENV, SigningKey, keygen};
pub use package::{
    PluginFiles, Report, Verified, describe_signature, inspect, pack, read_entries, require, sign,
    trust_list, verify,
};
pub use scaffold::{Scaffold, find_sdk, scaffold};
