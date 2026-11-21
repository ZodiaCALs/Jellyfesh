//! Get Info and Now Playing's Info pane (ADR-0022 §3, docs/ui.md "Details"), as the apps show
//! them: About This Song with Title, Artist, Album, Version, Year, Genre, Composer, Lyricist,
//! Label and Length, then Audio: Quality (the core's label), Format ("FLAC · 24-bit · 96 kHz ·
//! Stereo"), Bitrate, Size, ReplayGain, and File or Source. Rows without a value are left out.
//! The quality label and the audio format follow `jf-core/src/details.rs`; the rows and their
//! order follow the apps (`platforms/linux/src/logic/details.rs`). The page words the values.

use jf_plugin_model::{Fact, FactKey, ItemDetails};
use serde::{Deserialize, Serialize};

use crate::rules::ReplayGainInfo;

/// The core's quality label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityLabel {
    HiResLossless,
    Lossless,
    High,
    Standard,
}

/// The format of what plays or of a file, with its quality label.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioFormat {
    /// Short lowercase codec name: `flac`, `mp3`, `aac`, `alac`, `vorbis`, `opus`, `pcm`.
    pub codec: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub bit_depth: Option<u32>,
    pub channels: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub lossless: Option<bool>,
    pub quality: Option<QualityLabel>,
}

/// Whether a codec keeps every bit (`None` when the codec is unknown), as the core's.
pub fn lossless(codec: &str) -> Option<bool> {
    match codec {
        "flac" | "alac" | "pcm" | "pcm_float" | "wav" | "aiff" | "ape" | "wavpack" | "tta" => Some(true),
        "mp3" | "mp2" | "mp1" | "aac" | "vorbis" | "opus" | "musepack" | "speex" => Some(false),
        _ => None,
    }
}

/// The core's quality label: Hi-Res Lossless for lossless audio above 16 bits or 48 kHz,
/// Lossless, High for lossy audio at 256 kbit/s or more, Standard below; `None` when the format
/// is not known well enough to say.
pub fn quality(
    lossless: Option<bool>,
    bit_depth: Option<u32>,
    sample_rate_hz: Option<u32>,
    bitrate_kbps: Option<u32>,
) -> Option<QualityLabel> {
    match lossless? {
        true => {
            let hi_res = bit_depth.is_some_and(|b| b > 16) || sample_rate_hz.is_some_and(|r| r > 48_000);
            Some(if hi_res {
                QualityLabel::HiResLossless
            } else {
                QualityLabel::Lossless
            })
        }
        false => Some(match bitrate_kbps {
            Some(kbps) if kbps >= 256 => QualityLabel::High,
            _ => QualityLabel::Standard,
        }),
    }
}

/// An audio format with its quality label. Codecs come lowercase, PCM by any sample layout
/// (`pcm_s16le`) as `pcm`, and `m4a`'s usual codec as `aac`.
pub fn format(
    codec: Option<&str>,
    sample_rate_hz: Option<u32>,
    bit_depth: Option<u32>,
    channels: Option<u32>,
    bitrate_kbps: Option<u32>,
) -> AudioFormat {
    let codec = codec.map(str::trim).filter(|c| !c.is_empty()).map(|c| {
        let lower = c.to_ascii_lowercase();
        if lower.starts_with("pcm") {
            "pcm".to_owned()
        } else {
            lower
        }
    });
    let lossless = codec.as_deref().and_then(lossless);
    let sample_rate_hz = sample_rate_hz.filter(|&r| r > 0);
    let bitrate_kbps = bitrate_kbps.filter(|&k| k > 0);
    AudioFormat {
        codec,
        sample_rate_hz,
        // Lossy codecs have no bit depth of their own.
        bit_depth: bit_depth.filter(|&b| b > 0 && lossless != Some(false)),
        channels: channels.filter(|&c| c > 0),
        bitrate_kbps,
        lossless,
        quality: quality(lossless, bit_depth, sample_rate_hz, bitrate_kbps),
    }
}

/// The codec as people write it (`flac` → `FLAC`, `pcm_s16le` → `PCM`, `opus` → `Opus`).
pub fn codec_name(name: &str) -> String {
    let base = name.split('_').next().unwrap_or(name);
    match base {
        "vorbis" => "Vorbis".into(),
        "opus" => "Opus".into(),
        "alac" => "ALAC".into(),
        other => other.to_ascii_uppercase(),
    }
}

/// The parts of "FLAC · 24-bit · 96 kHz · Stereo"; the page words them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FormatParts {
    pub codec: Option<String>,
    pub bit_depth: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u32>,
}

/// One row of Get Info, by its key; the page words the key and the value.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "key", content = "value")]
pub enum Row {
    Title(String),
    Artist(String),
    Album(String),
    Version(String),
    Year(String),
    Genre(String),
    Composer(String),
    Lyricist(String),
    Label(String),
    /// ms.
    Length(u64),
    Quality(QualityLabel),
    Format(FormatParts),
    /// kbit/s.
    Bitrate(u32),
    /// Bytes.
    Size(u64),
    ReplayGain(ReplayGainInfo),
    File(String),
    Source(String),
}

