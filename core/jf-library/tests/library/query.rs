use jf_library::{TrackQuery, TrackSort};

use crate::fixtures::{Fixture, standard};

fn titles(fx: &Fixture, q: &TrackQuery) -> Vec<String> {
    fx.lib.tracks(q).unwrap().into_iter().map(|t| t.title).collect()
}

#[test]
fn tracks_page_and_sort() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();

    let by_title = TrackQuery {
        sort: TrackSort::Title,
        ..TrackQuery::default()
    };
    // Normalised titles in code-point order: Latin, then Arabic, then CJK.
    let all = [
        "Blue Moon Café",
        "Gain Check",
        "Moonlight Sonata",
        "plain_song",
        "سلام عليكم",
        "晴天",
    ];
    assert_eq!(titles(&fx, &by_title), all);
    assert_eq!(
        titles(
            &fx,
            &TrackQuery {
                offset: 2,
                limit: 2,
                ..by_title.clone()
            }
        ),
        all[2..4]
    );
    assert!(
        titles(
            &fx,
            &TrackQuery {
                offset: 10,
                ..by_title.clone()
            }
        )
        .is_empty()
    );
    assert!(
        titles(
            &fx,
            &TrackQuery {
                limit: 0,
                ..by_title.clone()
            }
        )
        .is_empty()
    );

    let mut reversed = all.to_vec();
    reversed.reverse();
    assert_eq!(
        titles(
            &fx,
            &TrackQuery {
                descending: true,
                ..by_title
            }
        ),
        reversed
    );

    // Artist order puts the untagged track (unknown artist) last.
    let by_artist = titles(&fx, &TrackQuery::default());
    assert_eq!(by_artist.first().map(String::as_str), Some("Gain Check"));
    assert_eq!(by_artist.last().map(String::as_str), Some("plain_song"));
    let by_artist_desc = titles(
        &fx,
        &TrackQuery {
            descending: true,
            ..TrackQuery::default()
        },
    );
    assert_eq!(
        by_artist_desc.last().map(String::as_str),
        Some("plain_song"),
        "unknown stays last"
    );
}

#[test]
fn tracks_filter_by_album_and_artist() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();

    let album_id = fx.track_at(standard::COVER_B).album_id.unwrap();
    let q = TrackQuery {
        album_id: Some(album_id),
        sort: TrackSort::Album,
        ..TrackQuery::default()
    };
    assert_eq!(
        titles(&fx, &q),
        ["Moonlight Sonata", "Blue Moon Café"],
        "disc and track order"
    );

    let beta = fx
        .lib
        .artists(0, 100)
        .unwrap()
        .into_iter()
        .find(|a| a.name == "Beta")
        .unwrap();
    let q = TrackQuery {
        artist_id: Some(beta.id),
        ..TrackQuery::default()
    };
    assert_eq!(titles(&fx, &q), ["Gain Check"]);
}

#[test]
fn albums_and_artists_are_listed_and_paged() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();

    let albums = fx.lib.albums(0, 100).unwrap();
    let names: Vec<&str> = albums.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(names, ["Album One", "Loud Album", "ألبوم", "叶惠美"]);
    let one = &albums[0];
    assert_eq!(
        (one.track_count, one.year, one.album_artist.as_deref()),
        (2, Some(2001), Some("Ludwig"))
    );
    assert_eq!(fx.lib.album(one.id).unwrap().as_ref(), Some(one));
    // Without an album artist tag the album is credited to the primary track artist.
    assert_eq!(albums[1].album_artist.as_deref(), Some("Alpha"));
    assert_eq!(fx.lib.albums(1, 2).unwrap(), albums[1..3]);

    let artists = fx.lib.artists(0, 100).unwrap();
    let names: Vec<&str> = artists.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Alpha", "Beta", "Ludwig", "فيروز", "周杰伦"]);
    assert_eq!(artists[2].track_count, 2);
    assert_eq!(fx.lib.artists(1, 2).unwrap(), artists[1..3]);
    assert_eq!(fx.lib.artist(artists[0].id).unwrap().as_ref(), Some(&artists[0]));

    let track = fx.track_at(standard::CJK);
    assert_eq!(fx.lib.track(track.id).unwrap(), Some(track));
    assert_eq!(fx.lib.track(-1).unwrap(), None);
}

#[test]
fn revision_bumps_on_every_change() {
    let fx = Fixture::new();
    let after_add_root = fx.lib.revision().unwrap();
    assert!(after_add_root > 0);
    fx.lib.add_root(&fx.music).unwrap();
    assert_eq!(
        fx.lib.revision().unwrap(),
        after_add_root,
        "re-adding a root changes nothing"
    );

    standard::build(&fx);
    fx.scan();
    let after_scan = fx.lib.revision().unwrap();
    assert!(after_scan > after_add_root);
    fx.scan();
    assert_eq!(fx.lib.revision().unwrap(), after_scan);
}
