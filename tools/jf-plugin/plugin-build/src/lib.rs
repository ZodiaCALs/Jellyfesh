//! Builds a Jellyfesh plugin crate into a WebAssembly component.
//!
//! Shared by `jf-plugin build`, `cargo xtask plugin build` and the plugin
//! tests, so there is one way to turn a crate into a `plugin.wasm`:
//!
//! 1. `cargo build --target wasm32-unknown-unknown` for the package, with
//!    `--cfg getrandom_backend="custom"` so `getrandom` uses the SDK's
//!    backend over the host's `crypto.random-bytes`;
//! 2. wrap the core module into a component with `wit-component` (no
//!    external `wasm-tools` needed): the module carries its world in the
//!    custom section `wit-bindgen` embeds;
//! 3. refuse the result if it imports anything but `jellyfesh:plugin/*@1.x`
//!    (no WASI, no stray functions);
//! 4. write `<target>/plugins/<package>.wasm`, or the chosen file.
//!
//! This crate stays small (no wasmtime) so `cargo xtask` builds quickly.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

/// The guest target.
pub const TARGET: &str = "wasm32-unknown-unknown";

/// What to build.
#[derive(Debug, Clone)]
pub struct Build {
    /// Where cargo runs: the workspace root or the plugin crate's folder.
    pub dir: PathBuf,
    /// The cargo package to build.
    pub package: String,
    /// `--release` (smaller and faster components).
    pub release: bool,
    /// `--target-dir`, if not cargo's default.
    pub target_dir: Option<PathBuf>,
    /// Where to write the component; default `<target>/plugins/<package>.wasm`.
    pub out: Option<PathBuf>,
    /// Pass `--quiet` to cargo (errors are still shown).
    pub quiet: bool,
}

impl Build {
    /// A debug build of `package`, run in `dir`.
    pub fn new(dir: impl Into<PathBuf>, package: impl Into<String>) -> Build {
        Build {
            dir: dir.into(),
            package: package.into(),
            release: false,
            target_dir: None,
            out: None,
            quiet: false,
        }
    }
}

/// A built component.
#[derive(Debug, Clone)]
pub struct Built {
    /// Where it was written.
    pub path: PathBuf,
    /// The component's bytes.
    pub component: Vec<u8>,
    /// Its imports, all `jellyfesh:plugin/<interface>@1.x`.
    pub imports: Vec<String>,
}

/// Builds the package and writes its component.
pub fn build(build: &Build) -> Result<Built> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut flags = std::env::var("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS").unwrap_or_default();
    if !flags.contains("getrandom_backend") {
        flags.push_str(" --cfg getrandom_backend=\"custom\"");
    }
    let mut command = Command::new(cargo);
    command
        .current_dir(&build.dir)
        .args(["build", "--package", &build.package, "--target", TARGET])
        .arg("--message-format=json-render-diagnostics")
        .env("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS", flags.trim())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if build.release {
        command.arg("--release");
    }
    if build.quiet {
        command.arg("--quiet");
    }
    if let Some(dir) = &build.target_dir {
        command.arg("--target-dir").arg(dir);
    }
    let output = command.output().context("running cargo")?;
    ensure!(
        output.status.success(),
        "cargo build failed for {} (is the target installed? `rustup target add {TARGET}`)",
        build.package
    );
    let module_path = wasm_artifact(&output.stdout, &build.package)?;
    let module = std::fs::read(&module_path).with_context(|| format!("reading {}", module_path.display()))?;
    let component =
        componentize(&module).with_context(|| format!("making a component of {}", build.package))?;
    let imports = check_component(&component)?.imports;

    let path = match &build.out {
        Some(path) => path.clone(),
        None => {
            // <target>/wasm32-unknown-unknown/<profile>/<name>.wasm → <target>/plugins/
            let target_dir = module_path
                .ancestors()
                .nth(3)
                .context("unexpected artifact path")?;
            target_dir.join("plugins").join(format!("{}.wasm", build.package))
        }
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &component).with_context(|| format!("writing {}", path.display()))?;
    Ok(Built {
        path,
        component,
        imports,
    })
}

