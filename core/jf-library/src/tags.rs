//! Tag and audio-property reading with lofty.

use std::fmt;
use std::io::{self, BufReader, Read, Seek};
use std::path::Path;

use lofty::config::ParseOptions;
use lofty::error::FileParseError;
use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::id3::v2::Id3v2Tag;
use lofty::picture::{Picture, PictureType};
use lofty::probe::Probe;
use lofty::tag::{ItemKey, Tag};

use crate::ReplayGain;
use crate::fs::FileSystem;
use crate::lyrics;

/// File extensions the scanner picks up (compared case-insensitively).
///
/// Some of these are read for tags only until the engine can decode them.
pub(crate) const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "mp4", "aac", "wav", "aif", "aiff", "wv", "ape",
];

/// Whether `path` has one of the [`AUDIO_EXTENSIONS`].
pub(crate) fn is_audio_path(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| {
        AUDIO_EXTENSIONS
            .iter()
            .any(|known| known.eq_ignore_ascii_case(ext))
    })
}

/// Everything the catalogue stores about one file, read from its tags and headers.
#[derive(Debug, Default)]
pub(crate) struct FileMeta {
    pub title: Option<String>,
    /// The artist tag as displayed (several values joined with "; ").
    pub artist: Option<String>,
    /// Individual artists for `track_artists`, see [`split_artists`].
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub duration_ms: Option<u64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bit_depth: Option<u8>,
    pub codec: Option<String>,
    pub replaygain: ReplayGain,
    pub isrc: Option<String>,
    pub has_lyrics: bool,
}

/// A parsed file: lofty's generic view plus what only the concrete formats expose.
pub(crate) struct Parsed {
    pub file: TaggedFile,
    pub codec: Option<&'static str>,
    /// ID3v2 SYLT lyrics with millisecond stamps. lofty's generic tag drops SYLT
    /// frames, so they are taken from the concrete file before conversion.
    pub synced: Vec<Vec<(u32, String)>>,
}

/// Why a file could not be parsed.
#[derive(Debug)]
pub(crate) enum ReadError {
    Io(io::Error),
    Format(String),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => err.fmt(f),
            Self::Format(message) => f.write_str(message),
        }
    }
}

/// Opens `path` through `fs` and parses its tags and audio properties.
pub(crate) fn parse(fs: &dyn FileSystem, path: &Path, read_cover_art: bool) -> Result<Parsed, ReadError> {
    let reader = fs.open(path).map_err(ReadError::Io)?;
    let mut reader = BufReader::new(reader);
    let options = ParseOptions::new().read_cover_art(read_cover_art);

    // The extension is the fallback; the content decides when it is recognisable
    // (an .ogg holding Opus, an .m4a that is really ADTS, and .oga which lofty has no
    // extension mapping for).
    let guessed = {
        let mut probe = Probe::new(&mut reader).options(options);
        if let Some(from_ext) = path.extension().and_then(FileType::from_ext) {
            probe = probe.set_file_type(from_ext);
        }
        probe.guess_file_type().map_err(ReadError::Io)?.file_type()
    };
    let Some(file_type) = guessed else {
        return Err(ReadError::Format("unrecognised audio format".to_owned()));
    };
    read_as(&mut reader, file_type, options).map_err(|err| ReadError::Format(err.to_string()))
}

