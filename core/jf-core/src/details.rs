//! Details and comments (ADR-0022 §3–4): About This Song, album and artist
//! notes, technical information with the core's quality label, and comments
//! from sources.
//!
//! Library items are described from the catalogue and the file's own tags
//! (read on a helper thread); source items by their plugin's `details` and
//! `comments` exports. Answers go out as `Event::Details` and
//! `Event::Comments`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use jf_library::FileDetails;
use jf_plugin_host::{self as host, CallError, PluginHost};

use crate::actor::Shared;
use crate::types::{
    ArtworkRef, AudioFormat, CommandId, CommentEntry, CommentsPage, DetailsPage, Event, Fact, FactKey,
    ItemRef, QualityLabel, ReplayGainValues, TechnicalInfo,
};

/// How long a plugin may take to describe an item.
const CALL_WAIT: Duration = Duration::from_secs(25);

/// Whether a codec keeps every bit (`None` when the codec is unknown).
pub(crate) fn lossless(codec: &str) -> Option<bool> {
    match codec {
        "flac" | "alac" | "pcm" | "pcm_float" | "wav" | "aiff" | "ape" | "wavpack" | "tta" => Some(true),
        "mp3" | "mp2" | "mp1" | "aac" | "vorbis" | "opus" | "musepack" | "speex" => Some(false),
        _ => None,
    }
}

/// The core's quality label (ADR-0022 §3): Hi-Res Lossless for lossless
/// audio above 16 bits or 48 kHz, Lossless, High for lossy audio at 256
/// kbit/s or more, Standard below. `None` when the format is not known well
/// enough to say.
pub(crate) fn quality(
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

/// An audio format with its quality label.
pub(crate) fn format(
    codec: Option<&str>,
    sample_rate_hz: Option<u32>,
    bit_depth: Option<u32>,
    channels: Option<u32>,
    bitrate_kbps: Option<u32>,
) -> AudioFormat {
    let lossless = codec.and_then(lossless);
    AudioFormat {
        codec: codec.map(str::to_owned),
        sample_rate_hz,
        // Lossy codecs have no bit depth of their own.
        bit_depth: bit_depth.filter(|_| lossless != Some(false)),
        channels,
        bitrate_kbps,
        lossless,
        quality: quality(lossless, bit_depth, sample_rate_hz, bitrate_kbps),
    }
}

/// What the engine decoded, with a source's hints filling in what it does
/// not say (ADR-0026 §1: a stream's codec and nominal bitrate).
pub(crate) fn with_hints(
    decoded: AudioFormat,
    codec: Option<&str>,
    bitrate_kbps: Option<u32>,
) -> AudioFormat {
    let codec = decoded
        .codec
        .clone()
        .or_else(|| codec.map(|c| c.trim().to_ascii_lowercase()))
        .filter(|c| !c.is_empty());
    let bitrate_kbps = decoded.bitrate_kbps.or(bitrate_kbps.filter(|&k| k > 0));
    if codec == decoded.codec && bitrate_kbps == decoded.bitrate_kbps {
        return decoded;
    }
    format(
        codec.as_deref(),
        decoded.sample_rate_hz,
        decoded.bit_depth,
        decoded.channels,
        bitrate_kbps,
    )
}

/// What the engine decoded, as an audio format with its quality label.
pub(crate) fn engine_format(info: &jf_engine::TrackInfo) -> AudioFormat {
    // Symphonia names PCM by its sample layout: pcm_s16le, pcm_f32le, ...
    let codec = if info.codec.starts_with("pcm") {
        "pcm"
    } else {
        info.codec.as_str()
    };
    format(
        Some(codec),
        Some(info.sample_rate).filter(|&r| r > 0),
        info.bits_per_sample,
        Some(u32::from(info.channels)).filter(|&c| c > 0),
        info.bitrate.map(|bps| bps / 1000).filter(|&k| k > 0),
    )
}

/// Technical information about a file from its tags and properties.
pub(crate) fn technical(details: &FileDetails, path: &Path) -> TechnicalInfo {
    let replaygain = details.replaygain;
    let has_gain = replaygain.track_gain_db.is_some() || replaygain.album_gain_db.is_some();
    TechnicalInfo {
        format: format(
            details.codec.as_deref(),
            details.sample_rate,
            details.bit_depth.map(u32::from),
            details.channels.map(u32::from),
            details.bitrate_kbps,
        ),
        container: details.container.clone(),
        file_size_bytes: details.file_size,
        duration_ms: details.duration_ms,
        replaygain: has_gain.then_some(ReplayGainValues {
            track_gain_db: replaygain.track_gain_db,
            track_peak: replaygain.track_peak,
            album_gain_db: replaygain.album_gain_db,
            album_peak: replaygain.album_peak,
        }),
        path: Some(path.to_owned()),
    }
}

/// "3/12", "3" or `None`.
fn of(number: Option<u32>, total: Option<u32>) -> Option<String> {
    match (number, total) {
        (Some(n), Some(t)) if t >= n => Some(format!("{n}/{t}")),
        (Some(n), _) => Some(n.to_string()),
        _ => None,
    }
}

/// The facts a file's tags give, in the order the apps show them.
pub(crate) fn file_facts(details: &FileDetails) -> Vec<Fact> {
    let pairs = [
        (FactKey::Version, details.subtitle.clone()),
        (FactKey::Genre, details.genre.clone()),
        (FactKey::Released, details.released.clone()),
        (FactKey::Label, details.label.clone()),
        (FactKey::Composer, details.composer.clone()),
        (FactKey::Lyricist, details.lyricist.clone()),
        (FactKey::Arranger, details.arranger.clone()),
        (FactKey::Producer, details.producer.clone()),
        (FactKey::Language, details.language.clone()),
        (FactKey::Disc, of(details.disc_no, details.disc_total)),
        (FactKey::Track, of(details.track_no, details.track_total)),
        (FactKey::Isrc, details.isrc.clone()),
    ];
    pairs
        .into_iter()
        .filter_map(|(key, value)| {
            let value = value?.trim().to_owned();
            (!value.is_empty()).then_some(Fact { key, value })
        })
        .collect()
}

/// A library song or a file, described from its tags.
#[derive(Debug, Clone)]
pub(crate) struct LocalSong {
    pub item: ItemRef,
    pub path: PathBuf,
    /// The catalogue's title and artist, when it is in the library.
    pub title: Option<String>,
    pub artist: Option<String>,
    pub image: Option<PathBuf>,
}

/// Describes a library song or a file on a helper thread and sends
/// `Event::Details`.
pub(crate) fn local_song(shared: Arc<Shared>, request: CommandId, song: LocalSong) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("jf-details".into())
        .spawn(move || {
            let page = describe_file(request, &song);
            shared.emit(Event::Details(Arc::new(page)));
        })
        .map(|_| ())
}

