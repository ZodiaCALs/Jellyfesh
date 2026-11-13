//! `.jfpkg` packages: reading, verifying, signing and writing.
//!
//! A package is a zip archive with exactly these entries, no folders:
//!
//! | Entry | Required | Limit |
//! | --- | --- | --- |
//! | `manifest.toml` | yes | 64 KiB, see [`crate::manifest`] |
//! | `plugin.wasm` | yes | 32 MiB, a component for `jellyfesh:plugin/source@1` |
//! | the settings schema named by `[settings] schema` | if named | 64 KiB, see [`crate::schema`] |
//! | `icon.png` | no | 64 KiB, PNG |
//! | `SIGNATURE` | no | 4 KiB, see below |
//!
//! Anything else is an error, including `icon.svg`: SVG icons need a
//! sanitiser and are not supported in v1.
//!
//! # Signatures
//!
//! The signed digest is the SHA-256 of this text (UTF-8, `\n` line ends):
//!
//! ```text
//! jellyfesh-package-v1
//! <name>
//! <length in bytes, decimal>
//! <lowercase hex SHA-256 of the contents>
//! … the three lines above for every entry except SIGNATURE, sorted by name (byte order)
//! ```
//!
//! `SIGNATURE` is four lines of text:
//!
//! ```text
//! jellyfesh-signature-v1
//! key-id: <16 hex digits: the first 8 bytes of the SHA-256 of the public key>
//! public-key: <64 hex digits: the ed25519 public key>
//! signature: <128 hex digits: ed25519 over the 32-byte digest>
//! ```
//!
//! A signature that does not verify fails the install. A valid signature by
//! a key in the host's [`TrustList`] makes the package official; any other
//! valid signature marks it as a community package with that fingerprint.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};
use sha2::{Digest, Sha256};

use crate::error::HostError;
use crate::manifest::{self, Manifest};
use crate::schema::{Format, PropertyKind, SettingsSchema};
use crate::trust::{self, TrustList, hex, key_id, unhex};
use crate::types::SignatureStatus;

/// Name of the manifest entry.
pub const MANIFEST: &str = "manifest.toml";
/// Name of the component entry.
pub const WASM: &str = "plugin.wasm";
/// Name of the icon entry.
pub const ICON: &str = "icon.png";
/// Name of the signature entry.
pub const SIGNATURE: &str = "SIGNATURE";

const MAX_ARCHIVE: u64 = 40 * 1024 * 1024;
const MAX_ENTRIES: usize = 8;
const MAX_MANIFEST: u64 = 64 * 1024;
const MAX_WASM: u64 = 32 * 1024 * 1024;
const MAX_SCHEMA: u64 = 64 * 1024;
const MAX_ICON: u64 = 64 * 1024;
const MAX_SIGNATURE: u64 = 4 * 1024;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// A package that passed every check except compiling the component.
#[derive(Debug, Clone)]
pub struct Package {
    /// The validated manifest.
    pub manifest: Manifest,
    /// The component.
    pub wasm: Vec<u8>,
    /// The icon, if any.
    pub icon: Option<Vec<u8>>,
    /// The settings schema (empty when the package has none).
    pub settings_schema: SettingsSchema,
    /// Who signed it.
    pub signature: SignatureStatus,
}

fn bad(message: impl Into<String>) -> HostError {
    HostError::Package(message.into())
}

/// Reads and checks a package: entries, sizes, manifest, schema, icon and
/// signature. The API version is checked too.
pub fn read(bytes: &[u8], trust: &TrustList) -> Result<Package, HostError> {
    if bytes.len() as u64 > MAX_ARCHIVE {
        return Err(bad("the package is larger than 40 MiB"));
    }
    let files = unzip(bytes)?;

    let manifest_text = files
        .get(MANIFEST)
        .ok_or_else(|| bad("manifest.toml is missing"))?;
    let manifest_text = std::str::from_utf8(manifest_text).map_err(|_| bad("manifest.toml is not UTF-8"))?;
    let manifest = manifest::parse(manifest_text)?;
    manifest.check_api_version()?;

    let wasm = files
        .get(WASM)
        .ok_or_else(|| bad("plugin.wasm is missing"))?
        .clone();

    let settings_schema = match &manifest.settings_schema {
        Some(name) => {
            let text = files
                .get(name.as_str())
                .ok_or_else(|| bad(format!("the settings schema {name} is missing")))?;
            let text = std::str::from_utf8(text).map_err(|_| bad(format!("{name} is not UTF-8")))?;
            SettingsSchema::parse(text).map_err(|e| HostError::Manifest(format!("{name}: {e}")))?
        }
        None => SettingsSchema::empty(),
    };
    check_settings_references(&manifest, &settings_schema)?;

    for name in files.keys() {
        let known = matches!(name.as_str(), MANIFEST | WASM | ICON | SIGNATURE)
            || manifest.settings_schema.as_deref() == Some(name.as_str());
        if !known {
            return Err(bad(format!("unexpected entry {name:?}")));
        }
    }

    let icon = match files.get(ICON) {
        Some(png) if png.starts_with(PNG_MAGIC) => Some(png.clone()),
        Some(_) => return Err(bad("icon.png is not a PNG file")),
        None => None,
    };

    let signature = match files.get(SIGNATURE) {
        Some(text) => verify_signature(text, &files, trust)?,
        None => SignatureStatus::Unsigned,
    };

    Ok(Package {
        manifest,
        wasm,
        icon,
        settings_schema,
        signature,
    })
}

