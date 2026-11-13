//! The network allowlist of one plugin: the manifest's `[network] allow`
//! rules with `${settings.<key>}` references filled in from the plugin's
//! current settings.

use serde_json::Value;
use url::Url;

use crate::manifest::AllowRule;

/// What an allowed URL may reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The origin the person configured: private and loopback addresses are
    /// allowed (a home server), and so is plain http if they typed it.
    UserServer,
    /// A fixed host from the manifest: https and public addresses only.
    Fixed,
}

/// A scheme, host and port triple.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Origin {
    scheme: String,
    host: String,
    port: u16,
}

/// The effective allowlist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Allowlist {
    user: Vec<Origin>,
    fixed: Vec<AllowRule>,
}

impl Allowlist {
    /// Builds the allowlist for these rules and settings. A settings
    /// reference whose value is empty or not an http(s) URL adds nothing.
    pub fn new(rules: &[AllowRule], settings: &Value) -> Allowlist {
        let mut list = Allowlist::default();
        for rule in rules {
            match rule {
                AllowRule::Setting(key) => {
                    let Some(text) = settings.get(key).and_then(Value::as_str) else {
                        continue;
                    };
                    if let Some(origin) = Url::parse(text).ok().as_ref().and_then(origin_of) {
                        list.user.push(origin);
                    }
                }
                AllowRule::Host { .. } => list.fixed.push(rule.clone()),
            }
        }
        list
    }

    /// Whether `url` may be requested, and with which access.
    pub fn check(&self, url: &Url) -> Result<Access, String> {
        let origin = origin_of(url).ok_or_else(|| format!("{url} is not an http(s) URL with a host"))?;
        self.check_origin(&origin.scheme, &origin.host, origin.port)
            .ok_or_else(|| {
                format!(
                    "{}://{}:{} is not on the allowlist",
                    origin.scheme, origin.host, origin.port
                )
            })
    }

    /// Like [`Allowlist::check`] for parts of a URL (as the resolver sees them).
    pub fn check_origin(&self, scheme: &str, host: &str, port: u16) -> Option<Access> {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        if self
            .user
            .iter()
            .any(|o| o.scheme == scheme && o.host == host && o.port == port)
        {
            return Some(Access::UserServer);
        }
        if scheme == "https" && self.fixed.iter().any(|rule| rule.matches_fixed(&host, port)) {
            return Some(Access::Fixed);
        }
        None
    }

    /// Whether nothing at all is allowed.
    pub fn is_empty(&self) -> bool {
        self.user.is_empty() && self.fixed.is_empty()
    }
}

fn origin_of(url: &Url) -> Option<Origin> {
    let scheme = url.scheme();
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    Some(Origin {
        scheme: scheme.to_string(),
        host,
        port: url.port_or_known_default()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rules() -> Vec<AllowRule> {
        vec![
            AllowRule::Setting("server_url".into()),
            AllowRule::Host {
                host: "api.example.com".into(),
                wildcard: false,
                port: 443,
            },
            AllowRule::Host {
                host: "cdn.example.net".into(),
                wildcard: true,
                port: 443,
            },
        ]
    }

    fn check(list: &Allowlist, url: &str) -> Option<Access> {
        list.check(&Url::parse(url).unwrap()).ok()
    }

    #[test]
    fn user_origin_is_exact() {
        let list = Allowlist::new(
            &rules(),
            &json!({ "server_url": "http://192.168.1.20:4533/navidrome/" }),
        );
        assert_eq!(
            check(&list, "http://192.168.1.20:4533/rest/ping"),
            Some(Access::UserServer)
        );
        assert_eq!(check(&list, "https://192.168.1.20:4533/"), None, "other scheme");
        assert_eq!(check(&list, "http://192.168.1.20:4534/"), None, "other port");
        assert_eq!(check(&list, "http://192.168.1.21:4533/"), None, "other host");
    }

    #[test]
    fn fixed_hosts_are_https_only() {
        let list = Allowlist::new(&rules(), &json!({}));
        assert_eq!(check(&list, "https://api.example.com/v1"), Some(Access::Fixed));
        assert_eq!(check(&list, "https://API.Example.com./v1"), Some(Access::Fixed));
        assert_eq!(check(&list, "http://api.example.com/v1"), None);
        assert_eq!(check(&list, "https://api.example.com:8443/"), None);
        assert_eq!(
            check(&list, "https://x.cdn.example.net/a.flac"),
            Some(Access::Fixed)
        );
        assert_eq!(check(&list, "https://cdn.example.net/"), None);
        assert_eq!(check(&list, "https://evil.com/?https://api.example.com"), None);
        assert_eq!(check(&list, "https://api.example.com.evil.com/"), None);
        assert_eq!(check(&list, "ftp://api.example.com/"), None);
    }

    #[test]
    fn unset_or_bad_settings_add_nothing() {
        for settings in [
            json!({}),
            json!({ "server_url": "" }),
            json!({ "server_url": "file:///etc" }),
        ] {
            let list = Allowlist::new(&[AllowRule::Setting("server_url".into())], &settings);
            assert!(list.is_empty(), "{settings}");
        }
    }
}
