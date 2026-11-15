//! Links that open an item in its service (API 1.1, ADR-0018): `https` URL
//! templates per item kind in the manifest's `[links]` table, filled in with
//! the item's id by the host. The person's browser opens them; the plugin
//! never fetches them, so they need not be in the network allowlist.
//!
//! ```toml
//! [links]
//! track = "https://music.example.com/song/{id}"
//! album = "https://music.example.com/album/{id}"
//! artist = "https://music.example.com/artist/{id}"
//! playlist = "https://music.example.com/playlist?id={id}"
//! ```

use serde::{Deserialize, Serialize};

use crate::{HostError, ItemKind};

/// The placeholder a template holds once.
const PLACEHOLDER: &str = "{id}";
/// The longest template accepted.
const MAX_TEMPLATE: usize = 512;

/// The link templates of a plugin, by item kind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Links {
    pub track: Option<String>,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub playlist: Option<String>,
}

impl Links {
    /// Whether the plugin declares no links.
    pub fn is_empty(&self) -> bool {
        self.track.is_none() && self.album.is_none() && self.artist.is_none() && self.playlist.is_none()
    }

    /// The link that opens the item `id` of `kind` in the service, if the
    /// plugin has a template for that kind and `id` is not empty.
    pub fn url_for(&self, kind: ItemKind, id: &str) -> Option<String> {
        let template = match kind {
            ItemKind::Track => self.track.as_deref(),
            ItemKind::Album => self.album.as_deref(),
            ItemKind::Artist => self.artist.as_deref(),
            ItemKind::Playlist => self.playlist.as_deref(),
            ItemKind::Folder => None,
        }?;
        if id.is_empty() {
            return None;
        }
        Some(template.replacen(PLACEHOLDER, &encode(id), 1))
    }
}

/// Percent-encodes everything but the unreserved characters, so an id is
/// safe in a path segment and in a query value alike.
fn encode(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for byte in id.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Checks one template from the manifest (`key` names it in errors).
pub(crate) fn check_template(key: &str, template: &str) -> Result<String, HostError> {
    let bad = |why: &str| HostError::Manifest(format!("links.{key} {template:?}: {why}"));
    if template.len() > MAX_TEMPLATE {
        return Err(bad("is longer than 512 bytes"));
    }
    if template.matches(PLACEHOLDER).count() != 1 {
        return Err(bad("must hold {id} exactly once"));
    }
    let probe = template.replacen(PLACEHOLDER, "id", 1);
    let url = url::Url::parse(&probe).map_err(|e| bad(&e.to_string()))?;
    if url.scheme() != "https" {
        return Err(bad("must be an https URL"));
    }
    if url.host_str().is_none_or(str::is_empty) || !url.username().is_empty() || url.password().is_some() {
        return Err(bad("must name a host and no user"));
    }
    // {id} must sit in the path, query or fragment, never in the host.
    let after_host = template
        .split_once("://")
        .and_then(|(_, rest)| rest.find('/').map(|at| &rest[at..]))
        .unwrap_or("");
    if !after_host.contains(PLACEHOLDER) {
        return Err(bad("must hold {id} after the host"));
    }
    Ok(template.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links() -> Links {
        Links {
            track: Some("https://music.example.com/song/{id}".into()),
            album: None,
            artist: Some("https://music.example.com/a?id={id}&from=jf".into()),
            playlist: None,
        }
    }

    #[test]
    fn fills_in_the_id_percent_encoded() {
        let l = links();
        assert_eq!(
            l.url_for(ItemKind::Track, "003aBc").as_deref(),
            Some("https://music.example.com/song/003aBc")
        );
        assert_eq!(
            l.url_for(ItemKind::Artist, "a/b c&d").as_deref(),
            Some("https://music.example.com/a?id=a%2Fb%20c%26d&from=jf")
        );
        assert_eq!(l.url_for(ItemKind::Album, "1"), None);
        assert_eq!(l.url_for(ItemKind::Folder, "1"), None);
        assert_eq!(l.url_for(ItemKind::Track, ""), None);
        assert!(!l.is_empty());
        assert!(Links::default().is_empty());
    }

    #[test]
    fn refuses_templates_that_could_go_anywhere() {
        for template in [
            "http://music.example.com/song/{id}",
            "https://music.example.com/song",
            "https://music.example.com/{id}/{id}",
            "https://{id}.example.com/song",
            "https://user@music.example.com/song/{id}",
            "javascript:alert({id})",
            "music.example.com/song/{id}",
        ] {
            assert!(check_template("track", template).is_err(), "{template}");
        }
        assert!(check_template("track", "https://music.example.com/song/{id}").is_ok());
        assert!(check_template("track", "https://music.example.com/#/song?id={id}").is_ok());
        assert!(check_template("track", &format!("https://e.com/{}{{id}}", "a".repeat(600))).is_err());
    }
}
