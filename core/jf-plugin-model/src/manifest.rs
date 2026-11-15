//! `manifest.toml`: the strict schema of ADR-0004.
//!
//! ```toml
//! id = "org.jellyfesh.subsonic"      # reverse-DNS, lowercase, unique
//! name = "Subsonic"
//! version = "1.0.0"                  # semver of the plugin
//! api_version = "1.0"                # WIT package major.minor it targets
//! author = "Jellyfesh"
//! description = "Navidrome and other Subsonic servers"   # optional
//! official = true                    # display claim only; trust comes from the signature
//! capabilities = ["auth", "search", "browse", "resolve_stream", "lyrics", "artwork"]
//! playback_mode = "stream"           # stream | vendor | remote
//! auth = ["password-exchange", "api-key"]   # qr | oauth | cookie-import | password-exchange | api-key
//!
//! [network]
//! allow = ["${settings.server_url}", "api.example.com", "*.cdn.example.com", "example.org:8443"]
//! max_rps = 10                       # 1–20, default 5; larger values are capped at 20
//!
//! [settings]
//! schema = "settings.schema.json"    # a file in the package, see `schema`
//!
//! [content_policy]                   # optional; permissive defaults
//! unified_search = true
//! mixed_queue = true
//! library_storage = "full"           # none | metadata | full
//! metadata_ttl = "24h"               # <n>s | <n>m | <n>h | <n>d
//! lyrics_sync = true
//! attribution = "example"
//!
//! [cookie_import]                    # required with auth = ["cookie-import"]
//! url = "https://music.example.com/" # or "${settings.<key>}"; must be allowlisted
//! names = ["session", "csrf"]        # cookies the host accepts from the pasted header
//!
//! [links]                            # API 1.1: open an item in its service
//! track = "https://music.example.com/song/{id}"    # also album, artist, playlist
//! ```
//!
//! API 1.1 also adds the `remote` capability, which goes with
//! `playback_mode = "remote"` (ADR-0018).
//!
//! Unknown keys anywhere are an error, so a typo never silently weakens a
//! policy. `network.allow` entries are a settings reference (the origin the
//! person entered, any scheme and address) or a fixed host name with an
//! optional port and an optional leading `*.` (https only, public addresses
//! only); IP literals, schemes and paths are not accepted there.

use serde::{Deserialize, Serialize};

use crate::error::HostError;
use crate::links::Links;
use crate::types::{AuthKind, Capability, ContentPolicy, LibraryStorage, PlaybackMode};
use crate::{API_MAJOR, API_MINOR};

/// Most entries allowed in `network.allow`.
const MAX_ALLOW_RULES: usize = 32;
/// The cap on `network.max_rps` (ADR-0004).
pub const MAX_RPS_CAP: u32 = 20;
/// `network.max_rps` when the manifest has none.
const DEFAULT_MAX_RPS: u32 = 5;

/// A validated manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    /// Reverse-DNS id: lowercase labels of `a-z 0-9 -`, at least two, ≤ 64 bytes.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Plugin version.
    pub version: semver::Version,
    /// Targeted WIT version.
    pub api_version: ApiVersion,
    /// Author.
    pub author: String,
    /// Optional one-line description.
    pub description: Option<String>,
    /// The manifest's own claim to be official: display metadata only.
    pub official_claim: bool,
    /// Declared capabilities, without duplicates.
    pub capabilities: Vec<Capability>,
    /// Playback mode.
    pub playback_mode: PlaybackMode,
    /// Login kinds, without duplicates.
    pub auth: Vec<AuthKind>,
    /// Network allowlist rules.
    pub allow: Vec<AllowRule>,
    /// Request rate limit, 1–20 per second.
    pub max_rps: u32,
    /// File name of the settings schema in the package.
    pub settings_schema: Option<String>,
    /// Content policy.
    pub content_policy: ContentPolicy,
    /// Cookie import rules, present when `auth` lists `cookie-import`.
    pub cookie_import: Option<CookieImport>,
    /// Link templates (API 1.1).
    pub links: Links,
}

