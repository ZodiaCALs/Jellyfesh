//! Discover, details and comments (plugin API 1.2, ADR-0022), from the web
//! player's own answers.
//!
//! * Discover: the daily picks when signed in
//!   (`weapi/v3/discovery/recommend/songs`), recommended playlists
//!   (`weapi/v1/discovery/recommend/resource` signed in,
//!   `weapi/personalized/playlist` otherwise) and the charts
//!   (`weapi/toplist`, which are playlists). Each has See All
//!   (`ncm:f:daily`, `ncm:f:recommended`, `ncm:f:charts`).
//! * Details: a song's release date and album picture
//!   (`weapi/v3/song/detail`), an album's notes, release date, label and kind
//!   (`weapi/v1/album/<id>`), an artist's short biography, other names and
//!   picture (`weapi/v1/artist/<id>`), a playlist's description, tags, plays,
//!   creation date and comment count (`weapi/v6/playlist/detail`).
//! * Comments on a song, an album or a playlist
//!   (`weapi/v1/resource/comments/<thread>`): the hot ones on the first page,
//!   then the latest, read-only. Artists have no comments.

use jf_community_common::{date_from_epoch_ms, join_artists, non_empty, paging};
use jf_plugin_sdk::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api;
use crate::catalog::{self, list_of};
use crate::items::{self, Id};
use crate::model::{self, Playlist, Song};

