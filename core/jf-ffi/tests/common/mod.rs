//! Shared test helpers: generated audio, and one value of every core type a UI sees, with every
//! optional field filled so conversions cannot skip one unnoticed.
//!
//! Maps hold at most one entry, so their JSON is the same on every run.

#![allow(dead_code)] // each test binary uses a different part

use std::collections::BTreeMap;
use std::f32::consts::TAU;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use jf_core as jf;
use jf_lyrics::{Agent, AgentKind, AuxLine, Direction, Format, SourceInfo, Timing};
use jf_session::SourceTrack;

/// Writes a stereo 16-bit sine tone.
pub fn sine_wav(path: &Path, rate: u32, freq: f32, seconds: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let frames = (rate as f32 * seconds) as u32;
    for i in 0..frames {
        let v = ((TAU * freq * i as f32 / rate as f32).sin() * 0.25 * f32::from(i16::MAX)) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();
}

/// `tests/contract/core-scenario.json`, the end-to-end scenario every binding replays.
pub fn scenario() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/contract/core-scenario.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Writes the scenario's tones into `dir/music` and returns that folder.
pub fn write_tones(dir: &Path, scenario: &serde_json::Value) -> PathBuf {
    let music = dir.join("music");
    std::fs::create_dir_all(&music).unwrap();
    let rate = scenario["sample_rate"].as_u64().unwrap() as u32;
    for tone in scenario["tones"].as_array().unwrap() {
        sine_wav(
            &music.join(tone["file"].as_str().unwrap()),
            rate,
            tone["frequency_hz"].as_f64().unwrap() as f32,
            tone["seconds"].as_f64().unwrap() as f32,
        );
    }
    music
}

/// The scenario's strings at `pointer` (a JSON pointer such as `/expect/now_playing_titles`).
pub fn strings(scenario: &serde_json::Value, pointer: &str) -> Vec<String> {
    scenario
        .pointer(pointer)
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("{pointer} is not a list"))
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect()
}

pub fn source_track() -> SourceTrack {
    SourceTrack {
        plugin: "org.jellyfesh.subsonic".into(),
        id: "tr-7".into(),
        title: "Tidal Hymn".into(),
        artist: Some("Sample Artist".into()),
        album: Some("Sample Album".into()),
        duration_ms: Some(201_000),
        artwork_url: Some("https://music.example/art/7.jpg".into()),
        isrc: Some("XX0000000007".into()),
    }
}

pub fn track_refs() -> Vec<jf::TrackRef> {
    vec![
        jf::TrackRef::Library(42),
        jf::TrackRef::File(PathBuf::from("/music/Night Swim/01 Lantern.flac")),
        jf::TrackRef::Source(source_track()),
    ]
}

pub fn collections() -> Vec<jf::Collection> {
    vec![
        jf::Collection::Album { id: 3 },
        jf::Collection::Artist { id: 4 },
        jf::Collection::Playlist { id: 5 },
        jf::Collection::Favorites,
        jf::Collection::Tracks { tracks: track_refs() },
        jf::Collection::Source {
            plugin: "org.jellyfesh.jellyfin".into(),
            id: "album-9".into(),
        },
    ]
}

pub fn eq() -> jf::EqSettings {
    jf::EqSettings {
        enabled: true,
        preamp_db: -1.5,
        bands: vec![
            jf::EqBand::low_shelf(80.0, 3.0, 0.7),
            jf::EqBand::peaking(1_000.0, -2.5, 1.41),
            jf::EqBand {
                enabled: false,
                ..jf::EqBand::high_shelf(8_000.0, 1.0, 0.7)
            },
        ],
    }
}

pub fn settings() -> jf::Settings {
    jf::Settings {
        appearance: jf::AppearancePref::Dark,
        style: jf::BackdropStyle::Misty,
        app_icon: "steel".into(),
        hue_source: jf::HueSource::Fixed { hue: 212.5 },
        reduce_effects: true,
        transparency: 0.25,
        lyrics_size: jf::LyricsSize::Large,
        lyrics_align: jf::LyricsAlign::Center,
        show_translation: false,
        show_romanization: true,
        crossfade: jf::CrossfadeSettings {
            enabled: true,
            duration_ms: 4_000,
            album_continuity: false,
        },
        replaygain_mode: jf::ReplayGainMode::Album,
        preamp_db: 2.5,
        eq: eq(),
        output_device: Some("USB DAC".into()),
        resume_on_launch: true,
        watch_folders: false,
        allow_community_plugins: true,
    }
}

