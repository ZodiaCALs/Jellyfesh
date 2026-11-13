use std::fs;

use jf_library::{LibraryError, ScanProgress, TrackQuery};
use lofty::tag::{Accessor, ItemKey, ItemValue, TagItem};

use crate::fixtures::{self, Fixture, standard};

#[test]
fn multi_value_and_explicit_artist_tags_are_used_as_given() {
    let fx = Fixture::new();
    let multi = fx.path("multi.flac");
    fixtures::flac(&multi, 50);
    fixtures::tag(&multi, |t| {
        t.set_title("Duet".to_owned());
        for name in ["Simon & Garfunkel", "AC/DC"] {
            t.push(TagItem::new(
                ItemKey::TrackArtist,
                ItemValue::Text(name.to_owned()),
            ));
        }
    });
    let explicit = fx.path("explicit.flac");
    fixtures::flac(&explicit, 50);
    fixtures::tag(&explicit, |t| {
        t.set_title("Credits".to_owned());
        for name in ["X feat. Y", "Z"] {
            t.push(TagItem::new(
                ItemKey::TrackArtists,
                ItemValue::Text(name.to_owned()),
            ));
        }
    });
    fx.scan();

    assert_eq!(
        fx.track_at("multi.flac").artist.as_deref(),
        Some("Simon & Garfunkel; AC/DC")
    );
    assert_eq!(
        fx.track_at("explicit.flac").artist.as_deref(),
        Some("X feat. Y; Z")
    );
    let names: Vec<String> = fx
        .lib
        .artists(0, 10)
        .unwrap()
        .into_iter()
        .map(|a| a.name)
        .collect();
    assert_eq!(
        names,
        ["AC/DC", "Simon & Garfunkel", "X feat. Y", "Z"],
        "values are not split again"
    );

    let simon = fx.lib.artists(0, 10).unwrap().remove(1);
    let q = TrackQuery {
        artist_id: Some(simon.id),
        ..TrackQuery::default()
    };
    assert_eq!(fx.lib.tracks(&q).unwrap()[0].title, "Duet");
}

#[test]
fn initial_scan_reads_every_audio_file() {
    let fx = Fixture::new();
    standard::build(&fx);

    let mut calls: Vec<ScanProgress> = Vec::new();
    let report = fx.lib.scan(&mut |p| calls.push(p)).unwrap();

    assert_eq!(report.added, standard::GOOD_FILES);
    assert_eq!((report.updated, report.removed, report.unchanged), (0, 0, 0));
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert_eq!(report.errors[0].0, fx.path(standard::BROKEN));

    assert_eq!(
        calls.len() as u64,
        standard::AUDIO_FILES,
        "non-audio and hidden files are skipped"
    );
    let last = calls.last().unwrap();
    assert_eq!(
        (last.files_seen, last.files_changed),
        (standard::AUDIO_FILES, standard::AUDIO_FILES)
    );

    let stats = fx.lib.stats().unwrap();
    assert_eq!(stats.roots, 1);
    assert_eq!(stats.tracks, standard::GOOD_FILES);
    // Album One (Ludwig), Loud Album (Alpha), 叶惠美, ألبوم; the untagged file has no album.
    assert_eq!(stats.albums, 4);
    // Ludwig, Alpha, Beta (split from "Alpha feat. Beta"), 周杰伦, فيروز.
    assert_eq!(stats.artists, 5);
    assert!(stats.total_duration_ms >= 1400, "{stats:?}");
    assert!(stats.total_bytes > 0);
}

#[test]
fn tags_and_audio_properties_are_read() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();

    let flac = fx.track_at(standard::COVER_A);
    assert_eq!(flac.title, "Moonlight Sonata");
    assert_eq!(flac.artist.as_deref(), Some("Ludwig"));
    assert_eq!(flac.album.as_deref(), Some("Album One"));
    assert_eq!(flac.album_artist.as_deref(), Some("Ludwig"));
    assert_eq!(flac.track_no, Some(1));
    assert_eq!(flac.year, Some(2001));
    assert_eq!(flac.genre.as_deref(), Some("Classical"));
    assert_eq!(flac.codec.as_deref(), Some("flac"));
    assert_eq!(
        (flac.sample_rate, flac.channels, flac.bit_depth),
        (Some(8000), Some(1), Some(16))
    );
    assert!(
        flac.duration_ms.is_some_and(|ms| (290..=310).contains(&ms)),
        "{:?}",
        flac.duration_ms
    );
    assert!(flac.album_id.is_some());

    let wav = fx.track_at(standard::RG);
    assert_eq!(wav.codec.as_deref(), Some("pcm"));
    assert_eq!(wav.artist.as_deref(), Some("Alpha feat. Beta"));
    assert_eq!(wav.isrc.as_deref(), Some("USRC17607839"));
    assert_eq!(wav.track_no, Some(3));

    let cjk = fx.track_at(standard::CJK);
    assert_eq!(cjk.title, "晴天");
    assert_eq!(cjk.artist.as_deref(), Some("周杰伦"));

    let arabic = fx.track_at(standard::ARABIC);
    assert_eq!(arabic.title, "سلام عليكم");
    assert_eq!(arabic.album.as_deref(), Some("ألبوم"));
}

#[test]
fn missing_tags_stay_empty_and_title_falls_back_to_the_file_stem() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();

    let plain = fx.track_at(standard::UNTAGGED);
    assert_eq!(plain.title, "plain_song");
    assert_eq!(
        (plain.artist, plain.album, plain.album_artist),
        (None, None, None)
    );
    assert_eq!((plain.album_id, plain.track_no, plain.year), (None, None, None));
    assert_eq!(plain.codec.as_deref(), Some("pcm"));
}

