//! Jellyfesh Pro licence keys (ADR-0021 §2).
//!
//! A key is `JFP1-` followed by the Crockford base32 of 80 bytes, in groups
//! of eight characters separated by `-`:
//!
//! | Bytes | What |
//! | --- | --- |
//! | 0 | format, 1 |
//! | 1 | the id of the signing key |
//! | 2 | the product: 1 is Jellyfesh Pro |
//! | 3 | flags, reserved (0) |
//! | 4–11 | a random licence id, big-endian |
//! | 12–13 | the issue day: days since 1970-01-01 (UTC), big-endian |
//! | 14–15 | reserved, 0 |
//! | 16–79 | an Ed25519 signature over `"jellyfesh licence v1\0"` followed by bytes 0–15 |
//!
//! Keys hold no name or e-mail. Reading one ignores case, spaces and `-`,
//! and takes `O` for `0` and `I` or `L` for `1`, as Crockford base32 does.
//! [`verify`] checks a key offline against public keys: the ones built into
//! this release ([`release_keys`], from `JF_LICENCE_PUBLIC_KEYS` at build
//! time; none are committed) and the revocation list shipped with it
//! ([`REVOKED`]). With the `issue` feature, [`generate_keypair`] makes a key
//! pair and [`issue`] signs keys (tools and tests only).

#![forbid(unsafe_code)]

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// The text every key starts with.
pub const PREFIX: &str = "JFP1";
/// The signed-message prefix.
pub const CONTEXT: &[u8] = b"jellyfesh licence v1\0";
/// Bytes of a key: the payload and the signature.
pub const KEY_BYTES: usize = PAYLOAD_BYTES + 64;
/// Bytes of the signed payload.
pub const PAYLOAD_BYTES: usize = 16;
/// The key format this crate reads and writes.
pub const FORMAT: u8 = 1;
/// Most release keys a build carries.
pub const MAX_RELEASE_KEYS: usize = 16;
/// Licence ids withdrawn after a refund or a leak; shipped with each
/// release.
pub const REVOKED: &[u64] = &[];

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// A public key that may sign licence keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicKey {
    /// The id licence keys name (byte 1).
    pub id: u8,
    /// The Ed25519 public key.
    pub bytes: [u8; 32],
}

/// What a licence unlocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Product {
    /// Jellyfesh Pro.
    Pro,
}

impl Product {
    #[cfg(feature = "issue")]
    fn code(self) -> u8 {
        match self {
            Product::Pro => 1,
        }
    }
}

/// A valid licence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Licence {
    /// The random licence id.
    pub id: u64,
    /// The signing key's id.
    pub key_id: u8,
    pub product: Product,
    /// Days since 1970-01-01 (UTC) when the key was issued.
    pub issued_day: u16,
    /// Reserved flags.
    pub flags: u8,
}

impl Licence {
    /// The licence id in Crockford base32 (13 characters).
    pub fn id_text(&self) -> String {
        let mut out = String::with_capacity(13);
        for i in (0..13).rev() {
            let digit = (self.id >> (i * 5)) & 0x1f;
            out.push(ALPHABET[digit as usize] as char);
        }
        out
    }

    /// The last four characters of [`Licence::id_text`], shown as
    /// "Licence …ABCD".
    pub fn id_suffix(&self) -> String {
        let text = self.id_text();
        text[text.len() - 4..].to_owned()
    }

    /// When it was issued: midnight UTC, in milliseconds since the Unix epoch.
    pub fn issued_at_ms(&self) -> i64 {
        i64::from(self.issued_day) * 86_400_000
    }

    #[cfg(feature = "issue")]
    fn payload(&self) -> [u8; PAYLOAD_BYTES] {
        let mut payload = [0u8; PAYLOAD_BYTES];
        payload[0] = FORMAT;
        payload[1] = self.key_id;
        payload[2] = self.product.code();
        payload[3] = self.flags;
        payload[4..12].copy_from_slice(&self.id.to_be_bytes());
        payload[12..14].copy_from_slice(&self.issued_day.to_be_bytes());
        payload
    }
}