pub(crate) fn describe_file(request: CommandId, song: &LocalSong) -> DetailsPage {
    let fallback_title = song
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match jf_library::read_file_details(&song.path) {
        Ok(details) => DetailsPage {
            request,
            item: song.item.clone(),
            title: song
                .title
                .clone()
                .or_else(|| details.title.clone())
                .unwrap_or(fallback_title),
            subtitle: song.artist.clone().or_else(|| details.artist.clone()),
            description: None,
            image: song.image.clone().map(ArtworkRef::File),
            facts: file_facts(&details),
            technical: Some(technical(&details, &song.path)),
            comment_count: None,
            attribution: None,
            error: None,
        },
        Err(e) => DetailsPage {
            request,
            item: song.item.clone(),
            title: song.title.clone().unwrap_or(fallback_title),
            subtitle: song.artist.clone(),
            description: None,
            image: song.image.clone().map(ArtworkRef::File),
            facts: Vec::new(),
            technical: None,
            comment_count: None,
            attribution: None,
            error: Some(format!("the file could not be read: {e}")),
        },
    }
}

fn fact_key(key: host::FactKey) -> FactKey {
    match key {
        host::FactKey::Version => FactKey::Version,
        host::FactKey::Genre => FactKey::Genre,
        host::FactKey::Language => FactKey::Language,
        host::FactKey::Released => FactKey::Released,
        host::FactKey::Label => FactKey::Label,
        host::FactKey::Composer => FactKey::Composer,
        host::FactKey::Lyricist => FactKey::Lyricist,
        host::FactKey::Arranger => FactKey::Arranger,
        host::FactKey::Producer => FactKey::Producer,
        host::FactKey::Popularity => FactKey::Popularity,
        host::FactKey::Quality => FactKey::Quality,
        host::FactKey::Other(label) => FactKey::Other { label },
    }
}

/// A source item to describe.
#[derive(Debug, Clone)]
pub(crate) struct SourceRequest {
    pub item: ItemRef,
    pub plugin: String,
    pub host_item: host::Item,
    /// The plugin's name, for the attribution.
    pub attribution: Option<String>,
    /// For the song playing: the stream's format.
    pub technical: Option<TechnicalInfo>,
}