fn read_as<R: Read + Seek>(
    reader: &mut R,
    file_type: FileType,
    options: ParseOptions,
) -> Result<Parsed, FileParseError> {
    use lofty::aac::AacFile;
    use lofty::iff::aiff::AiffFile;
    use lofty::iff::wav::{WavFile, WavFormat};
    use lofty::mp4::{Mp4Codec, Mp4File};
    use lofty::mpeg::{Layer, MpegFile};

    let parsed = match file_type {
        FileType::Mpeg => {
            let file = MpegFile::read_from(reader, options)?;
            let codec = match file.properties().layer() {
                Layer::Layer1 => "mp1",
                Layer::Layer2 => "mp2",
                Layer::Layer3 => "mp3",
            };
            Parsed {
                codec: Some(codec),
                synced: synced_lyrics(file.id3v2()),
                file: file.into(),
            }
        }
        FileType::Wav => {
            let file = WavFile::read_from(reader, options)?;
            let codec = match file.properties().format() {
                WavFormat::PCM => "pcm",
                WavFormat::IEEE_FLOAT => "pcm_float",
                WavFormat::Other(_) => "wav",
            };
            Parsed {
                codec: Some(codec),
                synced: synced_lyrics(file.id3v2()),
                file: file.into(),
            }
        }
        FileType::Aiff => {
            let file = AiffFile::read_from(reader, options)?;
            Parsed {
                codec: Some("pcm"),
                synced: synced_lyrics(file.id3v2()),
                file: file.into(),
            }
        }
        FileType::Aac => {
            let file = AacFile::read_from(reader, options)?;
            Parsed {
                codec: Some("aac"),
                synced: synced_lyrics(file.id3v2()),
                file: file.into(),
            }
        }
        FileType::Mp4 => {
            let file = Mp4File::read_from(reader, options)?;
            let codec = match file.properties().codec() {
                Some(Mp4Codec::AAC) => "aac",
                Some(Mp4Codec::ALAC) => "alac",
                Some(Mp4Codec::MP3) => "mp3",
                Some(Mp4Codec::FLAC) => "flac",
                _ => "mp4",
            };
            Parsed {
                codec: Some(codec),
                synced: Vec::new(),
                file: file.into(),
            }
        }
        other => Parsed {
            codec: codec_name(other),
            synced: Vec::new(),
            file: Probe::with_file_type(reader, other).options(options).read()?,
        },
    };
    Ok(parsed)
}

/// SYLT frames holding lyrics with millisecond timestamps. Frames stamped in MPEG
/// frames are skipped: converting them needs the frame duration.
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

impl Parsed {
    /// Tags in priority order: the format's primary tag first, then the others
    /// (an MP3's ID3v1 or APE tag only fills fields its ID3v2 tag lacks).
    fn tags(&self) -> Vec<&Tag> {
        let primary = self.file.primary_tag_type();
        let mut tags: Vec<&Tag> = self.file.tags().iter().collect();
        tags.sort_by_key(|tag| tag.tag_type() != primary);
        tags
    }

    /// Reads the catalogue fields.
    pub(crate) fn meta(&self) -> FileMeta {
        let tags = self.tags();
        let properties = self.file.properties();

        let artist_values = texts(&tags, ItemKey::TrackArtist);
        let explicit_artists = texts(&tags, ItemKey::TrackArtists);
        // Shown as tagged; without an ARTIST tag, the ARTISTS list stands in.
        let artist = joined(&artist_values).or_else(|| joined(&explicit_artists));
        let artists = if !explicit_artists.is_empty() {
            explicit_artists
        } else if artist_values.len() > 1 {
            artist_values
        } else {
            artist_values
                .first()
                .map(|one| split_artists(one))
                .unwrap_or_default()
        };

        let duration = properties.duration();
        FileMeta {
            title: text(&tags, ItemKey::TrackTitle),
            artist,
            artists: dedupe(artists),
            album: text(&tags, ItemKey::AlbumTitle),
            album_artist: joined(&texts(&tags, ItemKey::AlbumArtist)),
            track_no: text(&tags, ItemKey::TrackNumber).and_then(|s| leading_number(&s)),
            disc_no: text(&tags, ItemKey::DiscNumber).and_then(|s| leading_number(&s)),
            year: [
                ItemKey::RecordingDate,
                ItemKey::Year,
                ItemKey::OriginalReleaseDate,
                ItemKey::ReleaseDate,
            ]
            .into_iter()
            .find_map(|key| text(&tags, key).and_then(|s| parse_year(&s))),
            genre: joined(&texts(&tags, ItemKey::Genre)),
            duration_ms: (!duration.is_zero())
                .then(|| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)),
            sample_rate: properties.sample_rate().filter(|&rate| rate > 0),
            channels: properties.channels().filter(|&n| n > 0),
            bit_depth: properties.bit_depth().filter(|&bits| bits > 0),
            codec: self.codec.map(str::to_owned),
            replaygain: ReplayGain {
                track_gain_db: text(&tags, ItemKey::ReplayGainTrackGain).and_then(|s| parse_gain(&s)),
                track_peak: text(&tags, ItemKey::ReplayGainTrackPeak).and_then(|s| parse_peak(&s)),
                album_gain_db: text(&tags, ItemKey::ReplayGainAlbumGain).and_then(|s| parse_gain(&s)),
                album_peak: text(&tags, ItemKey::ReplayGainAlbumPeak).and_then(|s| parse_peak(&s)),
            },
            isrc: text(&tags, ItemKey::Isrc),
            has_lyrics: !self.synced.is_empty() || text_lyrics(&tags).is_some(),
        }
    }

    /// Embedded lyrics as LRC text: text lyrics (USLT, Vorbis LYRICS, MP4 ©lyr) that
    /// carry LRC time tags first, then SYLT converted to LRC, then unsynced text.
    pub(crate) fn lyrics(&self) -> Option<String> {
        let text = text_lyrics(&self.tags());
        if text.as_deref().is_some_and(lyrics::looks_like_lrc) {
            return text;
        }
        if let Some(synced) = self.synced.first() {
            return Some(lyrics::sylt_to_lrc(synced));
        }
        text
    }

    /// The embedded front cover; failing that, a picture typed "other", then any picture.
    pub(crate) fn front_cover(&self) -> Option<&Picture> {
        let tags = self.tags();
        let pictures = || tags.iter().copied().flat_map(Tag::pictures);
        pictures()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or_else(|| pictures().find(|p| p.pic_type() == PictureType::Other))
            .or_else(|| pictures().next())
            .filter(|p| !p.data().is_empty())
    }
}

