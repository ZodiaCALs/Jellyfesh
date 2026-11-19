//! Artwork cache, ReplayGain and lyrics.

use std::fs;
use std::path::Path;

use jf_library::ReplayGain;
use lofty::TextEncoding;
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::AudioFile;
use lofty::id3::v2::{
    BinaryFrame, Frame, FrameId, Id3v2Tag, SyncTextContentType, SynchronizedTextFrame, TimestampFormat,
};
use lofty::iff::wav::WavFile;
use lofty::tag::ItemKey;

use crate::fixtures::{self, Fixture, standard};

#[test]
fn embedded_front_covers_are_cached_once_by_hash() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();

    let expected = blake3::hash(&standard::cover()).to_hex().to_string();
    let a = fx.track_at(standard::COVER_A);
    let b = fx.track_at(standard::COVER_B);
    assert_eq!(a.artwork_hash.as_deref(), Some(expected.as_str()));
    assert_eq!(b.artwork_hash, a.artwork_hash, "same image, same hash");

    let cached: Vec<_> = fs::read_dir(fx.artwork_dir()).unwrap().collect();
    assert_eq!(cached.len(), 1, "written once");
    assert_eq!(fx.lib.stats().unwrap().artwork, 1);

    let path = fx.lib.artwork_path(a.id).unwrap().unwrap();
    assert_eq!(path, fx.artwork_dir().join(format!("{expected}.png")));
    assert_eq!(fs::read(&path).unwrap(), standard::cover());
    assert_eq!(fx.lib.artwork_path(b.id).unwrap(), Some(path));

    let album = fx.lib.album(a.album_id.unwrap()).unwrap().unwrap();
    assert_eq!(album.artwork_hash.as_deref(), Some(expected.as_str()));

    let plain = fx.track_at(standard::UNTAGGED);
    assert_eq!(plain.artwork_hash, None);
    assert_eq!(fx.lib.artwork_path(plain.id).unwrap(), None);
    assert_eq!(fx.lib.artwork_path(-1).unwrap(), None);
}

#[test]
fn a_purged_cache_file_is_restored_and_unused_artwork_is_deleted() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let a = fx.track_at(standard::COVER_A);
    let path = fx.lib.artwork_path(a.id).unwrap().unwrap();

    fs::remove_file(&path).unwrap();
    assert_eq!(fx.lib.artwork_path(a.id).unwrap(), Some(path.clone()));
    assert_eq!(fs::read(&path).unwrap(), standard::cover());

    fs::remove_file(fx.path(standard::COVER_A)).unwrap();
    fs::remove_file(fx.path(standard::COVER_B)).unwrap();
    assert_eq!(fx.scan().removed, 2);
    assert_eq!(fx.lib.stats().unwrap().artwork, 0);
    assert!(!path.exists(), "the orphaned image is deleted from the cache");
}

#[test]
fn replaygain_is_read_from_id3_txxx_and_vorbis_comments() {
    let fx = Fixture::new();
    standard::build(&fx);
    let vorbis = fx.path("rg/vorbis.flac");
    fixtures::flac(&vorbis, 100);
    fixtures::tag(&vorbis, |t| {
        fixtures::set_text(t, ItemKey::ReplayGainTrackGain, "-7.03 dB");
        fixtures::set_text(t, ItemKey::ReplayGainTrackPeak, "0.5");
        fixtures::set_text(t, ItemKey::ReplayGainAlbumGain, "garbage");
    });
    fx.scan();

    assert_eq!(
        fx.track_at(standard::RG).replaygain,
        ReplayGain {
            track_gain_db: Some(-6.2),
            track_peak: Some(0.988_525),
            album_gain_db: Some(1.5),
            album_peak: Some(1.0),
        }
    );
    assert_eq!(
        fx.track_at("rg/vorbis.flac").replaygain,
        ReplayGain {
            track_gain_db: Some(-7.03),
            track_peak: Some(0.5),
            album_gain_db: None,
            album_peak: None,
        }
    );
    assert_eq!(fx.track_at(standard::CJK).replaygain, ReplayGain::default());
}

/// Adds an ID3v2 SYLT frame (millisecond stamps) to a WAV file.
fn add_sylt(path: &Path, content: Vec<(u32, String)>) {
    let mut wav = WavFile::read_from(&mut fs::File::open(path).unwrap(), ParseOptions::new()).unwrap();
    if wav.id3v2().is_none() {
        wav.set_id3v2(Id3v2Tag::new());
    }
    let sylt = SynchronizedTextFrame::new(
        TextEncoding::UTF8,
        *b"eng",
        TimestampFormat::MS,
        SyncTextContentType::Lyrics,
        None,
        content,
    );
    let bytes = sylt.as_bytes(WriteOptions::default()).unwrap();
    let frame = Frame::Binary(BinaryFrame::new(FrameId::new("SYLT").unwrap(), bytes));
    wav.id3v2_mut().unwrap().insert(frame);
    wav.save_to_path(path, WriteOptions::default()).unwrap();
}

