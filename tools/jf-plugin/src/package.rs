//! Packing, signing, verifying and inspecting `.jfpkg` files. The format,
//! limits and signature scheme are the plugin host's
//! (`jf_plugin_host::package`); every package this tool writes is read back
//! with the host's own checks before it is written.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail, ensure};
use jf_plugin_host::manifest::{self, AllowRule, Manifest};
use jf_plugin_host::package::{self, ICON, MANIFEST, Package, PackageBuilder, SIGNATURE, WASM};
use jf_plugin_host::schema::{Format, PropertyKind, SettingsSchema};
use jf_plugin_host::trust::TrustList;
use jf_plugin_host::{HostError, LibraryStorage, SignatureStatus};
use serde_json::{Value, json};

use crate::key::SigningKey;

/// The most a package may weigh (the host's limit).
const MAX_PACKAGE: u64 = 40 * 1024 * 1024;
/// The most any entry may weigh here; the host checks the exact limits.
const MAX_ENTRY: u64 = 32 * 1024 * 1024;
/// The most entries a package may have (the host's limit).
const MAX_ENTRIES: usize = 8;

/// What goes into a package, read from a plugin crate's folder.
#[derive(Debug, Clone)]
pub struct PluginFiles {
    /// `manifest.toml`.
    pub manifest: String,
    /// The settings schema the manifest names: file name and contents.
    pub schema: Option<(String, Vec<u8>)>,
    /// `icon.png`, if the crate has one.
    pub icon: Option<Vec<u8>>,
}

impl PluginFiles {
    /// Reads `manifest.toml`, the settings schema it names and `icon.png`
    /// from a plugin crate's folder.
    pub fn read(dir: &Path) -> Result<PluginFiles> {
        let path = dir.join(MANIFEST);
        let manifest =
            std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let parsed = manifest::parse(&manifest).with_context(|| format!("checking {}", path.display()))?;
        let schema = match parsed.settings_schema {
            Some(name) => {
                let path = dir.join(&name);
                let text = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
                Some((name, text))
            }
            None => None,
        };
        let icon_path = dir.join(ICON);
        let icon = match std::fs::read(&icon_path) {
            Ok(png) => Some(png),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(anyhow!("reading {}: {e}", icon_path.display())),
        };
        Ok(PluginFiles {
            manifest,
            schema,
            icon,
        })
    }

    /// The parsed manifest.
    pub fn manifest(&self) -> Result<Manifest> {
        Ok(manifest::parse(&self.manifest)?)
    }
}

/// Packs `files` with the component `wasm`, signed with `key` if one is
/// given. The component must import nothing but `jellyfesh:plugin/*@1`, and
/// the result must pass the host's checks.
pub fn pack(files: &PluginFiles, wasm: Vec<u8>, key: Option<&SigningKey>) -> Result<Vec<u8>> {
    jf_plugin_build::check_component(&wasm).context("checking the component")?;
    let mut builder = PackageBuilder::new().manifest(&files.manifest).wasm(wasm);
    if let Some((name, text)) = &files.schema {
        builder = builder.file(name, text.clone());
    }
    if let Some(icon) = &files.icon {
        builder = builder.icon(icon.clone());
    }
    if let Some(key) = key {
        builder = builder.sign(key.seed());
    }
    let bytes = builder.build()?;
    package::read(&bytes, &TrustList::builtin()).map_err(explain)?;
    Ok(bytes)
}

/// The entries of a package, read with the host's size limits (a broken or
/// hostile archive fails here rather than filling memory).
pub fn read_entries(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    ensure!(
        bytes.len() as u64 <= MAX_PACKAGE,
        "the package is larger than 40 MiB"
    );
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| anyhow!("not a .jfpkg (zip) file: {e}"))?;
    ensure!(
        archive.len() <= MAX_ENTRIES,
        "the package has more than {MAX_ENTRIES} entries"
    );
    let mut entries = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| anyhow!("entry {index}: {e}"))?;
        let name = entry
            .name()
            .map_err(|e| anyhow!("entry {index}: {e}"))?
            .into_owned();
        ensure!(
            !entry.is_dir() && !name.contains('/') && !name.contains('\\'),
            "folders are not allowed in a package ({name:?})"
        );
        let mut contents = Vec::new();
        (&mut entry)
            .take(MAX_ENTRY + 1)
            .read_to_end(&mut contents)
            .with_context(|| format!("reading {name}"))?;
        ensure!(contents.len() as u64 <= MAX_ENTRY, "{name} is larger than 32 MiB");
        ensure!(
            entries.insert(name.clone(), contents).is_none(),
            "{name} appears twice"
        );
    }
    Ok(entries)
}