fn text_lyrics(tags: &[&Tag]) -> Option<String> {
    [ItemKey::Lyrics, ItemKey::UnsyncLyrics]
        .into_iter()
        .find_map(|key| text(tags, key))
}

/// Parses a ReplayGain gain: "-6.20 dB", "+3.5 dB", "-6,20 dB" (decimal comma), "1.5".
/// Values beyond ±64 dB are treated as garbage.
pub(crate) fn parse_gain(text: &str) -> Option<f32> {
    let text = text.trim();
    let number = match text.len().checked_sub(2) {
        Some(at) if text.is_char_boundary(at) && text[at..].eq_ignore_ascii_case("db") => {
            text[..at].trim_end()
        }
        _ => text,
    };
    parse_decimal(number).filter(|gain| gain.abs() <= 64.0)
}

/// Parses a ReplayGain peak (linear amplitude, 1.0 = full scale).
pub(crate) fn parse_peak(text: &str) -> Option<f32> {
    parse_decimal(text.trim()).filter(|peak| (0.0..=64.0).contains(peak))
}

fn parse_decimal(text: &str) -> Option<f32> {
    text.replace(',', ".")
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
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
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .find(|values| !values.is_empty())
        .unwrap_or_default()
}

fn joined(values: &[String]) -> Option<String> {
    (!values.is_empty()).then(|| values.join("; "))
}

fn dedupe(names: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    names
        .into_iter()
        .filter(|name| seen.insert(crate::normalize::normalize(name)))
        .collect()
}

/// "3", "03", "3/12" and " 3 " all give 3.
fn leading_number(text: &str) -> Option<u32> {
    let digits: String = text.trim().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok().filter(|&n| n > 0)
}

/// The first four-digit run: "2021", "2021-04-01", "2021/04" and "(P) 2021" give 2021.
pub fn parse_year(text: &str) -> Option<i32> {
    let bytes = text.as_bytes();
    bytes
        .windows(4)
        .enumerate()
        .find(|(start, window)| {
            window.iter().all(u8::is_ascii_digit)
                && !bytes.get(start + 4).is_some_and(u8::is_ascii_digit)
                && (*start == 0 || !bytes[start - 1].is_ascii_digit())
        })
        .and_then(|(_, window)| std::str::from_utf8(window).ok()?.parse().ok())
        .filter(|&year| year > 0)
}

/// Splits an artist tag into individual artists, conservatively.
///
/// Splits on:
/// * `;` and the full-width `；`,
/// * ` / ` with spaces on both sides (so "AC/DC" stays one artist),
/// * featuring markers `feat.`, `feat`, `ft.` and `featuring` as whole words,
///   optionally inside `(…)` or `[…]`: "A feat. B" and "A (feat. B)" give A and B.
///
/// Never splits on `&`, `,`, `and`, `x` or `vs.`, which occur inside names
/// ("Simon & Garfunkel", "Earth, Wind & Fire"). A tag with several values, or an
/// explicit ARTISTS tag, is used as given instead.
pub fn split_artists(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let (main, featured) = split_featuring(text);
    for piece in [Some(main), featured].into_iter().flatten() {
        for name in piece.split([';', '；']).flat_map(|chunk| chunk.split(" / ")) {
            let name = name.trim();
            if !name.is_empty() {
                parts.push(name.to_owned());
            }
        }
    }
    parts
}