/// Every command, once.
pub fn commands() -> Vec<jf::Command> {
    use jf::Command::*;
    let mut out = vec![
        PlayTracks {
            tracks: track_refs(),
            start: 1,
        },
        Enqueue {
            tracks: track_refs(),
            play_next: true,
        },
        RemoveFromQueue { uid: 7 },
        MoveInQueue { uid: 7, to: 2 },
        JumpTo { uid: 9 },
        ClearQueue,
        SetShuffle { on: true },
        SetRepeat {
            mode: jf::RepeatMode::One,
        },
        Play,
        Pause,
        TogglePlayPause,
        Stop,
        Next,
        Previous,
        Seek { position_ms: 61_500 },
        SetVolume { volume: 0.75 },
        SetEq { eq: eq() },
        SetReplayGain {
            mode: jf::ReplayGainMode::Track,
            preamp_db: -3.0,
        },
        AddRoot {
            path: "/music".into(),
        },
        RemoveRoot {
            path: "/old music".into(),
        },
        Rescan,
    ];
    out.extend(collections().into_iter().map(|collection| PlayCollection {
        collection,
        start: 2,
        shuffle: true,
    }));
    out.extend([
        EnqueueCollection {
            collection: jf::Collection::Album { id: 3 },
            play_next: false,
        },
        SetSettings { settings: settings() },
        SetMuted { muted: true },
        SetCrossfade {
            crossfade: jf::CrossfadeSettings::default(),
        },
        SetOutputDevice { name: None },
        SetOutputDevice {
            name: Some("Speakers".into()),
        },
        RefreshDevices,
        CreatePlaylist {
            name: "Evening".into(),
            tracks: track_refs(),
        },
        RenamePlaylist {
            id: 5,
            name: "Late evening".into(),
        },
        DeletePlaylist { id: 5 },
        AddToPlaylist {
            id: 5,
            tracks: vec![jf::TrackRef::Library(1)],
        },
        RemoveFromPlaylist {
            id: 5,
            entries: vec![11, 12],
        },
        MovePlaylistEntry {
            id: 5,
            entry: 11,
            to: 0,
        },
        ImportPlaylist {
            path: "/lists/mix.m3u8".into(),
        },
        SetFavorite {
            track: jf::TrackRef::Library(42),
            favorite: true,
        },
        InstallPlugin {
            path: "/downloads/subsonic.jfpkg".into(),
        },
        UninstallPlugin { id: "org.x".into() },
        SetPluginEnabled {
            id: "org.x".into(),
            enabled: false,
        },
        SetPluginSettings {
            id: "org.x".into(),
            settings_json: r#"{"server":"https://music.example"}"#.into(),
        },
        AcknowledgeUnofficial { id: "org.x".into() },
        BeginLogin {
            plugin: "org.x".into(),
            method: jf::AuthMethod::PasswordExchange,
        },
        ConnectSource {
            preset: "navidrome".into(),
            address: Some("http://192.168.1.20:4533".into()),
            method: jf::AuthMethod::PasswordExchange,
        },
        ConnectSource {
            preset: "jellyfin".into(),
            address: None,
            method: jf::AuthMethod::Qr,
        },
        SubmitLogin {
            plugin: "org.x".into(),
            flow: 3,
            fields: vec![("username".into(), "listener".into())],
        },
        CancelLogin {
            plugin: "org.x".into(),
            flow: 3,
        },
        Logout {
            plugin: "org.x".into(),
        },
        Search {
            query: "harbor".into(),
            scope: jf::SearchScope::All,
        },
        Search {
            query: "harbor".into(),
            scope: jf::SearchScope::Library,
        },
        Search {
            query: "harbor".into(),
            scope: jf::SearchScope::Plugin { id: "org.x".into() },
        },
        Browse {
            plugin: "org.x".into(),
            node: Some("artists".into()),
            page: 2,
        },
        Browse {
            plugin: "org.x".into(),
            node: None,
            page: 0,
        },
    ]);
    for method in [
        jf::AuthMethod::Qr,
        jf::AuthMethod::Oauth,
        jf::AuthMethod::CookieImport,
        jf::AuthMethod::ApiKey,
    ] {
        out.push(BeginLogin {
            plugin: "org.y".into(),
            method,
        });
    }
    out.push(SetSettings {
        settings: jf::Settings {
            hue_source: jf::HueSource::Artwork,
            appearance: jf::AppearancePref::Light,
            style: jf::BackdropStyle::Artwork,
            replaygain_mode: jf::ReplayGainMode::Off,
            ..jf::Settings::default()
        },
    });
    out.push(SetRepeat {
        mode: jf::RepeatMode::All,
    });
    out.push(SetRepeat {
        mode: jf::RepeatMode::Off,
    });
    for action in [
        jf::RemoteAction::Refresh,
        jf::RemoteAction::Transfer {
            device: "phone".into(),
        },
        jf::RemoteAction::SetVolume { percent: 40 },
        jf::RemoteAction::End,
    ] {
        out.push(Remote { action });
    }
    out
}