/// Why a key was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LicenceError {
    /// Not a Jellyfesh licence key: the wrong prefix, characters or length.
    Malformed,
    /// Signed by a key this build does not know.
    UnknownKey,
    /// The signature does not match: a mistyped or forged key.
    BadSignature,
    /// A valid key for another product.
    WrongProduct,
    /// Withdrawn (refunded or leaked).
    Revoked,
}

impl std::fmt::Display for LicenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LicenceError::Malformed => "that does not look like a Jellyfesh licence key",
            LicenceError::UnknownKey => "the licence key is not valid (unknown signing key)",
            LicenceError::BadSignature => "the licence key is not valid",
            LicenceError::WrongProduct => "the licence key is for another product",
            LicenceError::Revoked => "the licence key has been withdrawn",
        })
    }
}

impl std::error::Error for LicenceError {}

/// Checks a licence key against `keys` and the built-in revocation list.
pub fn verify(text: &str, keys: &[PublicKey]) -> Result<Licence, LicenceError> {
    verify_with(text, keys, REVOKED)
}

/// [`verify`] with another revocation list.
pub fn verify_with(text: &str, keys: &[PublicKey], revoked: &[u64]) -> Result<Licence, LicenceError> {
    let bytes = decode_key(text).ok_or(LicenceError::Malformed)?;
    let (payload, signature) = bytes.split_at(PAYLOAD_BYTES);
    if payload[0] != FORMAT || payload[14] != 0 || payload[15] != 0 {
        return Err(LicenceError::Malformed);
    }
    let key_id = payload[1];
    let key = keys
        .iter()
        .find(|k| k.id == key_id)
        .ok_or(LicenceError::UnknownKey)?;
    let verifying = VerifyingKey::from_bytes(&key.bytes).map_err(|_| LicenceError::UnknownKey)?;
    let signature = Signature::from_slice(signature).map_err(|_| LicenceError::BadSignature)?;
    let mut message = Vec::with_capacity(CONTEXT.len() + PAYLOAD_BYTES);
    message.extend_from_slice(CONTEXT);
    message.extend_from_slice(payload);
    verifying
        .verify(&message, &signature)
        .map_err(|_| LicenceError::BadSignature)?;
    let product = match payload[2] {
        1 => Product::Pro,
        _ => return Err(LicenceError::WrongProduct),
    };
    let mut id = [0u8; 8];
    id.copy_from_slice(&payload[4..12]);
    let licence = Licence {
        id: u64::from_be_bytes(id),
        key_id,
        product,
        issued_day: u16::from_be_bytes([payload[12], payload[13]]),
        flags: payload[3],
    };
    if revoked.contains(&licence.id) {
        return Err(LicenceError::Revoked);
    }
    Ok(licence)
}

/// The key text for `bytes` (a payload and its signature): `JFP1-` and the
/// Crockford base32, in groups of eight.
pub fn format_key(bytes: &[u8]) -> String {
    let encoded = encode(bytes);
    let mut out = String::from(PREFIX);
    for chunk in encoded.as_bytes().chunks(8) {
        out.push('-');
        out.push_str(std::str::from_utf8(chunk).unwrap_or_default());
    }
    out
}

/// The 80 bytes of a key, or `None` when the text is not one.
fn decode_key(text: &str) -> Option<[u8; KEY_BYTES]> {
    let compact: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_uppercase();
    let body = compact.strip_prefix(PREFIX)?;
    let bytes = decode(body)?;
    bytes.try_into().ok()
}

fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 8 / 5 + 1);
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 0x1f) as usize] as char);
    }
    out
}

fn value(c: u8) -> Option<u32> {
    let c = match c.to_ascii_uppercase() {
        b'O' => b'0',
        b'I' | b'L' => b'1',
        c => c,
    };
    ALPHABET.iter().position(|&a| a == c).map(|p| p as u32)
}

fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 5 / 8);
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for c in text.bytes() {
        buffer = (buffer << 5) | value(c)?;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    // Leftover bits are padding and must be zero.
    (buffer == 0).then_some(out)
}

// ---- the keys built into this release ---------------------------------------------------------

/// `JF_LICENCE_PUBLIC_KEYS` at build time: `id:hex,id:hex` (ids 0–255, 64
/// hex digits each, no spaces); empty or unset means none.
const RELEASE_KEY_TEXT: &str = match option_env!("JF_LICENCE_PUBLIC_KEYS") {
    Some(text) => text,
    None => "",
};