/// `api_version`: the WIT package `major.minor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiVersion {
    /// Must equal the host's major version.
    pub major: u64,
    /// Must not be newer than the host's minor version.
    pub minor: u64,
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// One `network.allow` entry. Serialisable, so a host can hand the rules to
/// a sandbox that runs elsewhere (Jellyfesh Web's plugin workers).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllowRule {
    /// `${settings.<key>}`: the origin of the URL the person entered.
    Setting(String),
    /// A fixed host (https only).
    Host {
        /// The host name, lowercase.
        host: String,
        /// Whether `*.` preceded it: any subdomain of `host`, not `host` itself.
        wildcard: bool,
        /// Port, 443 unless given.
        port: u16,
    },
}

/// `[cookie_import]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieImport {
    /// Where the cookies belong: a settings reference or an https URL.
    pub url: CookieUrl,
    /// The cookie names the host accepts.
    pub names: Vec<String>,
}

/// The URL a cookie import is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CookieUrl {
    /// `${settings.<key>}`.
    Setting(String),
    /// A fixed https URL.
    Fixed(url::Url),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    id: String,
    name: String,
    version: String,
    api_version: String,
    author: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    official: bool,
    capabilities: Vec<RawCapability>,
    playback_mode: RawPlaybackMode,
    #[serde(default)]
    auth: Vec<RawAuthKind>,
    #[serde(default)]
    network: Option<RawNetwork>,
    #[serde(default)]
    settings: Option<RawSettings>,
    #[serde(default)]
    content_policy: Option<RawContentPolicy>,
    #[serde(default)]
    cookie_import: Option<RawCookieImport>,
    #[serde(default)]
    links: Option<RawLinks>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RawCapability {
    Auth,
    Search,
    Browse,
    ResolveStream,
    Lyrics,
    Artwork,
    Recommendations,
    Remote,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLinks {
    #[serde(default)]
    track: Option<String>,
    #[serde(default)]
    album: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    playlist: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawPlaybackMode {
    Stream,
    Vendor,
    Remote,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum RawAuthKind {
    Qr,
    Oauth,
    CookieImport,
    PasswordExchange,
    ApiKey,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNetwork {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    max_rps: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSettings {
    schema: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContentPolicy {
    #[serde(default)]
    unified_search: Option<bool>,
    #[serde(default)]
    mixed_queue: Option<bool>,
    #[serde(default)]
    library_storage: Option<RawLibraryStorage>,
    #[serde(default)]
    metadata_ttl: Option<String>,
    #[serde(default)]
    lyrics_sync: Option<bool>,
    #[serde(default)]
    attribution: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RawLibraryStorage {
    None,
    Metadata,
    Full,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCookieImport {
    url: String,
    names: Vec<String>,
}

fn bad(message: impl Into<String>) -> HostError {
    HostError::Manifest(message.into())
}

/// Parses and validates `manifest.toml`. The API version is checked
/// separately ([`Manifest::check_api_version`]) so a mismatch gets its own error.
pub fn parse(text: &str) -> Result<Manifest, HostError> {
    let raw: RawManifest = toml::from_str(text).map_err(|e| bad(e.to_string()))?;

    if !valid_id(&raw.id) {
        return Err(bad(format!(
            "id {:?} must be reverse-DNS: lowercase labels of a-z, 0-9 and '-', at least two, at most 64 bytes",
            raw.id
        )));
    }
    let name = display_text("name", &raw.name, 64)?;
    let author = display_text("author", &raw.author, 128)?;
    let description = match raw.description {
        Some(d) => Some(display_text("description", &d, 300)?),
        None => None,
    };
    let version = semver::Version::parse(&raw.version)
        .map_err(|e| bad(format!("version {:?} is not semver: {e}", raw.version)))?;
    let api_version = parse_api_version(&raw.api_version)?;

    let mut capabilities = Vec::new();
    for raw_cap in &raw.capabilities {
        let cap = match raw_cap {
            RawCapability::Auth => Capability::Auth,
            RawCapability::Search => Capability::Search,
            RawCapability::Browse => Capability::Browse,
            RawCapability::ResolveStream => Capability::ResolveStream,
            RawCapability::Lyrics => Capability::Lyrics,
            RawCapability::Artwork => Capability::Artwork,
            RawCapability::Recommendations => Capability::Recommendations,
            RawCapability::Remote => Capability::Remote,
        };
        if capabilities.contains(&cap) {
            return Err(bad(format!("capability {cap} is listed twice")));
        }
        capabilities.push(cap);
    }
    if capabilities.is_empty() {
        return Err(bad("capabilities must not be empty"));
    }

    let playback_mode = match raw.playback_mode {
        RawPlaybackMode::Stream => PlaybackMode::Stream,
        RawPlaybackMode::Vendor => PlaybackMode::Vendor,
        RawPlaybackMode::Remote => PlaybackMode::Remote,
    };
    let needs_1_1 = |what: &str| -> Result<(), HostError> {
        if api_version.major == 1 && api_version.minor < 1 {
            return Err(bad(format!("{what} needs api_version 1.1")));
        }
        Ok(())
    };
    let remote = capabilities.contains(&Capability::Remote);
    if remote {
        needs_1_1("capability remote")?;
        if playback_mode != PlaybackMode::Remote {
            return Err(bad("capability remote needs playback_mode = \"remote\""));
        }
        if capabilities.contains(&Capability::ResolveStream) {
            return Err(bad("capability remote does not go with resolve_stream"));
        }
    } else if playback_mode == PlaybackMode::Remote {
        return Err(bad("playback_mode = \"remote\" needs capability remote"));
    }

    let mut auth = Vec::new();
    for raw_kind in &raw.auth {
        let kind = match raw_kind {
            RawAuthKind::Qr => AuthKind::Qr,
            RawAuthKind::Oauth => AuthKind::Oauth,
            RawAuthKind::CookieImport => AuthKind::CookieImport,
            RawAuthKind::PasswordExchange => AuthKind::PasswordExchange,
            RawAuthKind::ApiKey => AuthKind::ApiKey,
        };
        if auth.contains(&kind) {
            return Err(bad(format!("auth kind {kind} is listed twice")));
        }
        auth.push(kind);
    }
    let has_auth_capability = capabilities.contains(&Capability::Auth);
    if has_auth_capability && auth.is_empty() {
        return Err(bad("capability auth needs at least one kind in `auth`"));
    }
    if !has_auth_capability && !auth.is_empty() {
        return Err(bad("`auth` kinds need the auth capability"));
    }

    let (allow, max_rps) = match raw.network {
        Some(network) => {
            if network.allow.len() > MAX_ALLOW_RULES {
                return Err(bad(format!(
                    "network.allow has more than {MAX_ALLOW_RULES} entries"
                )));
            }
            let allow = network
                .allow
                .iter()
                .map(|entry| parse_allow_rule(entry))
                .collect::<Result<Vec<_>, _>>()?;
            let max_rps = match network.max_rps {
                Some(0) => return Err(bad("network.max_rps must be at least 1")),
                Some(n) => n.min(MAX_RPS_CAP),
                None => DEFAULT_MAX_RPS,
            };
            (allow, max_rps)
        }
        None => (Vec::new(), DEFAULT_MAX_RPS),
    };

    let settings_schema = match raw.settings {
        Some(settings) => {
            let file = settings.schema;
            if !valid_schema_file_name(&file) {
                return Err(bad(format!(
                    "settings.schema {file:?} must be a plain file name ending in .json (no folders)"
                )));
            }
            Some(file)
        }
        None => None,
    };

    let content_policy = match raw.content_policy {
        Some(raw) => {
            let defaults = ContentPolicy::default();
            ContentPolicy {
                unified_search: raw.unified_search.unwrap_or(defaults.unified_search),
                mixed_queue: raw.mixed_queue.unwrap_or(defaults.mixed_queue),
                library_storage: match raw.library_storage {
                    Some(RawLibraryStorage::None) => LibraryStorage::None,
                    Some(RawLibraryStorage::Metadata) => LibraryStorage::Metadata,
                    Some(RawLibraryStorage::Full) | None => LibraryStorage::Full,
                },
                metadata_ttl_secs: match raw.metadata_ttl {
                    Some(ttl) => Some(parse_ttl(&ttl)?),
                    None => None,
                },
                lyrics_sync: raw.lyrics_sync.unwrap_or(defaults.lyrics_sync),
                attribution: match raw.attribution {
                    Some(a) => Some(display_text("content_policy.attribution", &a, 64)?),
                    None => None,
                },
            }
        }
        None => ContentPolicy::default(),
    };

    let cookie_import = match raw.cookie_import {
        Some(raw) => {
            if !auth.contains(&AuthKind::CookieImport) {
                return Err(bad("[cookie_import] needs auth kind cookie-import"));
            }
            let url = match setting_reference(&raw.url) {
                Some(key) => {
                    let key = key?;
                    if !allow.contains(&AllowRule::Setting(key.clone())) {
                        return Err(bad(format!(
                            "cookie_import.url ${{settings.{key}}} must also be in network.allow"
                        )));
                    }
                    CookieUrl::Setting(key)
                }
                None => {
                    let url = url::Url::parse(&raw.url)
                        .map_err(|e| bad(format!("cookie_import.url {:?}: {e}", raw.url)))?;
                    if url.scheme() != "https" || url.host_str().is_none() {
                        return Err(bad(
                            "cookie_import.url must be an https URL or a settings reference",
                        ));
                    }
                    let host = url.host_str().unwrap_or_default();
                    let port = url.port_or_known_default().unwrap_or(443);
                    if !allow.iter().any(|rule| rule.matches_fixed(host, port)) {
                        return Err(bad("cookie_import.url must be covered by network.allow"));
                    }
                    CookieUrl::Fixed(url)
                }
            };
            if raw.names.is_empty() || raw.names.len() > 32 {
                return Err(bad("cookie_import.names must list 1–32 cookie names"));
            }
            for name in &raw.names {
                if !valid_cookie_name(name) {
                    return Err(bad(format!("cookie_import.names: {name:?} is not a cookie name")));
                }
            }
            Some(CookieImport {
                url,
                names: raw.names,
            })
        }
        None => {
            if auth.contains(&AuthKind::CookieImport) {
                return Err(bad("auth kind cookie-import needs a [cookie_import] table"));
            }
            None
        }
    };

    let links = match raw.links {
        Some(raw_links) => {
            needs_1_1("[links]")?;
            let check = |key: &str, value: Option<String>| -> Result<Option<String>, HostError> {
                value.map(|t| crate::links::check_template(key, &t)).transpose()
            };
            let links = Links {
                track: check("track", raw_links.track)?,
                album: check("album", raw_links.album)?,
                artist: check("artist", raw_links.artist)?,
                playlist: check("playlist", raw_links.playlist)?,
            };
            if links.is_empty() {
                return Err(bad("[links] must hold at least one template"));
            }
            links
        }
        None => Links::default(),
    };

    Ok(Manifest {
        id: raw.id,
        name,
        version,
        api_version,
        author,
        description,
        official_claim: raw.official,
        capabilities,
        playback_mode,
        auth,
        allow,
        max_rps,
        settings_schema,
        content_policy,
        cookie_import,
        links,
    })
}

impl Manifest {
    /// Checks `api_version` against the host: same major, minor not newer.
    pub fn check_api_version(&self) -> Result<(), HostError> {
        let v = self.api_version;
        if v.major == API_MAJOR && v.minor <= API_MINOR {
            Ok(())
        } else {
            Err(HostError::ApiVersion {
                found: v.to_string(),
                supported: format!("{API_MAJOR}.{API_MINOR}"),
            })
        }
    }

    /// Settings keys the allowlist (and cookie import) refer to.
    pub fn referenced_settings(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self
            .allow
            .iter()
            .filter_map(|rule| match rule {
                AllowRule::Setting(key) => Some(key.as_str()),
                AllowRule::Host { .. } => None,
            })
            .collect();
        if let Some(CookieImport {
            url: CookieUrl::Setting(key),
            ..
        }) = &self.cookie_import
        {
            keys.push(key);
        }
        keys.sort_unstable();
        keys.dedup();
        keys
    }
}

impl AllowRule {
    /// Whether this fixed-host rule covers `host:port`.
    pub fn matches_fixed(&self, host: &str, port: u16) -> bool {
        match self {
            AllowRule::Setting(_) => false,
            AllowRule::Host {
                host: pattern,
                wildcard,
                port: rule_port,
            } => {
                if port != *rule_port {
                    return false;
                }
                let host = host.trim_end_matches('.').to_ascii_lowercase();
                if *wildcard {
                    host.len() > pattern.len() + 1
                        && host.ends_with(pattern.as_str())
                        && host.as_bytes()[host.len() - pattern.len() - 1] == b'.'
                } else {
                    host == *pattern
                }
            }
        }
    }
}

fn parse_api_version(text: &str) -> Result<ApiVersion, HostError> {
    let error = || bad(format!("api_version {text:?} must be MAJOR.MINOR, like \"1.0\""));
    let (major, minor) = text.split_once('.').ok_or_else(error)?;
    let number = |s: &str| {
        if s.is_empty() || s.len() > 6 || !s.bytes().all(|b| b.is_ascii_digit()) {
            Err(error())
        } else {
            s.parse::<u64>().map_err(|_| error())
        }
    };
    Ok(ApiVersion {
        major: number(major)?,
        minor: number(minor)?,
    })
}

fn parse_ttl(text: &str) -> Result<u64, HostError> {
    let error = || {
        bad(format!(
            "content_policy.metadata_ttl {text:?} must look like 30s, 15m, 24h or 7d"
        ))
    };
    let (number, unit) = text.split_at(text.len().saturating_sub(1));
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) || number.len() > 9 {
        return Err(error());
    }
    let n: u64 = number.parse().map_err(|_| error())?;
    let factor = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        _ => return Err(error()),
    };
    Ok(n * factor)
}

/// `${settings.<key>}` → `Some(Ok(key))`; anything else → `None`.
fn setting_reference(text: &str) -> Option<Result<String, HostError>> {
    let inner = text.strip_prefix("${settings.")?.strip_suffix('}')?;
    Some(if valid_setting_key(inner) {
        Ok(inner.to_string())
    } else {
        Err(bad(format!(
            "{text:?}: settings keys are lowercase letters, digits and '_'"
        )))
    })
}

fn parse_allow_rule(entry: &str) -> Result<AllowRule, HostError> {
    if let Some(key) = setting_reference(entry) {
        return Ok(AllowRule::Setting(key?));
    }
    let error = |why: &str| bad(format!("network.allow entry {entry:?}: {why}"));
    if entry.contains("://") || entry.contains('/') || entry.contains('@') {
        return Err(error("give a host name (and port), not a URL"));
    }
    let (wildcard, rest) = match entry.strip_prefix("*.") {
        Some(rest) => (true, rest),
        None => (false, entry),
    };
    let (host, port) = match rest.rsplit_once(':') {
        Some((host, port)) => {
            let port: u16 = port
                .parse()
                .ok()
                .filter(|p| *p != 0)
                .ok_or_else(|| error("bad port"))?;
            (host, port)
        }
        None => (rest, 443),
    };
    if host.parse::<std::net::IpAddr>().is_ok() || host.starts_with('[') {
        return Err(error("IP addresses are not allowed; use a host name"));
    }
    if !valid_host_name(host) {
        return Err(error("not a valid lowercase host name"));
    }
    let labels = host.split('.').count();
    if labels < 2 {
        return Err(error("a host name needs at least two labels"));
    }
    if wildcard && labels < 2 {
        return Err(error("a wildcard needs a domain of at least two labels"));
    }
    Ok(AllowRule::Host {
        host: host.to_string(),
        wildcard,
        port,
    })
}

fn valid_host_name(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// Reverse-DNS plugin id. Also safe as a path component and a keystore key
/// segment.
pub fn valid_id(id: &str) -> bool {
    id.len() <= 64 && id.split('.').count() >= 2 && valid_host_name(id)
}

pub fn valid_setting_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key.as_bytes()[0].is_ascii_lowercase()
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn valid_schema_file_name(name: &str) -> bool {
    name.len() <= 64
        && name.ends_with(".json")
        && name.len() > ".json".len()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        && !name.starts_with('.')
        && !matches!(name, "manifest.toml" | "plugin.wasm" | "icon.png" | "SIGNATURE")
}

/// RFC 6265 cookie name: an HTTP token.
pub fn valid_cookie_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

fn display_text(field: &str, text: &str, max: usize) -> Result<String, HostError> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > max || trimmed.chars().any(char::is_control) {
        return Err(bad(format!(
            "{field} must be 1–{max} bytes of text without control characters"
        )));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
id = "org.example.music"
name = "Example"
version = "1.2.3"
api_version = "1.0"
author = "Example Ltd"
capabilities = ["search"]
playback_mode = "stream"
"#;

    #[test]
    fn parses_the_adr_example() {
        let text = r#"
id = "org.jellyfesh.subsonic"
name = "Subsonic"
version = "1.0.0"
api_version = "1.0"
author = "Jellyfesh"
official = true
capabilities = ["auth", "search", "browse", "resolve_stream", "lyrics", "artwork"]
playback_mode = "stream"
auth = ["password-exchange", "api-key"]

[network]
allow = ["${settings.server_url}", "api.example.com", "*.cdn.example.com", "example.org:8443"]
max_rps = 10

[settings]
schema = "settings.schema.json"

[content_policy]
unified_search = false
library_storage = "none"
metadata_ttl = "24h"
attribution = "spotify"
"#;
        let m = parse(text).unwrap();
        assert_eq!(m.id, "org.jellyfesh.subsonic");
        assert_eq!(m.version, semver::Version::new(1, 0, 0));
        assert_eq!(m.api_version, ApiVersion { major: 1, minor: 0 });
        assert!(m.official_claim);
        assert_eq!(m.capabilities.len(), 6);
        assert_eq!(m.auth, vec![AuthKind::PasswordExchange, AuthKind::ApiKey]);
        assert_eq!(m.max_rps, 10);
        assert_eq!(
            m.allow,
            vec![
                AllowRule::Setting("server_url".into()),
                AllowRule::Host {
                    host: "api.example.com".into(),
                    wildcard: false,
                    port: 443
                },
                AllowRule::Host {
                    host: "cdn.example.com".into(),
                    wildcard: true,
                    port: 443
                },
                AllowRule::Host {
                    host: "example.org".into(),
                    wildcard: false,
                    port: 8443
                },
            ]
        );
        assert_eq!(m.settings_schema.as_deref(), Some("settings.schema.json"));
        assert!(!m.content_policy.unified_search);
        assert!(m.content_policy.mixed_queue);
        assert_eq!(m.content_policy.library_storage, LibraryStorage::None);
        assert_eq!(m.content_policy.metadata_ttl_secs, Some(86_400));
        assert_eq!(m.referenced_settings(), vec!["server_url"]);
        m.check_api_version().unwrap();
    }

    #[test]
    fn unknown_keys_are_errors() {
        for extra in [
            "colour = \"red\"\n",
            "[network]\nallow = []\nallow_all = true\n",
            "[content_policy]\nmixed = false\n",
            "[settings]\nschema = \"s.json\"\nui = \"x\"\n",
        ] {
            let text = format!("{MINIMAL}{extra}");
            let err = parse(&text).unwrap_err().to_string();
            assert!(err.contains("unknown field"), "{extra}: {err}");
        }
    }

    #[test]
    fn rejects_bad_values() {
        let cases = [
            ("id = \"org.example.music\"", "id = \"Music\""),
            ("id = \"org.example.music\"", "id = \"org/../x\""),
            ("id = \"org.example.music\"", "id = \"single\""),
            ("version = \"1.2.3\"", "version = \"1.2\""),
            ("api_version = \"1.0\"", "api_version = \"1\""),
            ("capabilities = [\"search\"]", "capabilities = []"),
            (
                "capabilities = [\"search\"]",
                "capabilities = [\"search\", \"search\"]",
            ),
            ("capabilities = [\"search\"]", "capabilities = [\"telepathy\"]"),
            ("capabilities = [\"search\"]", "capabilities = [\"auth\"]"),
            ("playback_mode = \"stream\"", "playback_mode = \"magic\""),
            ("name = \"Example\"", "name = \"\""),
        ];
        for (from, to) in cases {
            let text = MINIMAL.replace(from, to);
            assert!(parse(&text).is_err(), "accepted {to}");
        }
    }

    #[test]
    fn auth_kinds_need_the_capability_and_cookie_rules() {
        let with_auth = MINIMAL.replace("[\"search\"]", "[\"search\", \"auth\"]");
        assert!(parse(&format!("{with_auth}auth = [\"qr\"]\n")).is_ok());
        assert!(parse(&format!("{MINIMAL}auth = [\"qr\"]\n")).is_err());
        assert!(parse(&format!("{with_auth}auth = [\"cookie-import\"]\n")).is_err());
        let ok = format!(
            "{with_auth}auth = [\"cookie-import\"]\n[network]\nallow = [\"music.example.com\"]\n\
             [cookie_import]\nurl = \"https://music.example.com/\"\nnames = [\"session\"]\n"
        );
        let m = parse(&ok).unwrap();
        assert_eq!(m.cookie_import.unwrap().names, vec!["session".to_string()]);
        let not_allowed = ok.replace(
            "allow = [\"music.example.com\"]",
            "allow = [\"other.example.com\"]",
        );
        assert!(parse(&not_allowed).is_err());
        let bad_name = ok.replace("[\"session\"]", "[\"sess ion\"]");
        assert!(parse(&bad_name).is_err());
    }

    #[test]
    fn allow_rules_are_host_names_only() {
        for entry in [
            "https://api.example.com",
            "api.example.com/path",
            "127.0.0.1",
            "[::1]",
            "localhost",
            "*.com",
            "API.example.com",
            "api.example.com:0",
            "${settings.Server}",
            "${settings.}",
        ] {
            let text = format!("{MINIMAL}[network]\nallow = [{entry:?}]\n");
            assert!(parse(&text).is_err(), "accepted {entry}");
        }
    }

    #[test]
    fn max_rps_is_capped_and_defaulted() {
        let m = parse(&format!("{MINIMAL}[network]\nmax_rps = 100\n")).unwrap();
        assert_eq!(m.max_rps, MAX_RPS_CAP);
        assert_eq!(parse(MINIMAL).unwrap().max_rps, DEFAULT_MAX_RPS);
        assert!(parse(&format!("{MINIMAL}[network]\nmax_rps = 0\n")).is_err());
    }

    #[test]
    fn api_version_must_match_the_host() {
        for (version, ok) in [
            ("1.0", true),
            ("1.1", true),
            ("1.2", false),
            ("2.0", false),
            ("0.9", false),
        ] {
            let m = parse(&MINIMAL.replace("\"1.0\"", &format!("{version:?}"))).unwrap();
            assert_eq!(m.check_api_version().is_ok(), ok, "{version}");
        }
    }

    #[test]
    fn wildcard_rules_match_subdomains_only() {
        let rule = AllowRule::Host {
            host: "example.com".into(),
            wildcard: true,
            port: 443,
        };
        assert!(rule.matches_fixed("a.example.com", 443));
        assert!(rule.matches_fixed("a.b.example.com", 443));
        assert!(!rule.matches_fixed("example.com", 443));
        assert!(!rule.matches_fixed("badexample.com", 443));
        assert!(!rule.matches_fixed("a.example.com", 8443));
    }

    #[test]
    fn links_need_api_1_1_and_safe_templates() {
        let links = "[links]\ntrack = \"https://music.example.com/song/{id}\"\n";
        let err = parse(&format!("{MINIMAL}{links}")).unwrap_err().to_string();
        assert!(err.contains("needs api_version 1.1"), "{err}");
        let v11 = MINIMAL.replace("api_version = \"1.0\"", "api_version = \"1.1\"");
        let m = parse(&format!("{v11}{links}")).unwrap();
        assert_eq!(
            m.links.url_for(crate::ItemKind::Track, "x 1").as_deref(),
            Some("https://music.example.com/song/x%201")
        );
        assert!(parse(&format!("{v11}[links]\n")).is_err(), "an empty [links]");
        assert!(parse(&format!("{v11}[links]\ntrack = \"http://e.com/{{id}}\"\n")).is_err());
        assert!(parse(&format!("{v11}[links]\nsong = \"https://e.com/{{id}}\"\n")).is_err());
        assert!(parse(&v11).unwrap().links.is_empty());
    }

    #[test]
    fn remote_goes_with_remote_playback_and_api_1_1() {
        let v11 = MINIMAL.replace("api_version = \"1.0\"", "api_version = \"1.1\"");
        let remote = v11
            .replace("[\"search\"]", "[\"search\", \"remote\"]")
            .replace("playback_mode = \"stream\"", "playback_mode = \"remote\"");
        let m = parse(&remote).unwrap();
        assert!(m.capabilities.contains(&Capability::Remote));
        assert_eq!(m.playback_mode, PlaybackMode::Remote);
        // Not in a 1.0 manifest, not with stream playback, not with resolve_stream.
        let on_1_0 = remote.replace("api_version = \"1.1\"", "api_version = \"1.0\"");
        assert!(
            parse(&on_1_0)
                .unwrap_err()
                .to_string()
                .contains("needs api_version 1.1")
        );
        assert!(parse(&remote.replace("playback_mode = \"remote\"", "playback_mode = \"stream\"")).is_err());
        assert!(parse(&remote.replace("\"remote\"]", "\"remote\", \"resolve_stream\"]")).is_err());
        // Remote playback without the capability is an error too.
        assert!(parse(&v11.replace("playback_mode = \"stream\"", "playback_mode = \"remote\"")).is_err());
    }
}
