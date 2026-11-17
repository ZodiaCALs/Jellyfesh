//! Base64 with the standard alphabet (RFC 4648, section 4), as both
//! services use it: NetEase's request encoding and QQ Music's signature and
//! lyrics.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes `data`, with `=` padding.
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(chunk[0]) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        let symbol = |shift: u32| ALPHABET[((n >> shift) & 63) as usize] as char;
        out.push(symbol(18));
        out.push(symbol(12));
        out.push(if chunk.len() > 1 { symbol(6) } else { '=' });
        out.push(if chunk.len() > 2 { symbol(0) } else { '=' });
    }
    out
}

/// Decodes base64 text. Lenient where services are: white space is
/// skipped, padding may be missing, and the URL-safe `-` and `_` are
/// accepted. `None` for anything else that is not base64.
pub fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3 + 3);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    let mut padded = false;
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => {
                padded = true;
                continue;
            }
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            _ => return None,
        };
        if padded {
            return None;
        }
        acc = (acc << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    // One symbol on its own (6 bits) cannot end a valid encoding.
    (bits < 6).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test vectors of RFC 4648, section 10.
    const VECTORS: &[(&str, &str)] = &[
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ];

    #[test]
    fn matches_rfc_4648() {
        for (plain, encoded) in VECTORS {
            assert_eq!(encode(plain.as_bytes()), *encoded, "{plain}");
            assert_eq!(decode(encoded).unwrap(), plain.as_bytes(), "{encoded}");
        }
    }

    #[test]
    fn round_trips_every_byte() {
        let data: Vec<u8> = (0..=255u8).chain((0..=255u8).rev()).collect();
        for len in [0, 1, 2, 3, 4, 255, 511, 512] {
            assert_eq!(decode(&encode(&data[..len])).unwrap(), &data[..len]);
        }
        assert_eq!(encode(&[0xfb, 0xff]), "+/8=");
    }

    #[test]
    fn decodes_leniently_but_not_garbage() {
        assert_eq!(decode("Zm9v\nYmFy").unwrap(), b"foobar");
        assert_eq!(decode("Zm9vYg").unwrap(), b"foob");
        assert_eq!(decode("-_8").unwrap(), [0xfb, 0xff]);
        assert!(decode("Zm9v!").is_none());
        assert!(decode("Z").is_none());
        assert!(decode("Zg==Zg==").is_none());
    }
}
