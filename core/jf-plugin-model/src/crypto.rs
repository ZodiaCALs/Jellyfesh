//! The `crypto` import, on RustCrypto crates (feature `crypto`).
//!
//! The native host (`jf-plugin-host`) and Jellyfesh Web's plugin workers
//! both run these, so a plugin gets the same answers everywhere.
//!
//! RSA is public-key encryption only: the host never holds an RSA private
//! key, which is why RUSTSEC-2023-0071 (a timing side channel in private-key
//! operations of the `rsa` crate) does not apply (see `deny.toml`).

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::consts::U12;
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyInit, KeyIvInit};
use aes::{Aes128, Aes192, Aes256};
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{AesGcm, Nonce};
use hmac::{Hmac, Mac};
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs8::DecodePublicKey;
use rsa::{BigUint, Oaep, Pkcs1v15Encrypt, RsaPublicKey};
use sha2::Digest;

use crate::types::{
    AesMode, CryptoError, GcmParams, HashAlgorithm, RsaComponents, RsaPadding, RsaPublicKey as WitRsaKey,
};

/// Largest input to any crypto call.
pub const MAX_INPUT: usize = 16 * 1024 * 1024;
/// Most random bytes per call.
pub const MAX_RANDOM: u32 = 64 * 1024;

fn check_size(data: &[u8]) -> Result<(), CryptoError> {
    if data.len() > MAX_INPUT {
        Err(CryptoError::TooLarge)
    } else {
        Ok(())
    }
}

/// `crypto.digest`.
pub fn digest(algorithm: HashAlgorithm, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    check_size(data)?;
    Ok(match algorithm {
        HashAlgorithm::Md5 => md5::Md5::digest(data).to_vec(),
        HashAlgorithm::Sha1 => sha1::Sha1::digest(data).to_vec(),
        HashAlgorithm::Sha256 => sha2::Sha256::digest(data).to_vec(),
        HashAlgorithm::Sha512 => sha2::Sha512::digest(data).to_vec(),
    })
}

/// `crypto.hmac`.
pub fn hmac(algorithm: HashAlgorithm, key: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    check_size(data)?;
    check_size(key)?;
    fn run<M: Mac + hmac::KeyInit>(key: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut mac = <M as hmac::KeyInit>::new_from_slice(key)
            .map_err(|_| CryptoError::InvalidKey("bad HMAC key".into()))?;
        mac.update(data);
        Ok(mac.finalize().into_bytes().to_vec())
    }
    match algorithm {
        HashAlgorithm::Md5 => run::<Hmac<md5::Md5>>(key, data),
        HashAlgorithm::Sha1 => run::<Hmac<sha1::Sha1>>(key, data),
        HashAlgorithm::Sha256 => run::<Hmac<sha2::Sha256>>(key, data),
        HashAlgorithm::Sha512 => run::<Hmac<sha2::Sha512>>(key, data),
    }
}

fn key_error(key: &[u8]) -> CryptoError {
    CryptoError::InvalidKey(format!("an AES key is 16, 24 or 32 bytes, not {}", key.len()))
}

/// Runs `$body` with the type alias `$cipher` set to AES-128, AES-192 or
/// AES-256 according to the key length.
macro_rules! with_aes {
    ($key:expr, $cipher:ident => $body:expr) => {
        match $key.len() {
            16 => {
                type $cipher = Aes128;
                $body
            }
            24 => {
                type $cipher = Aes192;
                $body
            }
            32 => {
                type $cipher = Aes256;
                $body
            }
            _ => Err(key_error($key)),
        }
    };
}

fn iv_error() -> CryptoError {
    CryptoError::InvalidKey("the CBC IV is 16 bytes".into())
}

fn padding_error() -> CryptoError {
    CryptoError::InvalidInput("bad padding or length".into())
}

/// `crypto.aes-encrypt`.
pub fn aes_encrypt(mode: &AesMode, key: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    check_size(data)?;
    match mode {
        AesMode::Cbc(iv) => with_aes!(key, C => {
            let enc = cbc::Encryptor::<C>::new_from_slices(key, iv).map_err(|_| iv_error())?;
            Ok(enc.encrypt_padded_vec::<Pkcs7>(data))
        }),
        AesMode::Ecb => with_aes!(key, C => {
            let enc = ecb::Encryptor::<C>::new_from_slice(key).map_err(|_| key_error(key))?;
            Ok(enc.encrypt_padded_vec::<Pkcs7>(data))
        }),
        AesMode::Gcm(params) => gcm(key, params, data, true),
    }
}

