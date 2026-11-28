//! The mirror types: conversions in both directions, JSON round trips, the JSON shapes C#
//! relies on, and `tests/contract/ffi-types.json`, which the C# tests replay.

mod common;

use std::fmt::Debug;
use std::path::Path;

use jf_core as jf;
use jf_ffi::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

fn json_round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
    let json = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}: {json}"));
    assert_eq!(&back, value, "{json}");
}

/// Converts to the mirror and back, checks the result equals the original by `Debug` (not
/// every core type implements `PartialEq`), and round-trips the mirror through JSON.
fn check<C, F>(core: C)
where
    C: Clone + Debug + From<F>,
    F: From<C> + Clone + Serialize + DeserializeOwned + PartialEq + Debug,
{
    let ffi = F::from(core.clone());
    assert_eq!(format!("{:?}", C::from(ffi.clone())), format!("{core:?}"));
    json_round_trip(&ffi);
}

#[test]
fn commands_convert_both_ways() {
    let commands = common::commands();
    assert!(commands.len() > 60);
    for command in commands {
        let ffi = Command::from(command.clone());
        assert_eq!(jf::Command::from(ffi.clone()), command);
        json_round_trip(&ffi);
    }
}

#[test]
fn events_and_snapshots_convert_both_ways() {
    for event in common::events() {
        check::<jf::Event, Event>(event);
    }
    check::<jf::Snapshot, Snapshot>(common::snapshot());
}

#[test]
fn eq_presets_come_from_the_engine() {
    let presets = eq_presets();
    let ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["flat", "bass_boost", "vocal", "treble"]);
    for preset in jf::eq_presets() {
        check::<jf::EqPreset, EqPreset>(preset);
    }
    assert_eq!(
        EqSettings::from(jf::EqSettings::bass_boost()),
        presets[1].settings
    );
}

#[test]
fn query_results_and_arguments_convert_both_ways() {
    check::<jf::Track, Track>(common::track());
    check::<jf::Album, Album>(common::album());
    check::<jf::Artist, Artist>(common::artist());
    check::<jf::SearchResults, SearchResults>(jf::SearchResults {
        tracks: vec![common::track()],
        albums: vec![common::album()],
        artists: vec![common::artist()],
    });
    check::<jf::LibraryStats, LibraryStats>(common::stats());
    check::<jf::TrackRow, TrackRow>(common::track_row());
    check::<jf::TrackRow, TrackRow>(common::source_track_row());
    check::<jf::PlaylistSummary, PlaylistSummary>(common::playlist());
    check::<jf::PlaylistEntry, PlaylistEntry>(common::playlist_entry());
    check::<jf::HistoryEntry, HistoryEntry>(jf::HistoryEntry {
        track: jf::TrackRef::Library(1),
        played_at_ms: 1_700_000_000_000,
    });
    check::<jf::TrackQuery, TrackQuery>(common::track_query());
    for sort in [
        jf::AlbumSort::Title,
        jf::AlbumSort::Artist,
        jf::AlbumSort::Year,
        jf::AlbumSort::Added,
    ] {
        check::<jf::AlbumSort, AlbumSort>(sort);
    }
    for track in common::track_refs() {
        check::<jf::TrackRef, TrackRef>(track);
    }
    for output in [
        jf::OutputKind::Device { name: None },
        jf::OutputKind::Device {
            name: Some("USB DAC".into()),
        },
        jf::OutputKind::WavFile {
            path: "/tmp/out.wav".into(),
            realtime: true,
        },
        jf::OutputKind::Null { realtime: false },
    ] {
        check::<jf::Config, CoreConfig>(jf::Config {
            output,
            ..common::config()
        });
    }
}

