//! Cryptography done by the host (`crypto` import), plus hex helpers.
//!
//! MD5, SHA-1 and AES-ECB are here for services that need them; do not use
//! them to protect anything new.

use crate::Result;
use crate::bindings::jellyfesh::plugin::crypto as wit;

pub use wit::{AesMode, CryptoError, GcmParams, HashAlgorithm, RsaComponents, RsaPadding, RsaPublicKey};

/// Hash of `data`.
pub fn digest(algorithm: HashAlgorithm, data: &[u8]) -> Result<Vec<u8>> {
    Ok(wit::digest(algorithm, data)?)
}

/// HMAC of `data` under `key`.
pub fn hmac(algorithm: HashAlgorithm, key: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    Ok(wit::hmac(algorithm, key, data)?)
}

/// Lowercase hex MD5 of `text` (Subsonic's token auth uses it).
pub fn md5_hex(text: &str) -> Result<String> {
    Ok(hex(&digest(HashAlgorithm::Md5, text.as_bytes())?))
}

/// SHA-256 of `data`.
pub fn sha256(data: &[u8]) -> Result<Vec<u8>> {
    digest(HashAlgorithm::Sha256, data)
}

/// AES encryption (see [`AesMode`]).
pub fn aes_encrypt(mode: &AesMode, key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    Ok(wit::aes_encrypt(mode, key, plaintext)?)
}

/// AES decryption (see [`AesMode`]).
pub fn aes_decrypt(mode: &AesMode, key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    Ok(wit::aes_decrypt(mode, key, ciphertext)?)
}

/// RSA public-key encryption.
pub fn rsa_encrypt(key: &RsaPublicKey, padding: RsaPadding, data: &[u8]) -> Result<Vec<u8>> {
    Ok(wit::rsa_encrypt(key, padding, data)?)
}

/// An RSASSA-PKCS1-v1_5 signature with SHA-256 over `data`, made by the host
/// with the RSA private key in the plugin's secret `secret` (API 1.1).
///
/// Store the key once with [`crate::kv::secret::set`] (PEM, or the DER in
/// base64) when the person enters it, and never read it back: the host signs
/// with it from the keystore, so it stays out of the plugin's memory. Errors
/// arrive as [`CryptoError`]: `InvalidKey` for an unknown secret or a key the
/// host cannot use. Jellyfesh Web cannot sign and answers `InvalidInput`.
pub fn rsa_sign_pkcs1_sha256(secret: &str, data: &[u8]) -> core::result::Result<Vec<u8>, CryptoError> {
    wit::rsa_sign_pkcs1_sha256(secret, data)
}

/// `len` random bytes from the operating system (at most 64 KiB per call).
pub fn random_bytes(len: u32) -> Result<Vec<u8>> {
    Ok(wit::random_bytes(len)?)
}

/// `bytes` random bytes as lowercase hex (a salt, a nonce, an id).
pub fn random_hex(bytes: u32) -> Result<String> {
    Ok(hex(&random_bytes(bytes)?))
}

/// Lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 0xf)] as char);
    }
    out
}