/// `crypto.aes-decrypt`.
pub fn aes_decrypt(mode: &AesMode, key: &[u8], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    check_size(data)?;
    match mode {
        AesMode::Cbc(iv) => with_aes!(key, C => {
            let dec = cbc::Decryptor::<C>::new_from_slices(key, iv).map_err(|_| iv_error())?;
            dec.decrypt_padded_vec::<Pkcs7>(data).map_err(|_| padding_error())
        }),
        AesMode::Ecb => with_aes!(key, C => {
            let dec = ecb::Decryptor::<C>::new_from_slice(key).map_err(|_| key_error(key))?;
            dec.decrypt_padded_vec::<Pkcs7>(data).map_err(|_| padding_error())
        }),
        AesMode::Gcm(params) => gcm(key, params, data, false),
    }
}

fn gcm(key: &[u8], params: &GcmParams, data: &[u8], encrypt: bool) -> Result<Vec<u8>, CryptoError> {
    check_size(&params.aad)?;
    let nonce = Nonce::<U12>::try_from(params.nonce.as_slice())
        .map_err(|_| CryptoError::InvalidKey("the GCM nonce is 12 bytes".into()))?;
    let payload = Payload {
        msg: data,
        aad: &params.aad,
    };
    with_aes!(key, C => {
        let cipher = AesGcm::<C, U12>::new_from_slice(key).map_err(|_| key_error(key))?;
        if encrypt {
            cipher
                .encrypt(&nonce, payload)
                .map_err(|_| CryptoError::InvalidInput("cannot encrypt".into()))
        } else {
            cipher.decrypt(&nonce, payload).map_err(|_| CryptoError::DecryptFailed)
        }
    })
}

/// `crypto.rsa-encrypt`: public-key encryption only.
pub fn rsa_encrypt(key: &WitRsaKey, padding: RsaPadding, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    check_size(data)?;
    let invalid = |e: &dyn std::fmt::Display| CryptoError::InvalidKey(format!("not an RSA public key: {e}"));
    let key = match key {
        WitRsaKey::Pem(text) => RsaPublicKey::from_public_key_pem(text)
            .or_else(|_| RsaPublicKey::from_pkcs1_pem(text))
            .map_err(|e| invalid(&e))?,
        WitRsaKey::Der(der) => RsaPublicKey::from_public_key_der(der)
            .or_else(|_| RsaPublicKey::from_pkcs1_der(der))
            .map_err(|e| invalid(&e))?,
        WitRsaKey::Components(RsaComponents { modulus, exponent }) => {
            let bits = modulus.iter().skip_while(|b| **b == 0).count() * 8;
            if !(1024..=8192).contains(&bits) {
                return Err(CryptoError::InvalidKey(
                    "the modulus must be 1024 to 8192 bits".into(),
                ));
            }
            RsaPublicKey::new(BigUint::from_bytes_be(modulus), BigUint::from_bytes_be(exponent))
                .map_err(|e| invalid(&e))?
        }
    };
    let mut rng = rsa::rand_core::OsRng;
    let result = match padding {
        RsaPadding::Pkcs1v15 => key.encrypt(&mut rng, Pkcs1v15Encrypt, data),
        RsaPadding::OaepSha1 => key.encrypt(&mut rng, Oaep::new::<sha1_for_rsa::Sha1>(), data),
        RsaPadding::OaepSha256 => key.encrypt(&mut rng, Oaep::new::<rsa::sha2::Sha256>(), data),
    };
    result.map_err(|e| CryptoError::InvalidInput(e.to_string()))
}

