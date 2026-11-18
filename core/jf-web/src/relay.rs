//! Services that refuse browsers, and the relay that reaches them (ADR-0022 §7,
//! `docs/web-relay.md`).
//!
//! A page can only reach servers that answer with CORS headers, and it may not set `Cookie`,
//! `Referer`, `User-Agent` or `Origin`. Which sources need what a page cannot do is decided here,
//! from what is known about the plugins Jellyfesh bundles ([`REFUSING`]): before signing in to
//! one, Jellyfesh Web says so instead of trying ("{Service} doesn't let browsers connect to it
//! directly …", docs/ui.md), unless a relay is set up in Settings › Sources › Relay, which the
//! source's requests then go through. A source not on the list that fails the same way is found
//! out by the page's probe (`platforms/web/src/lib/plugins/diagnose.ts`) and remembered.

use jf_plugin_model::manifest::AllowRule;
use url::Url;

/// A service a browser page cannot use directly, and how to recognise its plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refusing {
    /// The service's name, for "{Service} doesn't let browsers connect to it directly".
    pub service: &'static str,
    /// Plugin ids that reach it.
    pub plugins: &'static [&'static str],
    /// Hosts of its web interface: a plugin whose allowlist names one reaches it too.
    pub hosts: &'static [&'static str],
}

/// What is known about the services of the plugins Jellyfesh bundles:
///
/// * **QQ Music** (the unofficial community plugin): every request to the web player's API
///   (`u.y.qq.com`) carries the account's session in `Cookie`, with the web player's `Referer`,
///   `Origin` and a browser's `User-Agent`; WeChat's QR sign-in (`open.weixin.qq.com`,
///   `lp.open.weixin.qq.com`) answers without CORS headers (Jellyfesh Web reported "Cannot reach
///   WeChat: … does not allow this page (CORS)" in the owner's test of v0.0.6).
/// * **NetEase Cloud Music** (the unofficial community plugin): its requests to `music.163.com`
///   carry the web player's `Referer`, `Origin` and `User-Agent`, and the session lives in
///   cookies the service sets (`MUSIC_U`), which a page can neither read nor send.
///
/// The official plugins for Subsonic and Jellyfin reach the person's own server, which can allow
/// Jellyfesh Web (docs/web.md, "Connecting to your server"); the other official plugins are not
/// listed until something is known about their services.
pub const REFUSING: &[Refusing] = &[
    Refusing {
        service: "QQ Music",
        plugins: &["community.unofficial.qqmusic"],
        hosts: &["u.y.qq.com", "open.weixin.qq.com", "lp.open.weixin.qq.com"],
    },
    Refusing {
        service: "NetEase Cloud Music",
        plugins: &["community.unofficial.netease"],
        hosts: &["music.163.com"],
    },
];

/// The service a plugin reaches that refuses browsers, or `None`: by its id, or by a host its
/// allowlist names.
pub fn refusing_service(plugin: &str, rules: &[AllowRule]) -> Option<&'static str> {
    REFUSING
        .iter()
        .find(|known| {
            known.plugins.contains(&plugin)
                || rules.iter().any(|rule| match rule {
                    AllowRule::Host {
                        host,
                        wildcard: false,
                        ..
                    } => known.hosts.contains(&host.as_str()),
                    AllowRule::Host {
                        host, wildcard: true, ..
                    } => known
                        .hosts
                        .iter()
                        .any(|known| known.ends_with(&format!(".{host}"))),
                    AllowRule::Setting(_) => false,
                })
        })
        .map(|known| known.service)
}

fn is_loopback(host: &url::Host<&str>) -> bool {
    match host {
        url::Host::Domain(name) => *name == "localhost" || name.ends_with(".localhost"),
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => ip.is_loopback(),
    }
}

/// Why a relay address cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressProblem {
    /// Not an `http://` or `https://` address with a host.
    Invalid,
    /// It has a user name or password (the relay uses its token).
    Credentials,
    /// It has a query or a fragment.
    Query,
    /// An https page cannot reach an http relay beyond this computer (mixed content).
    MixedContent,
}

impl AddressProblem {
    /// The code the page translates into words.
    pub fn code(self) -> &'static str {
        match self {
            AddressProblem::Invalid => "invalid",
            AddressProblem::Credentials => "credentials",
            AddressProblem::Query => "query",
            AddressProblem::MixedContent => "mixed-content",
        }
    }
}