#[test]
fn embedded_lyrics_come_from_uslt_vorbis_and_sylt() {
    let fx = Fixture::new();
    let lrc = "[00:01.00]Hello\n[00:02.00]World";
    let uslt = fx.path("lyrics/uslt.wav");
    fixtures::wav(&uslt, 100);
    fixtures::tag(&uslt, |t| fixtures::set_text(t, ItemKey::UnsyncLyrics, lrc));

    let vorbis = fx.path("lyrics/vorbis.flac");
    fixtures::flac(&vorbis, 100);
    fixtures::tag(&vorbis, |t| {
        fixtures::set_text(t, ItemKey::Lyrics, "plain words\nno timing")
    });

    let sylt = fx.path("lyrics/sylt.wav");
    fixtures::wav(&sylt, 100);
    fixtures::tag(&sylt, |t| fixtures::basic(t, "Synced", "Singer", "Album", 1));
    add_sylt(
        &sylt,
        vec![(1000, "Hello".to_owned()), (62_500, "World".to_owned())],
    );

    let none = fx.path("lyrics/none.wav");
    fixtures::wav(&none, 100);
    fx.scan();

    let lyrics_of = |relative: &str| {
        let track = fx.track_at(relative);
        (
            track.has_embedded_lyrics,
            fx.lib.embedded_lyrics(track.id).unwrap(),
        )
    };
    assert_eq!(lyrics_of("lyrics/uslt.wav"), (true, Some(lrc.to_owned())));
    assert_eq!(
        lyrics_of("lyrics/vorbis.flac"),
        (true, Some("plain words\nno timing".to_owned()))
    );
    assert_eq!(
        lyrics_of("lyrics/sylt.wav"),
        (true, Some("[00:01.00]Hello\n[01:02.50]World\n".to_owned()))
    );
    assert_eq!(
        fx.track_at("lyrics/sylt.wav").title,
        "Synced",
        "other tags survive"
    );
    assert_eq!(lyrics_of("lyrics/none.wav"), (false, None));
    assert_eq!(fx.lib.embedded_lyrics(-1).unwrap(), None);
}

#[test]
fn sidecar_lyrics_are_found_next_to_the_file() {
    let fx = Fixture::new();
    standard::build(&fx);
    fx.scan();
    let cjk = fx.track_at(standard::CJK);
    assert_eq!(fx.lib.sidecar_lyrics(cjk.id).unwrap(), None);

    let lrc_path = fx.path("cjk/qingtian.lrc");
    fs::write(&lrc_path, "\u{feff}[00:01.00]刮风这天").unwrap();
    assert_eq!(
        fx.lib.sidecar_lyrics(cjk.id).unwrap(),
        Some((lrc_path, "[00:01.00]刮风这天".to_owned()))
    );

    // TTML wins over LRC; UTF-16 with a BOM is decoded.
    let ttml_path = fx.path("cjk/qingtian.ttml");
    let ttml = "<tt><body><p begin=\"1s\">晴天</p></body></tt>";
    let mut utf16 = vec![0xFF, 0xFE];
    utf16.extend(ttml.encode_utf16().flat_map(u16::to_le_bytes));
    fs::write(&ttml_path, utf16).unwrap();
    assert_eq!(
        fx.lib.sidecar_lyrics(cjk.id).unwrap(),
        Some((ttml_path, ttml.to_owned()))
    );

    // Only the same stem counts.
    let rg = fx.track_at(standard::RG);
    fs::write(fx.path("wav/other.vtt"), "WEBVTT").unwrap();
    assert_eq!(fx.lib.sidecar_lyrics(rg.id).unwrap(), None);
    fs::write(fx.path("wav/replaygain.vtt"), "WEBVTT").unwrap();
    assert_eq!(
        fx.lib.sidecar_lyrics(rg.id).unwrap(),
        Some((fx.path("wav/replaygain.vtt"), "WEBVTT".to_owned()))
    );
    assert_eq!(fx.lib.sidecar_lyrics(-1).unwrap(), None);
}

