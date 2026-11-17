//! NetEase Cloud Music's web request encoding ("weapi"), as the
//! music.163.com web player sends its requests. Written from the protocol,
//! not from anyone's code:
//!
//! 1. the request's parameters as JSON text;
//! 2. AES-128-CBC with PKCS #7 padding under the web client's fixed key
//!    `0CoJUm6Qyw8W8jud` and IV `0102030405060708`, then base64;
//! 3. the same again under a fresh random key of 16 letters and digits (same
//!    IV), then base64: the form field `params`;
//! 4. that random key, reversed and read as a big-endian number, raised to
//!    the public exponent 65537 modulo the web client's public modulus, as
//!    256 lowercase hex digits: the form field `encSecKey`. This is raw RSA
//!    without padding ([`crate::rsa_raw`]);
//! 5. `POST https://music.163.com/weapi/<path>` with the form
//!    `params=…&encSecKey=…`.
//!
//! The fixed key, IV and public key are what every browser receives with the
//! web player; they protect nothing. The plugin uses no key from NetEase's
//! apps (the `eapi` and `linuxapi` schemes need those and are not
//! implemented).

use std::cell::OnceCell;

use jf_community_common::base64;
use jf_plugin_sdk::prelude::*;

use crate::rsa_raw::Modulus;

/// The web client's fixed AES key for the inner layer.
pub const PRESET_KEY: &[u8; 16] = b"0CoJUm6Qyw8W8jud";
/// The IV of both AES layers.
pub const IV: &[u8; 16] = b"0102030405060708";
/// The public exponent.
pub const EXPONENT: [u8; 3] = [0x01, 0x00, 0x01];
/// The web client's 1024-bit public modulus, hex.
pub const MODULUS_HEX: &str = concat!(
    "00e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b725152b3ab17a876aea8a5",
    "aa76d2e417629ec4ee341f56135fccf695280104e0312ecbda92557c93870114af6c9d05c4f7f0c3685b7a4",
    "6bee255932575cce10b424d813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462db0a22b8e7",
);
/// Letters and digits for the random key.
const KEY_ALPHABET: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// A 1024-bit prime that stands in for the public modulus when the plugin
/// talks to the test suite's mock service. With a prime modulus anyone can
/// undo the step (the private exponent follows from the modulus), which is
/// exactly what the mock needs. Only built with the `mock-service` feature.
#[cfg(feature = "mock-service")]
pub const MOCK_MODULUS_HEX: &str = concat!(
    "c401a027e41a7f60d4068435b624201f496d66a178d5fb4aed4b8175a4cd17e9023ce7f036a428aeaa8903d06f",
    "bdc81ac0d529a952eba1f434727425c03e397b77209a562f0a47f371efacdea8d84efaa25619cf1116ec57f06cbd",
    "99be32ba687c61a3866e07267c6065d6659c9b696a6415a42497bac3fb2da6cffe9db0e3ad",
);

/// The two form fields of an encoded request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    /// `params`: the doubly encrypted JSON, base64.
    pub params: String,
    /// `encSecKey`: the random key under the public key, 256 hex digits.
    pub enc_sec_key: String,
}

/// AES-128-CBC encryption with PKCS #7 padding: `(key, iv, plaintext)`.
pub type AesCbc<'a> = &'a dyn Fn(&[u8], &[u8], &[u8]) -> Result<Vec<u8>>;

/// Encodes `json` with the random key `secret`.
pub fn encode_with(json: &str, secret: &[u8; 16], modulus: &Modulus, aes: AesCbc<'_>) -> Result<Encoded> {
    let inner = base64::encode(&aes(PRESET_KEY, IV, json.as_bytes())?);
    let params = base64::encode(&aes(secret, IV, inner.as_bytes())?);
    Ok(Encoded {
        params,
        enc_sec_key: enc_sec_key(secret, modulus)?,
    })
}

/// The key's step 4: reversed, raised to 65537 modulo the public modulus,
/// as 256 hex digits.
pub fn enc_sec_key(secret: &[u8; 16], modulus: &Modulus) -> Result<String> {
    let mut reversed = *secret;
    reversed.reverse();
    let number = modulus
        .pow(&reversed, &EXPONENT)
        .ok_or_else(|| PluginError::internal("the request key does not fit the public key"))?;
    let hex = crypto::hex(&number);
    Ok(format!("{hex:0>256}"))
}

/// Maps random bytes to a key of letters and digits.
pub fn key_from_random(bytes: &[u8]) -> [u8; 16] {
    let mut key = [b'0'; 16];
    for (slot, byte) in key.iter_mut().zip(bytes) {
        *slot = KEY_ALPHABET[usize::from(*byte) % KEY_ALPHABET.len()];
    }
    key
}

/// Prepares a modulus from hex.
pub fn modulus_from_hex(hex: &str) -> Modulus {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect();
    Modulus::from_be_bytes(&bytes).expect("a valid odd modulus")
}

thread_local! {
    static WEB_MODULUS: OnceCell<Modulus> = const { OnceCell::new() };
    #[cfg(feature = "mock-service")]
    static MOCK_MODULUS: OnceCell<Modulus> = const { OnceCell::new() };
}

