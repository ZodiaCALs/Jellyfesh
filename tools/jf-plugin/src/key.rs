//! Signing keys: an ed25519 seed written as 64 hex characters, kept in a
//! file only its owner can read (mode 0600) or, in CI, in the
//! `JF_PLUGIN_SIGNING_KEY` environment variable. The seed is never printed;
//! only the public key and its key id are.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use jf_plugin_host::trust;
use zeroize::Zeroizing;

/// The environment variable that may hold the signing key (64 hex
/// characters, optionally followed by a newline).
pub const KEY_ENV: &str = "JF_PLUGIN_SIGNING_KEY";

/// An ed25519 signing key. `Debug` shows only the key id, and the seed is
/// wiped from memory when the key is dropped.
pub struct SigningKey {
    seed: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningKey")
            .field("key_id", &self.key_id())
            .finish()
    }
}

impl SigningKey {
    /// A new key from the operating system's random number generator.
    pub fn generate() -> Result<SigningKey> {
        let mut seed = Zeroizing::new([0u8; 32]);
        getrandom::fill(seed.as_mut())
            .map_err(|e| anyhow!("the operating system gave no random bytes: {e}"))?;
        Ok(SigningKey { seed })
    }

    /// Parses 64 hex characters (either case), ignoring white space around
    /// them such as a trailing newline. The error never contains the text.
    pub fn parse(text: &str) -> Result<SigningKey> {
        let text = text.trim();
        if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            bail!(
                "a signing key is 64 hex characters (the ed25519 seed); this one has {} characters{}",
                text.chars().count(),
                if text.len() == 64 {
                    ", not all of them hex digits"
                } else {
                    ""
                }
            );
        }
        let mut seed = Zeroizing::new([0u8; 32]);
        for (i, pair) in text.as_bytes().chunks_exact(2).enumerate() {
            seed[i] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
        }
        Ok(SigningKey { seed })
    }

    /// Reads a key file (as written by [`keygen`]). Warns on standard error
    /// when other users may read it.
    pub fn from_file(path: &Path) -> Result<SigningKey> {
        let text = Zeroizing::new(
            std::fs::read_to_string(path)
                .with_context(|| format!("reading the key file {}", path.display()))?,
        );
        warn_if_shared(path);
        SigningKey::parse(&text).with_context(|| format!("the key file {}", path.display()))
    }

    /// The key in `JF_PLUGIN_SIGNING_KEY`, if that variable is set and not
    /// empty.
    pub fn from_env() -> Result<Option<SigningKey>> {
        match std::env::var(KEY_ENV) {
            Ok(text) => {
                let text = Zeroizing::new(text);
                if text.trim().is_empty() {
                    Ok(None)
                } else {
                    SigningKey::parse(&text).map(Some).context(KEY_ENV)
                }
            }
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => bail!("{KEY_ENV} is not valid text"),
        }
    }

    /// The key in `file` if one is given, otherwise the one in
    /// `JF_PLUGIN_SIGNING_KEY`.
    pub fn from_file_or_env(file: Option<&Path>) -> Result<SigningKey> {
        match file {
            Some(path) => SigningKey::from_file(path),
            None => SigningKey::from_env()?.ok_or_else(|| {
                anyhow!("no signing key: pass --key <file> or set {KEY_ENV} to the key's 64 hex characters")
            }),
        }
    }

    /// The seed, for `PackageBuilder::sign`.
    pub fn seed(&self) -> [u8; 32] {
        *self.seed
    }

    /// The public half.
    pub fn public_key(&self) -> [u8; 32] {
        trust::public_key(&self.seed)
    }

    /// The public key as 64 lowercase hex characters (safe to share).
    pub fn public_hex(&self) -> String {
        hex(&self.public_key())
    }

    /// The key id (16 hex digits) that signatures and the app show.
    pub fn key_id(&self) -> String {
        trust::key_id(&self.public_key())
    }

    /// The published test key (`jf_plugin_host::trust::test_key_seed`), which
    /// only test hosts trust.
    pub fn test_key() -> SigningKey {
        SigningKey {
            seed: Zeroizing::new(trust::test_key_seed()),
        }
    }
}

/// Writes a new key to `path`, readable and writable by its owner only
/// (mode 0600 on Unix). Refuses to replace an existing file unless `force`.
pub fn keygen(path: &Path, force: bool) -> Result<SigningKey> {
    let key = SigningKey::generate()?;
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            anyhow!("{} already exists; pass --force to replace it", path.display())
        } else {
            anyhow!("creating {}: {e}", path.display())
        }
    })?;
    // A replaced file keeps its old mode; make it private either way.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("making {} private", path.display()))?;
    }
    let text = Zeroizing::new(format!("{}\n", hex(&key.seed[..])));
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(key)
}

#[cfg(unix)]
fn warn_if_shared(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path)
        && meta.permissions().mode() & 0o077 != 0
    {
        eprintln!(
            "warning: other users can read the key file {}; run `chmod 600` on it",
            path.display()
        );
    }
}

#[cfg(not(unix))]
fn warn_if_shared(_path: &Path) {}

fn hex_value(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'a'..=b'f' => digit - b'a' + 10,
        _ => digit - b'A' + 10,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_seeds() {
        let test = SigningKey::test_key();
        let text = hex(&test.seed());
        let parsed = SigningKey::parse(&format!("{text}\n")).unwrap();
        assert_eq!(parsed.seed(), test.seed());
        assert_eq!(
            SigningKey::parse(&text.to_uppercase()).unwrap().seed(),
            test.seed()
        );
        assert_eq!(parsed.key_id().len(), 16);
        for bad in [
            "",
            "abc",
            &text[..63],
            &format!("{}g", &text[..63]),
            &format!("{text}00"),
        ] {
            let error = format!("{:#}", SigningKey::parse(bad).unwrap_err());
            assert!(error.contains("64 hex characters"), "{error}");
            if bad.len() > 8 {
                assert!(!error.contains(&bad[..8]), "the error shows the key: {error}");
            }
        }
    }

    #[test]
    fn debug_shows_only_the_key_id() {
        let key = SigningKey::test_key();
        let shown = format!("{key:?}");
        assert!(shown.contains(&key.key_id()));
        assert!(!shown.contains(&hex(&key.seed())));
    }
}
