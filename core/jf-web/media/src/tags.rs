//! Tags of audio files the person drags in: title, artist, album, length,
//! ReplayGain, embedded lyrics and the front cover, read with lofty from the
//! file's bytes, by the rules `jf-library` scans with (`jf-library` itself
//! needs SQLite and the file system, so the rules are mirrored here):
//!
//! * tags in priority order, the format's primary tag first;
//! * the artist as tagged, else the ARTISTS list joined;
//! * ReplayGain gains like `-6.20 dB` (decimal comma allowed, at most ±64 dB)
//!   and peaks (0 to 64);
//! * lyrics: text lyrics with LRC stamps first, then ID3v2 SYLT frames with
//!   millisecond stamps as (enhanced) LRC, then plain text;
//! * the front cover, else a picture typed "other", else any picture.

use std::io::Cursor;

use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::id3::v2::Id3v2Tag;
use lofty::picture::{Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};
use serde::Serialize;

/// ReplayGain 2.0 values from tags: gains in dB, peaks as linear sample
/// peaks. The same shape as `jf_web::rules::ReplayGainInfo` (and the
/// engine's), which the page hands these to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct ReplayGainInfo {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

/// What the tags of a file say.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FileTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub duration_ms: Option<u64>,
    /// `mp3`, `flac`, `aac`, `alac`, `opus`, `vorbis`, `pcm`, …
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bitrate_kbps: Option<u32>,
    pub replaygain: Option<ReplayGainInfo>,
    /// Embedded lyrics as text (LRC when they are timed).
    pub lyrics: Option<String>,
    /// MIME type of the embedded cover, when there is one.
    pub cover_mime: Option<String>,
    /// The embedded cover's bytes (not serialised; handed over separately).
    #[serde(skip)]
    pub cover: Option<Vec<u8>>,
}

/// Reads the tags of a file's bytes; `name` (its file name) helps when the
/// content alone does not tell the format.
pub fn read(bytes: &[u8], name: &str) -> Result<FileTags, String> {
    let from_ext = name.rsplit_once('.').and_then(|(_, ext)| FileType::from_ext(ext));
    let mut probe = Probe::new(Cursor::new(bytes));
    if let Some(file_type) = from_ext {
        probe = probe.set_file_type(file_type);
    }
    let probe = probe.guess_file_type().map_err(|e| e.to_string())?;
    let file_type = probe.file_type();
    let file = probe.read().map_err(|e| e.to_string())?;
    let synced = match file_type {
        Some(FileType::Mpeg) => {
            let mut reader = Cursor::new(bytes);
            lofty::mpeg::MpegFile::read_from(&mut reader, lofty::config::ParseOptions::new())
                .ok()
                .map(|f| synced_lyrics(f.id3v2()))
                .unwrap_or_default()
        }
        _ => Vec::new(),
    };
    Ok(tags_of(&file, file_type, &synced))
}

fn tags_of(file: &TaggedFile, file_type: Option<FileType>, synced: &[Vec<(u32, String)>]) -> FileTags {
    let primary = file.primary_tag_type();
    let mut tags: Vec<&Tag> = file.tags().iter().collect();
    tags.sort_by_key(|tag| tag.tag_type() != primary);
    let properties = file.properties();
    let artist =
        joined(&texts(&tags, ItemKey::TrackArtist)).or_else(|| joined(&texts(&tags, ItemKey::TrackArtists)));
    let replaygain = ReplayGainInfo {
        track_gain_db: text(&tags, ItemKey::ReplayGainTrackGain).and_then(|s| parse_gain(&s)),
        track_peak: text(&tags, ItemKey::ReplayGainTrackPeak).and_then(|s| parse_peak(&s)),
        album_gain_db: text(&tags, ItemKey::ReplayGainAlbumGain).and_then(|s| parse_gain(&s)),
        album_peak: text(&tags, ItemKey::ReplayGainAlbumPeak).and_then(|s| parse_peak(&s)),
    };
    let has_gain = replaygain.track_gain_db.is_some() || replaygain.album_gain_db.is_some();
    let text_lyrics = [ItemKey::Lyrics, ItemKey::UnsyncLyrics]
        .into_iter()
        .find_map(|key| text(&tags, key));
    let lyrics = if text_lyrics.as_deref().is_some_and(looks_like_lrc) {
        text_lyrics
    } else if let Some(first) = synced.first() {
        Some(sylt_to_lrc(first))
    } else {
        text_lyrics
    };
    let cover = front_cover(&tags);
    let duration = properties.duration();
    FileTags {
        title: text(&tags, ItemKey::TrackTitle),
        artist,
        album: text(&tags, ItemKey::AlbumTitle),
        album_artist: joined(&texts(&tags, ItemKey::AlbumArtist)),
        track_no: text(&tags, ItemKey::TrackNumber).and_then(|s| leading_number(&s)),
        disc_no: text(&tags, ItemKey::DiscNumber).and_then(|s| leading_number(&s)),
        duration_ms: (!duration.is_zero()).then_some(duration.as_millis() as u64),
        codec: file_type.and_then(codec_name).map(str::to_owned),
        sample_rate: properties.sample_rate(),
        channels: properties.channels(),
        bitrate_kbps: properties.audio_bitrate(),
        replaygain: has_gain.then_some(replaygain),
        lyrics,
        cover_mime: cover.and_then(|p| p.mime_type().map(|m| m.as_str().to_owned())),
        cover: cover.map(|p| p.data().to_vec()),
    }
}

