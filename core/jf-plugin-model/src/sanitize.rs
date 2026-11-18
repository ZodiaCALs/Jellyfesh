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
    Comment, CommentPage, Fact, FactKey, Header, HomeSection, ImageRef, Item, ItemDetails, ItemPage, Look,
    LyricFormat, LyricPayload, Made, MakerNeed, MakerPlan, Place, RemoteDevice, RemoteState, Shelf,
    SpeechSegment, SpeechWord, StreamInfo, Transcript, Unplayable, UnplayableReason,
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
/// Most devices a remote source may list.
pub const MAX_DEVICES: usize = 64;
/// Most tracks the host hands `remote.play` at once.
pub const MAX_REMOTE_TRACKS: usize = 200;
/// Most shelves of a Discover page, and items per shelf.
pub const MAX_SHELVES: usize = 12;
pub const MAX_SHELF_ITEMS: usize = 50;
/// Most facts in an item's details.
pub const MAX_FACTS: usize = 32;
/// Longest description in an item's details (characters).
pub const MAX_DESCRIPTION: usize = 20_000;
/// Most comments per page (and hot comments), and characters per comment.
pub const MAX_COMMENTS: usize = 50;
pub const MAX_COMMENT_CHARS: usize = 2000;
/// Most segments and words in a transcript.
pub const MAX_SEGMENTS: usize = 10_000;
pub const MAX_WORDS: usize = 100_000;
/// Longest text of a transcript segment (bytes).
const MAX_SEGMENT_TEXT: usize = 4096;

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

/// A remote device, or `None` if it has no id or name. The volume is cut to
/// 0–100 and dropped where the device has none.
pub fn remote_device(device: RemoteDevice) -> Option<RemoteDevice> {
    if device.id.is_empty() || device.id.len() > MAX_ID {
        return None;
    }
    let name = text(&device.name, MAX_TEXT);
    if name.is_empty() {
        return None;
    }
    Some(RemoteDevice {
        id: device.id,
        name,
        kind: device.kind,
        active: device.active,
        supports_volume: device.supports_volume,
        volume_percent: device
            .volume_percent
            .filter(|_| device.supports_volume)
            .map(|v| v.min(100)),
    })
}

/// The devices a remote source lists: unusable ones dropped, at most
/// [`MAX_DEVICES`], one entry per id, and at most one marked active.
pub fn remote_devices(devices: Vec<RemoteDevice>) -> Vec<RemoteDevice> {
    let mut out: Vec<RemoteDevice> = Vec::new();
    for device in devices.into_iter().filter_map(remote_device) {
        if out.len() == MAX_DEVICES {
            break;
        }
        if out.iter().any(|d| d.id == device.id) {
            continue;
        }
        let active = device.active && !out.iter().any(|d| d.active);
        out.push(RemoteDevice { active, ..device });
    }
    out
}

/// What a remote player says it is doing, checked like any other answer:
/// an unusable device, item or context becomes `None`, and the position
/// stays within the item's length when it has one.
pub fn remote_state(state: RemoteState, checker: &Checker<'_>) -> RemoteState {
    let item = state.item.and_then(|i| item(i, checker));
    let position_ms = match item.as_ref().and_then(|i| i.duration_ms) {
        Some(length) if length > 0 => state.position_ms.min(length),
        _ => state.position_ms,
    };
    RemoteState {
        device: state.device.and_then(remote_device),
        item,
        context: state.context.and_then(|c| self::item(c, checker)),
        playing: state.playing,
        position_ms,
        shuffle: state.shuffle,
        repeat: state.repeat,
    }
}

/// Cuts text to `max` characters (not bytes), without control characters
/// other than line breaks, trimmed.
pub fn long_text(value: &str, max: usize) -> String {
    let cleaned: String = value
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(max)
        .collect();
    cleaned.trim().to_string()
}

fn unit(value: f32, low: f32, high: f32) -> Option<f32> {
    value.is_finite().then(|| value.clamp(low, high))
}