/// Every `${settings.<key>}` must name a string property with `format: uri`.
fn check_settings_references(manifest: &Manifest, schema: &SettingsSchema) -> Result<(), HostError> {
    for key in manifest.referenced_settings() {
        let property = schema.property(key).ok_or_else(|| {
            HostError::Manifest(format!(
                "${{settings.{key}}} is not a property of the settings schema"
            ))
        })?;
        if property.kind != PropertyKind::String || property.format != Some(Format::Uri) {
            return Err(HostError::Manifest(format!(
                "${{settings.{key}}} must be a string property with \"format\": \"uri\""
            )));
        }
    }
    Ok(())
}

fn unzip(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, HostError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| bad(format!("not a zip archive: {e}")))?;
    if archive.len() > MAX_ENTRIES {
        return Err(bad(format!("more than {MAX_ENTRIES} entries")));
    }
    let mut files = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| bad(format!("entry {index}: {e}")))?;
        let name = entry
            .name()
            .map_err(|e| bad(format!("entry {index}: {e}")))?
            .into_owned();
        if entry.is_dir() || name.contains('/') || name.contains('\\') {
            return Err(bad(format!("folders are not allowed ({name:?})")));
        }
        if name == "icon.svg" {
            return Err(bad(
                "icon.svg is not supported yet: SVG icons need a sanitiser; use icon.png",
            ));
        }
        let limit = match name.as_str() {
            MANIFEST => MAX_MANIFEST,
            WASM => MAX_WASM,
            ICON => MAX_ICON,
            SIGNATURE => MAX_SIGNATURE,
            _ => MAX_SCHEMA,
        };
        // The declared size can lie; read at most one byte past the limit.
        let mut contents = Vec::new();
        (&mut entry)
            .take(limit + 1)
            .read_to_end(&mut contents)
            .map_err(|e| bad(format!("{name}: {e}")))?;
        if contents.len() as u64 > limit {
            return Err(bad(format!("{name} is larger than {} KiB", limit / 1024)));
        }
        if files.insert(name.clone(), contents).is_some() {
            return Err(bad(format!("{name} appears twice")));
        }
    }
    Ok(files)
}

/// The digest a package signature covers: see the module docs.
pub fn signing_digest(files: &BTreeMap<String, Vec<u8>>) -> [u8; 32] {
    let mut text = String::from("jellyfesh-package-v1\n");
    for (name, contents) in files {
        if name == SIGNATURE {
            continue;
        }
        text.push_str(name);
        text.push('\n');
        text.push_str(&contents.len().to_string());
        text.push('\n');
        text.push_str(&hex(&Sha256::digest(contents)));
        text.push('\n');
    }
    Sha256::digest(text.as_bytes()).into()
}

/// The `SIGNATURE` text for `files`, signed with the ed25519 key `seed`.
pub fn signature_file(files: &BTreeMap<String, Vec<u8>>, seed: &[u8; 32]) -> String {
    let key = SigningKey::from_bytes(seed);
    let public = key.verifying_key().to_bytes();
    let signature = key.sign(&signing_digest(files));
    format!(
        "jellyfesh-signature-v1\nkey-id: {}\npublic-key: {}\nsignature: {}\n",
        key_id(&public),
        hex(&public),
        hex(&signature.to_bytes())
    )
}

