//! Public value types: configuration, queue items, sources, ReplayGain data, events and errors.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::anchor::PlaybackAnchor;

/// How the engine is set up. `Default` plays to the system's default device with a
/// 200 ms output buffer, keeps HTTP streams in memory and never refreshes their URLs.
#[derive(Clone)]
pub struct EngineConfig {
    /// Where the audio goes. [`Engine::set_output`](crate::Engine::set_output) changes it later.
    pub output: OutputKind,
    /// Size of the buffer between the engine thread and the output, in milliseconds.
    /// It bounds the audible latency of seeks and skips. Real-time sinks (device, paced WAV and
    /// null) honour it; the offline WAV sink has no buffer.
    pub buffer_ms: u32,
    /// Where HTTP streams keep what they downloaded: one capped temporary file per stream,
    /// deleted when the stream closes. Seeking back and re-reading (Ogg and MP4 headers read the
    /// end of a file, then the start again) is then served from disk instead of the network.
    /// `None` keeps only a bounded window in memory.
    pub cache_dir: Option<PathBuf>,
    /// Refreshes the URL of an HTTP item before its [`HttpSource::expires_at_ms`], and after the
    /// server answers 401, 403 or 410. Called on the stream's own thread, never on the engine
    /// or audio thread; it may block on the network.
    pub resolver: Option<Arc<dyn StreamResolver>>,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            output: OutputKind::Device { name: None },
            buffer_ms: 200,
            cache_dir: None,
            resolver: None,
        }
    }
}

impl fmt::Debug for EngineConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineConfig")
            .field("output", &self.output)
            .field("buffer_ms", &self.buffer_ms)
            .field("cache_dir", &self.cache_dir)
            .field("resolver", &self.resolver.as_ref().map(|_| "<resolver>"))
            .finish()
    }
}

impl PartialEq for EngineConfig {
    fn eq(&self, other: &Self) -> bool {
        let same_resolver = match (&self.resolver, &other.resolver) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        self.output == other.output
            && self.buffer_ms == other.buffer_ms
            && self.cache_dir == other.cache_dir
            && same_resolver
    }
}

/// Gives a fresh [`HttpSource`] for a queue item whose URL expired or was refused. The core
/// implements it over the plugin that owns the item.
pub trait StreamResolver: Send + Sync {
    /// Resolves the item with this [`PlayItem::id`] again. An error leaves the old URL in use
    /// while it is still valid; after a refusal the item fails with the message.
    fn resolve(&self, id: u64) -> Result<HttpSource, String>;
}

/// The output the engine renders to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputKind {
    /// An audio device through cpal. `None` follows the system default output: when the
    /// default changes ([`Engine::devices_changed`](crate::Engine::devices_changed)) or the
    /// stream fails, the engine reopens the default. A name matches a device's
    /// [`id`](crate::DeviceInfo::id) or its [`name`](crate::DeviceInfo::name). Needs the
    /// `device` feature (on by default); without it, creating the engine fails.
    Device { name: Option<String> },
    /// A 32-bit float WAV file. It takes the sample rate of the first track played (later tracks
    /// are resampled to it). With `realtime: false` the engine renders as fast as it can, which
    /// is what offline renders and tests want; with `realtime: true` it is paced like a device.
    WavFile { path: PathBuf, realtime: bool },
    /// Discards the audio. With `realtime: true` it is paced like a device.
    Null { realtime: bool },
}

/// Where a queue item's audio comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaSource {
    /// A local file.
    File(PathBuf),
    /// A resource fetched with HTTP range requests.
    Http(HttpSource),
}

/// An HTTP(S) resource, as a source plugin resolves it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpSource {
    /// `http` or `https` URL. It may carry credentials in its query; logs and errors show only
    /// scheme, host and path.
    pub url: String,
    /// Extra request headers (for example `Authorization`). Values are never logged.
    pub headers: Vec<(String, String)>,
    /// When the URL stops working, in milliseconds since the Unix epoch. The stream asks the
    /// [`StreamResolver`] for a new one shortly before.
    pub expires_at_ms: Option<i64>,
    /// Size in bytes, when known in advance (the server's answer wins).
    pub size: Option<u64>,
    /// MIME type, when known (a hint for the demuxer, like a file extension).
    pub mime: Option<String>,
}

impl HttpSource {
    /// A source without headers, expiry or hints.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: Vec::new(),
            expires_at_ms: None,
            size: None,
            mime: None,
        }
    }

    /// The URL without query and fragment, safe to show in logs and messages.
    pub fn redacted_url(&self) -> &str {
        redact_url(&self.url)
    }
}

impl fmt::Debug for HttpSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header_names: Vec<&str> = self.headers.iter().map(|(name, _)| name.as_str()).collect();
        f.debug_struct("HttpSource")
            .field("url", &self.redacted_url())
            .field("headers", &header_names)
            .field("expires_at_ms", &self.expires_at_ms)
            .field("size", &self.size)
            .field("mime", &self.mime)
            .finish()
    }
}

