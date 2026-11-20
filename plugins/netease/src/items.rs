//! NetEase Cloud Music's songs and playlists as Jellyfesh items.
//!
//! Song ids are the encrypted ids (32 hex digits) the API takes; the numeric
//! ids that NetEase's links need are not kept (no `[links]`). Field names are
//! the ones the research saw in the official client's song lists; the
//! playlist lists' answers are not documented, so a few likely spellings are
//! read (to be checked against NetEase's documentation).

use jf_plugin_sdk::prelude::*;
use serde_json::Value;

fn text<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| value.get(*k).and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn id(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| match value.get(*k)? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

fn number(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|k| value.get(*k).and_then(Value::as_u64))
}

fn flag(value: &Value, key: &str) -> Option<bool> {
    value
        .get(key)
        .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0)))
}

/// The list in an answer's `data`: the array itself, or the first array
/// under a likely name (`records` in the official client's playlist lists).
pub fn list(data: &Value) -> &[Value] {
    if let Some(items) = data.as_array() {
        return items;
    }
    ["records", "list", "songs", "playlists", "data"]
        .iter()
        .find_map(|k| data.get(*k).and_then(Value::as_array))
        .map_or(&[], Vec::as_slice)
}

/// The one playlist an answer's `data` describes: the object itself (the
/// official client reads `playlist/star/get/v2`, the liked songs' playlist,
/// and `playlist/detail/get/v2` this way), or the first of a list.
pub fn one(data: &Value) -> Option<&Value> {
    if data.get("id").is_some() {
        return Some(data);
    }
    list(data).first()
}

/// Whether the list says it has more (when it says so at all).
pub fn has_more(data: &Value) -> Option<bool> {
    ["hasMore", "more"]
        .iter()
        .find_map(|k| data.get(*k).and_then(Value::as_bool))
}

/// Pictures come from NetEase's image servers, sometimes as `http`; the
/// sandbox allows its fixed hosts over https only, so those are asked for
/// over https (the servers serving https is not confirmed).
pub fn secure(url: &str) -> String {
    let Some(rest) = url.strip_prefix("http://") else {
        return url.to_owned();
    };
    let (authority, path) = rest.split_at(rest.find(['/', '?']).unwrap_or(rest.len()));
    let host = authority.strip_suffix(":80").unwrap_or(authority);
    if host.ends_with(".music.126.net") && !host.contains([':', '@']) {
        format!("https://{host}{path}")
    } else {
        url.to_owned()
    }
}

/// What NetEase lets this account do with a song (the official client's
/// order of checks, as the research found it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// The whole song (`playFlag`, or a free full listen).
    Full,
    /// No rights on this client (`visible` false).
    Hidden,
    /// Only a preview segment is free.
    Preview,
    /// A VIP song (`songFee` 1).
    Vip,
    /// Part of a digital album to buy (`songFee` 4).
    Album,
    /// Not allowed for another reason.
    Denied,
    /// The list says nothing; the stream request decides.
    Unknown,
}

pub fn access(song: &Value) -> Access {
    if flag(song, "playFlag") == Some(true) {
        return Access::Full;
    }
    if flag(song, "visible") == Some(false) {
        return Access::Hidden;
    }
    let privilege = song.get("freeTrialPrivilege").unwrap_or(song);
    if flag(privilege, "resConsumable") == Some(true) && flag(privilege, "userConsumable") == Some(true) {
        return Access::Full;
    }
    if flag(song, "freeTrailFlag") == Some(true) {
        return Access::Preview;
    }
    match song.get("songFee").and_then(Value::as_i64) {
        Some(1) => Access::Vip,
        Some(4) => Access::Album,
        _ if song.get("playFlag").is_some() => Access::Denied,
        _ => Access::Unknown,
    }
}

/// The reason shown for a song that does not play, in English with
/// NetEase's own words.
pub fn refusal(access: Access) -> Option<Unplayable> {
    let (reason, message) = match access {
        Access::Full | Access::Unknown => return None,
        Access::Hidden => (
            UnplayableReason::Unavailable,
            "NetEase Cloud Music has no rights to play this song here (当前端无版权).",
        ),
        Access::Preview => (
            UnplayableReason::Paywall,
            "Only a preview of this song is free; the whole song needs NetEase Cloud Music VIP. Jellyfesh \
             does not play previews.",
        ),
        Access::Vip => (
            UnplayableReason::Paywall,
            "A VIP song: it plays with NetEase Cloud Music VIP (该歌曲为VIP歌曲，请开通会员后收听).",
        ),
        Access::Album => (
            UnplayableReason::Paywall,
            "Part of a digital album: buy it in the NetEase Cloud Music app (该歌曲为数字专辑，请前往手机端购买后收听).",
        ),
        Access::Denied => (
            UnplayableReason::Other,
            "NetEase Cloud Music does not let this account play this song (无播放权限).",
        ),
    };
    Some(Unplayable {
        reason,
        message: Some(message.to_owned()),
    })
}