/// What an appearance plugin's `look` may say (ADR-0020 §1): numbers clamped
/// to their ranges (NaN and infinities dropped), duplicate places and
/// sections dropped, and at least one library place in the sidebar (the
/// first of the default order is added when none is left). A custom
/// highlight must be `0xRRGGBB`.
pub fn look(look: Look) -> Look {
    let mut sidebar = look.sidebar.map(|places| {
        let mut out: Vec<Place> = Vec::new();
        for place in places {
            if !out.contains(&place) {
                out.push(place);
            }
        }
        out
    });
    if let Some(places) = &mut sidebar
        && !places.iter().any(|p| p.is_library())
    {
        places.push(Place::Albums);
    }
    let home = look.home.map(|sections| {
        let mut out: Vec<HomeSection> = Vec::new();
        for section in sections {
            if !out.contains(&section) {
                out.push(section);
            }
        }
        out
    });
    Look {
        corner_scale: look.corner_scale.and_then(|v| unit(v, 0.0, 2.0)),
        glass: look.glass.and_then(|v| unit(v, 0.0, 1.0)),
        aurora: look.aurora.and_then(|v| unit(v, 0.0, 1.0)),
        typeface: look.typeface,
        text_scale: look.text_scale.and_then(|v| unit(v, 0.85, 1.30)),
        density: look.density,
        lyrics_weight: look.lyrics_weight,
        lyrics_glow: look.lyrics_glow.and_then(|v| unit(v, 0.0, 1.0)),
        lyrics_highlight: look.lyrics_highlight.filter(|h| match h {
            crate::types::Highlight::Custom(rgb) => *rgb <= 0xFF_FFFF,
            _ => true,
        }),
        sidebar,
        home,
        now_playing: look.now_playing,
        mini_player: look.mini_player,
    }
}