pub fn track() -> jf::Track {
    jf::Track {
        id: 42,
        path: "/music/Night Swim/01 Lantern.flac".into(),
        title: "Lantern".into(),
        artist: Some("Sample Artist".into()),
        album: Some("Night Swim".into()),
        album_artist: Some("Sample Artist".into()),
        album_id: Some(3),
        track_no: Some(1),
        disc_no: Some(1),
        year: Some(2024),
        genre: Some("Ambient".into()),
        duration_ms: Some(184_000),
        sample_rate: Some(44_100),
        channels: Some(2),
        bit_depth: Some(16),
        codec: Some("flac".into()),
        replaygain: jf_library::ReplayGain {
            track_gain_db: Some(-6.5),
            track_peak: Some(0.9),
            album_gain_db: Some(-7.0),
            album_peak: Some(0.95),
        },
        isrc: Some("XX0000000042".into()),
        has_embedded_lyrics: true,
        artwork_hash: Some("ab12cd".into()),
        file_size: 12_345_678,
        mtime_ns: 1_700_000_000_000_000_000,
    }
}

pub fn album() -> jf::Album {
    jf::Album {
        id: 3,
        title: "Night Swim".into(),
        album_artist: Some("Sample Artist".into()),
        year: Some(2024),
        track_count: 9,
        artwork_hash: Some("ab12cd".into()),
    }
}

pub fn artist() -> jf::Artist {
    jf::Artist {
        id: 4,
        name: "Sample Artist".into(),
        track_count: 9,
    }
}

pub fn artwork() -> Vec<jf::ArtworkRef> {
    vec![
        jf::ArtworkRef::File("/cache/artwork/ab12cd.jpg".into()),
        jf::ArtworkRef::Url("https://music.example/art/7.jpg".into()),
    ]
}

pub fn track_row() -> jf::TrackRow {
    jf::TrackRow {
        track: jf::TrackRef::Library(42),
        title: "Lantern".into(),
        artist: Some("Sample Artist".into()),
        album: Some("Night Swim".into()),
        duration_ms: Some(184_000),
        // Library rows carry a hash; the URL is for source rows (both set to cover both).
        artwork: Some(artwork()[1].clone()),
        artwork_hash: Some("ab12cd".into()),
        favorite: true,
        missing: false,
    }
}

pub fn source_item() -> jf::SourceItem {
    jf::SourceItem {
        source: "library".into(),
        kind: jf::ItemKind::Track,
        id: "42".into(),
        title: "Lantern".into(),
        subtitle: Some("Sample Artist".into()),
        album: Some("Night Swim".into()),
        duration_ms: Some(184_000),
        artwork: Some(artwork()[1].clone()),
        playable: false,
        unplayable_reason: Some("not in this region".into()),
        track: Some(jf::TrackRef::Library(42)),
        also_in: vec!["org.x".into()],
        link: Some("https://music.example.com/song/42".into()),
    }
}