/// Asks a source plugin for an item's details on a helper thread and sends
/// `Event::Details`.
pub(crate) fn source_details(
    shared: Arc<Shared>,
    host: PluginHost,
    request: CommandId,
    source: SourceRequest,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("jf-details".into())
        .spawn(move || {
            let answer = host
                .details(&source.plugin, source.host_item.clone())
                .recv_timeout(CALL_WAIT)
                .unwrap_or(Err(CallError::Timeout));
            let page = source_page(request, &source, answer);
            shared.emit(Event::Details(Arc::new(page)));
        })
        .map(|_| ())
}

pub(crate) fn source_page(
    request: CommandId,
    source: &SourceRequest,
    answer: Result<host::ItemDetails, CallError>,
) -> DetailsPage {
    let item = &source.host_item;
    let listed = crate::artwork::row_picture(&source.plugin, item.kind, &item.id, item.artwork.as_ref());
    let mut page = DetailsPage {
        request,
        item: source.item.clone(),
        title: item.title.clone(),
        subtitle: item.artist.clone(),
        description: None,
        image: listed.map(ArtworkRef::Url),
        facts: Vec::new(),
        technical: source.technical.clone(),
        comment_count: None,
        attribution: source.attribution.clone(),
        error: None,
    };
    match answer {
        Ok(details) => {
            page.description = details.description;
            if let Some(image) = details.image {
                crate::artwork::remember(&image);
                page.image = Some(ArtworkRef::Url(image.url));
            }
            page.facts = details
                .facts
                .into_iter()
                .map(|f| Fact {
                    key: fact_key(f.key),
                    value: f.value,
                })
                .collect();
            page.comment_count = details.comment_count;
        }
        Err(CallError::NotDeclared(_)) => {
            page.error = Some("this source does not describe its items".into());
        }
        Err(e) => page.error = Some(e.to_string()),
    }
    page
}

fn comment(c: host::Comment) -> CommentEntry {
    if let Some(image) = &c.avatar {
        crate::artwork::remember(image);
    }
    CommentEntry {
        id: c.id,
        author: c.author,
        avatar: c.avatar.map(|image| ArtworkRef::Url(image.url)),
        text: c.text,
        likes: c.likes,
        time_ms: c.time_ms.and_then(|t| i64::try_from(t).ok()),
        replies: c.replies,
    }
}

/// Asks a source plugin for a page of comments on a helper thread and sends
/// `Event::Comments`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn source_comments(
    shared: Arc<Shared>,
    host: PluginHost,
    request: CommandId,
    item: ItemRef,
    plugin: String,
    host_item: host::Item,
    page: Option<String>,
    attribution: Option<String>,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("jf-comments".into())
        .spawn(move || {
            let answer = host
                .comments(
                    &plugin,
                    host_item,
                    host::PageRequest {
                        cursor: page.clone(),
                        limit: 50,
                    },
                )
                .recv_timeout(CALL_WAIT)
                .unwrap_or(Err(CallError::Timeout));
            let page = comments_page(request, item, page.is_none(), attribution, answer);
            shared.emit(Event::Comments(Arc::new(page)));
        })
        .map(|_| ())
}

pub(crate) fn comments_page(
    request: CommandId,
    item: ItemRef,
    first: bool,
    attribution: Option<String>,
    answer: Result<host::CommentPage, CallError>,
) -> CommentsPage {
    match answer {
        Ok(page) => CommentsPage {
            request,
            item,
            hot: if first {
                page.hot.into_iter().map(comment).collect()
            } else {
                Vec::new()
            },
            comments: page.comments.into_iter().map(comment).collect(),
            total: page.total,
            next: page.next,
            attribution,
            error: None,
        },
        Err(e) => CommentsPage {
            request,
            item,
            hot: Vec::new(),
            comments: Vec::new(),
            total: None,
            next: None,
            attribution,
            error: Some(match e {
                CallError::NotDeclared(_) => "this source has no comments".into(),
                other => other.to_string(),
            }),
        },
    }
}

