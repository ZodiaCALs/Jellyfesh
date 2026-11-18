//! Test support for Jellyfesh plugins.
//!
//! A plugin's integration tests run the **real component in the real host**
//! against a **mock of the service**:
//!
//! * [`component`] builds the plugin crate for `wasm32-unknown-unknown` and
//!   wraps it into a component, exactly as `jf-plugin build` does (once per
//!   test binary);
//! * [`Env::install_crate`] packs it with the crate's `manifest.toml`,
//!   settings schema and `icon.png`, signs it with the published test key
//!   and installs it into a [`jf_plugin_host::PluginHost`] in a temporary
//!   folder, with production sandbox limits;
//! * [`TestServer`] is a small HTTP/1.1 server on 127.0.0.1 whose handler
//!   answers like the service's documented API. The host lets a plugin reach
//!   a loopback address only because it is the server the person configured,
//!   as it would a home server on the local network;
//! * [`fetch`] requests the URLs a plugin hands out (streams, artwork) the way
//!   the engine and the host would;
//! * [`TestAccess`] and [`make_job`] serve a lyrics maker's `audio` and
//!   `speech` imports the way the app does (API 1.2).
//!
//! ```no_run
//! use jf_plugin_testkit::*;
//!
//! let server = TestServer::start(|request| match request.path() {
//!     "/api/search" => Response::json(&serde_json::json!([])),
//!     _ => Response::text(404, "not found"),
//! });
//! let env = Env::new();
//! let info = env.install_crate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")), "my-plugin");
//! env.settings(&info.id, serde_json::json!({ "server_url": server.url() }));
//! let found = recv(env.host.search(&info.id, "blue", kinds(&[]), page(20))).unwrap();
//! assert!(found.items.is_empty());
//! ```

mod host;
mod maker;
mod server;

pub use host::{
    Env, PluginFiles, component, component_in, fields, final_step, item, kinds, login, next_page, next_step,
    page, recv, target_dir, workspace_root,
};
pub use maker::{TestAccess, make_job, silent_wav, speech_models, transcript};
pub use server::{Fetched, Request, Response, TestServer, closed_port_url, fetch, url_query};