pub fn lyric_doc() -> jf::LyricDoc {
    let word = |start_ms, end_ms, text: &str| jf::Word {
        start_ms,
        end_ms,
        text: text.into(),
        joins_next: false,
    };
    jf::LyricDoc {
        language: Some("en".into()),
        timing: Timing::Word,
        offset_ms: -120,
        agents: vec![Agent {
            id: "v1".into(),
            name: Some("Lead".into()),
            kind: AgentKind::Person,
        }],
        lines: vec![
            jf::Line {
                id: 0,
                start_ms: Some(1_000),
                end_ms: Some(2_000),
                agent: Some("v1".into()),
                text: "First test line".into(),
                words: vec![word(1_000, 1_400, "First"), word(1_500, 2_000, "line")],
                background: vec![word(1_200, 1_300, "(oh)")],
                direction: Direction::Ltr,
            },
            jf::Line {
                id: 1,
                start_ms: Some(3_000),
                end_ms: None,
                agent: None,
                text: String::new(),
                words: vec![],
                background: vec![],
                direction: Direction::Auto,
            },
        ],
        tracks: vec![jf::AuxTrack {
            kind: jf::AuxKind::Translation,
            language: Some("de".into()),
            lines: BTreeMap::from([(
                0,
                AuxLine {
                    text: "Erste Testzeile".into(),
                    words: vec![],
                },
            )]),
        }],
        source: SourceInfo {
            format: Format::EnhancedLrc,
            origin: Some("/music/Night Swim/01 Lantern.lrc".into()),
        },
        metadata: BTreeMap::from([("ti".to_owned(), "Lantern".to_owned())]),
    }
}

pub fn theme(appearance: jf::Appearance) -> jf::Theme {
    jf::Theme {
        hue: 212.5,
        appearance,
        colors: BTreeMap::from([
            ("accent.accent".to_owned(), [0x00, 0x59, 0xa7, 0xff]),
            ("ink.ink-90".to_owned(), [0x10, 0x18, 0x20, 0xeb]),
        ]),
    }
}

pub fn anchor() -> jf::PlaybackAnchor {
    jf::PlaybackAnchor {
        track_generation: 3,
        position_us: 12_345_678,
        anchor_mono_ns: 987_654_321_012,
        rate: 1.0,
        duration_us: Some(184_000_000),
    }
}

pub fn playback() -> jf::PlaybackSnapshot {
    jf::PlaybackSnapshot {
        version: 12,
        status: jf::Status::Playing,
        anchor: anchor(),
        volume: 0.5,
        eq: eq(),
        replaygain_mode: jf::ReplayGainMode::Track,
        preamp_db: 1.5,
    }
}

pub fn queue() -> jf::QueueSnapshot {
    let item = |uid, track| jf::QueueItem { uid, track };
    jf::QueueSnapshot {
        version: 5,
        len: 3,
        position: Some(1),
        current: Some(item(2, jf::TrackRef::Library(42))),
        shuffle: true,
        repeat: jf::RepeatMode::All,
        up_next: vec![item(3, jf::TrackRef::Source(source_track()))],
    }
}

pub fn now_playing() -> jf::NowPlaying {
    jf::NowPlaying {
        uid: 2,
        track: jf::TrackRef::Library(42),
        path: Some("/music/Night Swim/01 Lantern.flac".into()),
        title: "Lantern".into(),
        artist: Some("Sample Artist".into()),
        album: Some("Night Swim".into()),
        duration_ms: Some(184_000),
        artwork: Some(artwork()[0].clone()),
        format: Some(jf::TrackInfo {
            codec: "flac".into(),
            sample_rate: 44_100,
            channels: 2,
            duration_us: Some(184_000_000),
            gapless: true,
            bitrate: Some(912_000),
        }),
        link: None,
    }
}

/// A remote session playing on the second of two invented devices.
pub fn remote() -> jf::RemoteSnapshot {
    let desk = jf::RemoteDevice {
        id: "desk".into(),
        name: "Desk Speaker".into(),
        kind: jf::RemoteDeviceKind::Speaker,
        active: false,
        volume_percent: None,
    };
    let phone = jf::RemoteDevice {
        id: "phone".into(),
        name: "Phone".into(),
        kind: jf::RemoteDeviceKind::Smartphone,
        active: true,
        volume_percent: Some(70),
    };
    jf::RemoteSnapshot {
        version: 4,
        source: Some("org.x".into()),
        source_name: Some("Example Source".into()),
        status: jf::Status::Playing,
        item: Some(jf::SourceItem {
            source: "org.x".into(),
            id: "t9".into(),
            ..source_item()
        }),
        context: Some(jf::SourceItem {
            source: "org.x".into(),
            kind: jf::ItemKind::Album,
            id: "al3".into(),
            track: None,
            ..source_item()
        }),
        anchor: anchor(),
        shuffle: true,
        repeat: jf::RepeatMode::One,
        device: Some(phone.clone()),
        devices: vec![desk, phone],
        error: Some("the service asked to slow down".into()),
    }
}

