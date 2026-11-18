//! The host side of the world's imports, implemented on [`HostState`].
//!
//! Every import runs as host time ([`HostState::host`]), so waiting for the
//! network or the keystore never counts against the plugin's CPU budget.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::bindings::jellyfesh::plugin::crypto::{
    AesMode, CryptoError, HashAlgorithm, RsaPadding, RsaPublicKey,
};
use crate::bindings::jellyfesh::plugin::http::{HttpError, Request, Response};
use crate::bindings::jellyfesh::plugin::kv::KvError;
use crate::bindings::jellyfesh::plugin::log::Level;
use crate::bindings::jellyfesh::plugin::{clock, crypto, http, kv, log, types};
use crate::bindings::maker::jellyfesh::plugin::audio::{AudioError, ClipInfo};
use crate::bindings::maker::jellyfesh::plugin::speech::{Model, Options, SpeechError, Transcript};
use crate::bindings::maker::jellyfesh::plugin::{audio, speech};
use crate::maker::MakeStage;
use crate::plugin_log::{emit, sanitize};
use crate::runtime::HostState;
use crate::secrets::{secret_key, valid_secret_name};
use crate::storage::KvFailure;
use crate::types::{Permission, SpeechOptions};

/// The most audio one `audio.wav` call returns: 8 minutes (ADR-0019 §8).
pub const MAX_WAV_MS: u64 = 8 * 60 * 1000;
/// The most of a recognition prompt the host passes on.
const MAX_PROMPT: usize = 4096;

/// Longest secret value.
const MAX_SECRET: usize = 64 * 1024;
/// Shorter protected values are not matched inside stored values (too many
/// false positives); they are still masked in logs.
const MIN_PROTECTED_MATCH: usize = 4;

impl HostState {
    /// Whether `value` contains a password typed during the current submit.
    fn contains_protected(&self, value: &[u8]) -> bool {
        self.call.protected.values.iter().any(|secret| {
            let needle = secret.as_bytes();
            needle.len() >= MIN_PROTECTED_MATCH
                && value.len() >= needle.len()
                && value.windows(needle.len()).any(|window| window == needle)
        })
    }
}

impl HostState {
    /// Whether the current call read the song's audio: then HTTP goes only
    /// where the person allowed and `kv` takes no writes (ADR-0019 §4).
    fn holds_audio(&self) -> bool {
        self.call.maker.as_ref().is_some_and(|m| m.read_audio)
    }

    /// Reports a stage of the current `maker.make` call, if there is one.
    fn report(&mut self, stage: MakeStage) {
        if let Some(maker) = &mut self.call.maker {
            maker.report(stage);
        }
    }
}

impl types::Host for HostState {}

impl http::Host for HostState {
    fn send(&mut self, req: Request) -> Result<Response, HttpError> {
        self.host(|state| {
            if state.call.cancelled() {
                return Err(HttpError::Connection("the job was cancelled".into()));
            }
            let holding = state.holds_audio();
            if holding && let Some(origin) = jf_plugin_model::sanitize::origin(&req.url) {
                state.report(MakeStage::Sending { origin });
            }
            let services = state.services.clone();
            let result = services.http.send(req, &mut state.call.net);
            if let Err(error) = &result {
                tracing::debug!(plugin = %services.id, ?error, "plugin request refused or failed");
            } else if holding {
                state.report(MakeStage::TimingWords);
            }
            result
        })
    }
}

fn kv_error(failure: KvFailure) -> KvError {
    match failure {
        KvFailure::InvalidKey => KvError::InvalidKey("keys are 1–128 bytes of printable ASCII".into()),
        KvFailure::Quota => KvError::QuotaExceeded,
        KvFailure::Io(e) => KvError::Unavailable(e),
    }
}

fn forbidden() -> KvError {
    KvError::Forbidden("a password typed for this login cannot be stored; exchange it for a token".into())
}

fn bad_secret_name() -> KvError {
    KvError::InvalidKey("secret names are 1–64 characters of a-z 0-9 . _ -".into())
}

fn holding_audio() -> KvError {
    KvError::Forbidden("this call read the song's audio; it may not write to storage".into())
}

