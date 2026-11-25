//! The Jellyfesh source plugin for Subsonic and OpenSubsonic servers
//! (Navidrome, Gonic, Airsonic, LMS, Ampache and others).
//!
//! * Sign-in: an OpenSubsonic API key, or a user name and password exchanged
//!   for one salt and token pair; neither the password nor the pair leaves
//!   the keystore except as request parameters to the configured server
//!   ([`login`], ADR-0015).
//! * Search: `search3`. Browse: artists → albums → tracks, album lists,
//!   playlists, starred items. Stream: `stream` with a format and bit-rate
//!   limit from the level the app asks for ([`media`]). Artwork: `getCoverArt`. Lyrics:
//!   `getLyricsBySongId` (OpenSubsonic `songLyrics`) as LRC or plain text
//!   ([`lyrics`]), else `getLyrics`. Recommendations: `getSimilarSongs`.
//! * Discover (API 1.2): `getAlbumList2` recently added, most played,
//!   recently played and random. Details: `getAlbumInfo2` notes and
//!   `getAlbum` facts, `getArtistInfo2` biographies, `getSong` facts and
//!   playlist descriptions ([`details`]).
//! * Library (API 1.3): `getStarred2`, newest star first, and `getPlaylists`
//!   (the person's own, and others' public ones). Download: `download`, when
//!   the account's download role allows it ([`media`]).
//! * JSON answers (`f=json`), extensions found with
//!   `getOpenSubsonicExtensions`, form POST when the server offers it.

mod api;
mod details;
mod items;
mod lenient;
mod login;
mod lyrics;
mod media;
mod model;
mod settings;

use jf_plugin_sdk::prelude::*;

use crate::api::{Auth, Failure};
use crate::items::Id;
use crate::model::{
    AlbumList, Artist, Found, Indexes, LyricsList, PlainLyrics, Playlist, Playlists, Song, Songs,
};

/// Newest first by when the person starred each item; undated ones last, in
/// the server's order.
fn newest_first<T>(list: &mut [T], starred: impl Fn(&T) -> Option<&str>) {
    list.sort_by(|a, b| starred(b).cmp(&starred(a)));
}

struct Subsonic;

/// Album lists under "Albums": (`getAlbumList2` type, title).
const ALBUM_LISTS: &[(&str, &str)] = &[
    ("newest", "Recently added"),
    ("recent", "Recently played"),
    ("frequent", "Most played"),
    ("random", "Random"),
    ("alphabeticalByName", "A to Z"),
    ("alphabeticalByArtist", "By artist"),
];

/// Text for a plugin error, for people.
pub(crate) fn api_text(error: &PluginError) -> String {
    match error {
        PluginError::Unsupported => "Not supported.".into(),
        PluginError::AuthRequired => "Sign in again.".into(),
        PluginError::NotFound => "Not found.".into(),
        PluginError::InvalidInput(text)
        | PluginError::Network(text)
        | PluginError::Service(text)
        | PluginError::Internal(text) => text.clone(),
        PluginError::RateLimited => "The server asked Jellyfesh to slow down.".into(),
    }
}

/// Errors that mean "nothing here" for optional lookups (lyrics, similar
/// songs): Subsonic errors and odd answers; network and sign-in problems
/// still count.
pub(crate) fn soft<T>(result: core::result::Result<T, Failure>) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(Failure::Api(error)) if matches!(error.code, 40 | 41 | 42 | 44) => Err(error.into()),
        Err(Failure::Api(_)) | Err(Failure::Plugin(PluginError::Service(_))) => Ok(None),
        Err(Failure::Plugin(error)) => Err(error),
    }
}

fn field<T: serde::de::DeserializeOwned + Default>(envelope: &serde_json::Value, key: &str) -> Result<T> {
    match envelope.get(key) {
        None | Some(serde_json::Value::Null) => Ok(T::default()),
        Some(value) => Ok(serde_json::from_value(value.clone())?),
    }
}

