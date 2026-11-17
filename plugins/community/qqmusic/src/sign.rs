//! The y.qq.com web player's request signature ("zzb") and its `g_tk`
//! value, written from the protocol, not from anyone's code (Tencent's
//! minified JavaScript included):
//!
//! 1. the MD5 of the exact request body, as 32 uppercase hex digits;
//! 2. eight of those digits, at positions 21, 4, 9, 26, 16, 20, 27 and 30;
//! 3. the 16 digest bytes, each XORed with a fixed byte of the web client,
//!    in base64 without `/`, `+` and `=`;
//! 4. eight more digits, at positions 18, 11, 3, 2, 1, 7, 6 and 25;
//! 5. `zzb` followed by 2, 3 and 4, all in lowercase.
//!
//! The signed request goes to `musics.fcg?_=<ms>&sign=<signature>`. The
//! positions and the 16 bytes are what every browser receives with the web
//! player; they protect nothing. QQ Music has changed this scheme before; if
//! the service stops accepting it, this module is what needs updating (after
//! the same legal check, see the README).

/// Digit positions before the middle part.
const HEAD: [usize; 8] = [21, 4, 9, 26, 16, 20, 27, 30];
/// Digit positions after it.
const TAIL: [usize; 8] = [18, 11, 3, 2, 1, 7, 6, 25];
/// The bytes the digest is XORed with.
const MIX: [u8; 16] = [
    212, 45, 80, 68, 195, 163, 163, 203, 157, 220, 254, 91, 204, 79, 104, 6,
];

/// The signature for a request body whose MD5 digest is `digest` (16 bytes).
pub fn sign(digest: &[u8; 16]) -> String {
    let hex: Vec<u8> = digest
        .iter()
        .flat_map(|byte| [hex_digit(byte >> 4), hex_digit(byte & 15)])
        .collect();
    let mixed: Vec<u8> = digest.iter().zip(MIX).map(|(byte, mix)| byte ^ mix).collect();
    let middle: String = jf_community_common::base64::encode(&mixed)
        .chars()
        .filter(|c| !matches!(c, '/' | '+' | '='))
        .collect();
    let mut out = String::from("zzb");
    out.extend(HEAD.iter().map(|&i| hex[i] as char));
    out.push_str(&middle);
    out.extend(TAIL.iter().map(|&i| hex[i] as char));
    out.to_lowercase()
}

fn hex_digit(nibble: u8) -> u8 {
    b"0123456789ABCDEF"[usize::from(nibble & 15)]
}

/// `g_tk`: the web player's 31-bit hash of the session key (5381 for none).
pub fn g_tk(key: &str) -> u32 {
    let mut hash: u32 = 5381;
    for unit in key.encode_utf16() {
        hash = hash.wrapping_add(hash << 5).wrapping_add(u32::from(unit));
    }
    hash & 0x7fff_ffff
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(hex: &str) -> [u8; 16] {
        let bytes: Vec<u8> = (0..32)
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        bytes.try_into().unwrap()
    }

    /// Reference values from an independent implementation of the same
    /// description (Python, see the commit that added them). They show the
    /// code does what the description says; whether QQ Music still accepts
    /// it could not be checked from here.
    #[test]
    fn matches_the_reference() {
        for (md5, expected) in [
            // MD5 of the empty body.
            (
                "D41D8CD98F00B204E9800998ECF8427E",
                "zzb98ffe087addcnuyjec90xpfdilcqea80d149dc",
            ),
            // MD5 of "{}".
            (
                "99914B932BD37A50B983C5E7C90AE93B",
                "zzb54b0bca3tbwb1hw2zskxzu8buwbpq83199399",
            ),
            (
                "ACB14A2CBDE5764F868A915354B57282",
                "zzb14db8958ejwaah5g1yqbvm8impoaha851bcc24",
            ),
            (
                "82A4041BF0EF393E9D56A48AD67F6A6B",
                "zzb40079af6voluxznmmvuailrrgjacbq5f4a2b16",
            ),
        ] {
            assert_eq!(sign(&digest(md5)), expected, "{md5}");
        }
    }

    #[test]
    fn hashes_keys() {
        assert_eq!(g_tk(""), 5381);
        assert_eq!(g_tk("a"), 177_670);
        assert_eq!(g_tk("Q_H_L_5mockmusickey0123456789"), 1_389_036_381);
        assert_eq!(g_tk("W_X_mock"), 724_166_652);
    }
}