impl kv::Host for HostState {
    fn get(&mut self, key: String) -> Result<Option<Vec<u8>>, KvError> {
        self.host(|state| state.services.kv.lock().get(&key).map_err(kv_error))
    }

    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), KvError> {
        self.host(|state| {
            if state.holds_audio() {
                return Err(holding_audio());
            }
            if state.contains_protected(&value) || state.contains_protected(key.as_bytes()) {
                tracing::warn!(plugin = %state.services.id, "refused to store a submitted password in kv");
                return Err(forbidden());
            }
            state.services.kv.lock().set(&key, &value).map_err(kv_error)
        })
    }

    fn delete(&mut self, key: String) -> Result<(), KvError> {
        self.host(|state| {
            if state.holds_audio() {
                return Err(holding_audio());
            }
            state.services.kv.lock().delete(&key).map_err(kv_error)
        })
    }

    fn set_secret(&mut self, name: String, value: String) -> Result<(), KvError> {
        self.host(|state| {
            if state.holds_audio() {
                return Err(holding_audio());
            }
            if !valid_secret_name(&name) {
                return Err(bad_secret_name());
            }
            if value.len() > MAX_SECRET {
                return Err(KvError::InvalidKey("a secret is at most 64 KiB".into()));
            }
            if state.call.protected.keystore && state.contains_protected(value.as_bytes()) {
                tracing::warn!(plugin = %state.services.id, "refused to store a submitted password as a secret");
                return Err(forbidden());
            }
            let services = state.services.clone();
            services
                .secrets
                .set(&secret_key(&services.id, &name), &value)
                .map_err(|e| KvError::Unavailable(e.to_string()))?;
            if !services.state().secret_names.contains(&name) {
                services.update_state(|s| s.secret_names.insert(name));
            }
            Ok(())
        })
    }

    fn get_secret(&mut self, name: String) -> Result<Option<String>, KvError> {
        self.host(|state| {
            if !valid_secret_name(&name) {
                return Err(bad_secret_name());
            }
            let services = &state.services;
            services
                .secrets
                .get(&secret_key(&services.id, &name))
                .map_err(|e| KvError::Unavailable(e.to_string()))
        })
    }

    fn delete_secret(&mut self, name: String) -> Result<(), KvError> {
        self.host(|state| {
            if state.holds_audio() {
                return Err(holding_audio());
            }
            if !valid_secret_name(&name) {
                return Err(bad_secret_name());
            }
            let services = state.services.clone();
            services
                .secrets
                .delete(&secret_key(&services.id, &name))
                .map_err(|e| KvError::Unavailable(e.to_string()))?;
            if services.state().secret_names.contains(&name) {
                services.update_state(|s| s.secret_names.remove(&name));
            }
            Ok(())
        })
    }
}

/// The `crypto` import runs `jf_plugin_model::crypto` (shared with
/// Jellyfesh Web) on the model's types.
use jf_plugin_model::crypto as host_crypto;

impl crypto::Host for HostState {
    fn digest(&mut self, algorithm: HashAlgorithm, data: Vec<u8>) -> Result<Vec<u8>, CryptoError> {
        self.host(|_| host_crypto::digest(algorithm.into(), &data).map_err(Into::into))
    }

    fn hmac(
        &mut self,
        algorithm: HashAlgorithm,
        key: Vec<u8>,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, CryptoError> {
        self.host(|_| host_crypto::hmac(algorithm.into(), &key, &data).map_err(Into::into))
    }

    fn aes_encrypt(
        &mut self,
        mode: AesMode,
        key: Vec<u8>,
        plaintext: Vec<u8>,
    ) -> Result<Vec<u8>, CryptoError> {
        self.host(|_| host_crypto::aes_encrypt(&mode.into(), &key, &plaintext).map_err(Into::into))
    }

    fn aes_decrypt(
        &mut self,
        mode: AesMode,
        key: Vec<u8>,
        ciphertext: Vec<u8>,
    ) -> Result<Vec<u8>, CryptoError> {
        self.host(|_| host_crypto::aes_decrypt(&mode.into(), &key, &ciphertext).map_err(Into::into))
    }

    fn rsa_encrypt(
        &mut self,
        key: RsaPublicKey,
        padding: RsaPadding,
        data: Vec<u8>,
    ) -> Result<Vec<u8>, CryptoError> {
        self.host(|_| host_crypto::rsa_encrypt(&key.into(), padding.into(), &data).map_err(Into::into))
    }

    fn random_bytes(&mut self, len: u32) -> Result<Vec<u8>, CryptoError> {
        self.host(|_| host_crypto::random_bytes(len).map_err(Into::into))
    }

    /// Signs with the private key in the plugin's own secret `secret`, read
    /// here so that it never enters the plugin's memory.
    fn rsa_sign_pkcs1_sha256(&mut self, secret: String, data: Vec<u8>) -> Result<Vec<u8>, CryptoError> {
        self.host(|state| {
            if !valid_secret_name(&secret) {
                return Err(CryptoError::InvalidKey(
                    "secret names are 1–64 characters of a-z 0-9 . _ -".into(),
                ));
            }
            if data.len() > host_crypto::MAX_INPUT {
                return Err(CryptoError::TooLarge);
            }
            let services = &state.services;
            let key = services
                .secrets
                .get(&secret_key(&services.id, &secret))
                .map_err(|e| CryptoError::InvalidKey(format!("the keystore is unavailable: {e}")))?
                .map(zeroize::Zeroizing::new)
                .ok_or_else(|| CryptoError::InvalidKey(format!("there is no secret named {secret:?}")))?;
            host_crypto::rsa_sign_pkcs1_sha256(&key, &data).map_err(Into::into)
        })
    }
}

impl clock::Host for HostState {
    fn now_utc_ms(&mut self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    }