/// The relay's address as Jellyfesh Web uses it (`http(s)://host[:port][/path]`, without a
/// trailing slash), or why it cannot be used: on an https page only an https relay, or one on
/// this computer (`localhost`, `127.0.0.1`, `[::1]`), can be reached (mixed content).
pub fn relay_address(text: &str, page_is_https: bool) -> Result<String, AddressProblem> {
    let url = Url::parse(text.trim()).map_err(|_| AddressProblem::Invalid)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AddressProblem::Invalid);
    }
    let host = url.host().ok_or(AddressProblem::Invalid)?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AddressProblem::Credentials);
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(AddressProblem::Query);
    }
    if page_is_https && url.scheme() == "http" && !is_loopback(&host) {
        return Err(AddressProblem::MixedContent);
    }
    let mut address = url.as_str().to_owned();
    while address.ends_with('/') {
        address.pop();
    }
    Ok(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(host: &str, wildcard: bool) -> AllowRule {
        AllowRule::Host {
            host: host.into(),
            wildcard,
            port: 443,
        }
    }

    #[test]
    fn the_community_plugins_refuse_browsers() {
        assert_eq!(
            refusing_service("community.unofficial.qqmusic", &[]),
            Some("QQ Music")
        );
        assert_eq!(
            refusing_service("community.unofficial.netease", &[]),
            Some("NetEase Cloud Music")
        );
        // A test build whose allowlist is a mock's origin keeps its id.
        assert_eq!(
            refusing_service(
                "community.unofficial.qqmusic",
                &[AllowRule::Setting("mock_origin".into())]
            ),
            Some("QQ Music")
        );
        // Another plugin for the same service is recognised by its hosts.
        assert_eq!(
            refusing_service(
                "org.example.qq",
                &[host("y.gtimg.cn", false), host("u.y.qq.com", false)]
            ),
            Some("QQ Music")
        );
        assert_eq!(
            refusing_service("org.example.wx", &[host("weixin.qq.com", true)]),
            Some("QQ Music")
        );
        assert_eq!(
            refusing_service("org.example.ne", &[host("music.163.com", false)]),
            Some("NetEase Cloud Music")
        );
    }

    #[test]
    fn servers_and_other_plugins_do_not() {
        assert_eq!(
            refusing_service(
                "org.jellyfesh.subsonic",
                &[AllowRule::Setting("server_url".into())]
            ),
            None
        );
        assert_eq!(
            refusing_service(
                "org.jellyfesh.jellyfin",
                &[AllowRule::Setting("server_url".into())]
            ),
            None
        );
        assert_eq!(
            refusing_service("org.example.music", &[host("api.example.com", false)]),
            None
        );
        // y.gtimg.cn alone (pictures) is not the service's web interface.
        assert_eq!(
            refusing_service("org.example.pics", &[host("y.gtimg.cn", false)]),
            None
        );
        assert_eq!(
            refusing_service("org.example.qq", &[host("qq.com.evil.example", false)]),
            None
        );
    }

    #[test]
    fn relay_addresses_are_checked() {
        assert_eq!(
            relay_address(" http://127.0.0.1:8790/ ", true).as_deref(),
            Ok("http://127.0.0.1:8790")
        );
        assert_eq!(
            relay_address("http://localhost:8790", true).as_deref(),
            Ok("http://localhost:8790")
        );
        assert_eq!(
            relay_address("http://[::1]:8790", true).as_deref(),
            Ok("http://[::1]:8790")
        );
        assert_eq!(
            relay_address("https://relay.example.com/jellyfesh/", true).as_deref(),
            Ok("https://relay.example.com/jellyfesh")
        );
        assert_eq!(
            relay_address("http://192.168.1.20:8790", false).as_deref(),
            Ok("http://192.168.1.20:8790"),
            "an http page may use an http relay"
        );
        assert!(
            relay_address("http://192.168.1.20:8790", true).is_err(),
            "mixed content"
        );
        for bad in [
            "",
            "127.0.0.1:8790",
            "ftp://relay.example.com",
            "https://user:pw@relay.example.com",
            "https://relay.example.com/?token=x",
            "https://relay.example.com/#x",
        ] {
            assert!(relay_address(bad, false).is_err(), "{bad}");
        }
    }
}
