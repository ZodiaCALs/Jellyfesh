//! Conversions between the types wasmtime's `bindgen!` generates from
//! `plugins/wit` ([`crate::bindings`]) and the plain mirrors in
//! `jf_plugin_model::types`, which are the host's public types.
//!
//! Arguments go to the guest as generated types (`From<model> for wit`),
//! answers come back as model types (`From<wit> for model`). Every field is
//! named, so a WIT change that the mirror does not follow stops compiling
//! here.

use jf_plugin_model::types as model;

use crate::bindings::appearance::exports::jellyfesh::plugin::appearance;
use crate::bindings::comments::exports::jellyfesh::plugin::comments;
use crate::bindings::details::exports::jellyfesh::plugin::details;
use crate::bindings::discover::exports::jellyfesh::plugin::discover;
use crate::bindings::exports::jellyfesh::plugin::{auth, catalog, lyrics, meta, playback};
use crate::bindings::jellyfesh::plugin::{crypto, types};
use crate::bindings::maker::exports::jellyfesh::plugin::maker;
use crate::bindings::maker::jellyfesh::plugin::{audio, lyrics as maker_lyrics, speech};
use crate::bindings::remote::exports::jellyfesh::plugin::remote;

/// `From` both ways between two enums whose variants carry no data.
macro_rules! same_enum {
    ($wit:path, $model:path, [$($variant:ident),+ $(,)?]) => {
        impl From<$model> for $wit {
            fn from(value: $model) -> Self {
                use $model as M;
                match value {
                    $(M::$variant => Self::$variant,)+
                }
            }
        }
        impl From<$wit> for $model {
            fn from(value: $wit) -> Self {
                use $wit as W;
                match value {
                    $(W::$variant => Self::$variant,)+
                }
            }
        }
    };
}

same_enum!(
    types::ItemKind,
    model::ItemKind,
    [Track, Album, Artist, Playlist, Folder]
);
same_enum!(
    types::UnplayableReason,
    model::UnplayableReason,
    [Paywall, Region, Unavailable, Other]
);
/// What a plugin reports it can do. The WIT enum is 1.0's (`remote` is a
/// manifest capability only, ADR-0018), so this goes one way.
impl From<meta::Capability> for model::Capability {
    fn from(value: meta::Capability) -> Self {
        use meta::Capability as W;
        match value {
            W::Auth => Self::Auth,
            W::Search => Self::Search,
            W::Browse => Self::Browse,
            W::ResolveStream => Self::ResolveStream,
            W::Lyrics => Self::Lyrics,
            W::Artwork => Self::Artwork,
            W::Recommendations => Self::Recommendations,
        }
    }
}
same_enum!(
    auth::AuthKind,
    model::AuthKind,
    [Qr, Oauth, CookieImport, PasswordExchange, ApiKey]
);
same_enum!(
    auth::FieldKind,
    model::FieldKind,
    [Text, Email, Url, Number, CookieHeader]
);
same_enum!(auth::RedirectCapture, model::RedirectCapture, [None, Loopback]);
same_enum!(playback::Quality, model::Quality, [Low, Normal, High, Lossless]);
same_enum!(
    lyrics::LyricFormat,
    model::LyricFormat,
    [Plain, Lrc, Elrc, Ttml, Vtt, JsonSyllables]
);
same_enum!(
    crypto::HashAlgorithm,
    model::HashAlgorithm,
    [Md5, Sha1, Sha256, Sha512]
);
same_enum!(
    crypto::RsaPadding,
    model::RsaPadding,
    [Pkcs1v15, OaepSha1, OaepSha256]
);

fn list<A, B: From<A>>(items: Vec<A>) -> Vec<B> {
    items.into_iter().map(B::from).collect()
}

// ---- types -----------------------------------------------------------------------------------

impl From<model::Header> for types::Header {
    fn from(h: model::Header) -> Self {
        types::Header {
            name: h.name,
            value: h.value,
        }
    }
}

impl From<types::Header> for model::Header {
    fn from(h: types::Header) -> Self {
        model::Header {
            name: h.name,
            value: h.value,
        }
    }
}

impl From<model::Unplayable> for types::Unplayable {
    fn from(u: model::Unplayable) -> Self {
        types::Unplayable {
            reason: u.reason.into(),
            message: u.message,
        }
    }
}