fn front_cover<'a>(tags: &[&'a Tag]) -> Option<&'a Picture> {
    let pictures = || tags.iter().copied().flat_map(Tag::pictures);
    pictures()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures().find(|p| p.pic_type() == PictureType::Other))
        .or_else(|| pictures().next())
        .filter(|p| !p.data().is_empty())
}

/// SYLT frames holding lyrics with millisecond timestamps.
fn synced_lyrics(tag: Option<&Id3v2Tag>) -> Vec<Vec<(u32, String)>> {
    use lofty::id3::v2::{Frame, SyncTextContentType, SynchronizedTextFrame, TimestampFormat};

    tag.into_iter()
        .flatten()
        .filter_map(|frame| match frame {
            Frame::Binary(binary) if binary.id().as_str() == "SYLT" => {
                SynchronizedTextFrame::parse(&binary.data, binary.flags()).ok()
            }
            _ => None,
        })
        .filter(|sylt| {
            sylt.timestamp_format == TimestampFormat::MS
                && matches!(
                    sylt.content_type,
                    SyncTextContentType::Lyrics
                        | SyncTextContentType::TextTranscription
                        | SyncTextContentType::Other
                )
                && !sylt.content.is_empty()
        })
        .map(|sylt| sylt.content)
        .collect()
}

fn codec_name(file_type: FileType) -> Option<&'static str> {
    Some(match file_type {
        FileType::Aac => "aac",
        FileType::Aiff | FileType::Wav => "pcm",
        FileType::Ape => "ape",
        FileType::Flac => "flac",
        FileType::Mpeg => "mp3",
        FileType::Mp4 => "mp4",
        FileType::Mpc => "musepack",
        FileType::Opus => "opus",
        FileType::Vorbis => "vorbis",
        FileType::Speex => "speex",
        FileType::WavPack => "wavpack",
        _ => return None,
    })
}

/// The first non-empty value for `key`, trimmed.
fn text(tags: &[&Tag], key: ItemKey) -> Option<String> {
    tags.iter().find_map(|tag| {
        tag.get_strings(key)
            .map(str::trim)
            .find(|value| !value.is_empty())
            .map(str::to_owned)
    })
}

