//! What the relay may reach.
//!
//! * The **fixed hosts of the plugins Jellyfesh bundles**, official and community, read from
//!   their manifests when the relay is built (so a new release of a plugin's allowlist reaches
//!   the relay with the next build). Fixed hosts are https only and must resolve to public
//!   addresses, as in the apps' plugin host.
//! * What the person adds with `--allow`: a host pattern written as in a manifest
//!   (`music.example.com`, `*.example.com`, `music.example.com:8443`; https and public
//!   addresses only), or the origin of their own server (`http://192.168.1.20:4533`), which may
//!   be on the local network, as the server address of a Subsonic or Jellyfin source may.
//!
//! Matching is `jf_plugin_model::allow::Allowlist`'s, the plugin hosts' own rules.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use jf_plugin_model::allow::{Access, Allowlist};
use jf_plugin_model::manifest::{self, AllowRule};
use serde_json::{Map, Value};
use url::Url;

/// The manifests of the plugins the apps bundle: `(folder, manifest.toml)`.
pub const BUNDLED: &[(&str, &str)] = &[
    (
        "plugins/subsonic",
        include_str!("../../../plugins/subsonic/manifest.toml"),
    ),
    (
        "plugins/jellyfin",
        include_str!("../../../plugins/jellyfin/manifest.toml"),
    ),
    (
        "plugins/qqmusic",
        include_str!("../../../plugins/qqmusic/manifest.toml"),
    ),
    (
        "plugins/netease",
        include_str!("../../../plugins/netease/manifest.toml"),
    ),
    (
        "plugins/spotify",
        include_str!("../../../plugins/spotify/manifest.toml"),
    ),
    (
        "plugins/community/netease",
        include_str!("../../../plugins/community/netease/manifest.toml"),
    ),
    (
        "plugins/community/qqmusic",
        include_str!("../../../plugins/community/qqmusic/manifest.toml"),
    ),
];

/// The hosts and origins the relay forwards to.
#[derive(Debug, Clone)]
pub struct Allow {
    rules: Vec<AllowRule>,
    settings: Map<String, Value>,
    list: Allowlist,
    /// Each entry as a person reads it, with where it came from.
    entries: Vec<String>,
}

impl Allow {
    /// Nothing at all.
    pub fn none() -> Allow {
        Allow {
            rules: Vec::new(),
            settings: Map::new(),
            list: Allowlist::default(),
            entries: Vec::new(),
        }
    }

    /// The fixed hosts of every bundled plugin. `${settings.…}` entries (the server a person
    /// enters, as for Subsonic and Jellyfin) name no host until then: add those with
    /// [`Allow::add`].
    pub fn bundled() -> Result<Allow, String> {
        let mut allow = Allow::none();
        for (folder, text) in BUNDLED {
            let manifest =
                manifest::parse(text).map_err(|error| format!("{folder}/manifest.toml: {error}"))?;
            for rule in manifest.allow {
                if let AllowRule::Host { .. } = rule {
                    allow.push(rule.clone(), format!("{} ({})", describe(&rule), manifest.id));
                }
            }
        }
        Ok(allow)
    }

    /// Adds a `--allow` entry: an `http(s)://host[:port]` origin (the person's own server, which
    /// may be on the local network) or a host pattern as in a manifest (https only, public
    /// addresses only).
    pub fn add(&mut self, entry: &str) -> Result<(), String> {
        let entry = entry.trim();
        if entry.contains("://") {
            let url = Url::parse(entry).map_err(|e| format!("--allow {entry}: {e}"))?;
            if !matches!(url.scheme(), "http" | "https") {
                return Err(format!("--allow {entry}: only http and https origins"));
            }
            if !url.username().is_empty() || url.password().is_some() {
                return Err(format!(
                    "--allow {entry}: no user name or password in the address"
                ));
            }
            if url.host_str().is_none_or(str::is_empty) {
                return Err(format!("--allow {entry}: the address has no host"));
            }
            if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
                return Err(format!(
                    "--allow {entry}: give the origin only (scheme, host and port), without a path"
                ));
            }
            let key = format!("allow_{}", self.settings.len());
            self.settings
                .insert(key.clone(), Value::String(url.as_str().to_owned()));
            self.push(
                AllowRule::Setting(key),
                format!("{} (--allow)", url.origin().ascii_serialization()),
            );
            return Ok(());
        }
        let rule = host_rule(entry).map_err(|why| format!("--allow {entry}: {why}"))?;
        self.push(rule.clone(), format!("{} (--allow)", describe(&rule)));
        Ok(())
    }

    fn push(&mut self, rule: AllowRule, entry: String) {
        if !self.rules.contains(&rule) {
            self.rules.push(rule);
            self.entries.push(entry);
            self.list = Allowlist::new(&self.rules, &Value::Object(self.settings.clone()));
        }
    }

    /// Whether `url` may be reached, and how: [`Access::Fixed`] for a fixed host (https, public
    /// addresses only), [`Access::UserServer`] for an origin given with `--allow`.
    pub fn check(&self, url: &Url) -> Result<Access, String> {
        self.list.check(url)
    }

    /// [`Allow::check`] for the parts of a URL, as the resolver sees them.
    pub fn access(&self, scheme: &str, host: &str, port: u16) -> Option<Access> {
        self.list.check_origin(scheme, host, port)
    }

    /// Every entry, for people.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