fn names(value: Option<&Value>) -> Option<String> {
    let names: Vec<&str> = value?
        .as_array()?
        .iter()
        .filter_map(|a| a.get("name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

/// A song's cover where a list names one. The official client's lists were
/// not seen with one; the song detail calls it `coverImgUrl`, and NetEase's
/// song objects elsewhere `al.picUrl`, so those spellings are read.
fn song_cover(value: &Value) -> Option<&str> {
    value
        .get("al")
        .or_else(|| value.get("album"))
        .and_then(|a| text(a, &["picUrl", "coverImgUrl"]))
        .or_else(|| text(value, &["coverImgUrl", "picUrl", "albumPicUrl"]))
}

/// A song from a playlist's list.
pub fn song(value: &Value) -> Option<Item> {
    let id = id(value, &["encryptedId", "songId", "id"])?;
    let mut item = Item::new(ItemKind::Track, id, text(value, &["name", "songName"])?);
    item.artist = names(value.get("ar").or_else(|| value.get("artists")))
        .or_else(|| text(value, &["artistName"]).map(str::to_owned));
    item.album = value
        .get("al")
        .or_else(|| value.get("album"))
        .and_then(|a| text(a, &["name"]))
        .or_else(|| text(value, &["albumName"]))
        .map(str::to_owned);
    item.duration_ms = number(value, &["dt", "duration"]).and_then(|d| u32::try_from(d).ok());
    if let Some(cover) = song_cover(value) {
        item = item.artwork(secure(cover));
    }
    if let Some(unplayable) = refusal(access(value)) {
        item.playable = false;
        item.unplayable = Some(unplayable);
    }
    Some(item)
}

/// A playlist from one of the account's playlist lists.
pub fn playlist(value: &Value) -> Option<Item> {
    let id = id(value, &["id", "playlistId", "encryptedId"])?;
    let mut item = Item::new(ItemKind::Playlist, id, text(value, &["name", "playlistName"])?);
    item.artist = value
        .get("creator")
        .and_then(|c| text(c, &["nickname", "nickName"]))
        .or_else(|| text(value, &["creatorNickName", "creatorName"]))
        .map(str::to_owned);
    item.child_count = number(value, &["trackCount", "songCount"]).and_then(|n| u32::try_from(n).ok());
    if let Some(cover) = text(value, &["coverImgUrl", "coverUrl"]) {
        item = item.artwork(secure(cover));
    }
    Some(item)
}

/// `1700000000000` (ms) as `2023-11-14` (UTC).
fn date(ms: u64) -> Option<String> {
    let days = i64::try_from(ms / 86_400_000).ok()?;
    // Days to a civil date (proleptic Gregorian), after Howard Hinnant.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// `1234567` as `1,234,567`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// What `playlist/detail/get/v2` says about a playlist, read with the field
/// names the official client reads from playlists (`describe`,
/// `coverImgUrl`, `playCount`, `tags`, `createTime`).
pub fn playlist_details(value: &Value) -> ItemDetails {
    let tags: Vec<&str> = value
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let facts = [
        number(value, &["playCount"])
            .filter(|n| *n > 0)
            .map(|n| (FactKey::Popularity, format!("{} plays", thousands(n)))),
        (!tags.is_empty()).then(|| (FactKey::Other("Tags".into()), tags.join(", "))),
        number(value, &["createTime"])
            .and_then(date)
            .map(|d| (FactKey::Other("Created".into()), d)),
    ];
    ItemDetails {
        description: text(value, &["describe", "description"]).map(str::to_owned),
        image: text(value, &["coverImgUrl", "coverUrl"]).map(|url| ImageRef::new(secure(url))),
        facts: facts
            .into_iter()
            .flatten()
            .map(|(key, value)| Fact { key, value })
            .collect(),
        comment_count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn playability_follows_the_clients_order() {
        let cases = [
            (
                json!({ "playFlag": true, "visible": false, "songFee": 1 }),
                Access::Full,
            ),
            (json!({ "playFlag": false, "visible": false }), Access::Hidden),
            (
                json!({ "playFlag": false, "resConsumable": true, "userConsumable": true, "freeTrailFlag": true }),
                Access::Full,
            ),
            (
                json!({ "playFlag": false, "freeTrailFlag": true, "songFee": 1 }),
                Access::Preview,
            ),
            (json!({ "playFlag": false, "songFee": 1 }), Access::Vip),
            (json!({ "playFlag": false, "songFee": 4 }), Access::Album),
            (json!({ "playFlag": false, "songFee": 0 }), Access::Denied),
            (json!({ "name": "no flags" }), Access::Unknown),
        ];
        for (song, expected) in cases {
            assert_eq!(access(&song), expected, "{song}");
        }
        assert_eq!(refusal(Access::Vip).unwrap().reason, UnplayableReason::Paywall);
        assert_eq!(
            refusal(Access::Preview).unwrap().reason,
            UnplayableReason::Paywall
        );
        assert_eq!(
            refusal(Access::Hidden).unwrap().reason,
            UnplayableReason::Unavailable
        );
        assert!(refusal(Access::Unknown).is_none());
    }

    #[test]
    fn songs_and_playlists_read_either_spelling() {
        let s = song(&json!({
            "encryptedId": "0123456789abcdef0123456789abcdef", "originalId": 1234567, "name": "Tidal Clock",
            "ar": [{ "name": "Mei Example" }, { "name": "Quiet Pier" }], "al": { "name": "Harbour" }, "dt": 215000,
            "playFlag": false, "songFee": 1
        }))
        .unwrap();
        assert_eq!(s.id, "0123456789abcdef0123456789abcdef");
        assert_eq!(s.artist.as_deref(), Some("Mei Example, Quiet Pier"));
        assert_eq!(
            (s.album.as_deref(), s.duration_ms),
            (Some("Harbour"), Some(215_000))
        );
        assert!(!s.playable);
        let s = song(&json!({ "songId": "abc", "name": "B", "artists": [{ "name": "C" }], "album": { "name": "D" }, "duration": 1000 })).unwrap();
        assert!(s.playable, "no flags: the stream request decides");
        let p = playlist(
            &json!({ "id": 99, "name": "Mine", "creator": { "nickname": "me" }, "trackCount": 3,
                                  "coverImgUrl": "http://p1.music.126.net/x.jpg" }),
        )
        .unwrap();
        assert_eq!((p.id.as_str(), p.child_count), ("99", Some(3)));
        assert_eq!(p.artwork.unwrap().url, "https://p1.music.126.net/x.jpg");
        assert_eq!(list(&json!({ "records": [1, 2] })).len(), 2);
        assert_eq!(one(&json!({ "id": 5, "name": "Liked" })).unwrap()["id"], 5);
        assert_eq!(one(&json!({ "records": [{ "id": 6 }] })).unwrap()["id"], 6);
        assert!(one(&json!({ "records": [] })).is_none());
        let s = song(&json!({ "songId": "c1", "name": "Covered", "al": { "name": "A", "picUrl": "http://p2.music.126.net/a.jpg" } }))
            .unwrap();
        assert_eq!(s.artwork.unwrap().url, "https://p2.music.126.net/a.jpg");
        assert_eq!(list(&json!([1])).len(), 1);
        assert!(list(&json!({ "x": 1 })).is_empty());
        assert_eq!(secure("http://evil.example/x"), "http://evil.example/x");
        assert_eq!(
            secure("http://m701.music.126.net:80/a.mp3"),
            "https://m701.music.126.net/a.mp3"
        );
        assert_eq!(
            secure("http://user@x.music.126.net/a"),
            "http://user@x.music.126.net/a"
        );
    }

    #[test]
    fn playlist_details_read_the_clients_field_names() {
        let details = playlist_details(&json!({
            "id": 99, "name": "Mine", "describe": "Invented songs for a slow tide.",
            "coverImgUrl": "http://p1.music.126.net/x.jpg", "playCount": 1234567,
            "tags": ["华语", " ", "Chill"], "createTime": 1_700_000_000_000_u64, "subed": false
        }));
        assert_eq!(
            details.description.as_deref(),
            Some("Invented songs for a slow tide.")
        );
        assert_eq!(details.image.unwrap().url, "https://p1.music.126.net/x.jpg");
        let facts: Vec<(FactKey, String)> = details.facts.into_iter().map(|f| (f.key, f.value)).collect();
        assert_eq!(
            facts,
            [
                (FactKey::Popularity, "1,234,567 plays".to_string()),
                (FactKey::Other("Tags".into()), "华语, Chill".into()),
                (FactKey::Other("Created".into()), "2023-11-14".into()),
            ]
        );
        assert!(playlist_details(&json!({ "id": 1 })).facts.is_empty());
    }

    #[test]
    fn dates_are_civil_dates() {
        assert_eq!(date(0).as_deref(), Some("1970-01-01"));
        assert_eq!(date(951_782_400_000).as_deref(), Some("2000-02-29"));
        assert_eq!(date(1_767_225_599_000).as_deref(), Some("2025-12-31"));
    }
}
