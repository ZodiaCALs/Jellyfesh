//! Get Info and Now Playing's Info pane (ADR-0022 §3, docs/ui.md
//! "Details"): About This Song with Title, Artist, Album, Version, Year,
//! Genre, Composer, Lyricist, Label and Length, then Audio: Quality (the
//! core's label), Format ("FLAC · 24-bit · 96 kHz · Stereo"), Bitrate, Size,
//! ReplayGain, and File or Source. Rows without a value are left out. The
//! core says everything (`DetailsPage`, `NowPlaying::quality`); this picks
//! the rows, in their order, for the UI to word.

use std::path::PathBuf;

use jf_core::{AudioFormat, DetailsPage, Fact, FactKey, QualityLabel, ReplayGainValues};

/// A row of Get Info.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Title,
    Artist,
    Album,
    Version,
    Year,
    Genre,
    Composer,
    Lyricist,
    Label,
    Length,
    Quality,
    Format,
    Bitrate,
    Size,
    ReplayGain,
    File,
    Source,
}

/// The parts of "FLAC · 24-bit · 96 kHz · Stereo".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormatParts {
    /// The codec as people write it: FLAC, MP3, AAC, Opus (`format::codec`).
    pub codec: Option<String>,
    pub bit_depth: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u32>,
}

impl FormatParts {
    pub fn is_empty(&self) -> bool {
        *self == FormatParts::default()
    }
}

/// A row's value, for the UI to word.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    /// A length in ms.
    Length(u64),
    Quality(QualityLabel),
    Format(FormatParts),
    /// kbit/s.
    Bitrate(u32),
    /// A size in bytes.
    Size(u64),
    ReplayGain(ReplayGainValues),
    Path(PathBuf),
}

/// Rows of Get Info, in their order.
pub type Rows = Vec<(Key, Value)>;

/// What the view knows before (or without) the core's details: from the
/// row, the album page or Now Playing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Known {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<String>,
    pub duration_ms: Option<u64>,
    pub path: Option<PathBuf>,
    /// The source's name, for a source's item.
    pub source: Option<String>,
}

fn format_parts(format: &AudioFormat) -> FormatParts {
    FormatParts {
        codec: format
            .codec
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(crate::logic::format::codec),
        bit_depth: format.bit_depth.filter(|b| *b > 0),
        sample_rate_hz: format.sample_rate_hz.filter(|r| *r > 0),
        channels: format.channels.filter(|c| *c > 0),
    }
}

fn fact<'a>(facts: &'a [Fact], key: &FactKey) -> Option<&'a str> {
    facts
        .iter()
        .find(|f| f.key == *key)
        .map(|f| f.value.trim())
        .filter(|v| !v.is_empty())
}

/// The year a release date starts with ("2011-05-03" is 2011), or the
/// date as tagged when it does not start with one.
pub fn year(released: &str) -> &str {
    let released = released.trim();
    let digits = released.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 4 { &released[..4] } else { released }
}

fn text(value: Option<&str>) -> Option<Value> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| Value::Text(v.to_owned()))
}

