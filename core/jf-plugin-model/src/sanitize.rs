//! Checks on what plugins return: the host trusts nothing a plugin says.
//!
//! Strings are cut to sane lengths and stripped of control characters,
//! pages are cut to the requested size, ISRCs are normalised, unplayable
//! items always carry a reason, and every URL the host or engine will fetch
//! later (images, streams) must be covered by the plugin's allowlist.
//!
//! A [`Checker`] names the plugin (for logs) and says which URLs its
//! allowlist covers; every host builds one from its own allowlist.

use url::Url;

use crate::error::CallError;
use crate::types::{
    Header, ImageRef, Item, ItemPage, LyricPayload, StreamInfo, Unplayable, UnplayableReason,
};

const MAX_ID: usize = 1024;
const MAX_TEXT: usize = 1024;
const MAX_CURSOR: usize = 4096;
const MAX_URL: usize = 8192;
const MAX_HEADERS: usize = 16;
const MAX_HEADER_VALUE: usize = 8192;
const MAX_LYRICS: usize = 1024 * 1024;
/// Most items a page may carry.
pub const MAX_PAGE: u32 = 200;

/// What the checks need to know about the plugin that answered.
pub struct Checker<'a> {
    plugin: &'a str,
    allows: &'a dyn Fn(&Url) -> bool,
}

impl std::fmt::Debug for Checker<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Checker")
            .field("plugin", &self.plugin)
            .finish_non_exhaustive()
    }
}

impl<'a> Checker<'a> {
    /// Checks for `plugin`'s answers; `allows` says whether its allowlist
    /// covers a URL.
    pub fn new(plugin: &'a str, allows: &'a dyn Fn(&Url) -> bool) -> Checker<'a> {
        Checker { plugin, allows }
    }

    /// The plugin's id.
    pub fn plugin(&self) -> &str {
        self.plugin
    }

    /// Whether the plugin's allowlist covers `url`.
    pub fn allows(&self, url: &Url) -> bool {
        (self.allows)(url)
    }
}

/// Removes control characters, trims, and cuts at `max` bytes.
pub fn text(value: &str, max: usize) -> String {
    let cleaned: String = value.chars().filter(|c| !c.is_control()).collect();
    let mut s = cleaned.trim().to_string();
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

fn opt_text(value: Option<String>, max: usize) -> Option<String> {
    value.map(|v| text(&v, max)).filter(|v| !v.is_empty())
}

/// An ISRC in canonical form (12 characters, uppercase, no separators), or
/// `None` if it is not one.
pub fn isrc(value: &str) -> Option<String> {
    let canonical: String = value
        .chars()
        .filter(|c| !matches!(c, '-' | ' '))
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let bytes = canonical.as_bytes();
    let valid = bytes.len() == 12
        && bytes[..2].iter().all(u8::is_ascii_uppercase)
        && bytes[2..5].iter().all(u8::is_ascii_alphanumeric)
        && bytes[5..].iter().all(u8::is_ascii_digit);
    valid.then_some(canonical)
}

fn headers(list: Vec<Header>) -> Result<Vec<Header>, String> {
    if list.len() > MAX_HEADERS {
        return Err(format!("more than {MAX_HEADERS} headers"));
    }
    for header in &list {
        let name_ok = !header.name.is_empty()
            && header.name.len() <= 128
            && header
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b));
        let value_ok = header.value.len() <= MAX_HEADER_VALUE
            && header
                .value
                .bytes()
                .all(|b| b == b'\t' || (0x20..0x7f).contains(&b) || b >= 0x80);
        if !name_ok || !value_ok {
            return Err(format!("invalid header {:?}", header.name));
        }
    }
    Ok(list)
}