fn verify_signature(
    text: &[u8],
    files: &BTreeMap<String, Vec<u8>>,
    trust: &TrustList,
) -> Result<SignatureStatus, HostError> {
    let bad_sig = |why: &str| HostError::Signature(why.to_string());
    let text = std::str::from_utf8(text).map_err(|_| bad_sig("SIGNATURE is not UTF-8"))?;
    let lines: Vec<&str> = text.trim_end_matches('\n').split('\n').collect();
    let [magic, id_line, key_line, sig_line] = lines.as_slice() else {
        return Err(bad_sig("SIGNATURE must have exactly four lines"));
    };
    if *magic != "jellyfesh-signature-v1" {
        return Err(bad_sig("SIGNATURE does not start with jellyfesh-signature-v1"));
    }
    let field = |line: &str, name: &str| line.strip_prefix(name).map(str::to_string);
    let claimed_id = field(id_line, "key-id: ").ok_or_else(|| bad_sig("missing key-id"))?;
    let public: [u8; 32] = field(key_line, "public-key: ")
        .and_then(|h| unhex::<32>(&h))
        .ok_or_else(|| bad_sig("missing or malformed public-key"))?;
    let signature: [u8; 64] = field(sig_line, "signature: ")
        .and_then(|h| unhex::<64>(&h))
        .ok_or_else(|| bad_sig("missing or malformed signature"))?;
    let id = key_id(&public);
    if claimed_id != id {
        return Err(bad_sig("key-id does not belong to public-key"));
    }
    let key = trust::verifying_key(&public).ok_or_else(|| bad_sig("public-key is not an ed25519 key"))?;
    key.verify(&signing_digest(files), &Signature::from_bytes(&signature))
        .map_err(|_| bad_sig("the signature does not match the package contents"))?;
    Ok(match trust.find(&public) {
        Some(trusted) => SignatureStatus::Trusted {
            key_id: id,
            label: trusted.label.clone(),
        },
        None => SignatureStatus::Community { key_id: id },
    })
}

/// Builds a `.jfpkg`; used by tests and by the `jf-plugin` tool.
#[derive(Debug, Default, Clone)]
pub struct PackageBuilder {
    files: BTreeMap<String, Vec<u8>>,
    seed: Option<[u8; 32]>,
}

impl PackageBuilder {
    /// An empty package.
    pub fn new() -> PackageBuilder {
        PackageBuilder::default()
    }

    /// Sets `manifest.toml`.
    pub fn manifest(self, text: &str) -> PackageBuilder {
        self.file(MANIFEST, text.as_bytes().to_vec())
    }

    /// Sets `plugin.wasm`.
    pub fn wasm(self, bytes: Vec<u8>) -> PackageBuilder {
        self.file(WASM, bytes)
    }

    /// Sets `icon.png`.
    pub fn icon(self, png: Vec<u8>) -> PackageBuilder {
        self.file(ICON, png)
    }

    /// Adds any entry (the settings schema, or deliberately bad entries in tests).
    pub fn file(mut self, name: &str, contents: Vec<u8>) -> PackageBuilder {
        self.files.insert(name.to_string(), contents);
        self
    }

    /// Signs the package with the ed25519 key `seed` when it is built.
    pub fn sign(mut self, seed: [u8; 32]) -> PackageBuilder {
        self.seed = Some(seed);
        self
    }