/// `crypto.random-bytes`.
pub fn random_bytes(len: u32) -> Result<Vec<u8>, CryptoError> {
    if len > MAX_RANDOM {
        return Err(CryptoError::TooLarge);
    }
    let mut out = vec![0u8; len as usize];
    getrandom::fill(&mut out).map_err(|e| CryptoError::InvalidInput(format!("no randomness: {e}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::hex;

    #[test]
    fn digests_match_known_vectors() {
        let cases = [
            (HashAlgorithm::Md5, "900150983cd24fb0d6963f7d28e17f72"),
            (HashAlgorithm::Sha1, "a9993e364706816aba3e25717850c26c9cd0d89d"),
            (
                HashAlgorithm::Sha256,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                HashAlgorithm::Sha512,
                "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
                 2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
            ),
        ];
        for (algorithm, expected) in cases {
            assert_eq!(
                hex(&digest(algorithm, b"abc").unwrap()),
                expected,
                "{algorithm:?}"
            );
        }
    }

    #[test]
    fn hmacs_match_known_vectors() {
        let message = b"The quick brown fox jumps over the lazy dog";
        assert_eq!(
            hex(&hmac(HashAlgorithm::Sha256, b"key", message).unwrap()),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
        assert_eq!(
            hex(&hmac(HashAlgorithm::Md5, b"key", message).unwrap()),
            "80070713463e7749b90c2dc24911e275"
        );
    }

    #[test]
    fn aes_modes_match_vectors_and_round_trip() {
        let cbc = AesMode::Cbc(vec![0x24; 16]);
        let ct = aes_encrypt(&cbc, &[0x42; 16], b"hello world! this is my plaintext.").unwrap();
        assert_eq!(
            hex(&ct),
            "c7fe247ef97b21f07cbdd26cb5d346bfd27867cb00d9486723e159978fb9a5f9\
             14cfb228a710de4171e396e7b6cf859e"
        );
        assert_eq!(
            aes_decrypt(&cbc, &[0x42; 16], &ct).unwrap(),
            b"hello world! this is my plaintext."
        );

        let ecb = aes_encrypt(&AesMode::Ecb, &[0; 16], &[0; 16]).unwrap();
        assert_eq!(hex(&ecb[..16]), "66e94bd4ef8a2c3b884cfa59ca342b2e");
        assert_eq!(aes_decrypt(&AesMode::Ecb, &[0; 16], &ecb).unwrap(), vec![0; 16]);

        let gcm_mode = AesMode::Gcm(GcmParams {
            nonce: vec![7; 12],
            aad: b"header".to_vec(),
        });
        for key_len in [16, 24, 32] {
            let key = vec![9u8; key_len];
            let sealed = aes_encrypt(&gcm_mode, &key, b"secret").unwrap();
            assert_eq!(sealed.len(), 6 + 16);
            assert_eq!(aes_decrypt(&gcm_mode, &key, &sealed).unwrap(), b"secret");
            let mut tampered = sealed.clone();
            tampered[0] ^= 1;
            assert_eq!(
                aes_decrypt(&gcm_mode, &key, &tampered),
                Err(CryptoError::DecryptFailed)
            );
        }

        assert!(matches!(
            aes_encrypt(&AesMode::Ecb, &[0; 15], b"x"),
            Err(CryptoError::InvalidKey(_))
        ));
        assert!(matches!(
            aes_encrypt(&AesMode::Cbc(vec![0; 8]), &[0; 16], b"x"),
            Err(CryptoError::InvalidKey(_))
        ));
        assert!(matches!(
            aes_decrypt(&AesMode::Ecb, &[0; 16], &[1, 2, 3]),
            Err(CryptoError::InvalidInput(_))
        ));
    }

    #[test]
    fn rsa_encrypts_with_public_keys_only() {
        use rsa::pkcs8::EncodePublicKey;
        use rsa::traits::PublicKeyParts;
        // A private key exists only in this test, to check the ciphertexts.
        let private = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 1024).unwrap();
        let public = private.to_public_key();
        let pem = public.to_public_key_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let components = WitRsaKey::Components(RsaComponents {
            modulus: public.n().to_bytes_be(),
            exponent: public.e().to_bytes_be(),
        });
        for (key, padding) in [
            (WitRsaKey::Pem(pem.clone()), RsaPadding::Pkcs1v15),
            (WitRsaKey::Pem(pem.clone()), RsaPadding::OaepSha1),
            (components, RsaPadding::OaepSha256),
        ] {
            let ct = rsa_encrypt(&key, padding, b"token").unwrap();
            let pt = match padding {
                RsaPadding::Pkcs1v15 => private.decrypt(Pkcs1v15Encrypt, &ct),
                RsaPadding::OaepSha1 => private.decrypt(Oaep::new::<sha1_for_rsa::Sha1>(), &ct),
                RsaPadding::OaepSha256 => private.decrypt(Oaep::new::<rsa::sha2::Sha256>(), &ct),
            };
            assert_eq!(pt.unwrap(), b"token");
        }
        assert!(rsa_encrypt(&WitRsaKey::Pem("nope".into()), RsaPadding::Pkcs1v15, b"x").is_err());
        let tiny = WitRsaKey::Components(RsaComponents {
            modulus: vec![0xff; 16],
            exponent: vec![1, 0, 1],
        });
        assert!(rsa_encrypt(&tiny, RsaPadding::Pkcs1v15, b"x").is_err());
    }

    #[test]
    fn random_bytes_are_bounded() {
        assert_eq!(random_bytes(32).unwrap().len(), 32);
        assert_ne!(random_bytes(32).unwrap(), random_bytes(32).unwrap());
        assert_eq!(random_bytes(MAX_RANDOM + 1), Err(CryptoError::TooLarge));
    }
}