fn describe(rule: &AllowRule) -> String {
    match rule {
        AllowRule::Setting(key) => format!("${{settings.{key}}}"),
        AllowRule::Host { host, wildcard, port } => {
            let star = if *wildcard { "*." } else { "" };
            if *port == 443 {
                format!("https://{star}{host}")
            } else {
                format!("https://{star}{host}:{port}")
            }
        }
    }
}

/// `host`, `*.domain` or either with `:port`, as a manifest writes them.
fn host_rule(entry: &str) -> Result<AllowRule, String> {
    let entry = entry.to_ascii_lowercase();
    if entry.contains('/') || entry.contains('@') {
        return Err("give a host name (and port), or an origin such as http://192.168.1.20:4533".into());
    }
    let (wildcard, rest) = match entry.strip_prefix("*.") {
        Some(rest) => (true, rest),
        None => (false, entry.as_str()),
    };
    let (host, port) = match rest.rsplit_once(':') {
        Some((host, port)) => (
            host,
            port.parse::<u16>().ok().filter(|p| *p != 0).ok_or("bad port")?,
        ),
        None => (rest, 443),
    };
    if host.parse::<IpAddr>().is_ok() || host.starts_with('[') {
        return Err("for an address, give an origin such as http://192.168.1.20:4533".into());
    }
    let labels: Vec<&str> = host.split('.').collect();
    let valid = host.len() <= 253
        && labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        });
    if !valid {
        return Err("not a host name of at least two labels".into());
    }
    Ok(AllowRule::Host {
        host: host.to_owned(),
        wildcard,
        port,
    })
}

/// Where an address leads, as the apps' plugin host sorts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressClass {
    /// A public address.
    Public,
    /// Loopback, private, link-local, shared or otherwise local: only the person's own server
    /// may be there.
    Local,
    /// Never reachable through the relay (unspecified, broadcast, multicast, reserved).
    Forbidden,
}

/// The class of `ip`.
pub fn classify(ip: IpAddr) -> AddressClass {
    match ip {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => classify_v6(v6),
    }
}

fn classify_v4(ip: Ipv4Addr) -> AddressClass {
    let [a, b, c, _] = ip.octets();
    if ip.is_unspecified() || ip.is_broadcast() || ip.is_multicast() || a >= 240 {
        return AddressClass::Forbidden;
    }
    let local = a == 0
        || a == 10
        || a == 127
        || (a == 100 && (64..128).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..32).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && c == 0)
        || (a == 198 && (18..20).contains(&b));
    if local {
        AddressClass::Local
    } else {
        AddressClass::Public
    }
}

fn classify_v6(ip: Ipv6Addr) -> AddressClass {
    if ip.is_unspecified() || ip.is_multicast() {
        return AddressClass::Forbidden;
    }
    if ip.is_loopback() {
        return AddressClass::Local;
    }
    let segments = ip.segments();
    let embedded = |o: [u8; 16], at: usize| Ipv4Addr::new(o[at], o[at + 1], o[at + 2], o[at + 3]);
    // IPv4-mapped and IPv4-compatible, NAT64's well-known prefix, and 6to4 carry an IPv4 address.
    if segments[..5] == [0; 5] && (segments[5] == 0xffff || segments[5] == 0) {
        return classify_v4(embedded(ip.octets(), 12));
    }
    if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        return classify_v4(embedded(ip.octets(), 12));
    }
    if segments[0] == 0x2002 {
        return classify_v4(embedded(ip.octets(), 2));
    }
    let local = (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] & 0xffc0) == 0xfec0
        || segments[..3] == [0x64, 0xff9b, 1]
        || segments[..2] == [0x2001, 0]
        || segments[..4] == [0x100, 0, 0, 0];
    if local {
        AddressClass::Local
    } else {
        AddressClass::Public
    }
}

