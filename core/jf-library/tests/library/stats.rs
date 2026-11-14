//! Favourites, play statistics and recently added tracks.

use std::fs;

use jf_library::{ItemStats, LibraryError, PlaylistItem};
use lofty::tag::Accessor;

use crate::fixtures::{self, Fixture, standard};

fn remote(id: &str) -> PlaylistItem {
    PlaylistItem::Source {
        plugin: "org.example".to_owned(),
        id: id.to_owned(),
        meta_json: "{}".to_owned(),
    }
}

fn items(rows: &[ItemStats]) -> Vec<PlaylistItem> {
    rows.iter().map(|row| row.item.clone()).collect()
}

#[test]
fn favourites_are_listed_latest_first_and_keyed_by_path() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let a = fx.track_at(standard::COVER_A);
    let b = fx.track_at(standard::COVER_B);
    let loose = PlaylistItem::File(fx.dir.path().join("loose.flac"));
    let liked_a = PlaylistItem::Library(a.id);

    assert!(!fx.lib.is_favorite(&liked_a).unwrap());
    let revision = fx.lib.revision().unwrap();
    for item in [&liked_a, &loose, &remote("42"), &PlaylistItem::Library(b.id)] {
        fx.lib.set_favorite(item, true).unwrap();
    }
    assert_eq!(fx.lib.revision().unwrap(), revision + 4);

    let revision = fx.lib.revision().unwrap();
    fx.lib.set_favorite(&liked_a, true).unwrap();
    fx.lib.set_favorite(&remote("not liked"), false).unwrap();
    assert_eq!(fx.lib.revision().unwrap(), revision, "no change, no bump");
    let err = fx.lib.set_favorite(&PlaylistItem::Library(-1), true).unwrap_err();
    assert!(matches!(err, LibraryError::TrackNotFound(-1)), "{err}");

    let favorites = fx.lib.favorites(0, 10).unwrap();
    assert_eq!(
        items(&favorites),
        [
            PlaylistItem::Library(b.id),
            remote("42"),
            loose.clone(),
            liked_a.clone()
        ]
    );
    assert_eq!(
        favorites.iter().map(|f| f.track.as_ref()).collect::<Vec<_>>(),
        [Some(&b), None, None, Some(&a)]
    );
    assert!(favorites.iter().all(|f| f.favorite && f.play_count == 0));
    let times: Vec<i64> = favorites.iter().map(|f| f.favorited_at_ms.unwrap()).collect();
    assert!(times.windows(2).all(|w| w[0] > w[1]), "{times:?}");
    assert_eq!(fx.lib.favorites(1, 2).unwrap(), favorites[1..3]);
    assert!(fx.lib.favorites(4, 10).unwrap().is_empty());

    // A file at a track's path is the same item; keys are the path or plugin:id.
    assert!(fx.lib.is_favorite(&PlaylistItem::File(a.path.clone())).unwrap());
    assert!(fx.lib.is_favorite(&remote("42")).unwrap());
    assert!(!fx.lib.is_favorite(&remote("43")).unwrap());
    assert!(!fx.lib.is_favorite(&PlaylistItem::Library(-1)).unwrap());
    assert_eq!(fx.lib.item_key(&liked_a).unwrap().as_deref(), a.path.to_str());
    assert_eq!(
        fx.lib.item_key(&PlaylistItem::File(a.path.clone())).unwrap(),
        fx.lib.item_key(&liked_a).unwrap()
    );
    assert_eq!(
        fx.lib.item_key(&remote("42")).unwrap().as_deref(),
        Some("org.example:42")
    );
    assert_eq!(fx.lib.item_key(&PlaylistItem::Library(-1)).unwrap(), None);

    // Unliking removes it; liking it again puts it first.
    fx.lib.set_favorite(&liked_a, false).unwrap();
    assert!(!fx.lib.is_favorite(&liked_a).unwrap());
    assert_eq!(fx.lib.favorites(0, 10).unwrap().len(), 3);
    fx.lib.set_favorite(&liked_a, true).unwrap();
    assert_eq!(fx.lib.favorites(0, 1).unwrap()[0].item, liked_a);
}