    fn monotonic_ms(&mut self) -> u64 {
        self.clock_base.elapsed().as_millis() as u64
    }
}

impl log::Host for HostState {
    fn log(&mut self, level: Level, message: String) {
        self.host(|state| {
            let text = sanitize(&message, state.call.protected.values.iter().map(|s| s.as_str()));
            if text.starts_with("panic: ") {
                state.call.panic_message = Some(text.clone());
            }
            let admitted = state.services.log.lock().admit(Instant::now());
            match admitted {
                Some(0) => emit(&state.services.id, level, &text),
                Some(dropped) => {
                    emit(
                        &state.services.id,
                        Level::Warn,
                        &format!("{dropped} log lines dropped (rate limit)"),
                    );
                    emit(&state.services.id, level, &text);
                }
                None => {}
            }
        })
    }
}

// ---- API 1.2: the song's audio and speech recognition (ADR-0019) -------------------------------

fn no_song() -> String {
    "there is no song outside maker.make, or this handle is not this call's".into()
}

impl audio::Host for HostState {
    fn info(&mut self, track: u64) -> Result<ClipInfo, AudioError> {
        self.host(|state| {
            if !state.services.permits(Permission::Audio) {
                return Err(AudioError::NotDeclared);
            }
            let access = match &state.call.maker {
                Some(maker) if maker.track == track => maker.access.clone(),
                _ => return Err(AudioError::Unavailable(no_song())),
            };
            if state.call.cancelled() {
                return Err(AudioError::Cancelled);
            }
            access
                .duration_ms()
                .map(|duration_ms| ClipInfo { duration_ms })
                .map_err(Into::into)
        })
    }

    fn wav(&mut self, track: u64, start_ms: u64, length_ms: u64) -> Result<Vec<u8>, AudioError> {
        self.host(|state| {
            if !state.services.permits(Permission::Audio) {
                return Err(AudioError::NotDeclared);
            }
            let (access, allowed, upload_to) = match &state.call.maker {
                Some(maker) if maker.track == track => (
                    maker.access.clone(),
                    maker.allowed.clone(),
                    maker.upload_to.clone(),
                ),
                _ => return Err(AudioError::Unavailable(no_song())),
            };
            if state.call.cancelled() {
                return Err(AudioError::Cancelled);
            }
            if length_ms > MAX_WAV_MS {
                return Err(AudioError::Unavailable(format!(
                    "ask for at most {MAX_WAV_MS} ms of audio per call"
                )));
            }
            if allowed.is_empty() {
                return Err(AudioError::NoConsent(upload_to.into_iter().collect()));
            }
            state.report(MakeStage::ReadingAudio);
            let wav = access.wav(start_ms, length_ms).map_err(AudioError::from)?;
            if let Some(maker) = &mut state.call.maker {
                maker.read_audio = true;
            }
            state.call.net.only = Some(allowed);
            Ok(wav)
        })
    }
}

impl speech::Host for HostState {
    fn models(&mut self) -> Vec<Model> {
        self.host(|state| {
            if !state.services.permits(Permission::Speech) {
                return Vec::new();
            }
            state.global.speech_models().into_iter().map(Into::into).collect()
        })
    }

    fn transcribe(&mut self, track: u64, opts: Options) -> Result<Transcript, SpeechError> {
        self.host(|state| {
            if !state.services.permits(Permission::Speech) {
                return Err(SpeechError::NotDeclared);
            }
            let access = match &state.call.maker {
                Some(maker) if maker.track == track => maker.access.clone(),
                _ => return Err(SpeechError::Unavailable(no_song())),
            };
            let Some(cancel) = state.call.cancel.clone() else {
                return Err(SpeechError::Unavailable(no_song()));
            };
            if state.call.cancelled() {
                return Err(SpeechError::Cancelled);
            }
            let options = SpeechOptions {
                model: jf_plugin_model::sanitize::text(&opts.model, 64),
                language: opts
                    .language
                    .map(|l| jf_plugin_model::sanitize::text(&l, 35))
                    .filter(|l| !l.is_empty()),
                prompt: opts
                    .prompt
                    .map(|p| jf_plugin_model::sanitize::text(&p, MAX_PROMPT))
                    .filter(|p| !p.is_empty()),
            };
            state.report(MakeStage::Recognising);
            let transcript = access.transcribe(&options, &cancel).map_err(SpeechError::from)?;
            let duration = access
                .duration_ms()
                .ok()
                .map(|d| u32::try_from(d).unwrap_or(u32::MAX));
            state.report(MakeStage::TimingWords);
            Ok(jf_plugin_model::sanitize::transcript(transcript, duration).into())
        })
    }
}