/// The rows of About This Song and of Audio for a song: the core's details
/// where it has them, what the view knows otherwise, and for what is
/// playing the format actually being decoded (`decoded`) over the file's.
pub fn song_rows(page: Option<&DetailsPage>, known: &Known, decoded: Option<&AudioFormat>) -> (Rows, Rows) {
    let facts: &[Fact] = page.map_or(&[], |p| p.facts.as_slice());
    let technical = page.and_then(|p| p.technical.as_ref());
    let mut about: Vec<(Key, Value)> = Vec::new();
    let push = |rows: &mut Vec<(Key, Value)>, key: Key, value: Option<Value>| {
        if let Some(v) = value {
            rows.push((key, v));
        }
    };
    push(
        &mut about,
        Key::Title,
        text(page.map(|p| p.title.as_str())).or_else(|| text(known.title.as_deref())),
    );
    push(
        &mut about,
        Key::Artist,
        text(page.and_then(|p| p.subtitle.as_deref())).or_else(|| text(known.artist.as_deref())),
    );
    push(&mut about, Key::Album, text(known.album.as_deref()));
    push(&mut about, Key::Version, text(fact(facts, &FactKey::Version)));
    push(
        &mut about,
        Key::Year,
        text(fact(facts, &FactKey::Released).map(year)).or_else(|| text(known.year.as_deref())),
    );
    push(&mut about, Key::Genre, text(fact(facts, &FactKey::Genre)));
    push(&mut about, Key::Composer, text(fact(facts, &FactKey::Composer)));
    push(&mut about, Key::Lyricist, text(fact(facts, &FactKey::Lyricist)));
    push(&mut about, Key::Label, text(fact(facts, &FactKey::Label)));
    push(
        &mut about,
        Key::Length,
        technical
            .and_then(|t| t.duration_ms)
            .or(known.duration_ms)
            .filter(|ms| *ms > 0)
            .map(Value::Length),
    );

    let mut audio: Vec<(Key, Value)> = Vec::new();
    let format = decoded.or_else(|| technical.map(|t| &t.format));
    push(
        &mut audio,
        Key::Quality,
        format.and_then(|f| f.quality).map(Value::Quality),
    );
    push(
        &mut audio,
        Key::Format,
        format
            .map(format_parts)
            .filter(|p| !p.is_empty())
            .map(Value::Format),
    );
    push(
        &mut audio,
        Key::Bitrate,
        format
            .and_then(|f| f.bitrate_kbps)
            .filter(|b| *b > 0)
            .map(Value::Bitrate),
    );
    push(
        &mut audio,
        Key::Size,
        technical
            .and_then(|t| t.file_size_bytes)
            .filter(|b| *b > 0)
            .map(Value::Size),
    );
    push(
        &mut audio,
        Key::ReplayGain,
        technical
            .and_then(|t| t.replaygain)
            .filter(|r| r.track_gain_db.is_some() || r.album_gain_db.is_some())
            .map(Value::ReplayGain),
    );
    match (
        technical
            .and_then(|t| t.path.clone())
            .or_else(|| known.path.clone()),
        &known.source,
    ) {
        (Some(path), _) => audio.push((Key::File, Value::Path(path))),
        (None, Some(source)) => push(&mut audio, Key::Source, text(Some(source))),
        (None, None) => {}
    }
    (about, audio)
}

/// The facts of an album or an artist, in the core's order, without the
/// empty ones; the UI words their keys ("Released", "Producer").
pub fn item_facts(page: &DetailsPage) -> Vec<(FactKey, String)> {
    page.facts
        .iter()
        .map(|f| (f.key.clone(), f.value.trim()))
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| (k, v.to_owned()))
        .collect()
}

/// A ReplayGain gain as tags give it: "+1.50 dB", "−6.20 dB", "0.00 dB".
pub fn decibels(db: f32) -> String {
    let hundredths = (f64::from(db) * 100.0).round();
    let sign = if hundredths > 0.0 {
        "+"
    } else if hundredths < 0.0 {
        "\u{2212}"
    } else {
        ""
    };
    format!("{sign}{:.2} dB", hundredths.abs() / 100.0)
}

#[cfg(test)]
mod tests {
    use jf_core::{CommandId, ItemRef, TechnicalInfo};

    use super::*;

    fn page() -> DetailsPage {
        DetailsPage {
            request: CommandId(7),
            item: ItemRef::Track { id: 3 },
            title: "Harbour Lights".into(),
            subtitle: Some("The Quiet Fleet".into()),
            description: None,
            image: None,
            facts: vec![
                Fact {
                    key: FactKey::Genre,
                    value: "Ambient".into(),
                },
                Fact {
                    key: FactKey::Released,
                    value: "2024-03-15".into(),
                },
                Fact {
                    key: FactKey::Composer,
                    value: "  ".into(),
                },
            ],
            technical: Some(TechnicalInfo {
                format: AudioFormat {
                    codec: Some("flac".into()),
                    sample_rate_hz: Some(96_000),
                    bit_depth: Some(24),
                    channels: Some(2),
                    bitrate_kbps: Some(2_100),
                    lossless: Some(true),
                    quality: Some(QualityLabel::HiResLossless),
                },
                container: Some("flac".into()),
                file_size_bytes: Some(8_400_000),
                duration_ms: Some(32_000),
                replaygain: Some(ReplayGainValues {
                    track_gain_db: Some(-6.2),
                    ..ReplayGainValues::default()
                }),
                path: Some(PathBuf::from("/music/harbour.flac")),
            }),
            comment_count: None,
            attribution: None,
            error: None,
        }
    }