pub fn lyrics() -> jf::LyricsSnapshot {
    let doc = lyric_doc();
    jf::LyricsSnapshot {
        uid: 2,
        timeline: Some(doc.timeline()),
        doc: Some(doc),
        source: Some(jf::LyricsSource::Sidecar(
            "/music/Night Swim/01 Lantern.lrc".into(),
        )),
    }
}

pub fn theme_snapshot() -> jf::ThemeSnapshot {
    jf::ThemeSnapshot {
        uid: Some(2),
        hue: 212.5,
        from_artwork: true,
        light: theme(jf::Appearance::Light),
        dark: theme(jf::Appearance::Dark),
        backdrop_light: Some("/cache/backdrop/ab12cd-light.png".into()),
        backdrop_dark: Some("/cache/backdrop/ab12cd-dark.png".into()),
    }
}

pub fn devices() -> jf::DevicesSnapshot {
    jf::DevicesSnapshot {
        version: 2,
        devices: vec![jf::DeviceInfo {
            id: "alsa:hw:CARD=DAC".into(),
            name: "USB DAC".into(),
            is_default: false,
            sample_rates: vec![44_100, 48_000, 96_000],
        }],
        selected: Some("USB DAC".into()),
    }
}

pub fn sources() -> jf::SourcesSnapshot {
    jf::SourcesSnapshot {
        version: 1,
        plugins: vec![jf::PluginSummary {
            id: "org.x".into(),
            name: "Example Source".into(),
            version: "1.2.0".into(),
            author: "Example".into(),
            description: Some("Music from an example service".into()),
            badge: jf::PluginBadge::Community,
            signing_key: Some("0123456789abcdef".into()),
            vendor_playback: false,
            remote_playback: false,
            capabilities: vec!["search".into(), "resolve_stream".into()],
            auth_methods: vec![jf::AuthMethod::Oauth, jf::AuthMethod::ApiKey],
            enabled: false,
            disabled_reason: Some("Community plugins are turned off".into()),
            account: Some("listener".into()),
            needs_disclosure: true,
            attribution: Some("example".into()),
            settings_schema_json: r#"{"type":"object"}"#.into(),
            settings_json: "{}".into(),
            icon: Some("/cache/plugin-icons/0123456789abcdef.png".into()),
        }],
        presets: vec![jf::SourcePreset {
            id: "navidrome".into(),
            name: "Navidrome".into(),
            plugin: "org.jellyfesh.subsonic".into(),
            auth_methods: vec![jf::AuthMethod::PasswordExchange, jf::AuthMethod::ApiKey],
            needs_address: true,
            address: Some("https://music.example.com".into()),
            address_example: Some("http://192.168.1.20:4533".into()),
            account: None,
        }],
        allow_community: true,
        secret_storage: jf::SecretStorage::SessionOnly,
        unavailable: None,
    }
}

pub fn snapshot() -> jf::Snapshot {
    jf::Snapshot {
        playback: playback(),
        queue: queue(),
        now_playing: Some(Arc::new(now_playing())),
        lyrics: Some(Arc::new(lyrics())),
        theme: Some(Arc::new(theme_snapshot())),
        library_revision: 17,
        scanning: true,
        settings: Arc::new(settings()),
        devices: Arc::new(devices()),
        sources: Arc::new(sources()),
        remote: Arc::new(remote()),
        muted: true,
    }
}

pub fn scan_report() -> jf::ScanReport {
    jf::ScanReport {
        added: 9,
        updated: 1,
        removed: 2,
        unchanged: 30,
        errors: vec![("/music/broken.flac".into(), "not audio".into())],
    }
}