/// What the page knows of a song before (or without) the plugin's details.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Known {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub duration_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub replaygain: Option<ReplayGainInfo>,
    /// The file's path in its folder, or its name, for a library song or a file.
    pub file: Option<String>,
    /// The source's name, for a source's song.
    pub source: Option<String>,
}

/// About This Song and Audio.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SongRows {
    pub about: Vec<Row>,
    pub audio: Vec<Row>,
}

fn fact<'a>(facts: &'a [Fact], key: &FactKey) -> Option<&'a str> {
    facts
        .iter()
        .find(|f| f.key == *key)
        .map(|f| f.value.trim())
        .filter(|v| !v.is_empty())
}

/// The year a release date starts with ("2011-05-03" is 2011), or the date as tagged.
pub fn year(released: &str) -> &str {
    let released = released.trim();
    let digits = released.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 4 { &released[..4] } else { released }
}

fn text(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
}

fn parts(format: &AudioFormat) -> Option<FormatParts> {
    let parts = FormatParts {
        codec: format.codec.as_deref().map(codec_name),
        bit_depth: format.bit_depth.filter(|b| *b > 0),
        sample_rate_hz: format.sample_rate_hz.filter(|r| *r > 0),
        channels: format.channels.filter(|c| *c > 0),
    };
    (parts != FormatParts::default()).then_some(parts)
}

/// The rows for a song: the plugin's facts where it has them, what the page knows otherwise, and
/// the format of what plays (or of the file).
pub fn song_rows(details: Option<&ItemDetails>, known: &Known, format: Option<&AudioFormat>) -> SongRows {
    let facts: &[Fact] = details.map_or(&[], |d| d.facts.as_slice());
    let mut about = Vec::new();
    let push = |rows: &mut Vec<Row>, row: Option<Row>| rows.extend(row);
    push(&mut about, text(known.title.as_deref()).map(Row::Title));
    push(&mut about, text(known.artist.as_deref()).map(Row::Artist));
    push(&mut about, text(known.album.as_deref()).map(Row::Album));
    push(&mut about, text(fact(facts, &FactKey::Version)).map(Row::Version));
    push(
        &mut about,
        text(fact(facts, &FactKey::Released).map(year))
            .or_else(|| text(known.year.as_deref()))
            .map(Row::Year),
    );
    push(&mut about, text(fact(facts, &FactKey::Genre)).map(Row::Genre));
    push(
        &mut about,
        text(fact(facts, &FactKey::Composer)).map(Row::Composer),
    );
    push(
        &mut about,
        text(fact(facts, &FactKey::Lyricist)).map(Row::Lyricist),
    );
    push(&mut about, text(fact(facts, &FactKey::Label)).map(Row::Label));
    push(
        &mut about,
        known.duration_ms.filter(|ms| *ms > 0).map(Row::Length),
    );

    let mut audio = Vec::new();
    push(&mut audio, format.and_then(|f| f.quality).map(Row::Quality));
    push(&mut audio, format.and_then(parts).map(Row::Format));
    push(
        &mut audio,
        format
            .and_then(|f| f.bitrate_kbps)
            .filter(|b| *b > 0)
            .map(Row::Bitrate),
    );
    push(&mut audio, known.size_bytes.filter(|b| *b > 0).map(Row::Size));
    push(
        &mut audio,
        known
            .replaygain
            .filter(|r| r.track_gain_db.is_some() || r.album_gain_db.is_some())
            .map(Row::ReplayGain),
    );
    match (text(known.file.as_deref()), text(known.source.as_deref())) {
        (Some(file), _) => audio.push(Row::File(file)),
        (None, Some(source)) => audio.push(Row::Source(source)),
        (None, None) => {}
    }
    SongRows { about, audio }
}