/// The release keys, parsed while compiling: a malformed variable stops
/// the build with a message saying what is wrong.
const RELEASE_KEYS: ([PublicKey; MAX_RELEASE_KEYS], usize) = parse_keys(RELEASE_KEY_TEXT);

/// The public keys built into this release (none in builds without
/// `JF_LICENCE_PUBLIC_KEYS`).
pub fn release_keys() -> Vec<PublicKey> {
    RELEASE_KEYS.0[..RELEASE_KEYS.1].to_vec()
}

const fn hex_value(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("JF_LICENCE_PUBLIC_KEYS: a key is not 64 hex digits"),
    }
}

/// Parses `id:hex,id:hex` in a constant, so a malformed value fails the
/// build. Exposed for tests through [`parse_public_keys`].
const fn parse_keys(text: &str) -> ([PublicKey; MAX_RELEASE_KEYS], usize) {
    let bytes = text.as_bytes();
    let mut keys = [PublicKey {
        id: 0,
        bytes: [0; 32],
    }; MAX_RELEASE_KEYS];
    let mut count = 0;
    let mut i = 0;
    if bytes.is_empty() {
        return (keys, 0);
    }
    loop {
        if count == MAX_RELEASE_KEYS {
            panic!("JF_LICENCE_PUBLIC_KEYS: more than 16 keys");
        }
        // The id: 1–3 decimal digits, at most 255.
        let mut id: u32 = 0;
        let mut digits = 0;
        while i < bytes.len() && bytes[i] != b':' {
            let c = bytes[i];
            if c < b'0' || c > b'9' || digits == 3 {
                panic!("JF_LICENCE_PUBLIC_KEYS: an id is not a number 0-255 (the format is id:hex,id:hex)");
            }
            id = id * 10 + (c - b'0') as u32;
            digits += 1;
            i += 1;
        }
        if digits == 0 || id > 255 || i == bytes.len() {
            panic!("JF_LICENCE_PUBLIC_KEYS: expected id:hex (an id 0-255, a colon, 64 hex digits)");
        }
        i += 1; // the colon
        if i + 64 > bytes.len() {
            panic!("JF_LICENCE_PUBLIC_KEYS: a key is not 64 hex digits");
        }
        let mut key = [0u8; 32];
        let mut k = 0;
        while k < 32 {
            key[k] = hex_value(bytes[i + 2 * k]) << 4 | hex_value(bytes[i + 2 * k + 1]);
            k += 1;
        }
        i += 64;
        let mut j = 0;
        while j < count {
            if keys[j].id == id as u8 {
                panic!("JF_LICENCE_PUBLIC_KEYS: two keys have the same id");
            }
            j += 1;
        }
        keys[count] = PublicKey {
            id: id as u8,
            bytes: key,
        };
        count += 1;
        if i == bytes.len() {
            break;
        }
        if bytes[i] != b',' {
            panic!("JF_LICENCE_PUBLIC_KEYS: keys are separated by commas, without spaces");
        }
        i += 1;
    }
    (keys, count)
}

/// Parses public keys written as `JF_LICENCE_PUBLIC_KEYS` takes them
/// (`id:hex,id:hex`), or says what is wrong.
pub fn parse_public_keys(text: &str) -> Result<Vec<PublicKey>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut keys: Vec<PublicKey> = Vec::new();
    for entry in text.split(',') {
        let (id, hex) = entry
            .split_once(':')
            .ok_or_else(|| format!("{entry:?}: expected id:hex"))?;
        let id: u8 = id
            .parse()
            .map_err(|_| format!("{id:?} is not an id from 0 to 255"))?;
        if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("the key of id {id} is not 64 hex digits"));
        }
        let mut bytes = [0u8; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
        }
        if keys.iter().any(|k| k.id == id) {
            return Err(format!("two keys have the id {id}"));
        }
        keys.push(PublicKey { id, bytes });
    }
    if keys.len() > MAX_RELEASE_KEYS {
        return Err(format!("more than {MAX_RELEASE_KEYS} keys"));
    }
    Ok(keys)
}

/// `id:hex` for a public key, as `JF_LICENCE_PUBLIC_KEYS` takes it.
pub fn public_key_text(key: &PublicKey) -> String {
    let hex: String = key.bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}:{hex}", key.id)
}