impl From<types::Unplayable> for model::Unplayable {
    fn from(u: types::Unplayable) -> Self {
        model::Unplayable {
            reason: u.reason.into(),
            message: u.message,
        }
    }
}

impl From<model::ImageRef> for types::ImageRef {
    fn from(i: model::ImageRef) -> Self {
        types::ImageRef {
            url: i.url,
            headers: list(i.headers),
            width: i.width,
            height: i.height,
        }
    }
}

impl From<types::ImageRef> for model::ImageRef {
    fn from(i: types::ImageRef) -> Self {
        model::ImageRef {
            url: i.url,
            headers: list(i.headers),
            width: i.width,
            height: i.height,
        }
    }
}

impl From<model::Item> for types::Item {
    fn from(i: model::Item) -> Self {
        types::Item {
            id: i.id,
            kind: i.kind.into(),
            title: i.title,
            artist: i.artist,
            album: i.album,
            album_id: i.album_id,
            artist_id: i.artist_id,
            duration_ms: i.duration_ms,
            track_number: i.track_number,
            disc_number: i.disc_number,
            year: i.year,
            child_count: i.child_count,
            isrc: i.isrc,
            artwork: i.artwork.map(Into::into),
            explicit: i.explicit,
            playable: i.playable,
            unplayable: i.unplayable.map(Into::into),
        }
    }
}

impl From<types::Item> for model::Item {
    fn from(i: types::Item) -> Self {
        model::Item {
            id: i.id,
            kind: i.kind.into(),
            title: i.title,
            artist: i.artist,
            album: i.album,
            album_id: i.album_id,
            artist_id: i.artist_id,
            duration_ms: i.duration_ms,
            track_number: i.track_number,
            disc_number: i.disc_number,
            year: i.year,
            child_count: i.child_count,
            isrc: i.isrc,
            artwork: i.artwork.map(Into::into),
            explicit: i.explicit,
            playable: i.playable,
            unplayable: i.unplayable.map(Into::into),
        }
    }
}

impl From<model::PageRequest> for types::PageRequest {
    fn from(p: model::PageRequest) -> Self {
        types::PageRequest {
            cursor: p.cursor,
            limit: p.limit,
        }
    }
}

impl From<types::ItemPage> for model::ItemPage {
    fn from(p: types::ItemPage) -> Self {
        model::ItemPage {
            items: list(p.items),
            next: p.next,
            total: p.total,
        }
    }
}

impl From<types::PluginError> for model::PluginError {
    fn from(e: types::PluginError) -> Self {
        use types::PluginError as W;
        match e {
            W::Unsupported => Self::Unsupported,
            W::AuthRequired => Self::AuthRequired,
            W::NotFound => Self::NotFound,
            W::InvalidInput(text) => Self::InvalidInput(text),
            W::RateLimited => Self::RateLimited,
            W::Network(text) => Self::Network(text),
            W::Service(text) => Self::Service(text),
            W::Internal(text) => Self::Internal(text),
        }
    }
}

// ---- auth ------------------------------------------------------------------------------------

impl From<model::OauthRequest> for auth::OauthRequest {
    fn from(o: model::OauthRequest) -> Self {
        auth::OauthRequest {
            redirect_uri: o.redirect_uri,
            code_challenge: o.code_challenge,
            code_challenge_method: o.code_challenge_method,
            state: o.state,
        }
    }
}

impl From<model::Flow> for auth::Flow {
    fn from(f: model::Flow) -> Self {
        auth::Flow {
            id: f.id,
            kind: f.kind.into(),
            state: f.state,
            oauth: f.oauth.map(Into::into),
        }
    }
}

impl From<model::FieldValue> for auth::FieldValue {
    fn from(f: model::FieldValue) -> Self {
        auth::FieldValue {
            name: f.name,
            value: f.value,
        }
    }
}

impl From<auth::FieldSpec> for model::FieldSpec {
    fn from(s: auth::FieldSpec) -> Self {
        model::FieldSpec {
            name: s.name,
            label: s.label,
            kind: s.kind.into(),
            secret: s.secret,
            required: s.required,
            placeholder: s.placeholder,
            help: s.help,
        }
    }
}

