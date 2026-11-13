//! Test fixtures generated on the fly: sine-wave WAV (hound) and FLAC (flacenc)
//! files tagged with lofty, and a tiny PNG. No real recordings.

use std::f32::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use flacenc::component::BitRepr;
use flacenc::error::Verify;
use jf_library::{Library, ScanReport, Track, TrackQuery, TrackSort};
use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{Accessor, ItemKey, Tag, TagExt};

pub const RATE: u32 = 8000;

fn sine(millis: u32) -> impl Iterator<Item = i32> {
    let count = RATE * millis / 1000;
    (0..count).map(|i| ((2.0 * PI * 440.0 * i as f32 / RATE as f32).sin() * 8000.0) as i32)
}

/// A mono 16-bit sine WAV.
pub fn wav(path: &Path, millis: u32) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for sample in sine(millis) {
        writer.write_sample(sample as i16).unwrap();
    }
    writer.finalize().unwrap();
}

/// A mono 16-bit sine FLAC.
pub fn flac(path: &Path, millis: u32) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let samples: Vec<i32> = sine(millis).collect();
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .expect("flac config");
    let source = flacenc::source::MemSource::from_samples(&samples, 1, 16, RATE as usize);
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size).expect("encode");
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).expect("write flac");
    fs::write(path, sink.as_slice()).unwrap();
}

