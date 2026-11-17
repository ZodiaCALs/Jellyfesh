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
use crate::plugin_log::{emit, sanitize};
use crate::runtime::HostState;
use crate::secrets::{secret_key, valid_secret_name};
use crate::storage::KvFailure;

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

impl types::Host for HostState {}

impl http::Host for HostState {
    fn send(&mut self, req: Request) -> Result<Response, HttpError> {
        self.host(|state| {
            let services = state.services.clone();
            let result = services.http.send(req, &mut state.call.net);
            if let Err(error) = &result {
                tracing::debug!(plugin = %services.id, ?error, "plugin request refused or failed");
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

impl kv::Host for HostState {
    fn get(&mut self, key: String) -> Result<Option<Vec<u8>>, KvError> {
        self.host(|state| state.services.kv.lock().get(&key).map_err(kv_error))
    }

    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), KvError> {
        self.host(|state| {
            if state.contains_protected(&value) || state.contains_protected(key.as_bytes()) {
                tracing::warn!(plugin = %state.services.id, "refused to store a submitted password in kv");
                return Err(forbidden());
            }
            state.services.kv.lock().set(&key, &value).map_err(kv_error)
        })
    }

    fn delete(&mut self, key: String) -> Result<(), KvError> {
        self.host(|state| state.services.kv.lock().delete(&key).map_err(kv_error))
    }

    fn set_secret(&mut self, name: String, value: String) -> Result<(), KvError> {
        self.host(|state| {
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