#[test]
fn deviations_from_the_core_types() {
    // Colours pack as 0xRRGGBBAA, sorted by name.
    let theme = Theme::from(common::theme(jf::Appearance::Light));
    assert_eq!(
        theme.colors,
        vec![
            ThemeColor {
                name: "accent.accent".into(),
                rgba: 0x0059_a7ff
            },
            ThemeColor {
                name: "ink.ink-90".into(),
                rgba: 0x1018_20eb
            },
        ]
    );
    // Counts that do not fit in u32 saturate.
    let queue = QueueSnapshot::from(jf::QueueSnapshot {
        len: usize::MAX,
        ..common::queue()
    });
    assert_eq!(queue.len, u32::MAX);
    // Scan errors become records; login fields a map.
    let report = ScanReport::from(common::scan_report());
    assert_eq!(
        report.errors,
        vec![ScanError {
            path: "/music/broken.flac".into(),
            message: "not audio".into()
        }]
    );
    let Command::SubmitLogin { fields, .. } = Command::from(jf::Command::SubmitLogin {
        plugin: "p".into(),
        flow: 1,
        fields: vec![("b".into(), "2".into()), ("a".into(), "1".into())],
    }) else {
        unreachable!()
    };
    assert_eq!(fields.len(), 2);
    let jf::Command::SubmitLogin { fields, .. } = jf::Command::from(Command::SubmitLogin {
        plugin: "p".into(),
        flow: 1,
        fields,
    }) else {
        unreachable!()
    };
    assert_eq!(
        fields,
        vec![("a".into(), "1".into()), ("b".into(), "2".into())],
        "sorted by name"
    );
}

/// A config from an app that does not know `resume_on_start` lets playback resume.
#[test]
fn configs_without_resume_on_start_let_playback_resume() {
    let mut json = serde_json::to_value(CoreConfig::from(common::config())).unwrap();
    assert_eq!(json["resume_on_start"], false);
    json.as_object_mut().unwrap().remove("resume_on_start");
    let read: CoreConfig = serde_json::from_value(json).unwrap();
    assert!(read.resume_on_start);
    assert!(jf::Config::from(read).resume_on_start);
}

/// A source track or a row from before v0.0.9, without artist and album ids or a link (an
/// older app's JSON over the C ABI), reads with them empty; the new values cross both ways.
#[test]
fn source_tracks_and_rows_without_ids_read() {
    let mut json = serde_json::to_value(SourceTrack::from(common::source_track())).unwrap();
    for key in ["artist_id", "album_id"] {
        assert!(
            json.as_object_mut().unwrap().remove(key).is_some(),
            "{key} is written"
        );
    }
    let read: SourceTrack = serde_json::from_value(json).unwrap();
    assert_eq!(
        (read.artist_id.as_deref(), read.album_id.as_deref()),
        (None, None)
    );
    assert_eq!(read.title, "Tidal Hymn");

    let row = TrackRow::from(common::source_track_row());
    assert_eq!(row.artist_id.as_deref(), Some("ar-3"));
    assert_eq!(row.album_id.as_deref(), Some("al-9"));
    assert_eq!(row.link.as_deref(), Some("https://music.example/song/tr-7"));
    let mut json = serde_json::to_value(row).unwrap();
    for key in ["artist_id", "album_id", "link"] {
        assert!(
            json.as_object_mut().unwrap().remove(key).is_some(),
            "{key} is written"
        );
    }
    let read: TrackRow = serde_json::from_value(json).unwrap();
    assert_eq!((read.artist_id, read.album_id, read.link), (None, None, None));
}

/// Settings JSON from an older app or file, without the newer fields, reads with their
/// defaults: Check for Updates and Show Lyrics Bar on, Desktop Lyrics off.
#[test]
fn settings_without_newer_fields_take_their_defaults() {
    let mut json = serde_json::to_value(Settings::from(common::settings())).unwrap();
    let fields = json.as_object_mut().unwrap();
    for key in ["check_for_updates", "show_lyrics_bar", "desktop_lyrics"] {
        assert!(fields.remove(key).is_some(), "{key} is written");
    }
    let read: Settings = serde_json::from_value(json).unwrap();
    assert!(read.check_for_updates);
    assert!(read.show_lyrics_bar);
    assert!(!read.desktop_lyrics);
    // The values cross into the core and back unchanged.
    let set = Settings {
        show_lyrics_bar: false,
        desktop_lyrics: true,
        ..read
    };
    let core = jf::Settings::from(set.clone());
    assert!(!core.show_lyrics_bar && core.desktop_lyrics);
    assert_eq!(Settings::from(core), set);
}