/// Signs a package with `key`, replacing any signature it had. The package
/// must otherwise pass the host's checks.
pub fn sign(bytes: &[u8], key: &SigningKey) -> Result<Vec<u8>> {
    let mut entries = read_entries(bytes)?;
    entries.remove(SIGNATURE);
    let builder = entries
        .into_iter()
        .fold(PackageBuilder::new(), |builder, (name, contents)| {
            builder.file(&name, contents)
        });
    // Everything but the signature must already be right.
    package::read(&builder.build()?, &TrustList::builtin()).map_err(explain)?;
    let signed = builder.sign(key.seed()).build()?;
    package::read(&signed, &TrustList::builtin()).map_err(explain)?;
    Ok(signed)
}

/// A package that passed every check.
#[derive(Debug)]
pub struct Verified {
    /// The package as the host reads it.
    pub package: Package,
    /// The component's imports (all `jellyfesh:plugin/*@1`).
    pub imports: Vec<String>,
}

/// Checks a package the way the host does on install (entries, sizes,
/// manifest, API version, settings schema, icon, signature) and that the
/// component imports nothing but `jellyfesh:plugin/*@1`. `trust` decides
/// which valid signatures count as official.
pub fn verify(bytes: &[u8], trust: &TrustList) -> Result<Verified> {
    let package = package::read(bytes, trust).map_err(explain)?;
    let imports = jf_plugin_build::check_component(&package.wasm)
        .context("checking plugin.wasm")?
        .imports;
    Ok(Verified { package, imports })
}

/// The app's trust list, plus the published test key when `test_key`, plus
/// `extra` public keys (64 hex characters each).
pub fn trust_list(test_key: bool, extra: &[String]) -> Result<TrustList> {
    let mut list = TrustList::builtin();
    if test_key {
        list = list.with_test_key();
    }
    for (i, text) in extra.iter().enumerate() {
        let public = parse_public_key(text).with_context(|| format!("--trust-key number {}", i + 1))?;
        list = list.with_key("trusted on the command line", public);
    }
    Ok(list)
}

fn parse_public_key(text: &str) -> Result<[u8; 32]> {
    let text = text.trim();
    ensure!(
        text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()),
        "a public key is 64 hex characters"
    );
    let mut out = [0u8; 32];
    for (i, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(pair).unwrap_or("00");
        out[i] = u8::from_str_radix(pair, 16).map_err(|e| anyhow!("{e}"))?;
    }
    Ok(out)
}

/// One line about who signed a package.
pub fn describe_signature(status: &SignatureStatus) -> String {
    match status {
        SignatureStatus::Unsigned => "none: unsigned, the app shows it as Community (unofficial)".into(),
        SignatureStatus::Community { key_id } => {
            format!("valid, key id {key_id}: not a trusted key, the app shows it as Community (unofficial)")
        }
        SignatureStatus::Trusted { key_id, label } => {
            format!("valid, key id {key_id}: {label}, the app shows it as Official")
        }
    }
}

/// Host errors with the part of the package they are about.
fn explain(error: HostError) -> anyhow::Error {
    match error {
        HostError::Signature(why) => anyhow!("bad signature: {why}"),
        HostError::Manifest(why) => anyhow!("bad manifest: {why}"),
        HostError::Package(why) => anyhow!("bad package: {why}"),
        other => anyhow!("{other}"),
    }
}

/// What a package contains, for `jf-plugin inspect`.
#[derive(Debug, Clone)]
pub struct Report {
    /// Entry names and sizes in bytes.
    pub entries: Vec<(String, usize)>,
    /// The manifest, if it parses.
    pub manifest: Result<Manifest, String>,
    /// The settings schema, if the manifest names one and it parses.
    pub schema: Option<Result<SettingsSchema, String>>,
    /// The signature: its status, or why the package or signature is bad.
    pub signature: Result<SignatureStatus, String>,
    /// The key id the `SIGNATURE` file claims, if there is one.
    pub claimed_key_id: Option<String>,
    /// The component's imports and exports, or why they cannot be read.
    pub interfaces: Result<jf_plugin_build::Interfaces, String>,
}

