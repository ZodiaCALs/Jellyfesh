//! Core types to list items, with translated fallbacks for missing tags.

use jf_core::{
    Album, Artist, ArtworkRef, ItemKind, PlaylistEntry, PlaylistSummary, SourceItem, Track, TrackRef,
    TrackRow,
};

use crate::logic::format;
use crate::ui::i18n::{tr, trc, trf, trn};
use crate::ui::models::{ItemData, Kind};
use crate::ui::thumbs::ArtKey;

pub fn unknown_artist() -> String {
    tr("Unknown Artist")
}

pub fn unknown_album() -> String {
    tr("Unknown Album")
}

fn duration(ms: Option<u64>) -> String {
    ms.map(format::clock).unwrap_or_default()
}

/// The remote session's two lines: the track's title (the service's name
/// when it reports none) and "Playing on {Device}" (docs/ui.md).
pub fn remote_lines(view: &crate::logic::remote::RemoteView) -> (String, String) {
    let title = view.title.clone().unwrap_or_else(|| view.service.clone());
    let device = view.device.clone().unwrap_or_else(|| view.service.clone());
    (title, trf("Playing on {device}", &[("device", &device)]))
}

/// A source's name (its plugin's), or the id while the slice has no such
/// plugin.
fn source_name(id: &str) -> String {
    crate::ui::state::try_app().map_or_else(|| id.to_owned(), |a| a.source_name(id))
}

/// The artwork to load for a core `ArtworkRef`: a file, or a source's
/// picture the core fetches (never the app).
pub fn art_of(artwork: Option<&ArtworkRef>) -> Option<ArtKey> {
    match artwork? {
        ArtworkRef::File(path) => Some(ArtKey::File(path.clone())),
        ArtworkRef::Url(url) => Some(ArtKey::Url(url.clone())),
    }
}

pub fn album(a: &Album) -> ItemData {
    ItemData {
        kind: Kind::Album,
        id: a.id,
        title: a.title.clone(),
        subtitle: a.album_artist.clone().unwrap_or_else(unknown_artist),
        detail: a.year.map(|y| y.to_string()).unwrap_or_default(),
        art: a.artwork_hash.clone().map(ArtKey::Hash),
        album_id: Some(a.id),
        playable: true,
        ..ItemData::default()
    }
}

pub fn artist(a: &Artist) -> ItemData {
    ItemData {
        kind: Kind::Artist,
        id: a.id,
        title: a.name.clone(),
        subtitle: trn("{n} song", "{n} songs", u64::from(a.track_count)),
        artist_id: Some(a.id),
        playable: true,
        ..ItemData::default()
    }
}

pub fn track(t: &Track) -> ItemData {
    ItemData {
        kind: Kind::Track,
        id: t.id,
        title: t.title.clone(),
        subtitle: format::dot_join([
            Some(t.artist.as_deref().unwrap_or(&unknown_artist())),
            t.album.as_deref(),
        ]),
        detail: duration(t.duration_ms),
        number: t.track_no,
        art: t.artwork_hash.clone().map(ArtKey::Hash),
        track: Some(TrackRef::Library(t.id)),
        album_id: t.album_id,
        artist: t.artist.clone(),
        playable: true,
        ..ItemData::default()
    }
}

/// A library track with its favourite state (asked on the query thread).
pub fn track_with_favorite(core: &jf_core::Core, t: &Track) -> ItemData {
    ItemData {
        favorite: core.is_favorite(&TrackRef::Library(t.id)).unwrap_or(false),
        ..track(t)
    }
}