/// The JSON the C ABI speaks: enums with data are tagged by `type` (written first, which
/// .NET 8's System.Text.Json needs), plain enums are their name, fields keep their Rust names.
#[test]
fn json_shapes_for_csharp() {
    let json = |value: &dyn erased::Json| value.json();
    assert_eq!(json(&TrackRef::Library { id: 4 }), r#"{"type":"Library","id":4}"#);
    assert_eq!(json(&Command::Play), r#"{"type":"Play"}"#);
    assert_eq!(
        json(&Command::SetRepeat {
            mode: RepeatMode::One
        }),
        r#"{"type":"SetRepeat","mode":"One"}"#
    );
    assert_eq!(
        json(&Event::LibraryChanged { revision: 1 }),
        r#"{"type":"LibraryChanged","revision":1}"#
    );
    assert_eq!(
        json(&Event::NowPlaying { snapshot: None }),
        r#"{"type":"NowPlaying","snapshot":null}"#
    );
    assert_eq!(json(&Status::Playing), r#""Playing""#);
}

mod erased {
    pub trait Json {
        fn json(&self) -> String;
    }
    impl<T: serde::Serialize> Json for T {
        fn json(&self) -> String {
            serde_json::to_string(self).unwrap()
        }
    }
}

/// One JSON sample per line, as the C ABI writes it (field order kept, so the tag comes first).
fn contract() -> String {
    let mut samples: Vec<(&str, String)> = Vec::new();
    let mut add = |name: &'static str, value: &dyn erased::Json| samples.push((name, value.json()));
    for output in [
        jf::OutputKind::Device {
            name: Some("USB DAC".into()),
        },
        jf::OutputKind::WavFile {
            path: "/tmp/out.wav".into(),
            realtime: true,
        },
        jf::OutputKind::Null { realtime: false },
    ] {
        add(
            "CoreConfig",
            &CoreConfig::from(jf::Config {
                output,
                ..common::config()
            }),
        );
    }
    for command in common::commands() {
        add("Command", &Command::from(command));
    }
    for event in common::events() {
        add("Event", &Event::from(event));
    }
    add("Snapshot", &Snapshot::from(common::snapshot()));
    for track in common::track_refs() {
        add("TrackRef", &TrackRef::from(track));
    }
    add("TrackQuery", &TrackQuery::from(common::track_query()));
    add("Track", &Track::from(common::track()));
    add("Album", &Album::from(common::album()));
    add("Artist", &Artist::from(common::artist()));
    add(
        "SearchResults",
        &SearchResults::from(jf::SearchResults {
            tracks: vec![common::track()],
            albums: vec![common::album()],
            artists: vec![common::artist()],
        }),
    );
    add("LibraryStats", &LibraryStats::from(common::stats()));
    add("TrackRow", &TrackRow::from(common::track_row()));
    add("TrackRow", &TrackRow::from(common::source_track_row()));
    add("PlaylistSummary", &PlaylistSummary::from(common::playlist()));
    add("PlaylistEntry", &PlaylistEntry::from(common::playlist_entry()));
    add(
        "QueueItem",
        &QueueItem::from(jf::QueueItem {
            uid: 2,
            track: jf::TrackRef::Source(common::source_track()),
        }),
    );
    add(
        "HistoryEntry",
        &HistoryEntry::from(jf::HistoryEntry {
            track: jf::TrackRef::Library(42),
            played_at_ms: 1_700_000_000_000,
        }),
    );
    add(
        "DeviceInfo",
        &DeviceInfo::from(common::devices().devices[0].clone()),
    );
    add("EqPreset", &EqPreset::from(jf::eq_presets()[1].clone()));

    let mut out = String::from("{\n");
    out += "  \"about\": \"JSON contract of the jf-ffi C ABI. Each sample is one mirror type as the C ABI \
            writes it; the C# tests read each into its record and must write it back unchanged. Generated \
            by core/jf-ffi/tests/types.rs.\",\n";
    out += "  \"version\": 1,\n  \"samples\": [\n";
    for (i, (name, json)) in samples.iter().enumerate() {
        let comma = if i + 1 == samples.len() { "" } else { "," };
        out += &format!("    {{\"name\": \"{name}\", \"json\": {json}}}{comma}\n");
    }
    out += "  ]\n}\n";
    out
}

#[test]
fn json_contract_for_csharp_is_current() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/contract/ffi-types.json");
    let fresh = contract();
    // The file is valid JSON and every sample reads back into its type.
    let parsed: serde_json::Value = serde_json::from_str(&fresh).unwrap();
    assert!(parsed["samples"].as_array().is_some_and(|s| s.len() > 100));
    if std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::write(&path, &fresh).unwrap();
        return;
    }
    let stored = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (run with UPDATE_GOLDEN=1 to create it)", path.display()));
    assert!(
        stored.replace("\r\n", "\n") == fresh,
        "{} is out of date: run UPDATE_GOLDEN=1 cargo test -p jf-ffi --test types and review the diff",
        path.display()
    );
}
