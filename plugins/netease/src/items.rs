//! NetEase Cloud Music's songs, albums, artists and playlists as Jellyfesh
//! items.
//!
//! Ids are the encrypted ids (32 hex digits) the API takes; the numeric ids
//! that NetEase's links need are not kept (no `[links]`). Songs and playlists
//! keep the bare id, as rows stored before have it; albums and artists, which
//! only search and songs name, have the prefixes [`ALBUM`] and [`ARTIST`], so
//! browsing one works from its id alone. Field names are the ones the
//! research saw in the official client's song lists, and those independent
//! public clients of the open platform read from its search answers; the
//! answers are not documented where we could read, so a few likely spellings
//! are read (to be checked against NetEase's documentation).

use jf_plugin_sdk::prelude::*;
use serde_json::Value;

/// The id prefix of albums: `al:` and NetEase's album id.
pub const ALBUM: &str = "al:";
/// The id prefix of artists: `ar:` and NetEase's artist id.
pub const ARTIST: &str = "ar:";

/// Where an answer's list may be: `records` in the official client's
/// playlist lists and in public clients' search answers.
const LIST_KEYS: [&str; 5] = ["records", "list", "songs", "playlists", "data"];

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
/// under a likely name; empty when there is no data, an empty object, a
/// null list, a count of 0 or no list and no more. `None` when the answer
/// has data but no list Jellyfesh can read.
pub fn listing(data: &Value) -> Option<&[Value]> {
    match data {
        Value::Null => Some(&[]),
        Value::Array(items) => Some(items),
        Value::Object(map) => {
            for key in LIST_KEYS {
                match map.get(key) {
                    Some(Value::Array(items)) => return Some(items),
                    Some(Value::Null) => return Some(&[]),
                    _ => {}
                }
            }
            (map.is_empty() || count(data) == Some(0) || has_more(data) == Some(false)).then_some(&[][..])
        }
        _ => None,
    }
}

/// The list in an answer's `data`, or none.
pub fn list(data: &Value) -> &[Value] {
    listing(data).unwrap_or(&[])
}

/// How many entries the whole list has, when the answer says.
pub fn count(data: &Value) -> Option<u64> {
    number(data, &["recordCount", "total"])
}

/// The one playlist an answer's `data` describes: the object itself (the
/// official client reads `playlist/detail/get/v2` this way), or the first of
/// a list.
pub fn one(data: &Value) -> Option<&Value> {
    if data.get("id").is_some() {
        return Some(data);
    }
    list(data).first()
}

/// The liked songs' playlist in the answer of `playlist/star/get/v2`: its id
/// and how many songs NetEase says it holds. The answer is one playlist (the
/// official client reads its `data` as a playlist record); the record is
/// also looked for under `playlist` and as the first of a list. It needs
/// only an id.
pub fn liked(data: &Value) -> Option<(String, Option<u64>)> {
    [Some(data), data.get("playlist"), list(data).first()]
        .into_iter()
        .flatten()
        .find_map(|record| {
            let id = id(record, &["id", "playlistId", "encryptedId"])?;
            Some((id, number(record, &["trackCount", "songCount"])))
        })
}

/// Whether the list says it has more (when it says so at all).
pub fn has_more(data: &Value) -> Option<bool> {
    ["hasMore", "more"]
        .iter()
        .find_map(|k| data.get(*k).and_then(Value::as_bool))
}

/// Longest description [`shape`] gives.
const SHAPE_MAX: usize = 1500;

/// What an answer looks like, for the log: its keys and the types of their
/// values, never the values. Three levels deep; a list by its first entry;
/// keys sorted, and a key that does not look like a field name (an id used
/// as a key) shown as `<key>`.
pub fn shape(value: &Value) -> String {
    let mut out = String::new();
    describe(value, 3, &mut out);
    if out.len() > SHAPE_MAX {
        let mut end = SHAPE_MAX;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
        out.push('…');
    }
    out
}

fn describe(value: &Value, depth: usize, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(_) => out.push_str("bool"),
        Value::Number(_) => out.push_str("number"),
        Value::String(_) => out.push_str("string"),
        Value::Array(items) => match items.first() {
            None => out.push_str("[]"),
            Some(_) if depth == 0 => out.push_str("[…]"),
            Some(first) => {
                out.push('[');
                describe(first, depth - 1, out);
                out.push(']');
            }
        },
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(_) if depth == 0 => out.push_str("{…}"),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(if field_name(key) { key } else { "<key>" });
                out.push_str(": ");
                describe(&map[key], depth - 1, out);
            }
            out.push('}');
        }
    }
}