impl From<auth::Step> for model::Step {
    fn from(step: auth::Step) -> Self {
        use auth::Step as W;
        match step {
            W::ShowQr(qr) => Self::ShowQr(model::QrCode {
                data: qr.data,
                hint: qr.hint,
                expires_at_ms: qr.expires_at_ms,
            }),
            W::ShowCode(code) => Self::ShowCode(model::CodeDisplay {
                code: code.code,
                hint: code.hint,
                expires_at_ms: code.expires_at_ms,
            }),
            W::OpenUrl(open) => Self::OpenUrl(model::OpenUrl {
                url: open.url,
                capture: open.capture.into(),
            }),
            W::AskFields(specs) => Self::AskFields(list(specs)),
            W::Waiting(text) => Self::Waiting(text),
            W::Done(account) => Self::Done(account),
            W::Failed(message) => Self::Failed(message),
        }
    }
}

impl From<auth::FlowStep> for model::FlowStep {
    fn from(s: auth::FlowStep) -> Self {
        model::FlowStep {
            step: s.step.into(),
            state: s.state,
            poll_after_ms: s.poll_after_ms,
        }
    }
}

// ---- catalog, playback, lyrics ----------------------------------------------------------------

impl From<model::SearchFilters> for catalog::SearchFilters {
    fn from(f: model::SearchFilters) -> Self {
        catalog::SearchFilters { kinds: list(f.kinds) }
    }
}

impl From<playback::StreamInfo> for model::StreamInfo {
    fn from(s: playback::StreamInfo) -> Self {
        model::StreamInfo {
            url: s.url,
            headers: list(s.headers),
            expires_at_ms: s.expires_at_ms,
            codec: s.codec,
            bitrate_kbps: s.bitrate_kbps,
            cacheable: s.cacheable,
        }
    }
}

impl From<lyrics::LyricPayload> for model::LyricPayload {
    fn from(p: lyrics::LyricPayload) -> Self {
        model::LyricPayload {
            format: p.format.into(),
            text: p.text,
            language: p.language,
            attribution: p.attribution,
        }
    }
}

// ---- crypto (the import's arguments arrive generated) -----------------------------------------

impl From<crypto::AesMode> for model::AesMode {
    fn from(mode: crypto::AesMode) -> Self {
        match mode {
            crypto::AesMode::Cbc(iv) => Self::Cbc(iv),
            crypto::AesMode::Gcm(params) => Self::Gcm(model::GcmParams {
                nonce: params.nonce,
                aad: params.aad,
            }),
            crypto::AesMode::Ecb => Self::Ecb,
        }
    }
}

impl From<crypto::RsaPublicKey> for model::RsaPublicKey {
    fn from(key: crypto::RsaPublicKey) -> Self {
        match key {
            crypto::RsaPublicKey::Pem(text) => Self::Pem(text),
            crypto::RsaPublicKey::Der(der) => Self::Der(der),
            crypto::RsaPublicKey::Components(c) => Self::Components(model::RsaComponents {
                modulus: c.modulus,
                exponent: c.exponent,
            }),
        }
    }
}

impl From<model::CryptoError> for crypto::CryptoError {
    fn from(e: model::CryptoError) -> Self {
        match e {
            model::CryptoError::InvalidKey(text) => Self::InvalidKey(text),
            model::CryptoError::InvalidInput(text) => Self::InvalidInput(text),
            model::CryptoError::DecryptFailed => Self::DecryptFailed,
            model::CryptoError::TooLarge => Self::TooLarge,
        }
    }
}

same_enum!(
    remote::DeviceKind,
    model::RemoteDeviceKind,
    [Computer, Smartphone, Speaker, Tv, Car, Other]
);
same_enum!(remote::Repeat, model::RemoteRepeat, [Off, All, One]);

impl From<remote::Device> for model::RemoteDevice {
    fn from(d: remote::Device) -> Self {
        model::RemoteDevice {
            id: d.id,
            name: d.name,
            kind: d.kind.into(),
            active: d.active,
            supports_volume: d.supports_volume,
            volume_percent: d.volume_percent,
        }
    }
}

impl From<remote::PlaybackState> for model::RemoteState {
    fn from(s: remote::PlaybackState) -> Self {
        model::RemoteState {
            device: s.device.map(Into::into),
            item: s.item.map(Into::into),
            context: s.context.map(Into::into),
            playing: s.playing,
            position_ms: s.position_ms,
            shuffle: s.shuffle,
            repeat: s.repeat.into(),
        }
    }
}

// ---- API 1.2: audio and speech (the imports' answers go out generated) -------------------------