pub fn track_row(r: &TrackRow) -> ItemData {
    let id = match &r.track {
        TrackRef::Library(id) => *id,
        _ => 0,
    };
    ItemData {
        kind: Kind::Track,
        id,
        title: r.title.clone(),
        subtitle: format::dot_join([
            Some(r.artist.as_deref().unwrap_or(&unknown_artist())),
            r.album.as_deref(),
        ]),
        detail: duration(r.duration_ms),
        art: r
            .artwork_hash
            .clone()
            .map(ArtKey::Hash)
            .or_else(|| art_of(r.artwork.as_ref())),
        track: Some(r.track.clone()),
        artist: matches!(r.track, TrackRef::Library(_))
            .then(|| r.artist.clone())
            .flatten(),
        favorite: r.favorite,
        playable: !r.missing,
        downloaded: r.downloaded,
        ..ItemData::default()
    }
}

pub fn playlist_entry(e: &PlaylistEntry) -> ItemData {
    ItemData {
        kind: Kind::PlaylistEntry,
        id: e.entry_id,
        number: Some(e.position + 1),
        ..track_row(&e.row)
    }
}

pub fn playlist(p: &PlaylistSummary) -> ItemData {
    ItemData {
        kind: Kind::Playlist,
        id: p.id,
        title: p.name.clone(),
        subtitle: trn("{n} song", "{n} songs", u64::from(p.entry_count)),
        detail: if p.duration_ms > 0 {
            trn("{n} min", "{n} min", format::minutes(p.duration_ms))
        } else {
            String::new()
        },
        art: p.artwork_hash.clone().map(ArtKey::Hash),
        playable: true,
        ..ItemData::default()
    }
}

pub fn source_item(s: &SourceItem) -> ItemData {
    let library = s.source == crate::logic::search::LIBRARY;
    let id = if library { s.id.parse().unwrap_or(0) } else { 0 };
    let kind = match (library, s.kind) {
        (true, ItemKind::Album) => Kind::Album,
        (true, ItemKind::Artist) => Kind::Artist,
        (true, ItemKind::Playlist) => Kind::Playlist,
        (true, ItemKind::Track) => Kind::Track,
        _ => Kind::Source,
    };
    // The other sources that have the same item: "Also in …" (docs/ui.md,
    // "Search"; the core merges them).
    let also = crate::logic::search::also_in(&s.also_in, source_name, &trc("list", ", "))
        .map(|names| trf("Also in {sources}", &[("sources", &names)]));
    let subtitle = match s.kind {
        ItemKind::Track => format::dot_join([s.subtitle.as_deref(), s.album.as_deref(), also.as_deref()]),
        _ => format::dot_join([s.subtitle.as_deref(), also.as_deref()]),
    };
    // An item that cannot play here shows why (the source's words).
    let detail = match (&s.unplayable_reason, s.playable) {
        (Some(reason), false) => reason.clone(),
        _ => duration(s.duration_ms),
    };
    ItemData {
        kind,
        id,
        title: s.title.clone(),
        subtitle,
        detail,
        art: art_of(s.artwork.as_ref()),
        track: s.track.clone(),
        album_id: (kind == Kind::Album).then_some(id),
        artist_id: (kind == Kind::Artist).then_some(id),
        artist: (kind == Kind::Track).then(|| s.subtitle.clone()).flatten(),
        playable: s.playable,
        source: Some(Box::new(s.clone())),
        downloaded: s.downloaded,
        ..ItemData::default()
    }
}

/// A queue entry before its track is looked up.
pub fn queue_entry(uid: u64, track: &TrackRef) -> ItemData {
    let (title, subtitle) = match track {
        TrackRef::Library(_) => (String::new(), String::new()),
        TrackRef::File(path) => (
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            String::new(),
        ),
        TrackRef::Source(s) => (
            s.title.clone(),
            format::dot_join([s.artist.as_deref(), s.album.as_deref()]),
        ),
    };
    ItemData {
        kind: Kind::QueueEntry,
        title,
        subtitle,
        track: Some(track.clone()),
        uid,
        playable: true,
        ..ItemData::default()
    }
}

/// A queue entry with its library track.
pub fn queue_track(uid: u64, t: &Track) -> ItemData {
    ItemData {
        kind: Kind::QueueEntry,
        uid,
        ..track(t)
    }
}