/// A URL the plugin hands back for the host or engine to fetch.
fn fetchable(url: &str, checker: &Checker<'_>) -> Result<Url, String> {
    if url.len() > MAX_URL {
        return Err("URL too long".into());
    }
    let parsed = Url::parse(url).map_err(|e| format!("bad URL {url:?}: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("{url} is not an http(s) URL"));
    }
    if !checker.allows(&parsed) {
        return Err(format!(
            "{} is not on the plugin's allowlist",
            parsed.origin().ascii_serialization()
        ));
    }
    Ok(parsed)
}

/// An image reference, or `None` if it may not be fetched.
pub fn image(image: ImageRef, checker: &Checker<'_>) -> Option<ImageRef> {
    match (fetchable(&image.url, checker), headers(image.headers)) {
        (Ok(url), Ok(headers)) => Some(ImageRef {
            url: url.into(),
            headers,
            width: image.width,
            height: image.height,
        }),
        (Err(why), _) | (_, Err(why)) => {
            tracing::debug!(plugin = %checker.plugin, %why, "dropped an image reference");
            None
        }
    }
}

/// An item, or `None` if it is unusable (no id or title).
pub fn item(item: Item, checker: &Checker<'_>) -> Option<Item> {
    if item.id.is_empty() || item.id.len() > MAX_ID {
        return None;
    }
    let title = text(&item.title, MAX_TEXT);
    if title.is_empty() {
        return None;
    }
    let unplayable = if item.playable {
        None
    } else {
        Some(match item.unplayable {
            Some(u) => Unplayable {
                reason: u.reason,
                message: opt_text(u.message, MAX_TEXT),
            },
            None => Unplayable {
                reason: UnplayableReason::Other,
                message: None,
            },
        })
    };
    Some(Item {
        id: item.id,
        kind: item.kind,
        title,
        artist: opt_text(item.artist, MAX_TEXT),
        album: opt_text(item.album, MAX_TEXT),
        album_id: item.album_id.filter(|id| !id.is_empty() && id.len() <= MAX_ID),
        artist_id: item.artist_id.filter(|id| !id.is_empty() && id.len() <= MAX_ID),
        duration_ms: item.duration_ms,
        track_number: item.track_number,
        disc_number: item.disc_number,
        year: item.year,
        child_count: item.child_count,
        isrc: item.isrc.as_deref().and_then(isrc),
        artwork: item.artwork.and_then(|a| image(a, checker)),
        explicit: item.explicit,
        playable: item.playable,
        unplayable,
    })
}

/// A page cut to `limit` items, invalid items dropped.
pub fn page(page: ItemPage, limit: u32, checker: &Checker<'_>) -> ItemPage {
    let limit = limit.clamp(1, MAX_PAGE) as usize;
    ItemPage {
        items: page
            .items
            .into_iter()
            .take(limit)
            .filter_map(|i| item(i, checker))
            .collect(),
        next: page.next.filter(|c| !c.is_empty() && c.len() <= MAX_CURSOR),
        total: page.total,
    }
}

/// A stream the engine may fetch.
pub fn stream(info: StreamInfo, checker: &Checker<'_>) -> Result<StreamInfo, CallError> {
    let url = fetchable(&info.url, checker).map_err(CallError::InvalidResponse)?;
    let headers = headers(info.headers).map_err(CallError::InvalidResponse)?;
    Ok(StreamInfo {
        url: url.into(),
        headers,
        expires_at_ms: info.expires_at_ms,
        codec: opt_text(info.codec, 32).map(|c| c.to_ascii_lowercase()),
        bitrate_kbps: info.bitrate_kbps,
        cacheable: info.cacheable,
    })
}

/// Lyrics within the size limit.
pub fn lyrics(payload: LyricPayload) -> Result<LyricPayload, CallError> {
    if payload.text.len() > MAX_LYRICS {
        return Err(CallError::InvalidResponse("lyrics larger than 1 MiB".into()));
    }
    Ok(LyricPayload {
        format: payload.format,
        text: payload.text,
        language: opt_text(payload.language, 35),
        attribution: opt_text(payload.attribution, MAX_TEXT),
    })
}

/// An artwork answer: must be fetchable.
pub fn artwork(image_ref: ImageRef, checker: &Checker<'_>) -> Result<ImageRef, CallError> {
    let url = fetchable(&image_ref.url, checker).map_err(CallError::InvalidResponse)?;
    let headers = headers(image_ref.headers).map_err(CallError::InvalidResponse)?;
    Ok(ImageRef {
        url: url.into(),
        headers,
        width: image_ref.width,
        height: image_ref.height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_isrcs() {
        assert_eq!(isrc("us-rc1-76-07839").as_deref(), Some("USRC17607839"));
        assert_eq!(isrc("GBAYE0601498").as_deref(), Some("GBAYE0601498"));
        for bad in ["", "USRC1760783", "USRC176078390", "1SRC17607839", "USRC17A07839"] {
            assert_eq!(isrc(bad), None, "{bad}");
        }
    }

    #[test]
    fn cleans_text() {
        assert_eq!(text("  a\u{0}b\nc  ", 100), "abc");
        assert_eq!(text("ééé", 5), "éé");
    }
}