/// A whole list (fetched once and kept briefly for paging), sliced to a page.
fn whole(key: &str, page: &PageRequest, fetch: impl FnOnce() -> Result<Vec<Item>>) -> Result<ItemPage> {
    let list = match api::cached_list(key) {
        Some(list) => list,
        None => {
            let list = fetch()?;
            api::keep_list(key, &list);
            list
        }
    };
    items::slice(list, page)
}

/// Like [`whole`], but a first page always asks the server again: the app
/// asks for one each time it refreshes the list.
fn fresh(key: &str, page: &PageRequest, fetch: impl FnOnce() -> Result<Vec<Item>>) -> Result<ItemPage> {
    if page.cursor.as_deref().is_none_or(str::is_empty) {
        let list = fetch()?;
        api::keep_list(key, &list);
        return items::slice(list, page);
    }
    whole(key, page, fetch)
}

/// The person's starred songs, albums or artists (`library.likes`).
fn likes(kind: ItemKind, page: &PageRequest) -> Result<ItemPage> {
    let key = match kind {
        ItemKind::Track => "likes:songs",
        ItemKind::Album => "likes:albums",
        ItemKind::Artist => "likes:artists",
        _ => return Err(PluginError::Unsupported),
    };
    fresh(key, page, || {
        let mut found: Found = api::call_field("getStarred2", &[], "starred2")?;
        Ok(match kind {
            ItemKind::Track => {
                newest_first(&mut found.song, |s| s.starred.as_deref());
                items::tracks(&found.song)
            }
            ItemKind::Album => {
                newest_first(&mut found.album, |a| a.starred.as_deref());
                found.album.iter().filter_map(items::album).collect()
            }
            _ => {
                newest_first(&mut found.artist, |a| a.starred.as_deref());
                found.artist.iter().filter_map(items::artist).collect()
            }
        })
    })
}

/// The playlists the server lists for the account (`library.playlists`):
/// its own, and other accounts' public ones, which count as followed.
fn playlists(page: &PageRequest) -> Result<PlaylistPage> {
    let found: Playlists = api::call_field("getPlaylists", &[], "playlists")?;
    let me = api::username();
    let entries: Vec<PlaylistEntry> = found
        .playlist
        .iter()
        .filter_map(|p| {
            let item = items::playlist(p)?;
            let owned = match (&me, &p.owner) {
                (Some(me), Some(owner)) => owner.eq_ignore_ascii_case(me),
                _ => true,
            };
            Some(if owned {
                PlaylistEntry::owned(item)
            } else {
                PlaylistEntry::followed(item)
            })
        })
        .collect();
    let start = items::offset(page)?;
    let total = entries.len();
    let end = start.saturating_add(page.limit.max(1) as usize).min(total);
    Ok(PlaylistPage {
        entries: entries
            .into_iter()
            .skip(start)
            .take(end.saturating_sub(start))
            .collect(),
        next: (end < total).then(|| end.to_string()),
        total: u32::try_from(total).ok(),
    })
}

/// Where to fetch a song (`download.download`): always `download`, which the
/// server's download role governs, never `stream`.
fn download_info(id: &str, quality: DownloadQuality) -> Result<DownloadInfo> {
    // The account's download role (Navidrome: whether downloads are on at
    // all). When the server cannot say, it decides when the file is fetched.
    if api::can_download() == Some(false) {
        return Err(PluginError::Unsupported);
    }
    let song: Song = api::call_field("getSong", &[("id", id)], "song")?;
    if song.id.is_empty() {
        return Err(PluginError::NotFound);
    }
    if song.is_video == Some(true) {
        return Err(PluginError::service("Videos can't be downloaded in Jellyfesh."));
    }
    let converts = api::info()?
        .server_type
        .is_some_and(|kind| kind.eq_ignore_ascii_case("navidrome"));
    let plan = media::download(
        song.suffix.as_deref(),
        song.bit_rate,
        quality,
        converts,
        api::settings().transcode,
    );
    Ok(match plan {
        media::Download::Original => DownloadInfo {
            url: api::media_url("download", &[("id", id), ("format", "raw")])?,
            headers: Vec::new(),
            container: song.suffix.unwrap_or_default().to_ascii_lowercase(),
            size: song.size,
            original: true,
            // Only the name: the folders above it are the server's.
            file_name: song
                .path
                .as_deref()
                .and_then(|path| path.rsplit(['/', '\\']).next())
                .filter(|name| !name.is_empty())
                .map(str::to_string),
        },
        media::Download::Convert { format, kbps } => DownloadInfo {
            url: api::media_url(
                "download",
                &[("id", id), ("format", format), ("bitrate", &kbps.to_string())],
            )?,
            headers: Vec::new(),
            container: format.into(),
            size: None,
            original: false,
            file_name: None,
        },
    })
}

