//! QQ Music's records as Jellyfesh items.
//!
//! Item ids are the bare ids the API returns (`songMid`, `albumMid`,
//! `singerMid`, `dissId`), so the manifest's `[links]` open them in QQ Music.
//! Folders, which have no links, use prefixed ids (`charts`, `chart:<topId>`).
//! Nothing plays here: every track, album, artist and playlist carries the
//! reason, and opening it in QQ Music is the main action.

use jf_plugin_sdk::prelude::*;
use serde::{Deserialize, Deserializer};

/// Why nothing plays here, shown on every item.
pub const NOT_HERE: &str = "QQ Music's open API does not stream; open it in QQ Music.";

/// An id the API sends as a number (`int64`) or, to be safe, as text.
fn id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Id {
        Number(i64),
        Text(String),
    }
    Ok(match Option::<Id>::deserialize(deserializer)? {
        Some(Id::Number(n)) => n.to_string(),
        Some(Id::Text(t)) => t.trim().to_owned(),
        None => String::new(),
    })
}

fn not_here(item: Item) -> Item {
    item.unplayable(UnplayableReason::Other, Some(NOT_HERE.into()))
}

fn text(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// A song in search results, recommendations, charts and playlists.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    #[serde(default)]
    pub song_mid: String,
    #[serde(default)]
    pub song_name: String,
    #[serde(default)]
    pub singer_name: Option<String>,
}

impl Song {
    pub fn item(self) -> Option<Item> {
        let (id, title) = (self.song_mid.trim().to_owned(), self.song_name.trim().to_owned());
        if id.is_empty() || title.is_empty() {
            return None;
        }
        let mut item = Item::new(ItemKind::Track, id, title);
        item.artist = text(self.singer_name);
        Some(not_here(item))
    }
}

/// An album in search results.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    #[serde(default)]
    pub album_mid: String,
    #[serde(default)]
    pub album_name: String,
    #[serde(default)]
    pub singer_name: Option<String>,
    #[serde(default)]
    pub public_time: Option<String>,
}

impl Album {
    pub fn item(self) -> Option<Item> {
        let (id, title) = (
            self.album_mid.trim().to_owned(),
            self.album_name.trim().to_owned(),
        );
        if id.is_empty() || title.is_empty() {
            return None;
        }
        let mut item = Item::new(ItemKind::Album, id, title);
        item.artist = text(self.singer_name);
        item.year = self.public_time.as_deref().and_then(year);
        Some(not_here(item))
    }
}

/// The year at the start of a date such as `2003-07-31`.
pub fn year(date: &str) -> Option<u32> {
    let digits: String = date.trim().chars().take(4).collect();
    (digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
        .filter(|y| (1000..=2999).contains(y))
}

/// A picture the API names, as https (the hosts' http links upgraded); none
/// for anything that is not an http(s) URL.
pub fn picture(url: Option<&str>) -> Option<ImageRef> {
    let url = url?.trim();
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    (!host.is_empty() && !host.contains('@')).then(|| ImageRef::new(format!("https://{rest}")))
}

/// A playlist (`diss`) in search results and AI playlists.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    #[serde(default, deserialize_with = "id")]
    pub diss_id: String,
    #[serde(default)]
    pub diss_name: String,
    #[serde(default)]
    pub creator_name: Option<String>,
    /// Its cover, where an answer names one.
    #[serde(default, alias = "logo")]
    pub pic_url: Option<String>,
}

impl Playlist {
    pub fn item(self) -> Option<Item> {
        let title = self.diss_name.trim().to_owned();
        if self.diss_id.is_empty() || self.diss_id == "0" || title.is_empty() {
            return None;
        }
        let mut item = Item::new(ItemKind::Playlist, self.diss_id, title);
        item.artist = text(self.creator_name);
        item.artwork = picture(self.pic_url.as_deref());
        Some(not_here(item))
    }
}

/// A singer in search results.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Singer {
    #[serde(default)]
    pub singer_mid: String,
    #[serde(default)]
    pub singer_name: String,
}

impl Singer {
    pub fn item(self) -> Option<Item> {
        let (id, title) = (
            self.singer_mid.trim().to_owned(),
            self.singer_name.trim().to_owned(),
        );
        if id.is_empty() || title.is_empty() {
            return None;
        }
        Some(not_here(Item::new(ItemKind::Artist, id, title)))
    }
}