/// Comments for an item no source has.
pub(crate) fn no_comments(request: CommandId, item: ItemRef) -> CommentsPage {
    CommentsPage {
        request,
        item,
        hot: Vec::new(),
        comments: Vec::new(),
        total: None,
        next: None,
        attribution: None,
        error: Some("comments come from online sources".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sources_hints_fill_in_what_the_decoder_does_not_say() {
        let decoded = format(Some("mp3"), Some(44_100), None, Some(2), None);
        let filled = with_hints(decoded, Some("mp3"), Some(320));
        assert_eq!(filled.bitrate_kbps, Some(320));
        assert_eq!(filled.quality, Some(QualityLabel::High));
        let measured = format(Some("mp3"), Some(44_100), None, Some(2), Some(318));
        assert_eq!(
            with_hints(measured.clone(), Some("mp3"), Some(320)),
            measured,
            "what the decoder says wins"
        );
        let unnamed = format(None, Some(48_000), None, Some(2), None);
        assert_eq!(
            with_hints(unnamed, Some(" Opus"), Some(160)).codec.as_deref(),
            Some("opus")
        );
    }

    #[test]
    fn quality_labels_follow_the_rules() {
        use QualityLabel::*;
        let label = |codec: &str, bits, rate, kbps| quality(lossless(codec), bits, rate, kbps);
        assert_eq!(label("flac", Some(16), Some(44_100), Some(900)), Some(Lossless));
        assert_eq!(label("flac", Some(24), Some(44_100), None), Some(HiResLossless));
        assert_eq!(label("alac", Some(16), Some(96_000), None), Some(HiResLossless));
        assert_eq!(label("pcm", Some(16), Some(48_000), None), Some(Lossless));
        assert_eq!(label("mp3", None, Some(44_100), Some(320)), Some(High));
        assert_eq!(label("aac", None, Some(44_100), Some(256)), Some(High));
        assert_eq!(label("aac", None, Some(44_100), Some(255)), Some(Standard));
        assert_eq!(label("opus", None, Some(48_000), None), Some(Standard));
        assert_eq!(label("mystery", Some(24), Some(96_000), Some(320)), None);
        let f = format(Some("mp3"), Some(44_100), Some(16), Some(2), Some(320));
        assert_eq!(f.bit_depth, None, "lossy audio has no bit depth");
        assert_eq!(f.lossless, Some(false));
    }

    #[test]
    fn facts_from_tags_in_order() {
        let details = FileDetails {
            subtitle: Some("Live".into()),
            genre: Some("Pop".into()),
            released: Some("2011-05-03".into()),
            composer: Some(" ".into()),
            lyricist: Some("A. Writer".into()),
            disc_no: Some(1),
            disc_total: Some(2),
            track_no: Some(3),
            isrc: Some("XX0000000000".into()),
            ..FileDetails::default()
        };
        let facts = file_facts(&details);
        let keys: Vec<&FactKey> = facts.iter().map(|f| &f.key).collect();
        assert_eq!(
            keys,
            [
                &FactKey::Version,
                &FactKey::Genre,
                &FactKey::Released,
                &FactKey::Lyricist,
                &FactKey::Disc,
                &FactKey::Track,
                &FactKey::Isrc
            ]
        );
        assert_eq!(facts[4].value, "1/2");
        assert_eq!(facts[5].value, "3");
    }

    #[test]
    fn source_answers_become_pages() {
        let source = SourceRequest {
            item: ItemRef::Source {
                plugin: "org.example".into(),
                item_kind: crate::types::ItemKind::Track,
                id: "t1".into(),
            },
            plugin: "org.example".into(),
            host_item: crate::sources::bare_item("t1", host::ItemKind::Track),
            attribution: Some("Example".into()),
            technical: None,
        };
        let page = source_page(
            CommandId(4),
            &source,
            Ok(host::ItemDetails {
                description: Some("Notes".into()),
                image: None,
                facts: vec![host::Fact {
                    key: host::FactKey::Other("Plays".into()),
                    value: "12".into(),
                }],
                comment_count: Some(3),
            }),
        );
        assert_eq!(page.description.as_deref(), Some("Notes"));
        assert_eq!(
            page.facts[0].key,
            FactKey::Other {
                label: "Plays".into()
            }
        );
        assert_eq!(page.comment_count, Some(3));
        assert_eq!(page.attribution.as_deref(), Some("Example"));
        let failed = source_page(
            CommandId(5),
            &source,
            Err(CallError::NotDeclared("details".into())),
        );
        assert!(failed.error.is_some());
        let comments = comments_page(
            CommandId(6),
            source.item.clone(),
            false,
            None,
            Ok(host::CommentPage {
                hot: vec![host::Comment {
                    id: "h".into(),
                    author: "A".into(),
                    avatar: None,
                    text: "hot".into(),
                    likes: None,
                    time_ms: Some(5),
                    replies: None,
                }],
                comments: Vec::new(),
                total: Some(1),
                next: None,
            }),
        );
        assert!(
            comments.hot.is_empty(),
            "hot comments come with the first page only"
        );
    }
}
