//! Conversions between the types wasmtime's `bindgen!` generates from
//! `plugins/wit` ([`crate::bindings`]) and the plain mirrors in
//! `jf_plugin_model::types`, which are the host's public types.
//!
//! Arguments go to the guest as generated types (`From<model> for wit`),
//! answers come back as model types (`From<wit> for model`). Every field is
//! named, so a WIT change that the mirror does not follow stops compiling
//! here.

use jf_plugin_model::types as model;

use crate::bindings::exports::jellyfesh::plugin::{auth, catalog, lyrics, meta, playback};
use crate::bindings::jellyfesh::plugin::{crypto, types};

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
same_enum!(
    meta::Capability,
    model::Capability,
    [
        Auth,
        Search,
        Browse,
        ResolveStream,
        Lyrics,
        Artwork,
        Recommendations
    ]
);
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
            model::Capability::from(meta::Capability::from(model::Capability::ResolveStream)),
            model::Capability::ResolveStream
        );
    }
}