/// Whether a host reached with `access` may be at `ip`: fixed hosts only at public addresses,
/// the person's own servers also on the local network, nothing at a forbidden address.
pub fn permitted(access: Access, ip: IpAddr) -> Result<(), String> {
    match (classify(ip), access) {
        (AddressClass::Public, _) | (AddressClass::Local, Access::UserServer) => Ok(()),
        (AddressClass::Local, Access::Fixed) => Err(format!(
            "it resolved to {ip}, a local address; only a server given with --allow http://… may be on the local network"
        )),
        (AddressClass::Forbidden, _) => Err(format!("it resolved to {ip}, which is never allowed")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access(allow: &Allow, url: &str) -> Option<Access> {
        allow.check(&Url::parse(url).unwrap()).ok()
    }

    #[test]
    fn every_bundled_manifest_parses() {
        let allow = Allow::bundled().unwrap();
        assert!(!allow.is_empty());
        // The community plugins' services, which browsers cannot use directly.
        for url in [
            "https://u.y.qq.com/cgi-bin/musics.fcg",
            "https://open.weixin.qq.com/connect/qrconnect",
            "https://lp.open.weixin.qq.com/connect/l/qrconnect",
            "https://ws.stream.qqmusic.qq.com/C400.m4a",
            "https://y.gtimg.cn/music/photo_new/T002R300x300M000.jpg",
            "https://music.163.com/weapi/login/qrcode/unikey",
            "https://p1.music.126.net/a.jpg",
        ] {
            assert_eq!(access(&allow, url), Some(Access::Fixed), "{url}");
        }
        // Official plugins' fixed hosts too.
        assert_eq!(
            access(&allow, "https://api.spotify.com/v1/me"),
            Some(Access::Fixed)
        );
        assert_eq!(access(&allow, "https://a.y.qq.com/"), Some(Access::Fixed));
    }

    #[test]
    fn nothing_else_is_reachable() {
        let allow = Allow::bundled().unwrap();
        for url in [
            "http://u.y.qq.com/cgi-bin/musics.fcg",
            "https://u.y.qq.com:8443/",
            "https://y.qq.com/",
            "https://evil.example/?https://u.y.qq.com",
            "https://u.y.qq.com.evil.example/",
            "https://music.126.net/",
            "http://127.0.0.1:4533/rest/ping",
            "http://192.168.1.20:4533/",
            "ftp://music.163.com/",
        ] {
            assert_eq!(access(&allow, url), None, "{url}");
        }
    }

    #[test]
    fn allow_adds_hosts_and_origins() {
        let mut allow = Allow::none();
        allow.add("music.example.com").unwrap();
        allow.add("*.cdn.example.net:8443").unwrap();
        allow.add("http://192.168.1.20:4533").unwrap();
        allow.add("HTTPS://Navidrome.Example.org/").unwrap();
        assert_eq!(access(&allow, "https://music.example.com/x"), Some(Access::Fixed));
        assert_eq!(
            access(&allow, "http://music.example.com/x"),
            None,
            "fixed hosts are https only"
        );
        assert_eq!(
            access(&allow, "https://a.cdn.example.net:8443/x"),
            Some(Access::Fixed)
        );
        assert_eq!(access(&allow, "https://cdn.example.net:8443/x"), None);
        assert_eq!(
            access(&allow, "http://192.168.1.20:4533/rest"),
            Some(Access::UserServer)
        );
        assert_eq!(access(&allow, "http://192.168.1.20:4534/rest"), None);
        assert_eq!(
            access(&allow, "https://navidrome.example.org/rest"),
            Some(Access::UserServer)
        );
        assert_eq!(allow.entries().len(), 4);
        for bad in [
            "",
            "localhost",
            "192.168.1.20",
            "*.com",
            "music.example.com/path",
            "user@music.example.com",
            "ftp://music.example.com",
            "http://music.example.com/rest",
            "http://user:pw@music.example.com",
            "music.example.com:0",
        ] {
            assert!(Allow::none().add(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn addresses_are_sorted_like_the_plugin_host() {
        for public in ["8.8.8.8", "203.0.113.7", "2606:4700::1111"] {
            assert_eq!(
                classify(public.parse().unwrap()),
                AddressClass::Public,
                "{public}"
            );
        }
        for local in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.20",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:192.168.1.1",
            "2002:c0a8:0101::1",
        ] {
            assert_eq!(classify(local.parse().unwrap()), AddressClass::Local, "{local}");
        }
        for forbidden in [
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
            "240.0.0.1",
            "::",
            "ff02::1",
        ] {
            assert_eq!(
                classify(forbidden.parse().unwrap()),
                AddressClass::Forbidden,
                "{forbidden}"
            );
        }
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(permitted(Access::Fixed, ip("203.0.113.7")).is_ok());
        assert!(
            permitted(Access::Fixed, ip("127.0.0.1")).is_err(),
            "a fixed host on loopback"
        );
        assert!(permitted(Access::Fixed, ip("192.168.1.20")).is_err());
        assert!(permitted(Access::UserServer, ip("192.168.1.20")).is_ok());
        assert!(permitted(Access::UserServer, ip("0.0.0.0")).is_err());
    }
}
