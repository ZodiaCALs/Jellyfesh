//! Textbook ("raw") RSA: `m^e mod n` on big unsigned numbers, nothing else.
//!
//! NetEase's web request encoding (see [`crate::weapi`]) applies the public
//! key to the bare message, without PKCS #1 or OAEP padding. The host's
//! `crypto` import offers only padded RSA, so the plugin does this one step
//! itself. It is a public-key operation on public data; there is no private
//! key anywhere in the plugin.
//!
//! Numbers are little-endian vectors of 32-bit limbs. Modular
//! multiplication is Montgomery's (CIOS form), so the step stays well inside
//! the host's CPU budget even in debug builds.

/// A modulus prepared for exponentiation.
#[derive(Debug, Clone)]
pub struct Modulus {
    /// The modulus, `len` limbs, the top one not zero.
    n: Vec<u32>,
    /// `-n⁻¹ mod 2³²`.
    n0_inv: u32,
    /// `R² mod n` with `R = 2^(32·len)`.
    r2: Vec<u32>,
}

impl Modulus {
    /// Prepares an odd modulus given as big-endian bytes (leading zeros are
    /// fine). `None` for an even or zero modulus.
    pub fn from_be_bytes(bytes: &[u8]) -> Option<Modulus> {
        let n = trimmed(from_be_bytes(bytes));
        if n.is_empty() || n[0] & 1 == 0 || n == [1] {
            return None;
        }
        let n0_inv = inverse_mod_word(n[0]).wrapping_neg();
        // R mod n by doubling 1, then R² mod n by doubling R mod n as often.
        let bits = 32 * n.len();
        let mut x = vec![0u32; n.len()];
        x[0] = 1;
        for _ in 0..2 * bits {
            double_mod(&mut x, &n);
        }
        Some(Modulus { n0_inv, r2: x, n })
    }

    /// The modulus's length in bytes (128 for a 1024-bit key).
    pub fn byte_len(&self) -> usize {
        let top = self.n[self.n.len() - 1];
        (self.n.len() - 1) * 4 + (32 - top.leading_zeros() as usize).div_ceil(8)
    }

    /// `base^exponent mod n`, all big-endian bytes. The result has
    /// [`Modulus::byte_len`] bytes (zero-padded). `None` when `base` is not
    /// smaller than the modulus.
    pub fn pow(&self, base: &[u8], exponent: &[u8]) -> Option<Vec<u8>> {
        let len = self.n.len();
        let mut b = trimmed(from_be_bytes(base));
        if b.len() > len || (b.len() == len && !less(&b, &self.n)) {
            return None;
        }
        b.resize(len, 0);
        let mut one = vec![0u32; len];
        one[0] = 1;
        // Into Montgomery form: x·R mod n.
        let base_m = self.mul(&b, &self.r2);
        let mut acc = self.mul(&one, &self.r2);
        for byte in exponent {
            for bit in (0..8).rev() {
                acc = self.mul(&acc, &acc);
                if (byte >> bit) & 1 == 1 {
                    acc = self.mul(&acc, &base_m);
                }
            }
        }
        // Out of Montgomery form.
        let result = self.mul(&acc, &one);
        Some(to_be_bytes(&result, self.byte_len()))
    }

    /// Montgomery product `a·b·R⁻¹ mod n` of two numbers below `n`.
    fn mul(&self, a: &[u32], b: &[u32]) -> Vec<u32> {
        let n = &self.n;
        let s = n.len();
        let mut t = vec![0u32; s + 2];
        for &bi in b.iter().take(s) {
            let mut carry = 0u64;
            for j in 0..s {
                let sum = u64::from(t[j]) + u64::from(a[j]) * u64::from(bi) + carry;
                t[j] = sum as u32;
                carry = sum >> 32;
            }
            let sum = u64::from(t[s]) + carry;
            t[s] = sum as u32;
            t[s + 1] = (sum >> 32) as u32;

            let m = t[0].wrapping_mul(self.n0_inv);
            let mut carry = (u64::from(t[0]) + u64::from(m) * u64::from(n[0])) >> 32;
            for j in 1..s {
                let sum = u64::from(t[j]) + u64::from(m) * u64::from(n[j]) + carry;
                t[j - 1] = sum as u32;
                carry = sum >> 32;
            }
            let sum = u64::from(t[s]) + carry;
            t[s - 1] = sum as u32;
            t[s] = t[s + 1] + (sum >> 32) as u32;
            t[s + 1] = 0;
        }
        let mut result: Vec<u32> = t[..s].to_vec();
        if t[s] != 0 || !less(&result, n) {
            sub_in_place(&mut result, n);
        }
        result
    }
}