/// A chart in `/charts`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chart {
    #[serde(default, deserialize_with = "id")]
    pub top_id: String,
    #[serde(default)]
    pub top_name: String,
    #[serde(default)]
    pub total_num: Option<i64>,
}

impl Chart {
    pub fn item(self) -> Option<Item> {
        let title = self.top_name.trim().to_owned();
        if self.top_id.is_empty() || title.is_empty() {
            return None;
        }
        let mut item = Item::new(ItemKind::Folder, format!("chart:{}", self.top_id), title);
        item.child_count = self.total_num.and_then(|n| u32::try_from(n).ok());
        Some(item)
    }
}

/// A group of charts in `/charts`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartGroup {
    #[serde(default)]
    pub group_name: String,
    #[serde(default)]
    pub group_top_list: Option<Vec<Chart>>,
}

/// `null` lists as empty ones (search answers set the other types to `null`).
pub fn list<T>(value: Option<Vec<T>>) -> Vec<T> {
    value.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse<T: for<'de> Deserialize<'de>>(value: serde_json::Value) -> T {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn songs_are_unplayable_with_the_reason() {
        let song: Song = parse(json!({
            "songMid": "000Test01", "songName": " Night Tram ", "singerName": "Lin Example",
            "songH5Url": "https://y.qq.com/n/ryqq/songDetail/000Test01"
        }));
        let item = song.item().unwrap();
        assert_eq!(
            (item.id.as_str(), item.title.as_str()),
            ("000Test01", "Night Tram")
        );
        assert_eq!(item.artist.as_deref(), Some("Lin Example"));
        assert!(!item.playable);
        let unplayable = item.unplayable.unwrap();
        assert_eq!(unplayable.reason, UnplayableReason::Other);
        assert_eq!(unplayable.message.as_deref(), Some(NOT_HERE));
        assert!(
            parse::<Song>(json!({ "songMid": "", "songName": "x" }))
                .item()
                .is_none()
        );
    }

    #[test]
    fn ids_and_years_are_lenient() {
        let p: Playlist =
            parse(json!({ "dissId": 9_007_199_254_740_993_i64, "dissName": "Late", "creatorName": "" }));
        let item = p.item().unwrap();
        assert_eq!(item.id, "9007199254740993", "int64 ids stay exact");
        assert_eq!(item.artist, None);
        let p: Playlist = parse(json!({ "dissId": "77", "dissName": "Text id" }));
        assert_eq!(p.item().unwrap().id, "77");
        assert!(
            parse::<Playlist>(json!({ "dissId": 0, "dissName": "zero" }))
                .item()
                .is_none()
        );
        assert_eq!(year("2003-07-31"), Some(2003));
        assert_eq!(year("2003"), Some(2003));
        assert_eq!(year("03-07"), None);
        assert_eq!(year(""), None);
        let album: Album = parse(json!({ "albumMid": "a1", "albumName": "A", "publicTime": "1999-12-01" }));
        assert_eq!(album.item().unwrap().year, Some(1999));
    }

    #[test]
    fn pictures_are_https() {
        assert_eq!(
            picture(Some(" http://qpic.y.qq.com/music_cover/x/300?n=1 ")).map(|p| p.url),
            Some("https://qpic.y.qq.com/music_cover/x/300?n=1".into())
        );
        assert_eq!(picture(Some("ftp://x/y")), None);
        assert_eq!(picture(Some("https://user@host/x")), None);
        let p: Playlist =
            parse(json!({ "dissId": 5, "dissName": "With cover", "picUrl": "https://y.gtimg.cn/c.jpg" }));
        assert_eq!(
            p.item().unwrap().artwork.map(|a| a.url).as_deref(),
            Some("https://y.gtimg.cn/c.jpg")
        );
    }

    #[test]
    fn charts_are_folders() {
        let group: ChartGroup = parse(json!({
            "groupName": "Peak", "groupTopList": [
                { "topId": 26, "topName": "Hot", "totalNum": 300, "songList": [] },
                { "topId": 27, "topName": "" }
            ]
        }));
        let charts: Vec<Item> = list(group.group_top_list)
            .into_iter()
            .filter_map(Chart::item)
            .collect();
        assert_eq!(charts.len(), 1);
        assert_eq!(charts[0].id, "chart:26");
        assert_eq!(charts[0].kind, ItemKind::Folder);
        assert_eq!(charts[0].child_count, Some(300));
        assert!(charts[0].playable, "folders are for browsing");
    }
}
