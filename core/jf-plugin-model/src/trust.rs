//! The keys whose signatures earn a package the **Official** badge.
//!
//! [`TrustList::builtin`] holds the Jellyfesh release key (ed25519, created for
//! v0.0.3). Its private half never enters the repository: the maintainers keep
//! it offline, and the release workflow reads it from the
//! `JF_PLUGIN_SIGNING_KEY` secret to sign the official plugins (ADR-0004).
//!
//! [`test_key_seed`] is a published test key. Anyone can recompute its
//! private half, so it must never be trusted outside tests: only
//! [`TrustList::with_test_key`] adds it, and `jf_plugin_host::PluginHost::new` never
//! does.

use ed25519_dalek::{SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

/// The Jellyfesh release key (public half; see the module docs).
pub const RELEASE_KEY: [u8; 32] = [
    0x78, 0x55, 0x4c, 0x79, 0xcd, 0x53, 0x82, 0x89, 0xa5, 0x82, 0x89, 0x55, 0x0b, 0x7a, 0xd5, 0xba, 0x85,
    0xb9, 0x03, 0xa6, 0xca, 0xd2, 0xc2, 0x52, 0xbe, 0x0d, 0x5d, 0xad, 0xda, 0x15, 0x9b, 0xf2,
];

/// Seed of the test signing key: the SHA-256 of a fixed phrase, so anyone can
/// recompute it. For tests only.
pub fn test_key_seed() -> [u8; 32] {
    Sha256::digest(b"jellyfesh test signing key v1: never trust this key outside tests").into()
}

/// A key the host trusts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedKey {
    /// Fingerprint: [`key_id`] of `public`.
    pub id: String,
    /// What the UI calls the key ("Jellyfesh release key").
    pub label: String,
    /// The ed25519 public key.
    pub public: [u8; 32],
}

/// The set of trusted signing keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustList {
    keys: Vec<TrustedKey>,
}

impl TrustList {
    /// The list compiled into the app: the Jellyfesh release key.
    pub fn builtin() -> TrustList {
        TrustList::empty().with_key("Jellyfesh release key", RELEASE_KEY)
    }

    /// No trusted keys at all.
    pub fn empty() -> TrustList {
        TrustList { keys: Vec::new() }
    }

    /// Adds a key.
    pub fn with_key(mut self, label: &str, public: [u8; 32]) -> TrustList {
        self.keys.push(TrustedKey {
            id: key_id(&public),
            label: label.to_string(),
            public,
        });
        self
    }

    /// Adds the published test key. **Tests only.**
    pub fn with_test_key(self) -> TrustList {
        let public = SigningKey::from_bytes(&test_key_seed())
            .verifying_key()
            .to_bytes();
        self.with_key("Jellyfesh test key", public)
    }

    /// The trusted key with this public key, if any.
    pub fn find(&self, public: &[u8; 32]) -> Option<&TrustedKey> {
        self.keys.iter().find(|key| &key.public == public)
    }

    /// All trusted keys.
    pub fn keys(&self) -> &[TrustedKey] {
        &self.keys
    }
}

/// A key's fingerprint: the first 8 bytes of the SHA-256 of the public key,
/// as 16 lowercase hex digits.
pub fn key_id(public: &[u8; 32]) -> String {
    hex(&Sha256::digest(public)[..8])
}

/// The public key of a signing seed.
pub fn public_key(seed: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

pub(crate) fn verifying_key(public: &[u8; 32]) -> Option<VerifyingKey> {
    VerifyingKey::from_bytes(public).ok()
}

/// Lowercase hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 0xf)] as char);
    }
    out
}

/// Parses lowercase or uppercase hex of exactly `N` bytes.
pub(crate) fn unhex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let bytes = text.as_bytes();
    if bytes.len() != N * 2 {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    };
    let mut out = [0u8; N];
    for (i, pair) in bytes.chunks_exact(2).enumerate() {
        out[i] = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_list_has_only_the_release_key() {
        let list = TrustList::builtin();
        assert_eq!(list.keys().len(), 1);
        assert!(list.find(&RELEASE_KEY).is_some());
        assert!(
            verifying_key(&RELEASE_KEY).is_some(),
            "the release key must be a valid point"
        );
        let test_public = public_key(&test_key_seed());
        assert!(list.find(&test_public).is_none());
        assert!(list.with_test_key().find(&test_public).is_some());
    }

    #[test]
    fn key_ids_and_hex_round_trip() {
        let id = key_id(&RELEASE_KEY);
        assert_eq!(id.len(), 16);
        assert!(id.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(unhex::<32>(&hex(&RELEASE_KEY)), Some(RELEASE_KEY));
        assert_eq!(unhex::<2>("0aFf"), Some([0x0a, 0xff]));
        assert_eq!(unhex::<2>("0aF"), None);
        assert_eq!(unhex::<2>("0aFg"), None);
    }
}