/// `scheme://host/path` of a URL: everything before `?` or `#`.
pub(crate) fn redact_url(url: &str) -> &str {
    url.find(['?', '#']).map_or(url, |at| &url[..at])
}

/// One entry of the engine's play queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayItem {
    /// Caller-chosen identifier, echoed in events. The engine never interprets it.
    pub id: u64,
    /// Where the audio comes from.
    pub source: MediaSource,
    /// ReplayGain tags for the file, if the library read any.
    pub replaygain: Option<ReplayGainInfo>,
    /// An album key the core fills (any string that is equal for tracks of one album). With
    /// [`CrossfadeSettings::album_continuity`], consecutive items with the same key join
    /// gaplessly instead of crossfading.
    pub album: Option<String>,
}

impl PlayItem {
    /// A local file without ReplayGain data or album key.
    pub fn new(id: u64, path: impl Into<PathBuf>) -> Self {
        Self {
            id,
            source: MediaSource::File(path.into()),
            replaygain: None,
            album: None,
        }
    }

    /// An HTTP resource without ReplayGain data or album key.
    pub fn http(id: u64, source: HttpSource) -> Self {
        Self {
            id,
            source: MediaSource::Http(source),
            replaygain: None,
            album: None,
        }
    }

    /// The same item with an album key.
    pub fn with_album(mut self, album: impl Into<String>) -> Self {
        self.album = Some(album.into());
        self
    }

    /// Same id and same resource: the identity `set_upcoming` uses to decide whether an item
    /// that is already prepared can be kept. An HTTP URL counts without its query, which a
    /// re-resolved URL usually changes.
    pub(crate) fn same_as(&self, other: &PlayItem) -> bool {
        self.id == other.id
            && match (&self.source, &other.source) {
                (MediaSource::File(a), MediaSource::File(b)) => a == b,
                (MediaSource::Http(a), MediaSource::Http(b)) => a.redacted_url() == b.redacted_url(),
                _ => false,
            }
    }

    /// Whether `next` continues this item's album (both carry the same key).
    pub(crate) fn same_album(&self, next: &PlayItem) -> bool {
        matches!((&self.album, &next.album), (Some(a), Some(b)) if a == b)
    }
}

/// How automatic transitions between tracks sound. Manual skips never crossfade. Missing
/// fields deserialize to their defaults, so saved settings keep loading as fields are added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct CrossfadeSettings {
    /// Crossfade automatic transitions. Off joins every track gaplessly.
    pub enabled: bool,
    /// Overlap length, 1000 to 12000 ms (values outside are clamped). A transition never
    /// overlaps more than half of either track, so a short track keeps its middle.
    pub duration_ms: u32,
    /// Two consecutive items with the same [`PlayItem::album`] key join gaplessly instead.
    pub album_continuity: bool,
}

impl CrossfadeSettings {
    /// Shortest and longest overlap, in milliseconds.
    pub const DURATION_RANGE_MS: std::ops::RangeInclusive<u32> = 1_000..=12_000;
}

impl Default for CrossfadeSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            duration_ms: 6_000,
            album_continuity: true,
        }
    }
}

/// ReplayGain 2.0 values as read from tags. Gains in dB, peaks as linear sample peaks
/// (1.0 = full scale).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ReplayGainInfo {
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
}

/// Which ReplayGain value to apply.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplayGainMode {
    /// No ReplayGain.
    #[default]
    Off,
    /// Track gain (falls back to album gain when the track has none).
    Track,
    /// Album gain (falls back to track gain when the album has none).
    Album,
}

/// Facts about an opened track, sent with [`EngineEvent::TrackStarted`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackInfo {
    /// Symphonia's short codec name, for example `flac`, `mp3`, `aac`, `opus`, `vorbis` or
    /// `pcm_s16le`.
    pub codec: String,
    /// Sample rate of the source, in Hz.
    pub sample_rate: u32,
    /// Channel count of the source (the engine mixes everything to stereo).
    pub channels: u16,
    /// Playable duration, without encoder delay and padding, when the container states it.
    pub duration_us: Option<i64>,
    /// Whether joins into and out of this track are sample-exact: true for FLAC, PCM (WAV,
    /// AIFF), ALAC, Vorbis and Opus; for MP3 with a LAME/Xing header that states delay and
    /// padding; and for AAC in MP4 when `iTunSMPB` or an edit list (`elst`) states them.
    pub gapless: bool,
    /// Bit rate in bits per second, when known: exact for PCM, otherwise the average over the
    /// file (or the MP4 media data), which includes container overhead.
    pub bitrate: Option<u32>,
    /// Bits per sample of the source, for formats that have them (PCM, FLAC, ALAC); `None`
    /// for lossy codecs, which decode to floating point.
    #[serde(default)]
    pub bits_per_sample: Option<u32>,
}