/// Edits the file's primary tag (ID3v2 for WAV, Vorbis comments for FLAC).
pub fn tag(path: &Path, edit: impl FnOnce(&mut Tag)) {
    let mut file = lofty::read_from_path(path).unwrap();
    let tag_type = file.primary_tag_type();
    if file.tag(tag_type).is_none() {
        file.insert_tag(Tag::new(tag_type));
    }
    let tag = file.tag_mut(tag_type).unwrap();
    edit(tag);
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

/// Sets the common fields.
pub fn basic(tag: &mut Tag, title: &str, artist: &str, album: &str, track: u32) {
    tag.set_title(title.to_owned());
    tag.set_artist(artist.to_owned());
    tag.set_album(album.to_owned());
    tag.set_track(track);
}

pub fn front_cover(png: Vec<u8>) -> Picture {
    Picture::unchecked(png)
        .pic_type(PictureType::CoverFront)
        .mime_type(MimeType::Png)
        .build()
}

pub fn set_text(tag: &mut Tag, key: ItemKey, value: &str) {
    assert!(
        tag.insert_text(key, value.to_owned()),
        "{key:?} not supported by {:?}",
        tag.tag_type()
    );
}

/// A valid RGB PNG of one colour (stored deflate blocks, so no compressor needed).
pub fn png(width: u32, height: u32, rgb: [u8; 3]) -> Vec<u8> {
    // Each scanline: filter byte 0 (none), then the pixels.
    let row: Vec<u8> = std::iter::once(0).chain(rgb.repeat(width as usize)).collect();
    let raw = row.repeat(height as usize);
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (index, block) in blocks.iter().enumerate() {
        zlib.push(u8::from(index + 1 == blocks.len()));
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit RGB, no interlace
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    png_chunk(&mut out, b"IHDR", &ihdr);
    png_chunk(&mut out, b"IDAT", &zlib);
    png_chunk(&mut out, b"IEND", &[]);
    out
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

/// Moves a file's mtime without changing its contents.
pub fn touch(path: &Path, seconds_later: u64) {
    let file = fs::File::options().write(true).open(path).unwrap();
    let mtime = file.metadata().unwrap().modified().unwrap() + Duration::from_secs(seconds_later);
    file.set_modified(mtime).unwrap();
}

/// A temporary library with a `music` root.
pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub music: PathBuf,
    pub lib: Library,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let music = dir.path().join("music");
        fs::create_dir_all(&music).unwrap();
        let lib = Library::open(
            &dir.path().join("db").join("library.db"),
            &dir.path().join("cache"),
        )
        .unwrap();
        lib.add_root(&music).unwrap();
        Self { dir, music, lib }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.music.join(relative)
    }

    pub fn scan(&self) -> ScanReport {
        self.lib.scan(&mut |_| {}).unwrap()
    }

    pub fn artwork_dir(&self) -> PathBuf {
        self.dir.path().join("cache").join("artwork")
    }

    pub fn all_tracks(&self) -> Vec<Track> {
        self.lib
            .tracks(&TrackQuery {
                limit: 1000,
                sort: TrackSort::Path,
                ..TrackQuery::default()
            })
            .unwrap()
    }

    /// Runs FTS5's own integrity check on every search index: the external-content
    /// indexes must match their tables after any sequence of changes.
    pub fn assert_search_index_consistent(&self) {
        let conn = rusqlite::Connection::open(self.dir.path().join("db").join("library.db")).unwrap();
        for fts in [
            "tracks_fts",
            "tracks_tri",
            "albums_fts",
            "albums_tri",
            "artists_fts",
            "artists_tri",
        ] {
            conn.execute(
                &format!("INSERT INTO {fts} ({fts}, rank) VALUES ('integrity-check', 1)"),
                [],
            )
            .unwrap_or_else(|err| panic!("{fts}: {err}"));
        }
    }

    pub fn track_at(&self, relative: &str) -> Track {
        let path = self.path(relative);
        self.all_tracks()
            .into_iter()
            .find(|t| t.path == path)
            .unwrap_or_else(|| panic!("no track {relative}"))
    }
}

/// The standard library used by most tests.
pub mod standard {
    use super::*;

    pub const COVER_A: &str = "Artist A/Album One/01 first.flac";
    pub const COVER_B: &str = "Artist A/Album One/02 second.flac";
    pub const RG: &str = "wav/replaygain.wav";
    pub const CJK: &str = "cjk/qingtian.wav";
    pub const ARABIC: &str = "arabic/salam.flac";
    pub const UNTAGGED: &str = "untagged/plain_song.wav";
    pub const BROKEN: &str = "broken/not-really.mp3";
    pub const AUDIO_FILES: u64 = 7;
    pub const GOOD_FILES: u64 = 6;

    pub fn cover() -> Vec<u8> {
        png(4, 4, [200, 40, 90])
    }

    pub fn build(fx: &Fixture) {
        for (path, title, track) in [(COVER_A, "Moonlight Sonata", 1), (COVER_B, "Blue Moon Café", 2)] {
            let path = fx.path(path);
            flac(&path, 300);
            tag(&path, |t| {
                basic(t, title, "Ludwig", "Album One", track);
                set_text(t, ItemKey::AlbumArtist, "Ludwig");
                set_text(t, ItemKey::RecordingDate, "2001-05-06");
                set_text(t, ItemKey::Genre, "Classical");
                t.push_picture(front_cover(cover()));
            });
        }

        let rg = fx.path(RG);
        wav(&rg, 250);
        tag(&rg, |t| {
            basic(t, "Gain Check", "Alpha feat. Beta", "Loud Album", 3);
            set_text(t, ItemKey::ReplayGainTrackGain, "-6.20 dB");
            set_text(t, ItemKey::ReplayGainTrackPeak, "0.988525");
            set_text(t, ItemKey::ReplayGainAlbumGain, "+1,50 dB");
            set_text(t, ItemKey::ReplayGainAlbumPeak, "1.000000");
            set_text(t, ItemKey::Isrc, "USRC17607839");
        });

        let cjk = fx.path(CJK);
        wav(&cjk, 250);
        tag(&cjk, |t| basic(t, "晴天", "周杰伦", "叶惠美", 3));

        let arabic = fx.path(ARABIC);
        flac(&arabic, 250);
        tag(&arabic, |t| basic(t, "سلام عليكم", "فيروز", "ألبوم", 1));

        wav(&fx.path(UNTAGGED), 200);

        let broken = fx.path(BROKEN);
        fs::create_dir_all(broken.parent().unwrap()).unwrap();
        fs::write(&broken, b"definitely not an mpeg stream").unwrap();
        fs::write(fx.path("notes.txt"), b"not audio").unwrap();
        fs::write(fx.path(".hidden.mp3"), b"hidden").unwrap();
    }
}
