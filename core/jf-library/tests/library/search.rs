use std::fs;
use std::time::{Duration, Instant};

use jf_library::{Library, SearchResults};
use lofty::tag::Accessor;
use rusqlite::{Connection, params};

use crate::fixtures::{self, Fixture, standard};

/// The standard library plus tracks that exercise ranking and escaping.
fn library() -> Fixture {
    let fx = Fixture::new();
    standard::build(&fx);
    for (file, title, artist, album) in [
        ("moon", "Moon", "Nobody", "Night"),
        ("hello", r#"Say "Hello""#, "Quote Band", "Quotes"),
        ("near", "NEAR AND FAR", "Operators", "Syntax"),
        ("wawa", "晴天娃娃", "童谣", "儿歌"),
        ("gesheng", "我的歌声里", "曲婉婷", "Everything in the World"),
        ("pure", "100% Pure", "Percent", "Numbers"),
        ("halo", "Halo", "Beyoncé", "I Am... Sasha Fierce"),
    ] {
        let path = fx.path(&format!("search/{file}.wav"));
        fixtures::wav(&path, 50);
        fixtures::tag(&path, |t| fixtures::basic(t, title, artist, album, 1));
    }
    let report = fx.scan();
    assert_eq!(report.added, standard::GOOD_FILES + 7);
    fx
}

fn search(fx: &Fixture, text: &str) -> SearchResults {
    fx.lib.search(text, 20).unwrap()
}

fn tracks(fx: &Fixture, text: &str) -> Vec<String> {
    search(fx, text).tracks.into_iter().map(|t| t.title).collect()
}

fn albums(fx: &Fixture, text: &str) -> Vec<String> {
    search(fx, text).albums.into_iter().map(|a| a.title).collect()
}

fn artists(fx: &Fixture, text: &str) -> Vec<String> {
    search(fx, text).artists.into_iter().map(|a| a.name).collect()
}

#[test]
fn exact_and_prefix_title_matches_rank_first() {
    let fx = library();
    assert_eq!(
        tracks(&fx, "moon"),
        ["Moon", "Moonlight Sonata", "Blue Moon Café"]
    );
    assert_eq!(tracks(&fx, "Moonlight Sonata"), ["Moonlight Sonata"]);
    assert_eq!(albums(&fx, "album"), ["Album One", "Loud Album"]);
}

#[test]
fn words_match_by_prefix_in_any_column() {
    let fx = library();
    assert_eq!(tracks(&fx, "sona"), ["Moonlight Sonata"]);
    assert_eq!(tracks(&fx, "ludwig sonata"), ["Moonlight Sonata"]);
    let by_artist = tracks(&fx, "ludw");
    assert_eq!(by_artist.len(), 2);
    assert!(by_artist.contains(&"Blue Moon Café".to_owned()));
    assert_eq!(artists(&fx, "ludw"), ["Ludwig"]);
    assert_eq!(tracks(&fx, "fierce"), ["Halo"]);
    assert_eq!(albums(&fx, "fierce"), ["I Am... Sasha Fierce"]);
    assert_eq!(artists(&fx, "bet"), ["Beta"], "split from \"Alpha feat. Beta\"");
}

#[test]
fn substrings_come_from_the_trigram_index() {
    let fx = library();
    assert_eq!(tracks(&fx, "ligh"), ["Moonlight Sonata"], "inside a word");
    assert_eq!(tracks(&fx, "oonlight sonat"), ["Moonlight Sonata"]);
    // A CJK title is one unicode61 token, so a substring needs the trigram index.
    assert_eq!(tracks(&fx, "歌声里"), ["我的歌声里"]);
    // A long piece narrows by trigram, the short one is checked with LIKE.
    assert_eq!(tracks(&fx, "周杰伦 天"), ["晴天"]);
    assert_eq!(tracks(&fx, "周杰伦 晴天"), ["晴天"]);
}

#[test]
fn one_and_two_character_queries_use_the_bounded_scan() {
    let fx = library();
    assert_eq!(tracks(&fx, "晴天"), ["晴天", "晴天娃娃"], "exact, then prefix");
    assert_eq!(tracks(&fx, "晴"), ["晴天", "晴天娃娃"]);
    assert_eq!(tracks(&fx, "歌声"), ["我的歌声里"]);
    assert_eq!(tracks(&fx, "杰伦"), ["晴天"], "artist column");
    assert_eq!(artists(&fx, "杰伦"), ["周杰伦"]);
    assert_eq!(albums(&fx, "惠美"), ["叶惠美"]);
    assert_eq!(tracks(&fx, "杰伦 晴天"), ["晴天"], "every piece must match");
    assert!(tracks(&fx, "杰伦 娃娃").is_empty());
    assert_eq!(
        fx.lib.search("晴", 1).unwrap().tracks.len(),
        1,
        "LIMIT bounds the scan"
    );
}

#[test]
fn case_width_and_diacritics_are_ignored() {
    let fx = library();
    assert_eq!(tracks(&fx, "CAFE"), ["Blue Moon Café"]);
    assert_eq!(tracks(&fx, "café"), ["Blue Moon Café"]);
    assert_eq!(tracks(&fx, "beyonce"), ["Halo"]);
    assert_eq!(artists(&fx, "BEYONCÉ"), ["Beyoncé"]);
    assert_eq!(artists(&fx, "Beyonce\u{301}"), ["Beyoncé"], "decomposed input");
    assert_eq!(tracks(&fx, "ＭＯＯＮ"), tracks(&fx, "moon"), "full-width Latin");
    assert_eq!(tracks(&fx, "ｍｏｏｎｌｉｇｈｔ"), ["Moonlight Sonata"]);
    assert_eq!(tracks(&fx, "سلام"), ["سلام عليكم"]);
    assert_eq!(artists(&fx, "فيروز"), ["فيروز"]);
}

#[test]
fn fts_syntax_is_treated_as_text() {
    let fx = library();
    assert_eq!(tracks(&fx, r#""hello""#), [r#"Say "Hello""#]);
    assert_eq!(tracks(&fx, r#"say "hel"#), [r#"Say "Hello""#]);
    assert_eq!(tracks(&fx, "near"), ["NEAR AND FAR"]);
    assert_eq!(tracks(&fx, "and far"), ["NEAR AND FAR"]);
    assert_eq!(tracks(&fx, "NOT"), Vec::<String>::new());
    for text in [
        "\"",
        "\"\"\"",
        "*",
        "**",
        "moon*",
        "^moon",
        "-moon",
        "+",
        "(",
        "NEAR(",
        "NEAR(moon sonata, 2)",
        "moon AND",
        "AND OR NOT",
        "OR",
        "title:moon",
        "{title artist}: moon",
        "\u{0}",
        "'; DROP TABLE tracks; --",
    ] {
        fx.lib
            .search(text, 20)
            .unwrap_or_else(|err| panic!("{text:?}: {err}"));
    }
    assert_eq!(fx.lib.stats().unwrap().tracks, standard::GOOD_FILES + 7);
}

#[test]
fn like_wildcards_in_short_queries_are_escaped() {
    let fx = library();
    assert_eq!(tracks(&fx, "%"), ["100% Pure"]);
    assert_eq!(tracks(&fx, "0%"), ["100% Pure"]);
    assert_eq!(tracks(&fx, "_"), ["plain_song"]);
    assert!(tracks(&fx, "\\").is_empty());
}

#[test]
fn search_follows_rescans_and_respects_limits() {
    let fx = library();
    assert_eq!(fx.lib.search("moon", 1).unwrap().tracks.len(), 1);
    assert_eq!(fx.lib.search("moon", 0).unwrap(), SearchResults::default());
    assert_eq!(fx.lib.search("  ", 10).unwrap(), SearchResults::default());

    let path = fx.path(standard::COVER_A);
    fixtures::tag(&path, |t| t.set_title("Sunlight Sonata".to_owned()));
    fixtures::touch(&path, 60);
    fs::remove_file(fx.path(standard::COVER_B)).unwrap();
    let report = fx.scan();
    assert_eq!((report.updated, report.removed), (1, 1));

    assert_eq!(tracks(&fx, "moon"), ["Moon"]);
    assert!(tracks(&fx, "moonlight").is_empty());
    assert!(tracks(&fx, "café").is_empty());
    assert_eq!(tracks(&fx, "sunlight"), ["Sunlight Sonata"]);
    assert_eq!(tracks(&fx, "unligh"), ["Sunlight Sonata"]);
    fx.assert_search_index_consistent();
}

/// Risk R13: short CJK queries must stay fast on a large library.
#[test]
#[ignore = "timing check on 50k synthetic tracks; run with --ignored --nocapture"]
fn search_stays_fast_on_50k_tracks() {
    const WORDS: &[&str] = &[
        "love",
        "night",
        "moon",
        "river",
        "晴天",
        "歌声",
        "爱",
        "世界",
        "夜曲",
        "さくら",
        "사랑",
        "حب",
    ];
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("library.db");
    let lib = Library::open(&db, &dir.path().join("cache")).unwrap();
    let root = lib.add_root(dir.path()).unwrap();
    drop(lib);

    let started = Instant::now();
    let mut conn = Connection::open(&db).unwrap();
    let tx = conn.transaction().unwrap();
    for i in 0..50_000_usize {
        let title = format!("{}{} {}", WORDS[i % WORDS.len()], WORDS[i / 7 % WORDS.len()], i);
        let artist = format!("artist {}", i % 2000);
        let album = format!("album {}", i % 4000);
        tx.execute(
            "INSERT INTO tracks (root_id, path, file_size, mtime_ns, title, artist, album,
                title_norm, artist_norm, album_norm, added_at, updated_at)
             VALUES (?1, ?2, 1, 1, ?3, ?4, ?5, lower(?3), ?4, ?5, 0, 0)",
            params![root, format!("/m/{i}.flac"), title, artist, album],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    drop(conn);
    println!("inserted 50k tracks in {:?}", started.elapsed());

    let lib = Library::open(&db, &dir.path().join("cache")).unwrap();
    // Short misses ("龘", "zz") are the worst case: the bounded scan reads every row.
    for text in [
        "晴天",
        "爱",
        "歌声",
        "夜曲 1",
        "love",
        "moonl",
        "iver",
        "世界晴",
        "artist 17",
        "zzz",
        "zz",
        "龘",
    ] {
        let started = Instant::now();
        let found = lib.search(text, 50).unwrap().tracks.len();
        let took = started.elapsed();
        println!("{text:>10}: {found:>2} tracks in {took:?}");
        assert!(took < Duration::from_millis(500), "{text}: {took:?}");
    }
}