/// Runs `f` with the public modulus in use: the web client's, or the mock
/// service's in a test build pointed at the mock.
fn with_modulus<T>(f: impl FnOnce(&Modulus) -> T) -> T {
    #[cfg(feature = "mock-service")]
    if crate::api::mock_origin().is_some() {
        return MOCK_MODULUS.with(|cell| f(cell.get_or_init(|| modulus_from_hex(MOCK_MODULUS_HEX))));
    }
    WEB_MODULUS.with(|cell| f(cell.get_or_init(|| modulus_from_hex(MODULUS_HEX))))
}

/// Encodes `json` for a request: a fresh random key from the host, AES from
/// the host's `crypto` import.
pub fn encode(json: &str) -> Result<Encoded> {
    let secret = key_from_random(&crypto::random_bytes(16)?);
    let aes = |key: &[u8], iv: &[u8], data: &[u8]| {
        crypto::aes_encrypt(&crypto::AesMode::Cbc(iv.to_vec()), key, data)
    };
    with_modulus(|modulus| encode_with(json, &secret, modulus, &aes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values from an independent computation (Python's `pow`,
    /// see the commit that added them): the key reversed, as a big-endian
    /// number, to the power 65537 modulo the web modulus.
    const VECTORS: &[(&str, &str)] = &[
        (
            "abcdefghijklmnop",
            "d15a1683c992095d0c234c19966605c5c5964911268bbeda8cb8d08d834913e59d53b32358903a121b5fca784c1f5ae44951fd02524df58ecc98e52cc7cf8689b42c2e93ddf05b0592512d87f5960467e2f086c018849d76014d323500e30f13ef4cafbb0cf5a66731a3f1776c75ca35d0062dac70a3e33245afabcf47938487",
        ),
        (
            "0123456789ABCDEF",
            "76b7934417bc7e2c67d6f91ce791b7b5e0097c20cbf92fd92e28e732c2a4a8f39f32bb5994988cc3c9d3e4e70f7976fce22c6de0a6fa903eb7198c5ae8da9c11d1da84e1792ebe1ea7fce9ee7c6d6a6e6cb9aae484a023a0c5c46842d8d96d6601156201a48f8bbbf51e4ec34e598de36bb70a80be3a6a223751a8c27b749269",
        ),
        (
            "zzzzzzzzzzzzzzzz",
            "2c621acc2fc41e64a5c577e4136df4fc4aeb5ef4542ce24271394a50c74f11753175bce4b33f29c3a4e4dc2c7329b8a88d269efdba48b1314a7c7dfcd8edbc99fbfdaf5276d3463129bca9955e6a16f6fdce258b23f29c57b0b4238efa8bc251767cc4c86a9da5b32382ba83f6fda8ed9fed0e6184acc0857dc4b93368a712dd",
        ),
        (
            "a1B2c3D4e5F6g7H8",
            "6615842a2f8ce26ba9faafa041b4cfb5f2a13c8c310cd67fb74033f634a0720910cd511b3779d13e8cef5cf6d79e5105172e1a1a4a4a670b3d38ba682fdc2f1121c5c0a7405e4e2483d2d4d1b42c8b2243bffaa8ea672f7528e8b2335eacacc65bdfb1d04ff7a96555098aedd460386024b4186f98c81d235f6996b5256d103b",
        ),
    ];

    fn key(text: &str) -> [u8; 16] {
        text.as_bytes().try_into().unwrap()
    }

    #[test]
    fn the_web_modulus_is_1024_bits() {
        assert_eq!(MODULUS_HEX.len(), 258);
        assert_eq!(modulus_from_hex(MODULUS_HEX).byte_len(), 128);
    }

    #[test]
    fn key_step_matches_the_reference() {
        let modulus = modulus_from_hex(MODULUS_HEX);
        for (secret, expected) in VECTORS {
            let got = enc_sec_key(&key(secret), &modulus).unwrap();
            assert_eq!(got.len(), 256);
            assert_eq!(&got, expected, "{secret}");
        }
    }

    #[test]
    fn keys_are_letters_and_digits() {
        let bytes: Vec<u8> = (0u8..16).map(|i| i.wrapping_mul(37)).collect();
        let key = key_from_random(&bytes);
        assert!(key.iter().all(u8::is_ascii_alphanumeric));
        assert_eq!(key_from_random(&[0; 16]), *b"AAAAAAAAAAAAAAAA");
        assert_eq!(key_from_random(&[61, 62, 255]), *b"9AH0000000000000");
    }

    #[test]
    fn layers_are_applied_in_order() {
        // A stand-in "cipher" that tags its input, to see the layering
        // without the host: the real AES runs in the host (and in the
        // integration tests against the mock service).
        let fake = |key: &[u8], iv: &[u8], data: &[u8]| -> Result<Vec<u8>> {
            assert_eq!(iv, IV);
            let mut out = key.to_vec();
            out.push(b'|');
            out.extend_from_slice(data);
            Ok(out)
        };
        let modulus = modulus_from_hex(MODULUS_HEX);
        let secret = key("abcdefghijklmnop");
        let encoded = encode_with("{\"a\":1}", &secret, &modulus, &fake).unwrap();
        let outer = base64::decode(&encoded.params).unwrap();
        let (outer_key, inner_b64) = outer.split_at(17);
        assert_eq!(outer_key, b"abcdefghijklmnop|");
        let inner = base64::decode(std::str::from_utf8(inner_b64).unwrap()).unwrap();
        assert_eq!(inner, b"0CoJUm6Qyw8W8jud|{\"a\":1}");
        assert_eq!(encoded.enc_sec_key, VECTORS[0].1);
    }
}