#[test]
fn file_tags_are_read_without_a_catalogue() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loose.flac");
    fixtures::flac(&path, 300);
    fixtures::tag(&path, |tag| {
        fixtures::basic(tag, "Loose Leaf", "Test Artist", "Test Album", 1);
        tag.push_picture(fixtures::front_cover(fixtures::png(4, 4, [10, 120, 200])));
        fixtures::set_text(tag, ItemKey::Lyrics, "[00:00.10]An invented line");
        fixtures::set_text(tag, ItemKey::ReplayGainTrackGain, "-6.50 dB");
    });
    let tags = jf_library::read_file_tags(&path, true).unwrap();
    assert_eq!(tags.title.as_deref(), Some("Loose Leaf"));
    assert_eq!(tags.artist.as_deref(), Some("Test Artist"));
    assert_eq!(tags.album.as_deref(), Some("Test Album"));
    assert_eq!(tags.replaygain.track_gain_db, Some(-6.5));
    assert!(tags.duration_ms.is_some_and(|d| (250..=350).contains(&d)));
    assert!(
        tags.lyrics
            .as_deref()
            .is_some_and(|l| l.contains("An invented line"))
    );
    assert!(tags.cover.as_ref().is_some_and(|c| c.starts_with(b"\x89PNG")));

    let without = jf_library::read_file_tags(&path, false).unwrap();
    assert!(without.cover.is_none());
    assert!(jf_library::read_file_tags(&dir.path().join("missing.flac"), false).is_err());
}

#[test]
fn file_details_read_credits_numbers_and_the_format() {
    let fx = Fixture::new();
    let path = fx.path("details/invented.flac");
    fixtures::flac(&path, 1_500);
    fixtures::tag(&path, |tag| {
        fixtures::basic(tag, "Invented Song", "Test Artist", "Test Album", 3);
        fixtures::set_text(tag, ItemKey::TrackTotal, "12");
        fixtures::set_text(tag, ItemKey::DiscNumber, "1");
        fixtures::set_text(tag, ItemKey::DiscTotal, "2");
        fixtures::set_text(tag, ItemKey::TrackSubtitle, "Live");
        fixtures::set_text(tag, ItemKey::Composer, "A. Composer");
        fixtures::set_text(tag, ItemKey::Lyricist, "A. Lyricist");
        fixtures::set_text(tag, ItemKey::Label, "Test Label");
        fixtures::set_text(tag, ItemKey::ReleaseDate, "2011-05-03");
        fixtures::set_text(tag, ItemKey::Isrc, "XX0000000001");
    });
    let details = jf_library::read_file_details(&path).unwrap();
    assert_eq!(details.title.as_deref(), Some("Invented Song"));
    assert_eq!(details.artist.as_deref(), Some("Test Artist"));
    assert_eq!(details.subtitle.as_deref(), Some("Live"));
    assert_eq!(details.composer.as_deref(), Some("A. Composer"));
    assert_eq!(details.lyricist.as_deref(), Some("A. Lyricist"));
    assert_eq!(details.label.as_deref(), Some("Test Label"));
    assert_eq!(details.released.as_deref().map(|d| &d[..4]), Some("2011"));
    assert_eq!(details.isrc.as_deref(), Some("XX0000000001"));
    assert_eq!((details.track_no, details.track_total), (Some(3), Some(12)));
    assert_eq!((details.disc_no, details.disc_total), (Some(1), Some(2)));
    assert_eq!(details.codec.as_deref(), Some("flac"));
    assert_eq!(details.container.as_deref(), Some("flac"));
    assert_eq!(details.sample_rate, Some(fixtures::RATE));
    assert_eq!(details.bit_depth, Some(16));
    assert_eq!(details.channels, Some(1));
    assert!(details.duration_ms.is_some_and(|d| d.abs_diff(1_500) <= 5));
    assert_eq!(details.file_size, Some(fs::metadata(&path).unwrap().len()));

    // ID3v2 in a WAV: "3/12" in the track number.
    let wav = fx.path("details/invented.wav");
    fixtures::wav(&wav, 500);
    fixtures::tag(&wav, |tag| {
        fixtures::basic(tag, "Invented Tone", "Test Artist", "Test Album", 3);
        fixtures::set_text(tag, ItemKey::TrackTotal, "12");
    });
    let details = jf_library::read_file_details(&wav).unwrap();
    assert_eq!((details.track_no, details.track_total), (Some(3), Some(12)));
    assert_eq!(details.codec.as_deref(), Some("pcm"));
    assert_eq!(details.container.as_deref(), Some("wav"));
    assert!(jf_library::read_file_details(&fx.path("details/missing.flac")).is_err());
}

#[test]
fn thumbnails_of_any_picture() {
    let fx = Fixture::new();
    let target = fx.path("thumbs/one.jpg");
    jf_library::make_thumbnail(&fixtures::png(300, 150, [200, 30, 30]), 100, &target).unwrap();
    assert_eq!(jf_library::thumbnail_size(100), 128);
    let jpeg = fs::read(&target).unwrap();
    assert_eq!(&jpeg[..2], &[0xff, 0xd8], "a JPEG");
    assert!(jf_library::make_thumbnail(b"not a picture", 64, &fx.path("thumbs/bad.jpg")).is_err());
}