/// What the engine reports. Read them from [`Engine::events`](crate::Engine::events).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EngineEvent {
    /// A track became the audible one: on an explicit start (play, skip, `set_queue`) once it
    /// is open, and on a gapless join or crossfade when its first sample reaches the speakers.
    /// `index` is the position in the queue as last set by `set_queue` and extended by
    /// `set_upcoming` (which keeps the items up to the current one, so indices never shift).
    TrackStarted { index: usize, id: u64, info: TrackInfo },
    /// A new playback anchor: on play, pause, seek, stop, track change, buffering, and at most
    /// once a second during playback when the audio clock drifts from the last anchor.
    Anchor(PlaybackAnchor),
    /// A track played to its end (its last sample was heard; during a crossfade that is after
    /// the next track started). Not sent when a track is cut short by a skip, seek to another
    /// item, stop or `set_queue`.
    TrackEnded { index: usize, id: u64 },
    /// Playback ran past the last item of the queue (or `next` was called on the last item).
    QueueEnded,
    /// The output ran dry because the stream of item `id` has no data yet (`true`), or it has
    /// enough again and playback continues (`false`). Anchors carry rate 0 in between.
    Buffering { id: u64, buffering: bool },
    /// Something failed that playback survives. With an `id` it belongs to that queue item
    /// (missing file, unsupported codec, corrupt data, HTTP error) and the engine skips to the
    /// next item. Without one it is an output problem that changed nothing: `set_output` or a
    /// default-device change could not open the new output, and the old one keeps playing.
    Error { id: Option<u64>, message: String },
    /// The output failed while playing and could not be reopened (a device unplugged with no
    /// default to fall back to, a dead audio server): playback stopped where it was heard
    /// last, and the position is kept. The next `play` reopens the configured output (or asks
    /// the sink factory for a new sink) and continues from there; if that fails too, this is
    /// sent again.
    OutputFailed { message: String },
}

/// Errors from setting up the engine or an output.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EngineError {
    #[error("audio device error: {0}")]
    Device(String),
    #[error("audio output device not found: {0}")]
    DeviceNotFound(String),
    #[error("device output is not available: jf-engine was built without the `device` feature")]
    DeviceSupportDisabled,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WAV output error: {0}")]
    Wav(String),
    #[error("cannot open {path}: {message}")]
    Open { path: PathBuf, message: String },
    #[error("cannot stream {url}: {message}")]
    Stream { url: String, message: String },
    #[error("unsupported media: {0}")]
    Unsupported(String),
    #[error("decode error: {0}")]
    Decode(String),
    #[error("seek failed: {0}")]
    Seek(String),
    #[error("resampler error: {0}")]
    Resample(String),
    #[error("the engine thread is not running")]
    Disconnected,
}

impl From<hound::Error> for EngineError {
    fn from(err: hound::Error) -> Self {
        match err {
            hound::Error::IoError(io) => EngineError::Io(io),
            other => EngineError::Wav(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_shown_without_their_query() {
        let source = HttpSource {
            headers: vec![("Authorization".into(), "Bearer secret".into())],
            ..HttpSource::new("https://music.example/rest/stream?id=7&t=token#x")
        };
        assert_eq!(source.redacted_url(), "https://music.example/rest/stream");
        let shown = format!("{source:?}");
        assert!(!shown.contains("token") && !shown.contains("secret"), "{shown}");
        assert!(shown.contains("Authorization"), "{shown}");
    }

    #[test]
    fn items_match_by_id_and_resource() {
        let a = PlayItem::http(1, HttpSource::new("http://h/a.flac?t=1"));
        let refreshed = PlayItem::http(1, HttpSource::new("http://h/a.flac?t=2"));
        let other = PlayItem::http(1, HttpSource::new("http://h/b.flac?t=1"));
        assert!(a.same_as(&refreshed));
        assert!(!a.same_as(&other));
        assert!(!a.same_as(&PlayItem::new(1, "/a.flac")));
        assert!(PlayItem::new(1, "/a.flac").same_as(&PlayItem::new(1, "/a.flac")));
    }

    #[test]
    fn album_keys_must_be_present_and_equal() {
        let a = PlayItem::new(1, "/a").with_album("x");
        assert!(a.same_album(&PlayItem::new(2, "/b").with_album("x")));
        assert!(!a.same_album(&PlayItem::new(2, "/b").with_album("y")));
        assert!(!a.same_album(&PlayItem::new(2, "/b")));
        assert!(!PlayItem::new(1, "/a").same_album(&PlayItem::new(2, "/b")));
    }

    #[test]
    fn crossfade_defaults() {
        let d = CrossfadeSettings::default();
        assert_eq!(
            (d.enabled, d.duration_ms, d.album_continuity),
            (false, 6_000, true)
        );
    }

    #[test]
    fn crossfade_settings_load_with_missing_fields() {
        let partial: CrossfadeSettings = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
        assert_eq!(
            partial,
            CrossfadeSettings {
                enabled: true,
                ..CrossfadeSettings::default()
            }
        );
        let empty: CrossfadeSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, CrossfadeSettings::default());
    }
}