#[test]
fn rescan_skips_unchanged_files() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let revision = fx.lib.revision().unwrap();
    let before = fx.all_tracks();

    let mut changed = 0;
    let report = fx.lib.scan(&mut |p| changed = p.files_changed).unwrap();
    assert_eq!(report.unchanged, standard::GOOD_FILES);
    assert_eq!((report.added, report.updated, report.removed), (0, 0, 0));
    assert_eq!(changed, 1, "only the broken file is read again");
    assert_eq!(report.errors.len(), 1);
    assert_eq!(fx.lib.revision().unwrap(), revision, "nothing changed");
    assert_eq!(fx.all_tracks(), before);
}

#[test]
fn rescan_updates_touched_and_retagged_files_and_removes_deleted_ones() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let touched = fx.track_at(standard::COVER_A);
    let revision = fx.lib.revision().unwrap();

    fixtures::touch(&fx.path(standard::COVER_A), 60);
    fixtures::tag(&fx.path(standard::RG), |t| {
        t.set_title("Gain Check (Remix)".to_owned())
    });
    fixtures::touch(&fx.path(standard::RG), 60);
    fs::remove_file(fx.path(standard::CJK)).unwrap();
    fixtures::wav(&fx.path("new/fresh.wav"), 100);

    let report = fx.scan();
    assert_eq!(
        (report.added, report.updated, report.removed),
        (1, 2, 1),
        "{report:?}"
    );
    assert_eq!(report.unchanged, standard::GOOD_FILES - 3);
    assert!(fx.lib.revision().unwrap() > revision);

    let after = fx.track_at(standard::COVER_A);
    assert_eq!(after.id, touched.id, "ids survive updates");
    assert_ne!(after.mtime_ns, touched.mtime_ns);
    assert_eq!(fx.track_at(standard::RG).title, "Gain Check (Remix)");
    assert!(fx.all_tracks().iter().all(|t| t.path != fx.path(standard::CJK)));
    assert_eq!(fx.track_at("new/fresh.wav").title, "fresh");

    // The deleted file's album and artist went with it.
    let stats = fx.lib.stats().unwrap();
    assert_eq!((stats.tracks, stats.albums, stats.artists), (6, 3, 4));
    fx.assert_search_index_consistent();
}

#[test]
fn a_missing_root_reports_errors_and_keeps_its_tracks() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();

    let away = fx.dir.path().join("unmounted");
    fs::rename(&fx.music, &away).unwrap();
    let report = fx.scan();
    assert!(!report.errors.is_empty());
    assert_eq!(report.removed, 0, "an unreadable root must not empty the library");
    assert_eq!(fx.lib.stats().unwrap().tracks, standard::GOOD_FILES);

    fs::rename(&away, &fx.music).unwrap();
    let report = fx.scan();
    assert_eq!(report.unchanged, standard::GOOD_FILES);
}

#[test]
fn remove_root_drops_its_tracks_albums_and_artists() {
    let fx = Fixture::new();
    standard::build(&fx);
    let other = fx.dir.path().join("other");
    fixtures::wav(&other.join("elsewhere.wav"), 100);
    fixtures::tag(&other.join("elsewhere.wav"), |t| {
        fixtures::basic(t, "Elsewhere", "Gamma", "Far", 1)
    });
    fx.lib.add_root(&other).unwrap();
    fx.scan();
    assert_eq!(fx.lib.stats().unwrap().tracks, standard::GOOD_FILES + 1);
    let revision = fx.lib.revision().unwrap();

    fx.lib.remove_root(&fx.music).unwrap();
    assert_eq!(fx.lib.roots().unwrap(), vec![other.clone()]);
    let stats = fx.lib.stats().unwrap();
    assert_eq!(
        (stats.roots, stats.tracks, stats.albums, stats.artists),
        (1, 1, 1, 1)
    );
    assert!(fx.lib.revision().unwrap() > revision);
    assert_eq!(fx.all_tracks()[0].title, "Elsewhere");
    let found = fx.lib.search("ludwig", 10).unwrap();
    assert!(found.tracks.is_empty() && found.albums.is_empty() && found.artists.is_empty());
    assert_eq!(fx.lib.search("elsewhere", 10).unwrap().tracks.len(), 1);
    fx.assert_search_index_consistent();

    let err = fx.lib.remove_root(&fx.music).unwrap_err();
    assert!(matches!(err, LibraryError::RootNotFound(_)), "{err}");
}

#[test]
fn nested_roots_do_not_duplicate_tracks() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let inner = fx.music.join("cjk");
    fx.lib.add_root(&inner).unwrap();
    assert_eq!(
        fx.lib.add_root(&inner).unwrap(),
        fx.lib.add_root(&inner).unwrap(),
        "idempotent"
    );

    let report = fx.scan();
    assert_eq!(report.unchanged, standard::GOOD_FILES, "{report:?}");
    assert_eq!(fx.lib.stats().unwrap().tracks, standard::GOOD_FILES);

    // The CJK file now belongs to the inner root and goes with it.
    fx.lib.remove_root(&inner).unwrap();
    assert_eq!(fx.lib.stats().unwrap().tracks, standard::GOOD_FILES - 1);
    // The outer root picks it up again on the next scan.
    assert_eq!(fx.scan().added, 1);
    fx.assert_search_index_consistent();
}