/// `x⁻¹ mod 2³²` for odd `x` (Newton's iteration doubles the correct bits).
fn inverse_mod_word(x: u32) -> u32 {
    let mut inv: u32 = 1;
    for _ in 0..5 {
        inv = inv.wrapping_mul(2u32.wrapping_sub(x.wrapping_mul(inv)));
    }
    inv
}

/// `x = 2x mod n` for `x < n`.
fn double_mod(x: &mut [u32], n: &[u32]) {
    let mut carry = 0u32;
    for limb in x.iter_mut() {
        let next = *limb >> 31;
        *limb = (*limb << 1) | carry;
        carry = next;
    }
    if carry == 1 || !less(x, n) {
        sub_in_place(x, n);
    }
}

/// `a -= b`, modulo `2^(32·len)` (same lengths).
fn sub_in_place(a: &mut [u32], b: &[u32]) {
    let mut borrow = 0i64;
    for (x, y) in a.iter_mut().zip(b) {
        let diff = i64::from(*x) - i64::from(*y) - borrow;
        if diff < 0 {
            *x = (diff + (1i64 << 32)) as u32;
            borrow = 1;
        } else {
            *x = diff as u32;
            borrow = 0;
        }
    }
}

/// `a < b` for numbers of the same length.
fn less(a: &[u32], b: &[u32]) -> bool {
    for (x, y) in a.iter().rev().zip(b.iter().rev()) {
        if x != y {
            return x < y;
        }
    }
    false
}

fn from_be_bytes(bytes: &[u8]) -> Vec<u32> {
    let mut limbs = Vec::with_capacity(bytes.len().div_ceil(4));
    for chunk in bytes.rchunks(4) {
        let mut limb = 0u32;
        for byte in chunk {
            limb = (limb << 8) | u32::from(*byte);
        }
        limbs.push(limb);
    }
    limbs
}

fn trimmed(mut limbs: Vec<u32>) -> Vec<u32> {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
    limbs
}

fn to_be_bytes(limbs: &[u32], len: usize) -> Vec<u8> {
    let mut bytes: Vec<u8> = limbs.iter().rev().flat_map(|l| l.to_be_bytes()).collect();
    let skip = bytes.len().saturating_sub(len);
    bytes.drain(..skip);
    let mut out = vec![0u8; len - bytes.len()];
    out.extend(bytes);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn small_numbers() {
        // 4^13 mod 497 = 445 (the textbook example).
        let m = Modulus::from_be_bytes(&[0x01, 0xf1]).unwrap();
        assert_eq!(m.byte_len(), 2);
        assert_eq!(m.pow(&[4], &[13]).unwrap(), vec![0x01, 0xbd]);
        assert_eq!(m.pow(&[0], &[5]).unwrap(), vec![0, 0]);
        assert_eq!(m.pow(&[7], &[0]).unwrap(), vec![0, 1]);
        assert!(
            m.pow(&[0x01, 0xf1], &[1]).is_none(),
            "base must be below the modulus"
        );
        assert!(Modulus::from_be_bytes(&[0x10]).is_none(), "even");
        assert!(Modulus::from_be_bytes(&[0, 0]).is_none(), "zero");
    }

    #[test]
    fn word_inverse() {
        for x in [1u32, 3, 0xffff_ffff, 0x1234_5679, 0xe7b8_220b] {
            assert_eq!(x.wrapping_mul(inverse_mod_word(x)), 1);
        }
    }

    /// A modulus whose top limb is small, so `R mod n` is not just `R - n`.
    #[test]
    fn uneven_lengths() {
        // 2^64 + 13 (odd, three limbs with a top limb of 1); 3^5 = 243.
        let m = Modulus::from_be_bytes(&unhex("01000000000000000d")).unwrap();
        assert_eq!(m.byte_len(), 9);
        assert_eq!(hex(&m.pow(&[3], &[5]).unwrap()), "0000000000000000f3");
        // (2^64 + 12)^2 mod (2^64 + 13) = 1.
        let base = unhex("01000000000000000c");
        assert_eq!(hex(&m.pow(&base, &[2]).unwrap()), "000000000000000001");
    }
}