/// The facts of an album or an artist: Version, Year, Genre, Label, then Composer and Lyricist.
pub fn item_rows(details: &ItemDetails) -> Vec<Row> {
    let facts = details.facts.as_slice();
    let mut rows = Vec::new();
    rows.extend(text(fact(facts, &FactKey::Version)).map(Row::Version));
    rows.extend(text(fact(facts, &FactKey::Released).map(year)).map(Row::Year));
    rows.extend(text(fact(facts, &FactKey::Genre)).map(Row::Genre));
    rows.extend(text(fact(facts, &FactKey::Label)).map(Row::Label));
    rows.extend(text(fact(facts, &FactKey::Composer)).map(Row::Composer));
    rows.extend(text(fact(facts, &FactKey::Lyricist)).map(Row::Lyricist));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_labels_follow_the_core() {
        assert_eq!(
            format(Some("flac"), Some(96_000), Some(24), Some(2), None).quality,
            Some(QualityLabel::HiResLossless)
        );
        assert_eq!(
            format(Some("flac"), Some(44_100), Some(16), Some(2), None).quality,
            Some(QualityLabel::Lossless)
        );
        assert_eq!(
            format(Some("FLAC"), Some(44_100), None, None, None).quality,
            Some(QualityLabel::Lossless)
        );
        assert_eq!(
            format(Some("alac"), Some(48_000), Some(24), None, None).quality,
            Some(QualityLabel::HiResLossless)
        );
        assert_eq!(
            format(Some("mp3"), None, Some(16), None, Some(320)).quality,
            Some(QualityLabel::High)
        );
        assert_eq!(
            format(Some("aac"), None, None, None, Some(256)).quality,
            Some(QualityLabel::High)
        );
        assert_eq!(
            format(Some("opus"), None, None, None, Some(160)).quality,
            Some(QualityLabel::Standard)
        );
        assert_eq!(
            format(Some("mp3"), None, None, None, None).quality,
            Some(QualityLabel::Standard)
        );
        assert_eq!(
            format(Some("pcm_s24le"), Some(44_100), Some(24), None, None)
                .codec
                .as_deref(),
            Some("pcm")
        );
        assert_eq!(
            format(Some("mystery"), Some(96_000), Some(24), None, None).quality,
            None
        );
        assert_eq!(format(None, None, None, None, None).quality, None);
        // Lossy codecs have no bit depth.
        assert_eq!(
            format(Some("mp3"), None, Some(16), None, Some(320)).bit_depth,
            None
        );
    }

    #[test]
    fn codecs_are_named_as_people_write_them() {
        assert_eq!(codec_name("flac"), "FLAC");
        assert_eq!(codec_name("pcm_s16le"), "PCM");
        assert_eq!(codec_name("opus"), "Opus");
        assert_eq!(codec_name("vorbis"), "Vorbis");
        assert_eq!(codec_name("alac"), "ALAC");
    }

    fn fact(key: FactKey, value: &str) -> Fact {
        Fact {
            key,
            value: value.into(),
        }
    }

    #[test]
    fn a_songs_rows_in_the_apps_order_without_empty_ones() {
        let details = ItemDetails {
            description: None,
            image: None,
            facts: vec![
                fact(FactKey::Genre, "Ambient"),
                fact(FactKey::Released, "2024-03-15"),
                fact(FactKey::Composer, "  "),
                fact(FactKey::Label, "Quiet Records"),
            ],
            comment_count: None,
        };
        let known = Known {
            title: Some("Harbour Lights".into()),
            artist: Some("The Quiet Fleet".into()),
            album: None,
            year: Some("2020".into()),
            duration_ms: Some(215_000),
            source: Some("Subsonic".into()),
            ..Known::default()
        };
        let playing = format(Some("flac"), Some(96_000), Some(24), Some(2), Some(2400));
        let rows = song_rows(Some(&details), &known, Some(&playing));
        assert_eq!(
            rows.about,
            vec![
                Row::Title("Harbour Lights".into()),
                Row::Artist("The Quiet Fleet".into()),
                Row::Year("2024".into()),
                Row::Genre("Ambient".into()),
                Row::Label("Quiet Records".into()),
                Row::Length(215_000),
            ]
        );
        assert_eq!(rows.audio[0], Row::Quality(QualityLabel::HiResLossless));
        assert_eq!(
            rows.audio[1],
            Row::Format(FormatParts {
                codec: Some("FLAC".into()),
                bit_depth: Some(24),
                sample_rate_hz: Some(96_000),
                channels: Some(2)
            })
        );
        assert_eq!(rows.audio[2], Row::Bitrate(2400));
        assert_eq!(rows.audio[3], Row::Source("Subsonic".into()));
        // Without details: the year the page knows; a file instead of a source.
        let file = Known {
            file: Some("Music/01 Harbour.flac".into()),
            size_bytes: Some(8_400_000),
            replaygain: Some(ReplayGainInfo {
                track_gain_db: Some(-6.2),
                ..ReplayGainInfo::default()
            }),
            ..known
        };
        let rows = song_rows(None, &file, None);
        assert!(rows.about.contains(&Row::Year("2020".into())));
        assert_eq!(
            rows.audio.len(),
            3,
            "size, ReplayGain and the file: {:?}",
            rows.audio
        );
        assert!(matches!(rows.audio[2], Row::File(_)));
    }

    #[test]
    fn rows_serialize_by_key() {
        let json = serde_json::to_value(Row::Quality(QualityLabel::HiResLossless)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"key": "Quality", "value": "HiResLossless"})
        );
        assert_eq!(year("2011-05-03"), "2011");
        assert_eq!(year("Spring 2011"), "Spring 2011");
    }

    #[test]
    fn an_albums_facts() {
        let details = ItemDetails {
            description: Some("Notes".into()),
            image: None,
            facts: vec![
                fact(FactKey::Lyricist, "A"),
                fact(FactKey::Released, "1999"),
                fact(FactKey::Popularity, "9"),
            ],
            comment_count: None,
        };
        assert_eq!(
            item_rows(&details),
            vec![Row::Year("1999".into()), Row::Lyricist("A".into())]
        );
    }
}