/// The origin form the host and the core use for consent:
/// `scheme://host[:port]` of an http(s) URL (or of `host` alone, taken as
/// https), lowercase, the port only when it is not the scheme's default.
pub fn origin(text: &str) -> Option<String> {
    let text = text.trim();
    let with_scheme = if text.contains("://") {
        text.to_string()
    } else {
        format!("https://{text}")
    };
    let url = Url::parse(&with_scheme).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none_or(str::is_empty) {
        return None;
    }
    if !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

/// A maker's plan: the engine named (at most 128 bytes), `upload_to` an
/// origin on the plugin's allowlist, texts cut.
pub fn plan(plan: MakerPlan, checker: &Checker<'_>) -> Result<MakerPlan, CallError> {
    let upload_to = match plan.upload_to.filter(|u| !u.trim().is_empty()) {
        Some(text) => {
            let origin = origin(&text)
                .ok_or_else(|| CallError::InvalidResponse(format!("upload-to {text:?} is not an origin")))?;
            let url = Url::parse(&origin).map_err(|e| CallError::InvalidResponse(e.to_string()))?;
            if !checker.allows(&url) {
                return Err(CallError::InvalidResponse(format!(
                    "upload-to {origin} is not on the plugin's allowlist"
                )));
            }
            Some(origin)
        }
        None => None,
    };
    let engine = text(&plan.engine, 128);
    Ok(MakerPlan {
        mode: plan.mode,
        engine: if engine.is_empty() {
            "AI Lyrics".into()
        } else {
            engine
        },
        upload_to,
        need: match plan.need {
            MakerNeed::Model(id) => MakerNeed::Model(text(&id, 64)),
            MakerNeed::Unavailable(why) => MakerNeed::Unavailable(text(&why, MAX_TEXT)),
            other => other,
        },
        automatic: plan.automatic,
    })
}

/// Lyrics a maker made: synced text in a format the core reads (`elrc`,
/// `lrc`, `ttml` or `vtt`), at most 1 MiB, the engine named.
pub fn made(made: Made) -> Result<Made, CallError> {
    if !matches!(
        made.lyrics.format,
        LyricFormat::Elrc | LyricFormat::Lrc | LyricFormat::Ttml | LyricFormat::Vtt
    ) {
        return Err(CallError::InvalidResponse(
            "made lyrics must be synced (elrc, lrc, ttml or vtt)".into(),
        ));
    }
    if made.lyrics.text.trim().is_empty() {
        return Err(CallError::InvalidResponse("the made lyrics are empty".into()));
    }
    let lyrics = lyrics(made.lyrics)?;
    let engine = text(&made.engine, 128);
    Ok(Made {
        lyrics,
        engine: if engine.is_empty() {
            "AI Lyrics".into()
        } else {
            engine
        },
    })
}

/// A transcript for a plugin (from the app's recogniser): texts cut, times
/// within `[0, duration_ms]` (when known) with each end at or after its
/// start, confidence within 0–1 (NaN as 0), at most [`MAX_SEGMENTS`]
/// segments and [`MAX_WORDS`] words.
pub fn transcript(transcript: Transcript, duration_ms: Option<u32>) -> Transcript {
    let limit = duration_ms.unwrap_or(u32::MAX);
    let span = |start: u32, end: u32| {
        let start = start.min(limit);
        (start, end.clamp(start, limit))
    };
    let mut words_left = MAX_WORDS;
    let segments = transcript
        .segments
        .into_iter()
        .take(MAX_SEGMENTS)
        .map(|segment| {
            let (start_ms, end_ms) = span(segment.start_ms, segment.end_ms);
            let words: Vec<SpeechWord> = segment
                .words
                .into_iter()
                .take(words_left)
                .filter_map(|word| {
                    let text = text(&word.text, 256);
                    if text.is_empty() {
                        return None;
                    }
                    let (start_ms, end_ms) = span(word.start_ms, word.end_ms);
                    Some(SpeechWord {
                        text,
                        start_ms,
                        end_ms,
                        confidence: if word.confidence.is_finite() {
                            word.confidence.clamp(0.0, 1.0)
                        } else {
                            0.0
                        },
                    })
                })
                .collect();
            words_left -= words.len();
            SpeechSegment {
                text: text(&segment.text, MAX_SEGMENT_TEXT),
                start_ms,
                end_ms,
                words,
            }
        })
        .collect();
    Transcript {
        language: opt_text(transcript.language, 35),
        segments,
    }
}

/// A Discover page: at most [`MAX_SHELVES`] shelves with an id and a title,
/// each with at most [`MAX_SHELF_ITEMS`] usable items; empty shelves dropped.
pub fn shelves(shelves: Vec<Shelf>, checker: &Checker<'_>) -> Vec<Shelf> {
    let mut out: Vec<Shelf> = Vec::new();
    for shelf in shelves {
        if out.len() == MAX_SHELVES {
            break;
        }
        let title = text(&shelf.title, MAX_TEXT);
        if shelf.id.is_empty() || shelf.id.len() > MAX_ID || title.is_empty() {
            continue;
        }
        if out.iter().any(|s| s.id == shelf.id) {
            continue;
        }
        let items: Vec<Item> = shelf
            .items
            .into_iter()
            .filter_map(|i| item(i, checker))
            .take(MAX_SHELF_ITEMS)
            .collect();
        if items.is_empty() {
            continue;
        }
        out.push(Shelf {
            id: shelf.id,
            title,
            style: shelf.style,
            items,
            more: shelf.more.filter(|m| !m.is_empty() && m.len() <= MAX_ID),
        });
    }
    out
}

/// An item's details: the description cut, the image only from the
/// allowlist, at most [`MAX_FACTS`] facts with a value.
pub fn details(details: ItemDetails, checker: &Checker<'_>) -> ItemDetails {
    let facts = details
        .facts
        .into_iter()
        .filter_map(|fact| {
            let value = text(&fact.value, MAX_TEXT);
            let key = match fact.key {
                FactKey::Other(label) => {
                    let label = text(&label, 64);
                    if label.is_empty() {
                        return None;
                    }
                    FactKey::Other(label)
                }
                key => key,
            };
            (!value.is_empty()).then_some(Fact { key, value })
        })
        .take(MAX_FACTS)
        .collect();
    ItemDetails {
        description: details
            .description
            .map(|d| long_text(&d, MAX_DESCRIPTION))
            .filter(|d| !d.is_empty()),
        image: details.image.and_then(|i| image(i, checker)),
        facts,
        comment_count: details.comment_count,
    }
}

fn comment(comment: Comment, checker: &Checker<'_>) -> Option<Comment> {
    if comment.id.is_empty() || comment.id.len() > MAX_ID {
        return None;
    }
    let text = long_text(&comment.text, MAX_COMMENT_CHARS);
    if text.is_empty() {
        return None;
    }
    Some(Comment {
        id: comment.id,
        author: self::text(&comment.author, 128),
        avatar: comment.avatar.and_then(|a| image(a, checker)),
        text,
        likes: comment.likes,
        time_ms: comment.time_ms,
        replies: comment.replies,
    })
}

/// A page of comments: at most [`MAX_COMMENTS`] of each kind, texts at most
/// [`MAX_COMMENT_CHARS`] characters, avatars only from the allowlist.
pub fn comments(page: CommentPage, checker: &Checker<'_>) -> CommentPage {
    let list = |comments: Vec<Comment>| -> Vec<Comment> {
        comments
            .into_iter()
            .filter_map(|c| comment(c, checker))
            .take(MAX_COMMENTS)
            .collect()
    };
    CommentPage {
        hot: list(page.hot),
        comments: list(page.comments),
        total: page.total,
        next: page.next.filter(|c| !c.is_empty() && c.len() <= MAX_CURSOR),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Density, Highlight, ItemKind, MakerMode, NowPlayingLayout, RemoteDeviceKind, RemoteRepeat, ShelfStyle,
    };

    fn device(id: &str, name: &str, active: bool) -> RemoteDevice {
        RemoteDevice {
            id: id.into(),
            name: name.into(),
            kind: RemoteDeviceKind::Speaker,
            active,
            supports_volume: true,
            volume_percent: Some(140),
        }
    }

    #[test]
    fn checks_remote_devices() {
        let devices = remote_devices(vec![
            device("a", " Kitchen\u{7} ", true),
            device("", "No id", false),
            device("b", "  ", false),
            device("a", "Kitchen again", false),
            device("c", "Desk", true),
        ]);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Kitchen");
        assert_eq!(devices[0].volume_percent, Some(100));
        assert!(devices[0].active);
        assert!(!devices[1].active, "only one device is active");

        let silent = remote_device(RemoteDevice {
            supports_volume: false,
            ..device("d", "Car", false)
        })
        .unwrap();
        assert_eq!(silent.volume_percent, None);

        let many = remote_devices(
            (0..100)
                .map(|i| device(&i.to_string(), "Speaker", false))
                .collect(),
        );
        assert_eq!(many.len(), MAX_DEVICES);
    }

    #[test]
    fn checks_remote_state() {
        let allow_none = |_: &Url| false;
        let checker = Checker::new("test", &allow_none);
        let track = Item {
            id: "t1".into(),
            kind: ItemKind::Track,
            title: "A Song".into(),
            artist: None,
            album: None,
            album_id: None,
            artist_id: None,
            duration_ms: Some(1000),
            track_number: None,
            disc_number: None,
            year: None,
            child_count: None,
            isrc: None,
            artwork: None,
            explicit: false,
            playable: true,
            unplayable: None,
        };
        let state = remote_state(
            RemoteState {
                device: Some(device("", "Nameless", true)),
                item: Some(track.clone()),
                context: Some(Item {
                    title: String::new(),
                    ..track.clone()
                }),
                playing: true,
                position_ms: 5000,
                shuffle: false,
                repeat: RemoteRepeat::All,
            },
            &checker,
        );
        assert_eq!(state.device, None);
        assert_eq!(state.item.as_ref().map(|i| i.id.as_str()), Some("t1"));
        assert_eq!(state.context, None);
        assert_eq!(state.position_ms, 1000, "the position stays within the song");
        assert_eq!(state.repeat, RemoteRepeat::All);
    }

    fn a_track(id: &str) -> Item {
        Item {
            id: id.into(),
            kind: ItemKind::Track,
            title: "A Song".into(),
            artist: None,
            album: None,
            album_id: None,
            artist_id: None,
            duration_ms: Some(1000),
            track_number: None,
            disc_number: None,
            year: None,
            child_count: None,
            isrc: None,
            artwork: None,
            explicit: false,
            playable: true,
            unplayable: None,
        }
    }

    #[test]
    fn checks_looks() {
        let checked = look(Look {
            corner_scale: Some(3.0),
            glass: Some(f32::NAN),
            aurora: Some(-1.0),
            text_scale: Some(2.0),
            lyrics_glow: Some(f32::INFINITY),
            lyrics_highlight: Some(Highlight::Custom(0x1FF_FFFF)),
            sidebar: Some(vec![Place::Search, Place::Home, Place::Search, Place::Sources]),
            home: Some(vec![
                HomeSection::MostPlayed,
                HomeSection::MostPlayed,
                HomeSection::Summary,
            ]),
            density: Some(Density::Roomy),
            now_playing: Some(NowPlayingLayout::Lyrics),
            ..Look::default()
        });
        assert_eq!(checked.corner_scale, Some(2.0));
        assert_eq!(checked.glass, None, "NaN is ignored");
        assert_eq!(checked.aurora, Some(0.0));
        assert_eq!(checked.text_scale, Some(1.30));
        assert_eq!(checked.lyrics_glow, None);
        assert_eq!(checked.lyrics_highlight, None, "not a colour");
        assert_eq!(
            checked.sidebar,
            Some(vec![Place::Search, Place::Home, Place::Sources, Place::Albums]),
            "duplicates dropped, a library place added"
        );
        assert_eq!(
            checked.home,
            Some(vec![HomeSection::MostPlayed, HomeSection::Summary])
        );
        assert_eq!(checked.density, Some(Density::Roomy));
        let fine = look(Look {
            sidebar: Some(vec![Place::Songs]),
            lyrics_highlight: Some(Highlight::Custom(0xFF8800)),
            ..Look::default()
        });
        assert_eq!(fine.sidebar, Some(vec![Place::Songs]));
        assert_eq!(fine.lyrics_highlight, Some(Highlight::Custom(0xFF8800)));
        assert_eq!(look(Look::default()), Look::default());
    }

    #[test]
    fn checks_plans_and_made_lyrics() {
        let allow_openai = |url: &Url| url.host_str() == Some("api.openai.com");
        let checker = Checker::new("test", &allow_openai);
        let online = MakerPlan {
            mode: MakerMode::Online,
            engine: " whisper-1 at api.openai.com\u{7} ".into(),
            upload_to: Some("https://API.openai.com/v1/audio".into()),
            need: MakerNeed::Nothing,
            automatic: false,
        };
        let checked = plan(online.clone(), &checker).unwrap();
        assert_eq!(checked.upload_to.as_deref(), Some("https://api.openai.com"));
        assert_eq!(checked.engine, "whisper-1 at api.openai.com");
        let elsewhere = MakerPlan {
            upload_to: Some("https://evil.example".into()),
            ..online.clone()
        };
        assert!(plan(elsewhere, &checker).is_err());
        assert!(
            plan(
                MakerPlan {
                    upload_to: Some("not a url at all".into()),
                    ..online
                },
                &checker
            )
            .is_err()
        );

        let payload = |format, text: &str| LyricPayload {
            format,
            text: text.into(),
            language: None,
            attribution: None,
        };
        let good = made(Made {
            lyrics: payload(LyricFormat::Elrc, "[00:01.00]<00:01.00>Hi"),
            engine: String::new(),
        })
        .unwrap();
        assert_eq!(good.engine, "AI Lyrics");
        assert!(
            made(Made {
                lyrics: payload(LyricFormat::Plain, "Hi"),
                engine: "x".into()
            })
            .is_err()
        );
        assert!(
            made(Made {
                lyrics: payload(LyricFormat::Lrc, "  "),
                engine: "x".into()
            })
            .is_err()
        );
        assert!(
            made(Made {
                lyrics: payload(LyricFormat::Lrc, &"a".repeat(MAX_LYRICS + 1)),
                engine: "x".into()
            })
            .is_err()
        );
        assert_eq!(origin("api.groq.com"), Some("https://api.groq.com".into()));
        assert_eq!(
            origin("http://192.168.1.5:8000/v1"),
            Some("http://192.168.1.5:8000".into())
        );
        assert_eq!(
            origin("https://api.openai.com:443"),
            Some("https://api.openai.com".into())
        );
        assert_eq!(origin("ftp://x.example"), None);
        assert_eq!(origin("https://user:pw@x.example"), None);
    }

    #[test]
    fn checks_transcripts() {
        let word = |text: &str, start, end, confidence| SpeechWord {
            text: text.into(),
            start_ms: start,
            end_ms: end,
            confidence,
        };
        let checked = transcript(
            Transcript {
                language: Some("en".into()),
                segments: vec![SpeechSegment {
                    text: "hello\u{0} world".into(),
                    start_ms: 500,
                    end_ms: 100,
                    words: vec![
                        word("hello", 500, 900, f32::NAN),
                        word(" ", 900, 950, 1.0),
                        word("world", 9_000, 99_000, 7.0),
                    ],
                }],
            },
            Some(10_000),
        );
        let segment = &checked.segments[0];
        assert_eq!(segment.text, "hello world");
        assert_eq!(
            (segment.start_ms, segment.end_ms),
            (500, 500),
            "the end is not before the start"
        );
        assert_eq!(segment.words.len(), 2, "empty words are dropped");
        assert_eq!(segment.words[0].confidence, 0.0);
        assert_eq!(
            (segment.words[1].start_ms, segment.words[1].end_ms),
            (9_000, 10_000)
        );
        assert_eq!(segment.words[1].confidence, 1.0);
    }

    #[test]
    fn checks_shelves_details_and_comments() {
        let allow_cdn = |url: &Url| url.host_str() == Some("cdn.example.com");
        let checker = Checker::new("test", &allow_cdn);
        let shelf = |id: &str, title: &str, items: usize| Shelf {
            id: id.into(),
            title: title.into(),
            style: ShelfStyle::Row,
            items: (0..items).map(|i| a_track(&format!("t{i}"))).collect(),
            more: Some(String::new()),
        };
        let mut many = vec![
            shelf("", "No id", 1),
            shelf("a", "  ", 1),
            shelf("e", "Empty", 0),
            shelf("big", "Big", 80),
            shelf("big", "Again", 1),
        ];
        many.extend((0..20).map(|i| shelf(&format!("s{i}"), "Shelf", 1)));
        let checked = shelves(many, &checker);
        assert_eq!(checked.len(), MAX_SHELVES);
        assert_eq!(checked[0].id, "big");
        assert_eq!(checked[0].items.len(), MAX_SHELF_ITEMS);
        assert_eq!(checked[0].more, None);
        assert_eq!(checked[1].id, "s0");

        let image = |url: &str| ImageRef {
            url: url.into(),
            headers: Vec::new(),
            width: None,
            height: None,
        };
        let facts: Vec<Fact> = (0..40)
            .map(|i| Fact {
                key: if i == 0 {
                    FactKey::Other("  ".into())
                } else {
                    FactKey::Genre
                },
                value: if i == 1 {
                    String::new()
                } else {
                    format!("Fact {i}")
                },
            })
            .collect();
        let checked = details(
            ItemDetails {
                description: Some(format!("Line one\nline two\u{7}{}", "x".repeat(30_000))),
                image: Some(image("https://elsewhere.example/a.jpg")),
                facts,
                comment_count: Some(12),
            },
            &checker,
        );
        assert!(
            checked
                .description
                .as_deref()
                .unwrap()
                .starts_with("Line one\nline two")
        );
        assert_eq!(
            checked.description.as_deref().unwrap().chars().count(),
            MAX_DESCRIPTION
        );
        assert_eq!(checked.image, None, "not on the allowlist");
        assert_eq!(checked.facts.len(), MAX_FACTS);
        assert_eq!(checked.facts[0].value, "Fact 2");

        let comment = |id: &str, text: &str| Comment {
            id: id.into(),
            author: "Someone".into(),
            avatar: Some(image("https://cdn.example.com/face.png")),
            text: text.into(),
            likes: Some(3),
            time_ms: None,
            replies: None,
        };
        let page = comments(
            CommentPage {
                hot: vec![comment("h", "Hot")],
                comments: (0..70)
                    .map(|i| comment(&i.to_string(), &"é".repeat(3000)))
                    .chain([comment("", "x"), comment("e", " ")])
                    .collect(),
                total: Some(70),
                next: Some("2".into()),
            },
            &checker,
        );
        assert_eq!(page.hot.len(), 1);
        assert!(page.hot[0].avatar.is_some());
        assert_eq!(page.comments.len(), MAX_COMMENTS);
        assert_eq!(page.comments[0].text.chars().count(), MAX_COMMENT_CHARS);
        assert_eq!(page.next.as_deref(), Some("2"));
    }

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