/// Every event variant, with every auth step and scan status.
pub fn events() -> Vec<jf::Event> {
    use jf::Event::*;
    let update = |step| {
        Auth(jf::AuthUpdate {
            plugin: "org.x".into(),
            flow: 3,
            step,
        })
    };
    vec![
        Playback(playback()),
        Queue(queue()),
        NowPlaying(Some(Arc::new(now_playing()))),
        NowPlaying(None),
        Lyrics(Arc::new(lyrics())),
        Lyrics(Arc::new(jf::LyricsSnapshot {
            source: Some(jf::LyricsSource::Plugin("org.x".into())),
            ..lyrics()
        })),
        Theme(Arc::new(theme_snapshot())),
        Scan(jf::ScanStatus::Progress(jf::ScanProgress {
            files_seen: 10,
            files_changed: 4,
            current: "/music/Night Swim/02 Harbor.flac".into(),
        })),
        Scan(jf::ScanStatus::Finished(scan_report())),
        Scan(jf::ScanStatus::Failed("the folder is gone".into())),
        LibraryChanged { revision: 18 },
        PlaylistCreated {
            request: jf::CommandId(20),
            id: 5,
        },
        Settings(Arc::new(settings())),
        Devices(Arc::new(devices())),
        Sources(Arc::new(sources())),
        update(jf::AuthStep::ShowQr {
            data: "otpauth://x".into(),
            hint: Some("Scan with the Example app".into()),
            expires_at_ms: Some(1_700_000_000_000),
        }),
        update(jf::AuthStep::ShowCode {
            code: "482 913".into(),
            hint: "In Example, open Quick Connect".into(),
            expires_at_ms: None,
        }),
        update(jf::AuthStep::OpenUrl {
            url: "https://music.example/login".into(),
        }),
        update(jf::AuthStep::AskFields {
            fields: vec![
                jf::AuthField {
                    name: "username".into(),
                    label: "User name".into(),
                    kind: jf::AuthFieldKind::Email,
                    secret: false,
                    required: true,
                    placeholder: None,
                    help: Some("The e-mail address you signed up with".into()),
                },
                jf::AuthField {
                    name: "password".into(),
                    label: "Password".into(),
                    kind: jf::AuthFieldKind::Text,
                    secret: true,
                    required: true,
                    placeholder: Some("••••".into()),
                    help: None,
                },
            ],
        }),
        update(jf::AuthStep::Waiting {
            message: "Waiting for the browser".into(),
        }),
        update(jf::AuthStep::Done {
            account: "listener".into(),
        }),
        update(jf::AuthStep::Failed {
            message: "wrong password".into(),
        }),
        SearchResults(Arc::new(jf::SearchUpdate {
            request: jf::CommandId(21),
            query: "harbor".into(),
            source: "library".into(),
            items: vec![source_item()],
            error: Some("one source timed out".into()),
            done: true,
        })),
        BrowsePage(Arc::new(jf::BrowseUpdate {
            request: jf::CommandId(22),
            plugin: "org.x".into(),
            node: Some("artists".into()),
            items: vec![jf::SourceItem {
                kind: jf::ItemKind::Folder,
                ..source_item()
            }],
            next_page: Some(3),
            error: None,
        })),
        Remote(Arc::new(remote())),
        Remote(Arc::new(jf::RemoteSnapshot {
            source: None,
            source_name: None,
            status: jf::Status::Stopped,
            item: None,
            context: None,
            device: None,
            devices: Vec::new(),
            error: None,
            ..remote()
        })),
        Buffering {
            uid: 2,
            buffering: true,
        },
        CommandFailed {
            id: jf::CommandId(23),
            error: "nothing to play".into(),
        },
        Error {
            message: "cannot read /music/broken.flac".into(),
        },
    ]
}

pub fn playlist() -> jf::PlaylistSummary {
    jf::PlaylistSummary {
        id: 5,
        name: "Evening".into(),
        entry_count: 12,
        duration_ms: 2_400_000,
        updated_at_ms: 1_700_000_000_000,
        artwork_hash: Some("ab12cd".into()),
    }
}

pub fn playlist_entry() -> jf::PlaylistEntry {
    jf::PlaylistEntry {
        entry_id: 11,
        position: 0,
        row: track_row(),
    }
}

pub fn stats() -> jf::LibraryStats {
    jf::LibraryStats {
        roots: 1,
        tracks: 42,
        albums: 3,
        artists: 2,
        artwork: 3,
        total_duration_ms: 7_200_000,
        total_bytes: 512_000_000,
    }
}

pub fn track_query() -> jf::TrackQuery {
    jf::TrackQuery {
        offset: 10,
        limit: 20,
        sort: jf::TrackSort::Album,
        descending: true,
        album_id: Some(3),
        artist_id: Some(4),
    }
}

pub fn config() -> jf::Config {
    jf::Config {
        output: jf::OutputKind::Null { realtime: false },
        buffer_ms: 150,
        restore_session: false,
        persist_session: true,
        watch_folders: false,
        bundled_plugins: vec!["/app/plugins/subsonic.jfpkg".into()],
        ..jf::Config::new("/data/jellyfesh", "/cache/jellyfesh")
    }
}