/// Items on a Discover shelf.
const SHELF: usize = 30;
/// The large picture of details.
const LARGE: u32 = 1200;
/// Avatars next to comments.
const AVATAR: u32 = 100;
/// NetEase dates in China time.
const UTC_OFFSET_HOURS: i64 = 8;
/// Most comments NetEase lists from one offset.
const COMMENTS_MAX: usize = 50;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Daily {
    data: DailyData,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DailyData {
    #[serde(rename = "dailySongs")]
    daily_songs: Vec<Value>,
}

/// The daily picks of the signed-in account, as tracks. Songs that come
/// without their privileges get them in one more request, so nothing that
/// cannot play looks playable.
fn daily() -> Result<Vec<Item>> {
    api::require_session()?;
    let answer: Daily = api::call_as("/v3/discovery/recommend/songs", json!({}))?;
    let songs = list_of::<Song>(Value::Array(answer.data.daily_songs));
    let missing: Vec<u64> = songs
        .iter()
        .filter(|s| s.privilege.is_none())
        .map(|s| s.id)
        .collect();
    let privileges = if missing.is_empty() {
        Vec::new()
    } else {
        catalog::songs(&missing)?.1
    };
    Ok(items::tracks(&songs, &privileges))
}

/// Recommended playlists: the account's when signed in, else everyone's.
fn recommended() -> Result<Vec<Item>> {
    let lists = if api::session()?.is_some() {
        let body = api::call("/v1/discovery/recommend/resource", json!({}))?;
        body.get("recommend").cloned()
    } else {
        let body = api::call(
            "/personalized/playlist",
            json!({ "limit": SHELF, "total": true, "n": 1000 }),
        )?;
        body.get("result").cloned()
    };
    Ok(list_of::<Playlist>(lists.unwrap_or(Value::Null))
        .iter()
        .filter_map(items::playlist)
        .collect())
}

/// The charts, as playlists.
fn charts() -> Result<Vec<Item>> {
    let body = api::call("/toplist", json!({}))?;
    Ok(
        list_of::<Playlist>(body.get("list").cloned().unwrap_or(Value::Null))
            .iter()
            .filter_map(items::playlist)
            .collect(),
    )
}

/// A See All folder of Discover, listed in full; `None` for other folders.
pub fn browse_folder(name: &str, page: &PageRequest) -> Option<Result<ItemPage>> {
    let items = match name {
        "daily" => daily(),
        "recommended" => recommended(),
        "charts" => charts(),
        _ => return None,
    };
    Some(items.and_then(|items| paging::slice(items, page)))
}

/// The Discover page. A shelf whose list fails is left out; the sign-in,
/// the rate limit and the network fail the page.
pub fn discover() -> Result<Vec<Shelf>> {
    type Fetch = fn() -> Result<Vec<Item>>;
    let signed_in = api::session()?.is_some();
    let lists: [(&str, &str, ShelfStyle, Fetch, bool); 3] = [
        ("daily", "Daily Picks", ShelfStyle::Row, daily, signed_in),
        (
            "recommended",
            "Recommended Playlists",
            ShelfStyle::Grid,
            recommended,
            true,
        ),
        ("charts", "Charts", ShelfStyle::Grid, charts, true),
    ];
    let mut shelves = Vec::new();
    for (name, title, style, fetch, wanted) in lists {
        if !wanted {
            continue;
        }
        match fetch() {
            Ok(mut found) if !found.is_empty() => {
                found.truncate(SHELF);
                shelves.push(Shelf {
                    id: name.into(),
                    title: title.into(),
                    style,
                    items: found,
                    more: Some(items::folder(name, title).id),
                });
            }
            Ok(_) => {}
            Err(error @ (PluginError::AuthRequired | PluginError::RateLimited | PluginError::Network(_))) => {
                return Err(error);
            }
            Err(error) => jf_plugin_sdk::warn!("no {title} for Discover: {}", crate::error_text(&error)),
        }
    }
    Ok(shelves)
}

fn fact(key: FactKey, value: Option<String>) -> Option<Fact> {
    let value = value?.trim().to_owned();
    (!value.is_empty()).then_some(Fact { key, value })
}

fn date(ms: Option<i64>) -> Option<String> {
    date_from_epoch_ms(ms?, UTC_OFFSET_HOURS)
}

/// `1234567` as `1,234,567`.
fn thousands(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn count(value: Option<i64>) -> Option<u64> {
    value.and_then(|n| u64::try_from(n).ok())
}

fn large(url: Option<&str>) -> Option<ImageRef> {
    api::image(url?, LARGE)
}

fn song_details(id: u64) -> Result<ItemDetails> {
    let song = catalog::song(id)?;
    let picture = song.al.as_ref().and_then(|a| a.pic_url.as_deref());
    Ok(ItemDetails {
        description: None,
        image: large(picture),
        facts: fact(FactKey::Released, date(song.publish_time))
            .into_iter()
            .collect(),
        comment_count: None,
    })
}

fn album_details(id: u64) -> Result<ItemDetails> {
    let (album, _) = catalog::album(id)?;
    Ok(ItemDetails {
        description: non_empty(album.description.as_deref()),
        image: large(album.pic_url.as_deref()),
        facts: [
            fact(FactKey::Released, date(album.publish_time)),
            fact(FactKey::Label, album.company.clone()),
            fact(FactKey::Other("Type".into()), album.sub_type.clone()),
        ]
        .into_iter()
        .flatten()
        .collect(),
        comment_count: count(album.info.as_ref().and_then(|i| i.comment_count)),
    })
}

fn artist_details(id: u64) -> Result<ItemDetails> {
    let artist = catalog::artist(id)?;
    let picture = artist.pic_url.as_deref().or(artist.img1v1_url.as_deref());
    Ok(ItemDetails {
        description: non_empty(artist.brief_desc.as_deref()),
        image: large(picture),
        facts: fact(
            FactKey::Other("Also known as".into()),
            join_artists(artist.alias.iter().map(String::as_str)),
        )
        .into_iter()
        .collect(),
        comment_count: None,
    })
}

fn playlist_details(id: u64) -> Result<ItemDetails> {
    let playlist = catalog::playlist(id)?;
    let tags = playlist
        .tags
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    Ok(ItemDetails {
        description: non_empty(playlist.description.as_deref()),
        image: large(playlist.cover()),
        facts: [
            fact(
                FactKey::Popularity,
                playlist
                    .play_count
                    .filter(|n| *n > 0)
                    .map(|n| format!("{} plays", thousands(n))),
            ),
            fact(FactKey::Other("Tags".into()), Some(tags)),
            fact(
                FactKey::Other("Updated".into()),
                playlist.update_frequency.clone(),
            ),
            fact(FactKey::Other("Created".into()), date(playlist.create_time)),
        ]
        .into_iter()
        .flatten()
        .collect(),
        comment_count: count(playlist.comment_count),
    })
}

/// Details of one of the plugin's items.
pub fn details(item: &Item) -> Result<ItemDetails> {
    match Id::parse(&item.id) {
        Some(Id::Track(id)) => song_details(id),
        Some(Id::Album(id)) => album_details(id),
        Some(Id::Artist(id)) => artist_details(id),
        Some(Id::Playlist(id)) => playlist_details(id),
        Some(Id::Folder(_)) => Err(PluginError::Unsupported),
        None => Err(PluginError::NotFound),
    }
}

/// The comment thread of an item: `R_SO_4_` songs, `R_AL_3_` albums,
/// `A_PL_0_` playlists.
fn thread(item: &Item) -> Result<String> {
    match Id::parse(&item.id) {
        Some(Id::Track(id)) => Ok(format!("R_SO_4_{id}")),
        Some(Id::Album(id)) => Ok(format!("R_AL_3_{id}")),
        Some(Id::Playlist(id)) => Ok(format!("A_PL_0_{id}")),
        Some(Id::Artist(_) | Id::Folder(_)) => Err(PluginError::Unsupported),
        None => Err(PluginError::NotFound),
    }
}

fn comment(value: Value) -> Option<Comment> {
    let comment: model::Comment = serde_json::from_value(value).ok()?;
    let text = non_empty(comment.content.as_deref())?;
    if comment.comment_id == 0 {
        return None;
    }
    let user = comment.user.unwrap_or_default();
    Some(Comment {
        id: comment.comment_id.to_string(),
        author: non_empty(user.nickname.as_deref()).unwrap_or_else(|| "NetEase Cloud Music user".into()),
        avatar: user.avatar_url.as_deref().and_then(|url| api::image(url, AVATAR)),
        text,
        likes: count(comment.liked_count),
        time_ms: count(comment.time),
        replies: count(comment.reply_count.or(comment.floor.and_then(|f| f.reply_count))),
    })
}

/// A page of comments on `item`: the hot ones with the first page.
pub fn comments(item: &Item, page: &PageRequest) -> Result<CommentPage> {
    let thread = thread(item)?;
    let offset = paging::offset(page)?;
    let limit = paging::limit(page).min(COMMENTS_MAX);
    let answer: model::Comments = api::call_as(
        &format!("/v1/resource/comments/{thread}"),
        json!({ "rid": thread, "offset": offset, "limit": limit, "total": true }),
    )?;
    let latest: Vec<Comment> = answer.comments.into_iter().filter_map(comment).collect();
    let got = latest.len();
    Ok(CommentPage {
        hot: if offset == 0 {
            answer.hot.into_iter().filter_map(comment).collect()
        } else {
            Vec::new()
        },
        comments: latest,
        total: count(answer.total),
        next: (answer.more && got > 0).then(|| (offset + limit).to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_read_leniently() {
        let made = comment(json!({
            "commentId": "77", "content": " Invented words ", "time": 1_700_000_000_000_i64, "likedCount": "12",
            "user": { "nickname": "Mock Fan", "avatarUrl": "http://p1.music.126.net/mock/a.jpg" },
            "showFloorComment": { "replyCount": 3 }
        }))
        .unwrap();
        assert_eq!(
            (made.id.as_str(), made.author.as_str(), made.text.as_str()),
            ("77", "Mock Fan", "Invented words")
        );
        assert_eq!(
            (made.likes, made.time_ms, made.replies),
            (Some(12), Some(1_700_000_000_000), Some(3))
        );
        assert!(
            made.avatar
                .unwrap()
                .url
                .starts_with("https://p1.music.126.net/mock/a.jpg")
        );
        assert!(comment(json!({ "commentId": 1, "content": "  " })).is_none());
        assert!(comment(json!({ "commentId": 0, "content": "x" })).is_none());
        assert_eq!(
            comment(json!({ "commentId": 2, "content": "x" })).unwrap().author,
            "NetEase Cloud Music user"
        );
    }

    #[test]
    fn threads_by_kind() {
        let thread_of = |id: &str| thread(&Item::new(ItemKind::Track, id, "x"));
        assert_eq!(thread_of("ncm:tr:5").unwrap(), "R_SO_4_5");
        assert_eq!(thread_of("ncm:al:6").unwrap(), "R_AL_3_6");
        assert_eq!(thread_of("ncm:pl:7").unwrap(), "A_PL_0_7");
        assert_eq!(thread_of("ncm:ar:8"), Err(PluginError::Unsupported));
        assert_eq!(thread_of("other:1"), Err(PluginError::NotFound));
    }

    #[test]
    fn counts_get_thousands_separators() {
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
