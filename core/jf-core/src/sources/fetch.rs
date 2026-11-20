//! Downloading a source track's artwork, for theme colours and backdrops.
//!
//! The URL comes from the plugin through the host, which only passes URLs
//! on the plugin's network allowlist; the request carries the plugin's
//! headers for it (`image-ref.headers`) and no cookies. Redirects are
//! followed only within the same origin, so those headers never travel to
//! another host. Bodies are capped.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use jf_plugin_host::ImageRef;

/// Largest picture downloaded.
const MAX_IMAGE: u64 = 16 * 1024 * 1024;
/// Redirect hops followed (same origin only).
const MAX_REDIRECTS: usize = 3;

/// Downloads a picture, or says why not.
pub(crate) fn image(image: &ImageRef) -> Result<Vec<u8>, String> {
    let mut url = url::Url::parse(&image.url).map_err(|e| format!("bad artwork URL: {e}"))?;
    for _ in 0..=MAX_REDIRECTS {
        if !matches!(url.scheme(), "http" | "https") {
            return Err(format!(
                "artwork URLs must be http or https, not {}",
                url.scheme()
            ));
        }
        let mut request = agent().get(url.as_str());
        for header in &image.headers {
            request = request.header(header.name.as_str(), header.value.as_str());
        }
        let mut response = request.call().map_err(|e| format!("fetching artwork: {e}"))?;
        let status = response.status().as_u16();
        if matches!(status, 301 | 302 | 303 | 307 | 308) {
            let next = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .and_then(|location| url.join(location).ok())
                .ok_or("the artwork server sent a bad redirect")?;
            if next.origin() != url.origin() {
                return Err("the artwork server redirected to another host".into());
            }
            url = next;
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(format!("the artwork server answered {status}"));
        }
        return response
            .body_mut()
            .with_config()
            .limit(MAX_IMAGE)
            .read_to_vec()
            .map_err(|e| format!("reading artwork: {e}"));
    }
    Err("the artwork server redirected too often".into())
}

/// The core's HTTP agent: rustls with the OS roots, no redirects followed,
/// statuses as answers rather than errors, timeouts.
pub(crate) fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};
        let native = rustls_native_certs::load_native_certs();
        let roots: Vec<Certificate<'static>> = native
            .certs
            .iter()
            .map(|der| Certificate::from_der(der.as_ref()).to_owned())
            .collect();
        let tls = TlsConfig::builder()
            .provider(TlsProvider::Rustls)
            .root_certs(RootCerts::new_with_certs(&roots))
            .unversioned_rustls_crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .build();
        ureq::Agent::config_builder()
            .tls_config(tls)
            .http_status_as_error(false)
            .max_redirects(0)
            .user_agent(concat!("jellyfesh/", env!("CARGO_PKG_VERSION")))
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_recv_response(Some(Duration::from_secs(15)))
            .timeout_per_read(Some(Duration::from_secs(15)))
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .new_agent()
    })
}