fn root() -> ItemPage {
    ItemPage::new(vec![
        items::folder("artists", "Artists"),
        items::folder("albums", "Albums"),
        items::folder("playlists", "Playlists"),
        items::folder("starred", "Starred"),
    ])
}

fn browse_folder(path: &str, page: &PageRequest) -> Result<ItemPage> {
    match path {
        "artists" => whole("artists", page, || {
            let indexes: Indexes = api::call_field("getArtists", &[], "artists")?;
            Ok(indexes
                .index
                .iter()
                .flat_map(|i| i.artist.iter())
                .filter_map(items::artist)
                .collect())
        }),
        "albums" => Ok(ItemPage::new(
            ALBUM_LISTS
                .iter()
                .map(|(kind, title)| items::folder(&format!("albums:{kind}"), title))
                .collect(),
        )),
        "playlists" => whole("playlists", page, || {
            let found: Playlists = api::call_field("getPlaylists", &[], "playlists")?;
            Ok(found.playlist.iter().filter_map(items::playlist).collect())
        }),
        "starred" => Ok(ItemPage::new(vec![
            items::folder("starred:songs", "Songs"),
            items::folder("starred:albums", "Albums"),
            items::folder("starred:artists", "Artists"),
        ])),
        "starred:songs" | "starred:albums" | "starred:artists" => whole(path, page, || {
            let found: Found = api::call_field("getStarred2", &[], "starred2")?;
            Ok(match path {
                "starred:songs" => items::tracks(&found.song),
                "starred:albums" => found.album.iter().filter_map(items::album).collect(),
                _ => found.artist.iter().filter_map(items::artist).collect(),
            })
        }),
        _ => match path.strip_prefix("albums:") {
            Some(kind) if ALBUM_LISTS.iter().any(|(k, _)| *k == kind) => album_list(kind, page),
            _ => Err(PluginError::NotFound),
        },
    }
}

/// Discover's shelves: album lists (`getAlbumList2` type, title, style).
const SHELVES: [(&str, &str, ShelfStyle); 4] = [
    ("newest", "Recently Added", ShelfStyle::Row),
    ("frequent", "Most Played", ShelfStyle::Row),
    ("recent", "Recently Played", ShelfStyle::Row),
    ("random", "Random Albums", ShelfStyle::Grid),
];
/// Albums per shelf.
const SHELF_SIZE: u32 = 20;

/// The Discover page: the album lists that have albums. A list the server
/// cannot give is left out; sign-in and network problems fail the page.
fn discover_shelves() -> Result<Vec<Shelf>> {
    let mut shelves = Vec::new();
    for (kind, title, style) in SHELVES {
        let page = PageRequest {
            cursor: None,
            limit: SHELF_SIZE,
        };
        let albums = match album_list(kind, &page) {
            Ok(page) => page.items,
            Err(error @ (PluginError::AuthRequired | PluginError::Network(_) | PluginError::RateLimited)) => {
                return Err(error);
            }
            Err(error) => {
                jf_plugin_sdk::warn!("no {kind} albums for Discover: {}", api_text(&error));
                continue;
            }
        };
        if albums.is_empty() {
            continue;
        }
        shelves.push(Shelf {
            id: kind.into(),
            title: title.into(),
            style,
            items: albums,
            more: Some(format!("f:albums:{kind}")),
        });
    }
    Ok(shelves)
}