    /// The zip archive.
    pub fn build(&self) -> Result<Vec<u8>, HostError> {
        let mut files = self.files.clone();
        if let Some(seed) = &self.seed {
            files.remove(SIGNATURE);
            let signature = signature_file(&files, seed);
            files.insert(SIGNATURE.to_string(), signature.into_bytes());
        }
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        for (name, contents) in &files {
            writer
                .start_file(name.as_str(), options)
                .map_err(|e| HostError::Other(format!("zip: {e}")))?;
            writer.write_all(contents)?;
        }
        let cursor = writer
            .finish()
            .map_err(|e| HostError::Other(format!("zip: {e}")))?;
        Ok(cursor.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trust::{public_key, test_key_seed};

    pub(crate) const MANIFEST_TEXT: &str = r#"
id = "org.example.test"
name = "Test"
version = "0.1.0"
api_version = "1.0"
author = "Tests"
capabilities = ["search"]
playback_mode = "stream"

[network]
allow = ["${settings.server_url}"]

[settings]
schema = "settings.schema.json"
"#;
    const SCHEMA_TEXT: &str =
        r#"{"type": "object", "properties": {"server_url": {"type": "string", "format": "uri"}}}"#;

    fn builder() -> PackageBuilder {
        PackageBuilder::new()
            .manifest(MANIFEST_TEXT)
            .wasm(b"\0asm-not-really".to_vec())
            .file("settings.schema.json", SCHEMA_TEXT.as_bytes().to_vec())
    }

    #[test]
    fn reads_an_unsigned_package() {
        let bytes = builder().build().unwrap();
        let package = read(&bytes, &TrustList::builtin()).unwrap();
        assert_eq!(package.manifest.id, "org.example.test");
        assert_eq!(package.signature, SignatureStatus::Unsigned);
        assert!(package.settings_schema.property("server_url").is_some());
        assert!(package.icon.is_none());
    }

    #[test]
    fn trusted_and_community_signatures() {
        let bytes = builder().sign(test_key_seed()).build().unwrap();
        let trusted = read(&bytes, &TrustList::builtin().with_test_key()).unwrap();
        let id = key_id(&public_key(&test_key_seed()));
        assert!(matches!(&trusted.signature, SignatureStatus::Trusted { key_id, .. } if *key_id == id));
        let community = read(&bytes, &TrustList::builtin()).unwrap();
        assert_eq!(community.signature, SignatureStatus::Community { key_id: id });
    }

    #[test]
    fn tampering_breaks_the_signature() {
        let signed = builder().sign(test_key_seed());
        let signature = {
            let files = unzip(&signed.build().unwrap()).unwrap();
            files[SIGNATURE].clone()
        };
        // Same signature, different component.
        let tampered = builder()
            .wasm(b"\0asm-evil".to_vec())
            .file(SIGNATURE, signature.clone())
            .build()
            .unwrap();
        let err = read(&tampered, &TrustList::builtin().with_test_key()).unwrap_err();
        assert!(matches!(err, HostError::Signature(_)), "{err}");
        // A signature that claims a trusted key id but carries another key.
        let text = String::from_utf8(signature).unwrap();
        let swapped = text.replace(
            &hex(&public_key(&test_key_seed())),
            &hex(&crate::trust::RELEASE_KEY),
        );
        let forged = builder().file(SIGNATURE, swapped.into_bytes()).build().unwrap();
        assert!(read(&forged, &TrustList::builtin().with_test_key()).is_err());
    }

    #[test]
    fn rejects_bad_entries() {
        let cases: Vec<(&str, PackageBuilder)> = vec![
            ("unknown entry", builder().file("notes.txt", b"hi".to_vec())),
            ("svg icon", builder().file("icon.svg", b"<svg/>".to_vec())),
            ("folder", builder().file("dir/plugin.wasm", b"x".to_vec())),
            ("fake png", builder().icon(b"GIF89a".to_vec())),
            (
                "big icon",
                builder().icon([PNG_MAGIC, &vec![0u8; 70_000]].concat()),
            ),
            ("no wasm", {
                let mut b = builder();
                b.files.remove(WASM);
                b
            }),
            ("no schema", {
                let mut b = builder();
                b.files.remove("settings.schema.json");
                b
            }),
            (
                "api 2.0",
                builder().manifest(&MANIFEST_TEXT.replace("api_version = \"1.0\"", "api_version = \"2.0\"")),
            ),
            (
                "setting not a uri",
                builder().file(
                    "settings.schema.json",
                    br#"{"type": "object", "properties": {"server_url": {"type": "string"}}}"#.to_vec(),
                ),
            ),
            ("garbage signature", builder().file(SIGNATURE, b"hello".to_vec())),
        ];
        for (what, case) in cases {
            let bytes = case.build().unwrap();
            assert!(read(&bytes, &TrustList::builtin()).is_err(), "accepted {what}");
        }
        assert!(read(b"not a zip", &TrustList::builtin()).is_err());
    }

    #[test]
    fn digest_is_order_independent_and_covers_names() {
        let mut a = BTreeMap::new();
        a.insert("b".to_string(), b"1".to_vec());
        a.insert("a".to_string(), b"2".to_vec());
        let mut renamed = BTreeMap::new();
        renamed.insert("b".to_string(), b"2".to_vec());
        renamed.insert("a".to_string(), b"1".to_vec());
        assert_ne!(signing_digest(&a), signing_digest(&renamed));
        let mut with_sig = a.clone();
        with_sig.insert(SIGNATURE.to_string(), b"ignored".to_vec());
        assert_eq!(signing_digest(&a), signing_digest(&with_sig));
    }
}