/// Reads everything there is to show about a package, even a broken one.
pub fn inspect(bytes: &[u8], trust: &TrustList) -> Result<Report> {
    let entries = read_entries(bytes)?;
    let manifest = match entries.get(MANIFEST) {
        Some(text) => std::str::from_utf8(text)
            .map_err(|_| "manifest.toml is not UTF-8".to_string())
            .and_then(|text| manifest::parse(text).map_err(|e| e.to_string())),
        None => Err("manifest.toml is missing".to_string()),
    };
    let schema = match &manifest {
        Ok(Manifest {
            settings_schema: Some(name),
            ..
        }) => Some(match entries.get(name.as_str()) {
            Some(text) => std::str::from_utf8(text)
                .map_err(|_| format!("{name} is not UTF-8"))
                .and_then(SettingsSchema::parse),
            None => Err(format!("{name} is missing")),
        }),
        _ => None,
    };
    let signature = package::read(bytes, trust)
        .map(|p| p.signature)
        .map_err(|e| explain(e).to_string());
    let claimed_key_id = entries.get(SIGNATURE).and_then(|text| {
        String::from_utf8_lossy(text)
            .lines()
            .find_map(|line| line.strip_prefix("key-id: ").map(str::to_string))
    });
    let interfaces = match entries.get(WASM) {
        Some(wasm) => jf_plugin_build::interfaces(wasm).map_err(|e| format!("{e:#}")),
        None => Err("plugin.wasm is missing".to_string()),
    };
    Ok(Report {
        entries: entries
            .iter()
            .map(|(name, bytes)| (name.clone(), bytes.len()))
            .collect(),
        manifest,
        schema,
        signature,
        claimed_key_id,
        interfaces,
    })
}

fn list<T: ToString>(items: &[T]) -> String {
    if items.is_empty() {
        "none".into()
    } else {
        items.iter().map(T::to_string).collect::<Vec<_>>().join(", ")
    }
}

fn size(bytes: usize) -> String {
    if bytes < 10 * 1024 {
        format!("{bytes} B")
    } else {
        format!("{} KiB", bytes.div_ceil(1024))
    }
}

fn allow_rule(rule: &AllowRule) -> String {
    match rule {
        AllowRule::Setting(key) => format!("${{settings.{key}}} (the address the person enters)"),
        AllowRule::Host { host, wildcard, port } => {
            let star = if *wildcard { "*." } else { "" };
            let port = if *port == 443 {
                String::new()
            } else {
                format!(":{port}")
            };
            format!("https://{star}{host}{port}")
        }
    }
}

fn property(schema: &SettingsSchema, name: &str) -> String {
    let Some(p) = schema.property(name) else {
        return name.to_string();
    };
    let mut text = format!(
        "{name} ({}",
        match p.kind {
            PropertyKind::String => "text",
            PropertyKind::Integer => "whole number",
            PropertyKind::Number => "number",
            PropertyKind::Boolean => "yes/no",
        }
    );
    match p.format {
        Some(Format::Uri) => text += ", URL",
        Some(Format::Email) => text += ", e-mail",
        None => {}
    }
    if let Some(choices) = &p.choices {
        let _ = write!(text, ": {}", choices.join(" | "));
    }
    if let Some(default) = &p.default {
        let _ = write!(text, "; default {default}");
    }
    text.push(')');
    text
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.manifest {
            Ok(m) => {
                writeln!(f, "{} {}: {}", m.id, m.version, m.name)?;
                if let Some(description) = &m.description {
                    writeln!(f, "  description    {description}")?;
                }
                writeln!(f, "  author         {}", m.author)?;
                writeln!(f, "  api version    {}", m.api_version)?;
                writeln!(f, "  capabilities   {}", list(&m.capabilities))?;
                writeln!(f, "  playback       {:?}", m.playback_mode)?;
                writeln!(f, "  sign-in        {}", list(&m.auth))?;
                let rules: Vec<String> = m.allow.iter().map(allow_rule).collect();
                writeln!(f, "  network        {}", list(&rules))?;
                writeln!(f, "  requests       at most {} per second", m.max_rps)?;
                let policy = &m.content_policy;
                if *policy != Default::default() {
                    writeln!(
                        f,
                        "  content policy unified search {}, mixed queue {}, library {}, lyrics sync {}{}{}",
                        policy.unified_search,
                        policy.mixed_queue,
                        match policy.library_storage {
                            LibraryStorage::None => "none",
                            LibraryStorage::Metadata => "metadata",
                            LibraryStorage::Full => "full",
                        },
                        policy.lyrics_sync,
                        policy
                            .metadata_ttl_secs
                            .map(|s| format!(", metadata kept {s} s"))
                            .unwrap_or_default(),
                        policy
                            .attribution
                            .as_ref()
                            .map(|a| format!(", attribution {a}"))
                            .unwrap_or_default(),
                    )?;
                }
            }
            Err(why) => writeln!(f, "manifest       INVALID: {why}")?,
        }
        match &self.schema {
            Some(Ok(schema)) => {
                let names: Vec<String> = schema
                    .properties()
                    .iter()
                    .map(|p| property(schema, &p.name))
                    .collect();
                writeln!(f, "  settings       {}", list(&names))?;
            }
            Some(Err(why)) => writeln!(f, "  settings       INVALID: {why}")?,
            None => writeln!(f, "  settings       none")?,
        }
        match &self.signature {
            Ok(status) => writeln!(f, "  signature      {}", describe_signature(status))?,
            Err(why) => writeln!(
                f,
                "  signature      INVALID ({why}){}",
                self.claimed_key_id
                    .as_ref()
                    .map(|id| format!("; it claims key id {id}"))
                    .unwrap_or_default()
            )?,
        }
        let entries: Vec<String> = self
            .entries
            .iter()
            .map(|(name, bytes)| format!("{name} {}", size(*bytes)))
            .collect();
        writeln!(f, "  entries        {}", list(&entries))?;
        match &self.interfaces {
            Ok(interfaces) => {
                let imports: Vec<String> = interfaces
                    .imports
                    .iter()
                    .map(|name| {
                        if jf_plugin_build::import_allowed(name) {
                            name.clone()
                        } else {
                            format!("{name} (NOT ALLOWED)")
                        }
                    })
                    .collect();
                writeln!(f, "  imports        {}", list(&imports))?;
                writeln!(f, "  exports        {}", list(&interfaces.exports))?;
            }
            Err(why) => writeln!(f, "  component      INVALID: {why}")?,
        }
        Ok(())
    }
}