#[test]
fn plays_and_skips_are_counted_and_ordered() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let a = PlaylistItem::Library(fx.track_at(standard::COVER_A).id);
    let b = PlaylistItem::Library(fx.track_at(standard::COVER_B).id);
    let skipped = PlaylistItem::Library(fx.track_at(standard::CJK).id);

    let revision = fx.lib.revision().unwrap();
    let plays = [
        (&a, 1_000, true),
        (&b, 2_000, true),
        (&b, 3_000, true),
        (&skipped, 4_000, false),
        (&remote("r"), 5_000, true),
        (&a, 6_000, true),
        // An older play recorded late keeps the latest time.
        (&a, 500, true),
        (&b, 7_000, false),
    ];
    for (item, at, completed) in plays {
        fx.lib.record_play(item, at, completed).unwrap();
    }
    assert_eq!(fx.lib.revision().unwrap(), revision + plays.len() as u64);

    let most = fx.lib.most_played(10).unwrap();
    assert_eq!(items(&most), [a.clone(), b.clone(), remote("r")]);
    let counts: Vec<(u64, u64, Option<i64>)> = most
        .iter()
        .map(|s| (s.play_count, s.skip_count, s.last_played_ms))
        .collect();
    assert_eq!(
        counts,
        [(3, 0, Some(6_000)), (2, 1, Some(3_000)), (1, 0, Some(5_000))]
    );
    assert!(most.iter().all(|s| !s.favorite));
    assert_eq!(fx.lib.most_played(1).unwrap(), most[..1]);

    let recent = fx.lib.recently_played(10).unwrap();
    assert_eq!(
        items(&recent),
        [a.clone(), remote("r"), b.clone()],
        "skips are not plays"
    );

    // Equal counts order by the latest play.
    fx.lib.record_play(&remote("r"), 8_000, true).unwrap();
    assert_eq!(
        items(&fx.lib.most_played(10).unwrap()),
        [a.clone(), remote("r"), b.clone()]
    );

    // A favourite shares its row with the counts; unliking keeps them.
    fx.lib.set_favorite(&b, true).unwrap();
    let liked = fx.lib.favorites(0, 10).unwrap();
    assert_eq!((liked[0].play_count, liked[0].skip_count), (2, 1));
    fx.lib.set_favorite(&b, false).unwrap();
    assert_eq!(fx.lib.most_played(10).unwrap()[2].play_count, 2);

    let err = fx
        .lib
        .record_play(&PlaylistItem::Library(-1), 1, true)
        .unwrap_err();
    assert!(matches!(err, LibraryError::TrackNotFound(-1)), "{err}");
}

#[test]
fn statistics_follow_a_track_that_is_scanned_in_again() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let a = fx.track_at(standard::COVER_A);
    fx.lib
        .record_play(&PlaylistItem::Library(a.id), 1_000, true)
        .unwrap();

    let away = fx.dir.path().join("away.flac");
    fs::rename(fx.path(standard::COVER_A), &away).unwrap();
    fx.scan();
    let gone = fx.lib.most_played(10).unwrap();
    assert_eq!(gone[0].item, PlaylistItem::File(fx.path(standard::COVER_A)));
    assert_eq!(gone[0].track, None);

    fs::rename(&away, fx.path(standard::COVER_A)).unwrap();
    fx.scan();
    let back = fx.track_at(standard::COVER_A);
    fx.lib
        .record_play(&PlaylistItem::Library(back.id), 2_000, true)
        .unwrap();
    let most = fx.lib.most_played(10).unwrap();
    assert_eq!(most.len(), 1);
    assert_eq!(most[0].item, PlaylistItem::Library(back.id));
    assert_eq!(most[0].track.as_ref(), Some(&back));
    assert_eq!((most[0].play_count, most[0].last_played_ms), (2, Some(2_000)));
}

#[test]
fn recently_added_lists_the_newest_tracks_first() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    assert_eq!(
        fx.lib.recently_added(100).unwrap().len() as u64,
        standard::GOOD_FILES
    );
    assert!(fx.lib.recently_added(0).unwrap().is_empty());

    fixtures::wav(&fx.path("new/fresh.wav"), 100);
    fx.scan();
    let newest = fx.lib.recently_added(2).unwrap();
    assert_eq!(newest.len(), 2);
    assert_eq!(newest[0].title, "fresh");

    // Updating a track keeps when it was added: backdated, it stays the oldest.
    let conn = rusqlite::Connection::open(fx.dir.path().join("db").join("library.db")).unwrap();
    let rg = fx.path(standard::RG);
    // By id: the stored path is spelled as the scanner found it (on Windows with
    // backslashes only), which a path built here need not match as text.
    let id = fx.track_at(standard::RG).id;
    let backdated = conn
        .execute("UPDATE tracks SET added_at = 1 WHERE id = ?1", [id])
        .unwrap();
    assert_eq!(backdated, 1);
    fixtures::tag(&rg, |t| t.set_title("Retitled".to_owned()));
    fixtures::touch(&rg, 60);
    assert_eq!(fx.scan().updated, 1);
    let all = fx.lib.recently_added(100).unwrap();
    assert_eq!(all[0].title, "fresh");
    assert_eq!(all.last().unwrap().title, "Retitled");
}