// ---- issuing (tools and tests) ----------------------------------------------------------------

/// A new Ed25519 key pair: the 32-byte secret seed and its public key under
/// `id`. Keep the secret outside the repository.
#[cfg(feature = "issue")]
pub fn generate_keypair_with_id(id: u8) -> ([u8; 32], PublicKey) {
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).expect("the operating system's random numbers");
    let public = ed25519_dalek::SigningKey::from_bytes(&secret)
        .verifying_key()
        .to_bytes();
    (secret, PublicKey { id, bytes: public })
}

/// A new key pair with key id 0 (see [`generate_keypair_with_id`]).
#[cfg(feature = "issue")]
pub fn generate_keypair() -> ([u8; 32], PublicKey) {
    generate_keypair_with_id(0)
}

/// The public key of a secret seed, under `id`.
#[cfg(feature = "issue")]
pub fn public_key(secret: &[u8; 32], id: u8) -> PublicKey {
    PublicKey {
        id,
        bytes: ed25519_dalek::SigningKey::from_bytes(secret)
            .verifying_key()
            .to_bytes(),
    }
}

/// Signs `licence` with `secret` as key `key_id` and returns the key text.
/// `licence.key_id` is replaced by `key_id`.
#[cfg(feature = "issue")]
pub fn issue(secret: &[u8; 32], key_id: u8, licence: &Licence) -> String {
    use ed25519_dalek::Signer;
    let payload = Licence { key_id, ..*licence }.payload();
    let mut message = Vec::with_capacity(CONTEXT.len() + PAYLOAD_BYTES);
    message.extend_from_slice(CONTEXT);
    message.extend_from_slice(&payload);
    let signature = ed25519_dalek::SigningKey::from_bytes(secret).sign(&message);
    let mut bytes = Vec::with_capacity(KEY_BYTES);
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(&signature.to_bytes());
    format_key(&bytes)
}

/// A new Pro licence with a random id, issued `day` (days since 1970-01-01).
#[cfg(feature = "issue")]
pub fn new_licence(day: u16) -> Licence {
    let mut id = [0u8; 8];
    getrandom::fill(&mut id).expect("the operating system's random numbers");
    Licence {
        id: u64::from_be_bytes(id),
        key_id: 0,
        product: Product::Pro,
        issued_day: day,
        flags: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_round_trips_and_is_tolerant() {
        for len in [0usize, 1, 5, 16, 80] {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            let text = encode(&bytes);
            assert_eq!(decode(&text).unwrap(), bytes, "{len}");
            assert_eq!(decode(&text.to_ascii_lowercase()).unwrap(), bytes);
        }
        assert_eq!(decode("O1IL"), decode("0111"));
        assert_eq!(decode("U"), None, "U is not a Crockford digit");
        assert_eq!(encode(&[0u8; 80]).len(), 128);
    }

    #[test]
    fn the_key_list_parser_checks_the_format() {
        let hex = "0".repeat(63) + "1";
        let keys = parse_public_keys(&format!("7:{hex},255:{}", "a".repeat(64))).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].id, 7);
        assert_eq!(keys[0].bytes[31], 1);
        assert_eq!(parse_keys(&format!("7:{hex}")).1, 1);
        assert_eq!(parse_keys("").1, 0);
        for bad in [
            "x:00".to_string(),
            format!("256:{hex}"),
            format!("1:{}", "g".repeat(64)),
            format!("1:{hex}, 2:{hex}"),
            format!("1:{hex},1:{hex}"),
            format!("1:{}", &hex[..62]),
        ] {
            assert!(parse_public_keys(&bad).is_err(), "accepted {bad}");
        }
        assert!(parse_public_keys("  ").unwrap().is_empty());
    }

    #[test]
    #[should_panic(expected = "JF_LICENCE_PUBLIC_KEYS")]
    fn a_malformed_key_list_fails_to_parse() {
        parse_keys("1:abc");
    }

    #[test]
    fn this_test_build_has_no_release_keys_unless_given() {
        // CI and local builds carry none: keys made in tests never verify against them.
        if option_env!("JF_LICENCE_PUBLIC_KEYS").is_none_or(str::is_empty) {
            assert!(release_keys().is_empty());
        }
    }
}