/// `getAlbumList2`, paged by the server.
fn album_list(kind: &str, page: &PageRequest) -> Result<ItemPage> {
    let offset = items::offset(page)?;
    let size = page.limit.clamp(1, 200);
    let list: AlbumList = api::call_field(
        "getAlbumList2",
        &[
            ("type", kind),
            ("size", &size.to_string()),
            ("offset", &offset.to_string()),
        ],
        "albumList2",
    )?;
    let got = list.album.len();
    let albums: Vec<Item> = list.album.iter().filter_map(items::album).collect();
    Ok(ItemPage {
        items: albums,
        next: (got >= size as usize).then(|| (offset + got).to_string()),
        total: None,
    })
}

/// Where a search continues: offsets of artists, albums and songs, or `-`
/// for a kind that has no more results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SearchCursor {
    offsets: [usize; 3],
    done: [bool; 3],
}

impl SearchCursor {
    fn parse(cursor: Option<&str>) -> Result<SearchCursor> {
        let Some(cursor) = cursor.filter(|c| !c.is_empty()) else {
            return Ok(SearchCursor::default());
        };
        let bad = || PluginError::invalid_input(format!("bad cursor {cursor:?}"));
        let parts: Vec<&str> = cursor.split(',').collect();
        let [a, b, c] = parts.as_slice() else {
            return Err(bad());
        };
        let mut out = SearchCursor::default();
        for (i, part) in [a, b, c].into_iter().enumerate() {
            if *part == "-" {
                out.done[i] = true;
            } else {
                out.offsets[i] = part.parse().map_err(|_| bad())?;
            }
        }
        Ok(out)
    }