/// All non-empty values for `key` from the first tag that has any.
fn texts(tags: &[&Tag], key: ItemKey) -> Vec<String> {
    tags.iter()
        .map(|tag| {
            tag.get_strings(key)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .find(|values| !values.is_empty())
        .unwrap_or_default()
}

fn joined(values: &[String]) -> Option<String> {
    (!values.is_empty()).then(|| values.join(", "))
}

fn leading_number(text: &str) -> Option<u32> {
    let digits: String = text.trim().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok().filter(|n| *n > 0)
}

/// A ReplayGain gain: "-6.20 dB", "+3.5 dB", "-6,20 dB", "1.5"; at most ±64 dB.
pub fn parse_gain(text: &str) -> Option<f32> {
    let text = text.trim();
    let number = match text.len().checked_sub(2) {
        Some(at) if text.is_char_boundary(at) && text[at..].eq_ignore_ascii_case("db") => {
            text[..at].trim_end()
        }
        _ => text,
    };
    parse_decimal(number).filter(|gain| gain.abs() <= 64.0)
}

/// A ReplayGain peak (linear amplitude, 1.0 = full scale).
pub fn parse_peak(text: &str) -> Option<f32> {
    parse_decimal(text.trim()).filter(|peak| (0.0..=64.0).contains(peak))
}

fn parse_decimal(text: &str) -> Option<f32> {
    text.replace(',', ".")
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

/// Whether any line starts with an LRC time tag such as `[01:23.45]`.
pub fn looks_like_lrc(text: &str) -> bool {
    text.lines().any(|line| {
        let Some(rest) = line.trim_start().strip_prefix('[') else {
            return false;
        };
        let Some((minutes, rest)) = rest.split_once(':') else {
            return false;
        };
        let seconds: String = rest.chars().take_while(char::is_ascii_digit).collect();
        !minutes.is_empty()
            && minutes.bytes().all(|b| b.is_ascii_digit())
            && !seconds.is_empty()
            && rest[seconds.len()..].starts_with(['.', ':', ']'])
    })
}

/// SYLT entries (milliseconds, text) as LRC: one entry per line, or one per
/// word with a leading newline marking each new line (enhanced LRC).
pub fn sylt_to_lrc(entries: &[(u32, String)]) -> String {
    let is_break = |text: &str| text.starts_with(['\n', '\r']);
    let per_word = entries.iter().skip(1).any(|(_, text)| is_break(text));
    let mut out = String::new();
    for (index, (ms, text)) in entries.iter().enumerate() {
        if !per_word {
            out.push_str(&format!("[{}]{}\n", stamp(*ms), text.trim()));
            continue;
        }
        if index == 0 || is_break(text) {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&format!("[{}]", stamp(*ms)));
        }
        out.push_str(&format!(
            "<{}>{}",
            stamp(*ms),
            text.trim_start_matches(['\n', '\r'])
        ));
    }
    if per_word {
        out.push('\n');
    }
    out
}

/// `mm:ss.xx`; minutes keep counting past 59.
fn stamp(ms: u32) -> String {
    let centis = ms / 10;
    format!(
        "{:02}:{:02}.{:02}",
        centis / 6000,
        centis / 100 % 60,
        centis % 100
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_replaygain_strings() {
        assert_eq!(parse_gain("-6.20 dB"), Some(-6.2));
        assert_eq!(parse_gain("+3.5 dB"), Some(3.5));
        assert_eq!(parse_gain("-6,20 dB"), Some(-6.2));
        assert_eq!(parse_gain("1.5"), Some(1.5));
        assert_eq!(parse_gain("99 dB"), None);
        assert_eq!(parse_peak("0.988"), Some(0.988));
        assert_eq!(parse_peak("-1"), None);
    }

    #[test]
    fn detects_lrc_and_converts_sylt() {
        assert!(looks_like_lrc("[ti:Song]\n[00:12.34]Line"));
        assert!(!looks_like_lrc("[ti:Song]\nplain words"));
        let lines = [(1000, "Hello".to_owned()), (62_500, "World".to_owned())];
        assert_eq!(sylt_to_lrc(&lines), "[00:01.00]Hello\n[01:02.50]World\n");
        let words = [
            (1000, "Hel".to_owned()),
            (1500, "lo".to_owned()),
            (3000, "\nWorld".to_owned()),
        ];
        assert_eq!(
            sylt_to_lrc(&words),
            "[00:01.00]<00:01.00>Hel<00:01.50>lo\n[00:03.00]<00:03.00>World\n"
        );
    }

    #[test]
    fn reads_a_wav_file() {
        // A 0.1 s mono 8 kHz silent WAV with a LIST INFO title.
        let samples = 800u32;
        let mut wav = Vec::new();
        let info = b"INFOINAM\x05\x00\x00\x00Tone\x00\x00";
        let data_len = samples * 2;
        let riff_len = 4 + (8 + 16) + (8 + data_len) + (8 + info.len() as u32);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&riff_len.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8000u32.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.extend(std::iter::repeat_n(0u8, data_len as usize));
        wav.extend_from_slice(b"LIST");
        wav.extend_from_slice(&(info.len() as u32).to_le_bytes());
        wav.extend_from_slice(info);
        let tags = read(&wav, "tone.wav").unwrap();
        assert_eq!(tags.title.as_deref(), Some("Tone"));
        assert_eq!(tags.duration_ms, Some(100));
        assert_eq!(tags.codec.as_deref(), Some("pcm"));
        assert!(read(b"not audio", "x.txt").is_err());
    }
}
