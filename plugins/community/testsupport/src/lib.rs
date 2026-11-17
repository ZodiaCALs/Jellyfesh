//! Running an unofficial plugin in a real Jellyfesh plugin host, against a
//! mock of its service.
//!
//! The published plugins reach only their services' fixed hosts, over https,
//! which a test cannot serve (and the live services are out of reach from
//! where these plugins were written). So the tests build the same crate with
//! its `mock-service` feature, which sends every request to the origin in
//! the `mock_origin` setting (`http://127.0.0.1:<port>/<host>/<path>`), and
//! pack it with a test copy of the manifest whose allowlist is that origin
//! and a schema that has that setting. Everything else is the real thing:
//! the host with its production sandbox limits, its HTTP client and cookie
//! jar, the keystore stand-in, the checks on every answer, and the rules for
//! community plugins (the package is unsigned, installs only once community
//! plugins are allowed, and starts turned off until it is turned on).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use jf_plugin_host::{HostError, PluginInfo};
pub use jf_plugin_testkit::{
    Env, PluginFiles, Request, Response, TestServer, fetch, final_step, item, kinds, login, next_page,
    next_step, page, recv,
};
use serde_json::{Value, json};

/// The setting the `mock-service` feature reads.
pub const MOCK_SETTING: &str = "mock_origin";

/// The component of the plugin crate in `dir` (cargo package `package`),
/// built for `wasm32-unknown-unknown` with `features` in the test binary's
/// target folder, wrapped and checked like `jf-plugin build` does it. Built
/// once per test binary and feature set.
pub fn component(dir: &Path, package: &str, features: &[&str]) -> Vec<u8> {
    static BUILT: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    let key = format!("{package}+{}", features.join(","));
    let cache = BUILT.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(wasm) = cache.get(&key) {
        return wasm.clone();
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut flags = std::env::var("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS").unwrap_or_default();
    if !flags.contains("getrandom_backend") {
        flags.push_str(" --cfg getrandom_backend=\"custom\"");
    }
    let mut command = Command::new(cargo);
    command
        .current_dir(dir)
        .args(["build", "--quiet", "--package", package])
        .args(["--target", jf_plugin_build::TARGET])
        .arg("--message-format=json-render-diagnostics")
        .arg("--target-dir")
        .arg(jf_plugin_testkit::target_dir())
        .env("CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS", flags.trim())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if !features.is_empty() {
        command.arg("--features").arg(features.join(","));
    }
    let output = command.output().expect("running cargo");
    assert!(output.status.success(), "cargo build failed for {package}");
    let module_path = artifact(&output.stdout, package);
    let module = std::fs::read(&module_path).expect("reading the module");
    let wasm = jf_plugin_build::componentize(&module).expect("making a component");
    jf_plugin_build::check_component(&wasm).expect("a valid plugin component");
    cache.insert(key, wasm.clone());
    wasm
}

/// The `.wasm` cargo wrote for `package`, from its JSON messages.
fn artifact(messages: &[u8], package: &str) -> PathBuf {
    let name = package.replace('-', "_");
    for line in String::from_utf8_lossy(messages).lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if message["reason"] != "compiler-artifact" || message["target"]["name"] != name.as_str() {
            continue;
        }
        if let Some(path) = message["filenames"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .find(|f| f.ends_with(".wasm"))
        {
            return PathBuf::from(path);
        }
    }
    panic!("cargo produced no .wasm for {package}")
}

/// The crate's own `manifest.toml`, settings schema and icon, as published.
pub fn production_files(dir: &Path) -> PluginFiles {
    PluginFiles::read(dir)
}

/// The test copy: the allowlist is only `${settings.mock_origin}`, and the
/// schema gains that setting. Nothing else changes.
pub fn mock_files(dir: &Path) -> PluginFiles {
    let files = PluginFiles::read(dir);
    let mut replaced = false;
    let mut in_list = false;
    let mut lines = Vec::new();
    for line in files.manifest.lines() {
        if in_list {
            // The rest of a list written over several lines.
            in_list = !line.contains(']');
            continue;
        }
        if line.trim_start().starts_with("allow = ") {
            replaced = true;
            in_list = !line.contains(']');
            lines.push(format!("allow = [\"${{settings.{MOCK_SETTING}}}\"]"));
        } else {
            lines.push(line.to_string());
        }
    }
    assert!(replaced, "the manifest has no `allow = ` line");
    let manifest = lines.join("\n");
    jf_plugin_host::manifest::parse(&manifest).expect("the test manifest is valid");
    let (name, text) = files.schema.clone().expect("a settings schema");
    let mut schema: Value = serde_json::from_str(&text).expect("the schema is JSON");
    schema["properties"][MOCK_SETTING] = json!({
        "type": "string",
        "format": "uri",
        "title": "Mock service (tests only)"
    });
    PluginFiles {
        manifest: manifest + "\n",
        schema: Some((name, serde_json::to_string_pretty(&schema).expect("JSON"))),
        icon: files.icon,
    }
}

/// Installs `wasm` with `files` as an unsigned community package: community
/// plugins are allowed first, and the plugin is turned on after installing,
/// as a person does after reading the disclosure.
pub fn install_community(env: &Env, files: &PluginFiles, wasm: Vec<u8>) -> Result<PluginInfo, HostError> {
    let bytes = files.package(wasm, false);
    let path = env.dir.path().join(format!("{}.jfpkg", files.id()));
    std::fs::write(&path, bytes).expect("writing the package");
    let info = env.host.install(&path)?;
    env.host.set_enabled(&info.id, true)?;
    Ok(env.host.plugin(&info.id).expect("installed"))
}

/// A host with the plugin crate in `dir` installed as a community plugin,
/// built with `mock-service` and pointed at `server`, with `settings` on top.
pub fn with_mock(dir: &Path, package: &str, server: &TestServer, settings: Value) -> (Env, PluginInfo) {
    let env = Env::new();
    env.host.set_allow_community(true);
    let wasm = component(dir, package, &["mock-service"]);
    let info = install_community(&env, &mock_files(dir), wasm).expect("installing the plugin");
    let mut all = settings;
    all[MOCK_SETTING] = Value::String(server.url());
    env.settings(&info.id, all);
    (env, info)
}

/// Splits a mock request path into the service host it stands for and the
/// path on that host: `/music.163.com/weapi/x` → (`music.163.com`, `/weapi/x`).
pub fn split_host(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix('/')?;
    match rest.find('/') {
        Some(at) => Some((&rest[..at], &rest[at..])),
        None => Some((rest, "/")),
    }
}

/// The service host a mock request was meant for.
pub fn host_of(request: &Request) -> String {
    split_host(request.path())
        .map(|(h, _)| h.to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_hosts() {
        assert_eq!(
            split_host("/music.163.com/weapi/x"),
            Some(("music.163.com", "/weapi/x"))
        );
        assert_eq!(split_host("/y.gtimg.cn"), Some(("y.gtimg.cn", "/")));
        assert_eq!(split_host("nope"), None);
    }
}