/// A key that looks like a field name (`recordCount`, `img1v1Url`), not like
/// data: at most 40 ASCII letters, digits and underscores, starting with a
/// letter, and not a long run of hex digits (an id).
fn field_name(key: &str) -> bool {
    let hex = key.len() >= 16 && key.bytes().all(|b| b.is_ascii_hexdigit());
    key.len() <= 40
        && key.starts_with(|c: char| c.is_ascii_alphabetic())
        && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !hex
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

/// The id of the first artist of a list (the primary one), with the artist
/// prefix.
fn first_artist(artists: Option<&Value>) -> Option<String> {
    let first = artists?.as_array()?.first()?;
    id(first, &["id"]).map(|id| format!("{ARTIST}{id}"))
}

/// A song from a list (a playlist's, an album's, an artist's) or a search.
/// Its album and first artist, where the record gives their ids, are where
/// Go to Album and Go to Artist lead.
pub fn song(value: &Value) -> Option<Item> {
    let mut item = Item::new(
        ItemKind::Track,
        id(value, &["encryptedId", "songId", "id"])?,
        text(value, &["name", "songName"])?,
    );
    let artists = value.get("ar").or_else(|| value.get("artists"));
    item.artist = names(artists).or_else(|| text(value, &["artistName"]).map(str::to_owned));
    item.artist_id = first_artist(artists);
    let album = value.get("al").or_else(|| value.get("album"));
    item.album = album
        .and_then(|a| text(a, &["name"]))
        .or_else(|| text(value, &["albumName"]))
        .map(str::to_owned);
    item.album_id = album
        .and_then(|a| id(a, &["id"]))
        .map(|id| format!("{ALBUM}{id}"));
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

/// An album from a search (`records` of `search/album/get/v2`, as public
/// clients read them: `id`, `name`, `artists`, a cover as `coverImgUrl` or
/// `picUrl`).
pub fn album(value: &Value) -> Option<Item> {
    let mut item = Item::new(
        ItemKind::Album,
        format!("{ALBUM}{}", id(value, &["id", "albumId"])?),
        text(value, &["name", "albumName"])?,
    );
    let artists = value.get("artists").or_else(|| value.get("ar"));
    item.artist = names(artists).or_else(|| text(value, &["artistName"]).map(str::to_owned));
    item.artist_id = first_artist(artists);
    if let Some(cover) = text(value, &["coverImgUrl", "picUrl"]) {
        item = item.artwork(secure(cover));
    }
    Some(item)
}

/// An artist from a search (`records` of `search/artists/get/v2`). Public
/// clients read `id` and `name`, the picture under one of several names,
/// and some each artist under `artist`.
pub fn artist(value: &Value) -> Option<Item> {
    let value = value.get("artist").filter(|a| a.is_object()).unwrap_or(value);
    let mut item = Item::new(
        ItemKind::Artist,
        format!("{ARTIST}{}", id(value, &["id", "artistId"])?),
        text(value, &["name", "artistName"])?,
    );
    if let Some(picture) = text(value, &["coverImgUrl", "picUrl", "img1v1Url", "avatar", "cover"]) {
        item = item.artwork(secure(picture));
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
    fn search_records_read_as_songs_albums_and_artists_with_their_ids() {
        // A song as public clients of the open platform read its search
        // records: the encrypted `id`, `artists` and `album` with ids.
        let s = song(&json!({
            "id": "5eac0000000000000000000000000001", "originalId": 1901, "name": "Low Water",
            "artists": [{ "id": "a7000000000000000000000000000001", "originalId": 31, "name": "Mei Example" },
                        { "id": "a7000000000000000000000000000002", "name": "Quiet Pier" }],
            "album": { "id": "a1b00000000000000000000000000001", "originalId": 41, "name": "Tide Tables" },
            "duration": 201000, "coverImgUrl": "http://p1.music.126.net/mock/search.jpg", "playFlag": true,
            "visible": true
        }))
        .unwrap();
        assert_eq!(s.id, "5eac0000000000000000000000000001");
        assert_eq!(s.artist.as_deref(), Some("Mei Example, Quiet Pier"));
        assert_eq!(
            s.artist_id.as_deref(),
            Some("ar:a7000000000000000000000000000001")
        );
        assert_eq!(
            (s.album.as_deref(), s.album_id.as_deref()),
            (Some("Tide Tables"), Some("al:a1b00000000000000000000000000001"))
        );
        assert_eq!(s.duration_ms, Some(201_000));
        assert_eq!(s.artwork.unwrap().url, "https://p1.music.126.net/mock/search.jpg");
        assert!(s.playable);
        // Without ids there is nowhere to go.
        let s =
            song(&json!({ "id": "x", "name": "Y", "artists": [{ "name": "Z" }], "albumName": "W" })).unwrap();
        assert_eq!(
            (s.artist_id, s.album_id, s.album.as_deref()),
            (None, None, Some("W"))
        );

        let a = album(&json!({
            "id": "a1b00000000000000000000000000001", "name": "Tide Tables",
            "artists": [{ "id": "a7000000000000000000000000000001", "name": "Mei Example" }],
            "coverImgUrl": "http://p1.music.126.net/mock/album.jpg", "company": "Invented Records"
        }))
        .unwrap();
        assert_eq!(
            (a.kind, a.id.as_str()),
            (ItemKind::Album, "al:a1b00000000000000000000000000001")
        );
        assert_eq!(
            (a.artist.as_deref(), a.artist_id.as_deref()),
            (Some("Mei Example"), Some("ar:a7000000000000000000000000000001"))
        );
        assert_eq!(a.artwork.unwrap().url, "https://p1.music.126.net/mock/album.jpg");
        let a =
            album(&json!({ "id": "b2", "name": "B", "picUrl": "https://p2.music.126.net/b.jpg" })).unwrap();
        assert_eq!(a.artwork.unwrap().url, "https://p2.music.126.net/b.jpg");
        assert!(album(&json!({ "name": "no id" })).is_none());

        let r = artist(&json!({
            "id": "a7000000000000000000000000000001", "name": "Mei Example",
            "picUrl": "http://p1.music.126.net/mock/artist.jpg"
        }))
        .unwrap();
        assert_eq!(
            (r.kind, r.id.as_str(), r.artist.as_deref()),
            (ItemKind::Artist, "ar:a7000000000000000000000000000001", None)
        );
        assert_eq!(r.artwork.unwrap().url, "https://p1.music.126.net/mock/artist.jpg");
        let r = artist(&json!({ "artist": { "id": 31, "name": "Quiet Pier", "img1v1Url": "https://p1.music.126.net/q.jpg" } }))
            .unwrap();
        assert_eq!((r.id.as_str(), r.title.as_str()), ("ar:31", "Quiet Pier"));
        assert!(artist(&json!({ "id": "x" })).is_none());
    }

    #[test]
    fn lists_tell_an_empty_answer_from_one_that_cannot_be_read() {
        for empty in [
            json!(null),
            json!([]),
            json!({}),
            json!({ "records": [] }),
            json!({ "records": null }),
            json!({ "recordCount": 0 }),
            json!({ "hasMore": false }),
        ] {
            assert_eq!(listing(&empty).map(<[Value]>::len), Some(0), "{empty}");
        }
        assert_eq!(
            listing(&json!({ "records": [1, 2], "recordCount": 9 })).map(<[Value]>::len),
            Some(2)
        );
        for unreadable in [
            json!({ "recordCount": 3 }),
            json!({ "result": { "songs": [1] } }),
            json!("text"),
            json!(7),
        ] {
            assert!(listing(&unreadable).is_none(), "{unreadable}");
            assert!(list(&unreadable).is_empty());
        }
        assert_eq!(count(&json!({ "recordCount": 307, "records": [] })), Some(307));
    }

    #[test]
    fn the_liked_songs_playlist_needs_only_an_id() {
        assert_eq!(
            liked(&json!({ "id": "pl", "originalId": 9, "name": "Liked", "trackCount": 6 })),
            Some(("pl".into(), Some(6)))
        );
        assert_eq!(liked(&json!({ "playlistId": "pl2" })), Some(("pl2".into(), None)));
        assert_eq!(
            liked(&json!({ "playlist": { "id": "pl3", "trackCount": 0 } })),
            Some(("pl3".into(), Some(0)))
        );
        assert_eq!(
            liked(&json!({ "records": [{ "id": "pl4" }] })),
            Some(("pl4".into(), None))
        );
        for none in [
            json!(null),
            json!({}),
            json!({ "name": "Liked" }),
            json!({ "records": [] }),
            json!([]),
        ] {
            assert_eq!(liked(&none), None, "{none}");
        }
    }

    #[test]
    fn shapes_show_keys_and_types_never_values() {
        let answer = json!({
            "id": "0123456789abcdef0123456789abcdef", "name": "Someone's liked songs", "trackCount": 6,
            "records": [{ "id": "secret-song", "name": "A Private Title" }], "tags": [], "creator": null,
            "subed": false, "fedcba9876543210fedc": 1, "nested": { "deeper": { "deepest": { "x": 1 } } }
        });
        let described = shape(&answer);
        assert_eq!(
            described,
            "{creator: null, <key>: number, id: string, name: string, nested: {deeper: {deepest: {…}}}, \
             records: [{id: string, name: string}], subed: bool, tags: [], trackCount: number}"
        );
        for value in ["0123456789abcdef", "Someone", "secret", "Private", "fedcba"] {
            assert!(!described.contains(value), "{value} in {described}");
        }
        assert_eq!(shape(&json!(null)), "null");
        let wide: serde_json::Map<String, Value> =
            (0..500).map(|i| (format!("field{i}"), json!(i))).collect();
        assert!(shape(&Value::Object(wide)).len() <= SHAPE_MAX + '…'.len_utf8());
    }

    #[test]
    fn dates_are_civil_dates() {
        assert_eq!(date(0).as_deref(), Some("1970-01-01"));
        assert_eq!(date(951_782_400_000).as_deref(), Some("2000-02-29"));
        assert_eq!(date(1_767_225_599_000).as_deref(), Some("2025-12-31"));
    }
}