impl From<model::AudioError> for audio::AudioError {
    fn from(e: model::AudioError) -> Self {
        match e {
            model::AudioError::NotDeclared => Self::NotDeclared,
            model::AudioError::NoConsent(origins) => Self::NoConsent(origins),
            model::AudioError::Unavailable(text) => Self::Unavailable(text),
            model::AudioError::Cancelled => Self::Cancelled,
        }
    }
}

impl From<model::SpeechError> for speech::SpeechError {
    fn from(e: model::SpeechError) -> Self {
        match e {
            model::SpeechError::NotDeclared => Self::NotDeclared,
            model::SpeechError::ModelMissing(id) => Self::ModelMissing(id),
            model::SpeechError::Unavailable(text) => Self::Unavailable(text),
            model::SpeechError::Cancelled => Self::Cancelled,
            model::SpeechError::Failed(text) => Self::Failed(text),
        }
    }
}

impl From<model::SpeechModel> for speech::Model {
    fn from(m: model::SpeechModel) -> Self {
        speech::Model {
            id: m.id,
            name: m.name,
            bytes: m.bytes,
            ready: m.ready,
        }
    }
}

impl From<model::Transcript> for speech::Transcript {
    fn from(t: model::Transcript) -> Self {
        speech::Transcript {
            language: t.language,
            segments: t
                .segments
                .into_iter()
                .map(|s| speech::Segment {
                    text: s.text,
                    start_ms: s.start_ms,
                    end_ms: s.end_ms,
                    words: s
                        .words
                        .into_iter()
                        .map(|w| speech::Word {
                            text: w.text,
                            start_ms: w.start_ms,
                            end_ms: w.end_ms,
                            confidence: w.confidence,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

// ---- API 1.2: maker ----------------------------------------------------------------------------

same_enum!(maker::Mode, model::MakerMode, [OnDevice, Online]);
same_enum!(
    maker_lyrics::LyricFormat,
    model::LyricFormat,
    [Plain, Lrc, Elrc, Ttml, Vtt, JsonSyllables]
);

impl From<maker::Need> for model::MakerNeed {
    fn from(n: maker::Need) -> Self {
        match n {
            maker::Need::Nothing => Self::Nothing,
            maker::Need::SignIn => Self::SignIn,
            maker::Need::Model(id) => Self::Model(id),
            maker::Need::Unavailable(text) => Self::Unavailable(text),
        }
    }
}

impl From<maker::MakerPlan> for model::MakerPlan {
    fn from(p: maker::MakerPlan) -> Self {
        model::MakerPlan {
            mode: p.mode.into(),
            engine: p.engine,
            upload_to: p.upload_to,
            need: p.need.into(),
            automatic: p.automatic,
        }
    }
}

impl From<maker::Made> for model::Made {
    fn from(m: maker::Made) -> Self {
        model::Made {
            lyrics: model::LyricPayload {
                format: m.lyrics.format.into(),
                text: m.lyrics.text,
                language: m.lyrics.language,
                attribution: m.lyrics.attribution,
            },
            engine: m.engine,
        }
    }
}

/// The song of a `maker.make` call, with the handle the host gave it.
pub(crate) fn song(song: model::MakeSong, track: u64) -> maker::Song {
    maker::Song {
        track,
        title: song.title,
        artist: song.artist,
        album: song.album,
        duration_ms: song.duration_ms,
        text: song.text,
        language: song.language,
    }
}

// ---- API 1.2: appearance -------------------------------------------------------------------------

same_enum!(
    appearance::Typeface,
    model::Typeface,
    [System, Rounded, Serif, Mono]
);
same_enum!(appearance::Density, model::Density, [Compact, Standard, Roomy]);
same_enum!(
    appearance::LyricsWeight,
    model::LyricsWeight,
    [Regular, Medium, Semibold, Bold, Heavy]
);
same_enum!(
    appearance::Place,
    model::Place,
    [
        Home, Albums, Artists, Songs, Playlists, Favourites, Search, Sources
    ]
);
same_enum!(
    appearance::HomeSection,
    model::HomeSection,
    [Summary, RecentlyAdded, RecentlyPlayed, MostPlayed, Favourites]
);
same_enum!(
    appearance::NowPlayingLayout,
    model::NowPlayingLayout,
    [Stage, Artwork, Lyrics]
);
same_enum!(
    appearance::MiniPlayerStyle,
    model::MiniPlayerStyle,
    [Bar, Compact, Floating]
);

impl From<appearance::Highlight> for model::Highlight {
    fn from(h: appearance::Highlight) -> Self {
        match h {
            appearance::Highlight::Artwork => Self::Artwork,
            appearance::Highlight::Accent => Self::Accent,
            appearance::Highlight::Custom(rgb) => Self::Custom(rgb),
        }
    }
}

impl From<appearance::AppearanceLook> for model::Look {
    fn from(l: appearance::AppearanceLook) -> Self {
        model::Look {
            corner_scale: l.corner_scale,
            glass: l.glass,
            aurora: l.aurora,
            typeface: l.typeface.map(Into::into),
            text_scale: l.text_scale,
            density: l.density.map(Into::into),
            lyrics_weight: l.lyrics_weight.map(Into::into),
            lyrics_glow: l.lyrics_glow,
            lyrics_highlight: l.lyrics_highlight.map(Into::into),
            sidebar: l.sidebar.map(list),
            home: l.home.map(list),
            now_playing: l.now_playing.map(Into::into),
            mini_player: l.mini_player.map(Into::into),
        }
    }
}

// ---- API 1.2: discover, details, comments --------------------------------------------------------

same_enum!(discover::ShelfStyle, model::ShelfStyle, [Row, Grid, Chart]);

impl From<discover::Shelf> for model::Shelf {
    fn from(s: discover::Shelf) -> Self {
        model::Shelf {
            id: s.id,
            title: s.title,
            style: s.style.into(),
            items: list(s.items),
            more: s.more,
        }
    }
}

impl From<details::FactKey> for model::FactKey {
    fn from(k: details::FactKey) -> Self {
        use details::FactKey as W;
        match k {
            W::Version => Self::Version,
            W::Genre => Self::Genre,
            W::Language => Self::Language,
            W::Released => Self::Released,
            W::Label => Self::Label,
            W::Composer => Self::Composer,
            W::Lyricist => Self::Lyricist,
            W::Arranger => Self::Arranger,
            W::Producer => Self::Producer,
            W::Popularity => Self::Popularity,
            W::Quality => Self::Quality,
            W::Other(label) => Self::Other(label),
        }
    }
}

impl From<details::ItemDetails> for model::ItemDetails {
    fn from(d: details::ItemDetails) -> Self {
        model::ItemDetails {
            description: d.description,
            image: d.image.map(Into::into),
            facts: d
                .facts
                .into_iter()
                .map(|f| model::Fact {
                    key: f.key.into(),
                    value: f.value,
                })
                .collect(),
            comment_count: d.comment_count,
        }
    }
}

impl From<comments::Comment> for model::Comment {
    fn from(c: comments::Comment) -> Self {
        model::Comment {
            id: c.id,
            author: c.author,
            avatar: c.avatar.map(Into::into),
            text: c.text,
            likes: c.likes,
            time_ms: c.time_ms,
            replies: c.replies,
        }
    }
}

impl From<comments::CommentPage> for model::CommentPage {
    fn from(p: comments::CommentPage) -> Self {
        model::CommentPage {
            hot: list(p.hot),
            comments: list(p.comments),
            total: p.total,
            next: p.next,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_survive_the_round_trip() {
        let item = model::Item {
            id: "t1".into(),
            kind: model::ItemKind::Track,
            title: "Song".into(),
            artist: Some("Artist".into()),
            album: Some("Album".into()),
            album_id: Some("a1".into()),
            artist_id: None,
            duration_ms: Some(1000),
            track_number: Some(3),
            disc_number: Some(1),
            year: Some(2026),
            child_count: None,
            isrc: Some("USRC17607839".into()),
            artwork: Some(model::ImageRef {
                url: "https://music.example.com/art".into(),
                headers: vec![model::Header {
                    name: "x".into(),
                    value: "y".into(),
                }],
                width: Some(300),
                height: None,
            }),
            explicit: true,
            playable: false,
            unplayable: Some(model::Unplayable {
                reason: model::UnplayableReason::Region,
                message: Some("Not here".into()),
            }),
        };
        let wit: types::Item = item.clone().into();
        assert_eq!(model::Item::from(wit), item);
        assert_eq!(
            model::Capability::from(meta::Capability::ResolveStream),
            model::Capability::ResolveStream
        );
        assert_eq!(
            model::RemoteRepeat::from(remote::Repeat::from(model::RemoteRepeat::One)),
            model::RemoteRepeat::One
        );
    }
}