impl Report {
    /// The report as JSON, for scripts.
    pub fn json(&self) -> Value {
        let manifest = match &self.manifest {
            Ok(m) => json!({
                "id": m.id,
                "name": m.name,
                "version": m.version.to_string(),
                "api_version": m.api_version.to_string(),
                "author": m.author,
                "description": m.description,
                "capabilities": m.capabilities.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "playback_mode": format!("{:?}", m.playback_mode).to_lowercase(),
                "auth": m.auth.iter().map(ToString::to_string).collect::<Vec<_>>(),
                "network": m.allow.iter().map(allow_rule).collect::<Vec<_>>(),
                "max_rps": m.max_rps,
            }),
            Err(why) => json!({ "error": why }),
        };
        let signature = match &self.signature {
            Ok(SignatureStatus::Unsigned) => json!({ "status": "unsigned" }),
            Ok(SignatureStatus::Community { key_id }) => json!({ "status": "community", "key_id": key_id }),
            Ok(SignatureStatus::Trusted { key_id, label }) => {
                json!({ "status": "official", "key_id": key_id, "label": label })
            }
            Err(why) => json!({ "status": "invalid", "error": why, "key_id": self.claimed_key_id }),
        };
        let settings = match &self.schema {
            Some(Ok(schema)) => schema.json().clone(),
            Some(Err(why)) => json!({ "error": why }),
            None => Value::Null,
        };
        let interfaces = match &self.interfaces {
            Ok(i) => json!({ "imports": i.imports, "exports": i.exports }),
            Err(why) => json!({ "error": why }),
        };
        json!({
            "manifest": manifest,
            "settings_schema": settings,
            "signature": signature,
            "entries": self.entries.iter().map(|(n, b)| json!({ "name": n, "bytes": b })).collect::<Vec<_>>(),
            "component": interfaces,
        })
    }
}

/// Fails unless the package's signature meets the requirement.
pub fn require(status: &SignatureStatus, signed: bool, trusted: bool) -> Result<()> {
    match status {
        SignatureStatus::Unsigned if signed || trusted => bail!("the package is not signed"),
        SignatureStatus::Community { key_id } if trusted => {
            bail!("the package is signed with key {key_id}, which is not a trusted key")
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_requirements() {
        let community = SignatureStatus::Community { key_id: "ab".into() };
        let trusted = SignatureStatus::Trusted {
            key_id: "cd".into(),
            label: "Test".into(),
        };
        assert!(require(&SignatureStatus::Unsigned, false, false).is_ok());
        assert!(require(&SignatureStatus::Unsigned, true, false).is_err());
        assert!(require(&community, true, false).is_ok());
        assert!(require(&community, false, true).is_err());
        assert!(require(&trusted, true, true).is_ok());
    }

    #[test]
    fn rejects_bad_archives() {
        assert!(read_entries(b"not a zip").is_err());
        assert!(sign(b"not a zip", &SigningKey::test_key()).is_err());
        assert!(trust_list(false, &["zz".into()]).is_err());
        let key = SigningKey::test_key();
        assert!(trust_list(true, &[key.public_hex()]).is_ok());
    }
}