/// Finds the `.wasm` cargo produced for `package` in its JSON messages.
fn wasm_artifact(messages: &[u8], package: &str) -> Result<PathBuf> {
    let crate_name = package.replace('-', "_");
    for line in String::from_utf8_lossy(messages).lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if message["reason"] != "compiler-artifact" || message["target"]["name"] != crate_name.as_str() {
            continue;
        }
        let is_cdylib = message["target"]["kind"]
            .as_array()
            .is_some_and(|kinds| kinds.iter().any(|k| k == "cdylib"));
        ensure!(is_cdylib, "{package} must have crate-type = [\"cdylib\"]");
        if let Some(path) = message["filenames"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .find(|f| f.ends_with(".wasm"))
        {
            return Ok(PathBuf::from(path));
        }
    }
    bail!("cargo produced no .wasm for {package}")
}

/// Wraps a core module built with `wit-bindgen` into a component.
pub fn componentize(module: &[u8]) -> Result<Vec<u8>> {
    wit_component::ComponentEncoder::default()
        .module(module)?
        .validate(true)
        .encode()
}

/// A component's imports and exports, as WIT names
/// (`jellyfesh:plugin/http@1.0.0`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interfaces {
    /// What the component needs from the host.
    pub imports: Vec<String>,
    /// What it provides.
    pub exports: Vec<String>,
}

/// The interfaces every plugin exports (the `source` world in `plugins/wit`).
pub const REQUIRED_EXPORTS: &[&str] = &[
    "meta",
    "auth",
    "catalog",
    "playback",
    "lyrics",
    "artwork",
    "recommendations",
];

/// Whether `bytes` start like a WebAssembly component (a core module has
/// another layer number in its header).
pub fn is_component(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && bytes[..4] == *b"\0asm" && bytes[6..8] == [1, 0]
}

/// Lists a component's imports and exports, whatever they are.
pub fn interfaces(component: &[u8]) -> Result<Interfaces> {
    ensure!(
        is_component(component),
        "not a WebAssembly component (a core module must be wrapped with `jf-plugin build`)"
    );
    let wit_component::DecodedWasm::Component(resolve, world) = wit_component::decode(component)? else {
        bail!("not a component");
    };
    let world = &resolve.worlds[world];
    Ok(Interfaces {
        imports: world.imports.keys().map(|k| resolve.name_world_key(k)).collect(),
        exports: world.exports.keys().map(|k| resolve.name_world_key(k)).collect(),
    })
}

/// Whether a plugin may import `name`: only `jellyfesh:plugin/<interface>@1.x`.
pub fn import_allowed(name: &str) -> bool {
    name.strip_prefix("jellyfesh:plugin/")
        .and_then(|rest| rest.split_once('@'))
        .is_some_and(|(_, version)| version.starts_with("1."))
}

/// Whether the component exports the `remote` interface of API 1.1 or
/// later, which a manifest that declares `remote` needs (ADR-0018).
pub fn exports_remote(found: &Interfaces) -> bool {
    exports_since(found, "remote", 1)
}

/// Whether the component exports `jellyfesh:plugin/<interface>` of API
/// `1.<minor>` or later (`maker`, `appearance`, `discover`, `details` and
/// `comments` came with 1.2).
pub fn exports_since(found: &Interfaces, interface: &str, minor: u32) -> bool {
    let prefix = format!("jellyfesh:plugin/{interface}@1.");
    found.exports.iter().any(|name| {
        name.strip_prefix(&prefix)
            .and_then(|rest| rest.split('.').next())
            .and_then(|m| m.parse::<u32>().ok())
            .is_some_and(|m| m >= minor)
    })
}

/// Whether the component imports `jellyfesh:plugin/<interface>` (any 1.x).
pub fn imports_interface(found: &Interfaces, interface: &str) -> bool {
    let prefix = format!("jellyfesh:plugin/{interface}@1.");
    found.imports.iter().any(|name| name.starts_with(&prefix))
}

/// Lists a component's imports and fails unless they are all
/// `jellyfesh:plugin/<interface>@1.x`.
pub fn check_imports(component: &[u8]) -> Result<Vec<String>> {
    Ok(check_component(component)?.imports)
}

/// Checks that `component` is a plugin component: it imports nothing but
/// `jellyfesh:plugin/<interface>@1.x` and exports every interface of the
/// `source` world at version 1.x.
pub fn check_component(component: &[u8]) -> Result<Interfaces> {
    let found = interfaces(component)?;
    for name in &found.imports {
        ensure!(
            import_allowed(name),
            "the component imports {name}; plugins may only import jellyfesh:plugin/*@1 (no WASI)"
        );
    }
    for interface in REQUIRED_EXPORTS {
        let prefix = format!("jellyfesh:plugin/{interface}@1.");
        ensure!(
            found.exports.iter().any(|name| name.starts_with(&prefix)),
            "the component does not export jellyfesh:plugin/{interface}@1 (use the SDK's export_plugin!)"
        );
    }
    Ok(found)
}

