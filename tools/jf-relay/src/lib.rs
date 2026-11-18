//! `jf-relay`: a relay you run yourself, so Jellyfesh Web can reach services that refuse
//! browsers (ADR-0022 §7, `docs/web-relay.md`).
//!
//! A web page can only reach servers that answer with CORS headers, and it may not set
//! `Cookie`, `Referer`, `User-Agent` or `Origin` at all. Some services' web interfaces need both
//! (the unofficial QQ Music and NetEase Cloud Music plugins'). Jellyfesh Web can send such
//! requests through this relay instead: it wraps the request in a few headers, the relay checks
//! it and makes it, and answers with the service's status, headers and body, plus CORS headers
//! for Jellyfesh Web's origin.
//!
//! What the relay checks ([`relay`]): the request's `Origin` is one given with `--origin`; it
//! carries the token the person set; the URL is on the allowlist ([`allow`]: the fixed hosts of
//! the plugins Jellyfesh bundles, and `--allow` entries); the address the host resolves to is
//! public for a fixed host. It never follows a redirect (the page checks each hop against the
//! plugin's own allowlist), keeps nothing, and logs no path, query, header, body or token.
//!
//! The Jellyfesh project runs no relay: the person runs this on their computer (it listens on
//! 127.0.0.1 by default) or on their own server.

pub mod allow;
pub mod http;
pub mod relay;

pub use allow::Allow;
pub use relay::{Config, Log, MIN_TOKEN, Relay, new_token, normalize_origin};