fn split_featuring(text: &str) -> (&str, Option<&str>) {
    const MARKERS: &[&str] = &["feat.", "feat", "ft.", "featuring"];
    // ASCII lowercasing keeps byte offsets aligned with `text`.
    let lower = text.to_ascii_lowercase();
    let mut best: Option<(usize, usize, Option<char>)> = None;
    for marker in MARKERS {
        for (start, _) in lower.match_indices(marker) {
            let end = start + marker.len();
            let before = lower[..start].chars().next_back();
            let after = lower[end..].chars().next();
            let opener = match before {
                Some(c @ ('(' | '[')) => Some(c),
                Some(c) if c.is_whitespace() => None,
                _ => continue,
            };
            if !after.is_some_and(char::is_whitespace) {
                continue;
            }
            if best.is_none_or(|(best_start, ..)| start < best_start) {
                best = Some((start, end, opener));
            }
        }
    }
    let Some((start, end, opener)) = best else {
        return (text, None);
    };
    let main_end = if opener.is_some() { start - 1 } else { start };
    let mut featured = text[end..].trim();
    if let Some(open) = opener {
        let close = if open == '(' { ')' } else { ']' };
        featured = featured.strip_suffix(close).unwrap_or(featured).trim_end();
    }
    let main = text[..main_end].trim_end();
    if main.is_empty() || featured.is_empty() {
        return (text, None);
    }
    (main, Some(featured))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_artists_conservatively() {
        assert_eq!(split_artists("A; B"), ["A", "B"]);
        assert_eq!(split_artists("A；B"), ["A", "B"]);
        assert_eq!(split_artists("A / B"), ["A", "B"]);
        assert_eq!(split_artists("AC/DC"), ["AC/DC"]);
        assert_eq!(split_artists("A feat. B"), ["A", "B"]);
        assert_eq!(split_artists("A Feat B"), ["A", "B"]);
        assert_eq!(split_artists("A ft. B; C"), ["A", "B", "C"]);
        assert_eq!(split_artists("A (feat. B)"), ["A", "B"]);
        assert_eq!(split_artists("A [featuring B / C]"), ["A", "B", "C"]);
        assert_eq!(split_artists("Simon & Garfunkel"), ["Simon & Garfunkel"]);
        assert_eq!(split_artists("Earth, Wind & Fire"), ["Earth, Wind & Fire"]);
        assert_eq!(split_artists("Defeated"), ["Defeated"]);
        assert_eq!(split_artists("Soft Cell"), ["Soft Cell"]);
        assert_eq!(split_artists("feat. B"), ["feat. B"]);
        assert_eq!(split_artists(" ; "), Vec::<String>::new());
    }

    #[test]
    fn parses_replaygain_strings() {
        assert_eq!(parse_gain("-6.20 dB"), Some(-6.2));
        assert_eq!(parse_gain("+3.5 dB"), Some(3.5));
        assert_eq!(parse_gain(" -6,20 DB "), Some(-6.2));
        assert_eq!(parse_gain("-7.03dB"), Some(-7.03));
        assert_eq!(parse_gain("1.5"), Some(1.5));
        assert_eq!(parse_gain("loud"), None);
        assert_eq!(parse_gain("dB"), None);
        assert_eq!(parse_gain("-120 dB"), None);
        assert_eq!(parse_gain("NaN dB"), None);
        assert_eq!(parse_peak("0.988525"), Some(0.988_525));
        assert_eq!(parse_peak("1,000000"), Some(1.0));
        assert_eq!(parse_peak("-1"), None);
        assert_eq!(parse_peak("inf"), None);
    }

    #[test]
    fn parses_numbers_and_years() {
        assert_eq!(leading_number("03"), Some(3));
        assert_eq!(leading_number("3/12"), Some(3));
        assert_eq!(leading_number("x"), None);
        assert_eq!(leading_number("0"), None);
        assert_eq!(parse_year("2021-04-01"), Some(2021));
        assert_eq!(parse_year("(P) 1999 Label"), Some(1999));
        assert_eq!(parse_year("12345"), None);
        assert_eq!(parse_year("n/a"), None);
    }

    #[test]
    fn knows_audio_extensions() {
        assert!(is_audio_path(Path::new("/m/a.FLAC")));
        assert!(is_audio_path(Path::new("/m/a.oga")));
        assert!(!is_audio_path(Path::new("/m/a.lrc")));
        assert!(!is_audio_path(Path::new("/m/flac")));
    }
}