/// The cargo package name in a crate's `Cargo.toml` (the `name` key of its
/// `[package]` table), without a TOML parser.
pub fn package_name(cargo_toml: &Path) -> Result<String> {
    let text =
        std::fs::read_to_string(cargo_toml).with_context(|| format!("reading {}", cargo_toml.display()))?;
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package
            && let Some(rest) = line.strip_prefix("name")
            && let Some(value) = rest.trim_start().strip_prefix('=')
        {
            let value = value.trim().trim_matches('"');
            ensure!(
                !value.is_empty(),
                "empty package name in {}",
                cargo_toml.display()
            );
            return Ok(value.to_string());
        }
    }
    bail!("{} has no [package] name", cargo_toml.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_cdylib_artifact() {
        let messages = concat!(
            r#"{"reason":"compiler-artifact","target":{"name":"serde","kind":["lib"]},"filenames":["/t/libserde.rlib"]}"#,
            "\n",
            r#"{"reason":"compiler-artifact","target":{"name":"jf_demo","kind":["cdylib"]},"filenames":["/t/wasm32-unknown-unknown/debug/jf_demo.wasm"]}"#,
            "\n",
            r#"{"reason":"build-finished","success":true}"#,
        );
        assert_eq!(
            wasm_artifact(messages.as_bytes(), "jf-demo").unwrap(),
            PathBuf::from("/t/wasm32-unknown-unknown/debug/jf_demo.wasm")
        );
        assert!(wasm_artifact(messages.as_bytes(), "jf-other").is_err());
        let rlib = r#"{"reason":"compiler-artifact","target":{"name":"jf_demo","kind":["lib"]},"filenames":["/t/x.rlib"]}"#;
        assert!(wasm_artifact(rlib.as_bytes(), "jf-demo").is_err());
    }

    #[test]
    fn reads_the_package_name() {
        let dir = std::env::temp_dir().join(format!("jf-plugin-build-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Cargo.toml");
        std::fs::write(
            &path,
            "[workspace]\n\n[package]\nname = \"my-plugin\"\nversion = \"0.1.0\"\n\n[dependencies]\nname = \"x\"\n",
        )
        .unwrap();
        assert_eq!(package_name(&path).unwrap(), "my-plugin");
        std::fs::write(&path, "[dependencies]\nname = \"x\"\n").unwrap();
        assert!(package_name(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_garbage() {
        assert!(check_imports(b"not wasm").is_err());
        assert!(interfaces(b"not wasm").is_err());
        // An empty core module decodes as a world with nothing in it.
        assert!(!is_component(b"\0asm\x01\0\0\0"));
        assert!(check_component(b"\0asm\x01\0\0\0").is_err());
        assert!(is_component(b"\0asm\x0d\0\x01\0"));
        assert!(componentize(b"not wasm").is_err());
    }

    #[test]
    fn allows_only_the_plugin_world_imports() {
        assert!(import_allowed("jellyfesh:plugin/http@1.0.0"));
        assert!(import_allowed("jellyfesh:plugin/http@1.1.0"));
        let with = |exports: &[&str]| Interfaces {
            imports: Vec::new(),
            exports: exports.iter().map(|e| (*e).to_string()).collect(),
        };
        assert!(exports_remote(&with(&["jellyfesh:plugin/remote@1.1.0"])));
        assert!(exports_since(
            &with(&["jellyfesh:plugin/maker@1.2.0"]),
            "maker",
            2
        ));
        assert!(!exports_since(
            &with(&["jellyfesh:plugin/maker@1.1.0"]),
            "maker",
            2
        ));
        assert!(!exports_since(
            &with(&["jellyfesh:plugin/makers@1.2.0"]),
            "maker",
            2
        ));
        assert!(exports_remote(&with(&["jellyfesh:plugin/remote@1.2.3"])));
        assert!(!exports_remote(&with(&["jellyfesh:plugin/remote@1.0.0"])));
        assert!(!exports_remote(&with(&["jellyfesh:plugin/meta@1.1.0"])));
        assert!(import_allowed("jellyfesh:plugin/kv@1.2.0"));
        assert!(!import_allowed("jellyfesh:plugin/http@2.0.0"));
        assert!(!import_allowed("wasi:filesystem/types@0.2.0"));
        assert!(!import_allowed("jellyfesh:plugin/http"));
        assert!(!import_allowed("env"));
    }
}