    fn encode(&self) -> String {
        (0..3)
            .map(|i| {
                if self.done[i] {
                    "-".to_string()
                } else {
                    self.offsets[i].to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// How many artists, albums and songs to ask for: a short list of artists
/// and albums before the songs, or everything for the only kind left.
fn allocation(wanted: [bool; 3], limit: u32) -> [u32; 3] {
    let left: Vec<usize> = (0..3).filter(|i| wanted[*i]).collect();
    match left.as_slice() {
        [] => [0; 3],
        [only] => {
            let mut out = [0; 3];
            out[*only] = limit;
            out
        }
        _ if wanted[2] => {
            // A fifth each for artists and albums; tiny pages go to songs
            // first, and the others follow once the songs run out.
            let small = limit / 5;
            let artists = if wanted[0] { small } else { 0 };
            let albums = if wanted[1] { small } else { 0 };
            [artists, albums, limit - artists - albums]
        }
        _ => [limit / 2, limit - limit / 2, 0],
    }
}

fn find_song(item: &Item) -> Result<Option<String>> {
    if item.title.trim().is_empty() {
        return Ok(None);
    }
    let found: Found = api::call_field(
        "search3",
        &[
            ("query", item.title.trim()),
            ("artistCount", "0"),
            ("albumCount", "0"),
            ("songCount", "20"),
        ],
        "searchResult3",
    )?;
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    Ok(found
        .song
        .iter()
        .find(|song| {
            same(&song.title, &item.title)
                && match (&item.artist, &song.artist) {
                    (Some(wanted), Some(artist)) => {
                        let wanted = wanted.to_lowercase();
                        let artist = artist.to_lowercase();
                        wanted.contains(&artist) || artist.contains(&wanted)
                    }
                    _ => true,
                }
                && match (item.duration_ms, song.duration) {
                    (Some(ms), Some(s)) => (i64::from(ms) - i64::from(s) * 1000).abs() <= 3000,
                    _ => true,
                }
        })
        .map(|song| song.id.clone()))
}

fn lookup_cover(id: Id<'_>) -> Result<Option<String>> {
    Ok(match id {
        Id::Track(id) => api::call_field::<Song>("getSong", &[("id", id)], "song")?.cover_art,
        Id::Album(id) => api::call_field::<model::Album>("getAlbum", &[("id", id)], "album")?.cover_art,
        Id::Artist(id) => api::call_field::<Artist>("getArtist", &[("id", id)], "artist")?.cover_art,
        Id::Playlist(id) => api::call_field::<Playlist>("getPlaylist", &[("id", id)], "playlist")?.cover_art,
        Id::Folder(_) => None,
    })
}

impl Plugin for Subsonic {
    fn capabilities() -> Vec<Capability> {
        let mut capabilities = vec![
            Capability::Auth,
            Capability::Search,
            Capability::Browse,
            Capability::ResolveStream,
            Capability::Lyrics,
            Capability::Artwork,
        ];
        // getSimilarSongs exists since API 1.11.0.
        if api::remembered_info().and_then(|i| i.api_at_least(1, 11)) != Some(false) {
            capabilities.push(Capability::Recommendations);
        }
        capabilities
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        api::set_settings(settings::Settings::parse(settings)?);
        Ok(())
    }

    fn auth_begin(flow: Flow) -> Result<FlowStep> {
        login::begin(flow)
    }

    fn auth_poll(_flow: Flow) -> Result<FlowStep> {
        Ok(FlowStep::failed("Nothing to wait for. Start again."))
    }

    fn auth_submit(flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
        login::submit(flow, fields)
    }

    fn logout() -> Result<()> {
        // A Subsonic sign-in is not a session on the server, and API keys
        // can only be revoked in the server's own settings; the host deletes
        // the stored sign-in.
        api::forget();
        Ok(())
    }

    fn search(query: String, filters: SearchFilters, page: PageRequest) -> Result<ItemPage> {
        let wants = |kind| filters.kinds.is_empty() || filters.kinds.contains(&kind);
        let wanted = [
            wants(ItemKind::Artist),
            wants(ItemKind::Album),
            wants(ItemKind::Track),
        ];
        let mut cursor = SearchCursor::parse(page.cursor.as_deref())?;
        let left = [0, 1, 2].map(|i| wanted[i] && !cursor.done[i]);
        let counts = allocation(left, page.limit.clamp(1, 200));
        if counts == [0; 3] {
            return Ok(ItemPage::new(Vec::new()));
        }
        let text = |n: usize| n.to_string();
        let (c, o) = (counts.map(|n| n.to_string()), cursor.offsets.map(text));
        let mut found: Found = api::call_field(
            "search3",
            &[
                ("query", query.trim()),
                ("artistCount", &c[0]),
                ("artistOffset", &o[0]),
                ("albumCount", &c[1]),
                ("albumOffset", &o[1]),
                ("songCount", &c[2]),
                ("songOffset", &o[2]),
            ],
            "searchResult3",
        )?;
        // Some servers treat a count of 0 as their default; keep what was asked.
        found.artist.truncate(counts[0] as usize);
        found.album.truncate(counts[1] as usize);
        found.song.truncate(counts[2] as usize);
        let got = [found.artist.len(), found.album.len(), found.song.len()];
        for i in 0..3 {
            if counts[i] > 0 {
                cursor.offsets[i] += got[i];
                cursor.done[i] = got[i] < counts[i] as usize;
            } else if !wanted[i] {
                cursor.done[i] = true;
            }
        }
        let mut results: Vec<Item> = found.artist.iter().filter_map(items::artist).collect();
        results.extend(found.album.iter().filter_map(items::album));
        results.extend(items::tracks(&found.song));
        let more = (0..3).any(|i| wanted[i] && !cursor.done[i]);
        Ok(ItemPage {
            items: results,
            next: more.then(|| cursor.encode()),
            total: None,
        })
    }

    fn browse(node: Option<Item>, page: PageRequest) -> Result<ItemPage> {
        let Some(node) = node else {
            return Ok(root());
        };
        match Id::parse(&node.id) {
            Some(Id::Folder(path)) => browse_folder(path, &page),
            Some(Id::Artist(id)) => whole(&node.id, &page, || {
                let artist: Artist = api::call_field("getArtist", &[("id", id)], "artist")?;
                Ok(artist.album.iter().filter_map(items::album).collect())
            }),
            Some(Id::Album(id)) => whole(&node.id, &page, || {
                let album: model::Album = api::call_field("getAlbum", &[("id", id)], "album")?;
                Ok(items::tracks(&album.song))
            }),
            Some(Id::Playlist(id)) => whole(&node.id, &page, || {
                let playlist: Playlist = api::call_field("getPlaylist", &[("id", id)], "playlist")?;
                Ok(items::tracks(&playlist.entry))
            }),
            Some(Id::Track(_)) => Err(PluginError::invalid_input("a track has no children")),
            None => Err(PluginError::NotFound),
        }
    }

    fn resolve_stream(item: Item, quality: Quality) -> Result<StreamInfo> {
        let Some(Id::Track(id)) = Id::parse(&item.id) else {
            return Err(PluginError::invalid_input("not a track from this server"));
        };
        let settings = api::settings();
        let song: Song = api::call_field("getSong", &[("id", id)], "song")?;
        if song.id.is_empty() {
            return Err(PluginError::NotFound);
        }
        if let Some(reason) = items::unplayable(&song, api::can_stream()) {
            return Err(PluginError::service(
                reason
                    .message
                    .unwrap_or_else(|| "This track can't be played.".into()),
            ));
        }
        // The level the app asks for (ADR-0026); the plugin has no quality
        // setting of its own.
        let plan = media::plan(song.suffix.as_deref(), song.bit_rate, quality, settings.transcode);
        let rate = plan.max_bit_rate.map(|r| r.to_string());
        let mut params = vec![("id", id), ("format", plan.format.as_str())];
        if let Some(rate) = &rate {
            params.push(("maxBitRate", rate));
        }
        let mut info = StreamInfo::new(api::media_url("stream", &params)?);
        info.codec = plan.codec;
        info.bitrate_kbps = plan.bitrate;
        info.cacheable = true;
        Ok(info)
    }

    fn lyrics(item: Item) -> Result<Option<LyricPayload>> {
        let song_id = match Id::parse(&item.id) {
            Some(Id::Track(id)) => Some(id.to_string()),
            Some(_) => return Err(PluginError::invalid_input("lyrics are for tracks")),
            None => find_song(&item)?,
        };
        if let Some(id) = &song_id
            && api::info()?.has("songLyrics")
            && let Some(envelope) = soft(api::request("getLyricsBySongId", &[("id", id)], Auth::Session))?
        {
            let list: LyricsList = field(&envelope, "lyricsList")?;
            if let Some(chosen) = lyrics::choose(&list.structured_lyrics) {
                return Ok(Some(lyrics::payload(chosen)));
            }
        }
        // Plain lyrics by artist and title (Subsonic 1.2.0).
        let title = item.title.trim();
        if title.is_empty() {
            return Ok(None);
        }
        let artist = item.artist.as_deref().unwrap_or_default().trim();
        let Some(envelope) = soft(api::request(
            "getLyrics",
            &[("artist", artist), ("title", title)],
            Auth::Session,
        ))?
        else {
            return Ok(None);
        };
        let plain: PlainLyrics = field(&envelope, "lyrics")?;
        let text = plain.value.replace("\r\n", "\n").replace('\r', "\n");
        if text.trim().is_empty() {
            return Ok(None);
        }
        Ok(Some(LyricPayload::new(
            LyricFormat::Plain,
            text.trim().to_string() + "\n",
        )))
    }

    fn artwork(item: Item, size: u32) -> Result<Option<ImageRef>> {
        let Some(id) = Id::parse(&item.id) else {
            return Ok(None);
        };
        let cover = match api::cover_of(&item.id) {
            Some(cover) => Some(cover),
            None => {
                let cover = lookup_cover(id)?;
                api::remember_cover(&item.id, &cover);
                cover
            }
        };
        let Some(cover) = cover else {
            return Ok(None);
        };
        let url = api::media_url("getCoverArt", &[("id", &cover), ("size", &size.to_string())])?;
        Ok(Some(ImageRef::new(url)))
    }

    fn discover() -> Result<Vec<Shelf>> {
        discover_shelves()
    }

    fn details(item: Item) -> Result<ItemDetails> {
        details::details(&item)
    }

    fn library_likes(kind: ItemKind, page: PageRequest) -> Result<ItemPage> {
        likes(kind, &page)
    }

    fn library_playlists(page: PageRequest) -> Result<PlaylistPage> {
        playlists(&page)
    }

    fn download(item: Item, quality: DownloadQuality) -> Result<DownloadInfo> {
        match Id::parse(&item.id) {
            Some(Id::Track(id)) => download_info(id, quality),
            _ => Err(PluginError::invalid_input("not a track from this server")),
        }
    }

    fn recommendations_for_item(item: Item, page: PageRequest) -> Result<ItemPage> {
        if api::info()?.api_at_least(1, 11) == Some(false) {
            return Err(PluginError::Unsupported);
        }
        if page.cursor.is_some() {
            // One page of similar songs: the server picks them at random.
            return Ok(ItemPage::new(Vec::new()));
        }
        let id = match Id::parse(&item.id) {
            Some(Id::Track(id) | Id::Album(id) | Id::Artist(id)) => id.to_string(),
            Some(_) => return Err(PluginError::Unsupported),
            None => match find_song(&item)? {
                Some(id) => id,
                None => return Ok(ItemPage::new(Vec::new())),
            },
        };
        let count = page.limit.clamp(1, 100).to_string();
        let found = soft(api::request(
            "getSimilarSongs",
            &[("id", &id), ("count", &count)],
            Auth::Session,
        ))?;
        let songs: Songs = match found {
            Some(envelope) => field(&envelope, "similarSongs")?,
            None => Songs::default(),
        };
        let own = format!("tr:{id}");
        Ok(ItemPage::new(
            items::tracks(&songs.song)
                .into_iter()
                .filter(|track| track.id != own)
                .collect(),
        ))
    }
}

jf_plugin_sdk::export_plugin!(Subsonic);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_cursors_round_trip() {
        let cursor = SearchCursor::parse(Some("4,-,20")).unwrap();
        assert_eq!(cursor.offsets, [4, 0, 20]);
        assert_eq!(cursor.done, [false, true, false]);
        assert_eq!(cursor.encode(), "4,-,20");
        assert_eq!(SearchCursor::parse(None).unwrap(), SearchCursor::default());
        assert!(SearchCursor::parse(Some("1,2")).is_err());
        assert!(SearchCursor::parse(Some("a,b,c")).is_err());
    }

    #[test]
    fn search_shares_the_page_between_kinds() {
        assert_eq!(allocation([true, true, true], 50), [10, 10, 30]);
        assert_eq!(allocation([false, false, true], 50), [0, 0, 50]);
        assert_eq!(allocation([true, false, false], 7), [7, 0, 0]);
        assert_eq!(allocation([true, true, false], 7), [3, 4, 0]);
        assert_eq!(allocation([true, true, true], 2), [0, 0, 2]);
        assert_eq!(allocation([true, true, false], 1), [0, 1, 0]);
        assert_eq!(allocation([false, false, false], 9), [0, 0, 0]);
    }

    #[test]
    fn manifest_version_matches_the_crate() {
        let manifest = include_str!("../manifest.toml");
        let line = format!("version = \"{}\"", env!("CARGO_PKG_VERSION"));
        assert!(
            manifest.lines().any(|l| l == line),
            "manifest.toml must say {line}"
        );
    }
}