    fn keys(rows: &[(Key, Value)]) -> Vec<Key> {
        rows.iter().map(|(k, _)| *k).collect()
    }

    #[test]
    fn rows_in_order_and_empty_ones_left_out() {
        let known = Known {
            album: Some("Lanterns of the Bay".into()),
            ..Known::default()
        };
        let (about, audio) = song_rows(Some(&page()), &known, None);
        assert_eq!(
            keys(&about),
            [
                Key::Title,
                Key::Artist,
                Key::Album,
                Key::Year,
                Key::Genre,
                Key::Length
            ]
        );
        assert_eq!(
            keys(&audio),
            [
                Key::Quality,
                Key::Format,
                Key::Bitrate,
                Key::Size,
                Key::ReplayGain,
                Key::File
            ]
        );
        assert_eq!(about[3].1, Value::Text("2024".into()));
        assert_eq!(audio[0].1, Value::Quality(QualityLabel::HiResLossless));
        assert_eq!(
            audio[1].1,
            Value::Format(FormatParts {
                codec: Some("FLAC".into()),
                bit_depth: Some(24),
                sample_rate_hz: Some(96_000),
                channels: Some(2),
            })
        );
    }

    #[test]
    fn what_is_decoded_wins_over_the_file() {
        let decoded = AudioFormat {
            codec: Some("pcm".into()),
            sample_rate_hz: Some(48_000),
            bit_depth: Some(16),
            channels: Some(2),
            bitrate_kbps: None,
            lossless: Some(true),
            quality: Some(QualityLabel::Lossless),
        };
        let (_, audio) = song_rows(Some(&page()), &Known::default(), Some(&decoded));
        assert_eq!(audio[0].1, Value::Quality(QualityLabel::Lossless));
        assert!(!keys(&audio).contains(&Key::Bitrate));
    }

    #[test]
    fn without_details_the_known_facts_show() {
        let known = Known {
            title: Some("Paper Boats".into()),
            artist: Some("The Quiet Fleet".into()),
            duration_ms: Some(28_000),
            source: Some("Navidrome".into()),
            ..Known::default()
        };
        let (about, audio) = song_rows(None, &known, None);
        assert_eq!(keys(&about), [Key::Title, Key::Artist, Key::Length]);
        assert_eq!(audio, vec![(Key::Source, Value::Text("Navidrome".into()))]);
    }

    #[test]
    fn an_albums_facts_in_the_cores_order() {
        assert_eq!(
            item_facts(&page()),
            [
                (FactKey::Genre, "Ambient".to_owned()),
                (FactKey::Released, "2024-03-15".to_owned())
            ]
        );
    }

    #[test]
    fn gains_as_tags_give_them() {
        assert_eq!(decibels(-6.2), "\u{2212}6.20 dB");
        assert_eq!(decibels(1.5), "+1.50 dB");
        assert_eq!(decibels(0.0), "0.00 dB");
        assert_eq!(decibels(-0.001), "0.00 dB");
    }

    #[test]
    fn the_year_of_a_release_date() {
        assert_eq!(year("2011-05-03"), "2011");
        assert_eq!(year("2011"), "2011");
        assert_eq!(year(" 1999-12-31T00:00:00Z "), "1999");
        // Not a year at the start: as tagged.
        assert_eq!(year("20110503"), "20110503");
        assert_eq!(year("Spring 2011"), "Spring 2011");
    }
}
