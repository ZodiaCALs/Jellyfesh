//! `jf-licence`: the owner's tool for Jellyfesh Pro licence keys (ADR-0021 §2).
//!
//! ```sh
//! jf-licence keygen --key-id 1 --out ~/keys/jellyfesh-licence.key   # prints 1:<public hex>
//! jf-licence issue --secret ~/keys/jellyfesh-licence.key --key-id 1  # prints a JFP1 key
//! JF_LICENCE_SIGNING_KEY=… jf-licence issue --key-id 1 --count 10
//! jf-licence verify JFP1-… --public 1:<public hex>
//! ```
//!
//! The secret is an Ed25519 seed written as 64 hex characters, in a file only
//! its owner can read or in `JF_LICENCE_SIGNING_KEY`; the tool never prints
//! it, and `keygen` refuses to write it inside a git working tree. The public
//! line it prints goes into the repository variable `JF_LICENCE_PUBLIC_KEYS`
//! (`id:hex,id:hex`) that release builds are made with.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand};
use jf_licence::{Licence, LicenceError, Product, PublicKey};

#[derive(Parser)]
#[command(
    name = "jf-licence",
    version,
    about = "Jellyfesh Pro licence keys: key pairs, issuing, checking"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Makes a signing key pair: writes the secret (mode 0600) and prints the public `id:hex`.
    Keygen {
        /// The key id licence keys will name (0–255).
        #[arg(long)]
        key_id: u8,
        /// Where to write the secret; never inside a git working tree.
        #[arg(long)]
        out: PathBuf,
        /// Replace an existing file.
        #[arg(long)]
        force: bool,
    },
    /// Issues Jellyfesh Pro licence keys, one per line.
    Issue {
        /// The secret from `keygen` (or set JF_LICENCE_SIGNING_KEY).
        #[arg(long)]
        secret: Option<PathBuf>,
        /// The key id of that secret.
        #[arg(long)]
        key_id: u8,
        /// The issue day as days since 1970-01-01 (default: today, UTC).
        #[arg(long)]
        day: Option<u16>,
        /// How many keys.
        #[arg(long, default_value_t = 1)]
        count: u32,
    },
    /// Checks a licence key against public keys; exit status 1 when it is not valid.
    Verify {
        /// The key (`JFP1-…`).
        key: String,
        /// Public keys as `id:hex,id:hex` (default: the ones built into this tool).
        #[arg(long)]
        public: Option<String>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("jf-licence: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<ExitCode> {
    match command {
        Command::Keygen { key_id, out, force } => {
            keygen(key_id, &out, force)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Issue {
            secret,
            key_id,
            day,
            count,
        } => {
            let secret = read_secret(secret.as_deref())?;
            let public = jf_licence::public_key(&secret, key_id);
            let day = match day {
                Some(day) => day,
                None => today()?,
            };
            ensure!((1..=10_000).contains(&count), "--count is 1 to 10000");
            for _ in 0..count {
                let licence = jf_licence::new_licence(day);
                let key = jf_licence::issue(&secret, key_id, &licence);
                // Check what was made with the public key, as the apps will.
                jf_licence::verify_with(&key, &[public], &[])
                    .map_err(|e| anyhow::anyhow!("the issued key does not verify: {e}"))?;
                println!("{key}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Verify { key, public } => {
            let keys: Vec<PublicKey> = match public {
                Some(text) => {
                    jf_licence::parse_public_keys(&text).map_err(|e| anyhow::anyhow!("--public: {e}"))?
                }
                None => jf_licence::release_keys(),
            };
            ensure!(
                !keys.is_empty(),
                "no public keys: pass --public id:hex (this build has none built in)"
            );
            match jf_licence::verify(&key, &keys) {
                Ok(licence) => {
                    print_licence(&licence);
                    Ok(ExitCode::SUCCESS)
                }
                Err(error) => {
                    println!("invalid: {} ({error})", error_name(error));
                    Ok(ExitCode::from(1))
                }
            }
        }
    }
}

fn keygen(key_id: u8, out: &Path, force: bool) -> Result<()> {
    let absolute = std::path::absolute(out).with_context(|| format!("{}", out.display()))?;
    if let Some(tree) = git_tree(&absolute) {
        bail!(
            "{} is inside the git working tree {}: keep the secret outside any repository",
            absolute.display(),
            tree.display()
        );
    }
    ensure!(
        force || !absolute.exists(),
        "{} exists; pass --force to replace it",
        absolute.display()
    );
    let (secret, public) = jf_licence::generate_keypair_with_id(key_id);
    let hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
    write_private(&absolute, format!("{hex}\n").as_bytes())?;
    println!("{}", jf_licence::public_key_text(&public));
    eprintln!(
        "jf-licence: the secret is in {} (mode 0600). Add the line above to JF_LICENCE_PUBLIC_KEYS.",
        absolute.display()
    );
    Ok(())
}

/// The git working tree `path` is in, if any.
fn git_tree(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("writing {}", path.display()))?;
    file.write_all(bytes)?;
    // An existing file keeps its mode: set it.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))
}

fn read_secret(path: Option<&Path>) -> Result<[u8; 32]> {
    let text = match path {
        Some(path) => std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
        None => std::env::var("JF_LICENCE_SIGNING_KEY")
            .context("pass --secret PATH or set JF_LICENCE_SIGNING_KEY")?,
    };
    let text = text.trim();
    ensure!(
        text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()),
        "the secret must be 64 hex characters"
    );
    let mut secret = [0u8; 32];
    for (i, byte) in secret.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)?;
    }
    Ok(secret)
}

fn today() -> Result<u16> {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs()
        / 86_400;
    u16::try_from(days).context("the date is past what a key can hold")
}

fn print_licence(licence: &Licence) {
    let product = match licence.product {
        Product::Pro => "Jellyfesh Pro",
    };
    println!("valid: {product}");
    println!("licence id  {} (…{})", licence.id_text(), licence.id_suffix());
    println!("signing key {}", licence.key_id);
    println!("issued day  {} (days since 1970-01-01)", licence.issued_day);
}

fn error_name(error: LicenceError) -> &'static str {
    match error {
        LicenceError::Malformed => "malformed",
        LicenceError::UnknownKey => "unknown key",
        LicenceError::BadSignature => "bad signature",
        LicenceError::WrongProduct => "wrong product",
        LicenceError::Revoked => "revoked",
    }
}
