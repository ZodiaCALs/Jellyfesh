//! Running a plugin crate in a real [`PluginHost`]: building its component,
//! packing it with the crate's `manifest.toml`, settings schema and icon,
//! signing it with the published test key, and installing it into a host in
//! a temporary folder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use jf_plugin_build::Build;
use jf_plugin_host::package::{ICON, PackageBuilder};
use jf_plugin_host::trust::{TrustList, test_key_seed};
use jf_plugin_host::{
    AuthFlow, AuthKind, AuthStep, CallError, FieldValue, HostConfig, HostOptions, Item, ItemKind, Limits,
    MemorySecretStore, PageRequest, PluginHost, PluginInfo, Reply, SearchFilters,
};

/// The workspace root (this crate lives in `plugins/testkit`).
pub fn workspace_root() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.ancestors().nth(2).unwrap_or(here).to_path_buf()
}

/// The cargo target folder the running test binary was built in.
pub fn target_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path");
    // <target>/<profile>/deps/<test binary>
    exe.ancestors().nth(3).expect("target folder").to_path_buf()
}

/// The component of a plugin package of the Jellyfesh workspace (debug
/// build), built once per test binary with the same steps as
/// `jf-plugin build`.
pub fn component(package: &str) -> Vec<u8> {
    component_in(&workspace_root(), package)
}

/// The component of the plugin package `package`, built by cargo run in
/// `dir` (the crate's folder, so a plugin outside this repository builds in
/// its own workspace), once per test binary.
pub fn component_in(dir: &Path, package: &str) -> Vec<u8> {
    static BUILT: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    let cache = BUILT.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(wasm) = cache.get(package) {
        return wasm.clone();
    }
    let target = target_dir();
    let built = jf_plugin_build::build(&Build {
        target_dir: Some(target.clone()),
        out: Some(target.join("plugin-tests").join(format!("{package}.wasm"))),
        quiet: true,
        ..Build::new(dir, package)
    })
    .unwrap_or_else(|e| panic!("building {package}: {e:#}"));
    cache.insert(package.to_string(), built.component.clone());
    built.component
}

/// The files of a plugin crate that go into its package.
#[derive(Debug, Clone)]
pub struct PluginFiles {
    /// `manifest.toml`.
    pub manifest: String,
    /// The settings schema's file name and text, if the manifest names one.
    pub schema: Option<(String, String)>,
    /// `icon.png`, if the crate has one.
    pub icon: Option<Vec<u8>>,
}

impl PluginFiles {
    /// Reads `manifest.toml`, the schema it names and `icon.png` from a
    /// plugin crate's folder.
    pub fn read(crate_dir: &Path) -> PluginFiles {
        let manifest = std::fs::read_to_string(crate_dir.join("manifest.toml")).expect("manifest.toml");
        let parsed = jf_plugin_host::manifest::parse(&manifest).expect("a valid manifest.toml");
        let schema = parsed.settings_schema.map(|name| {
            let text = std::fs::read_to_string(crate_dir.join(&name)).expect("the settings schema");
            (name, text)
        });
        let icon = std::fs::read(crate_dir.join(ICON)).ok();
        PluginFiles {
            manifest,
            schema,
            icon,
        }
    }

    /// For a plugin of an online service, whose manifest allows only the
    /// service's own hosts: the same files with a string setting `key`
    /// (format `uri`) added to the settings schema and `${settings.<key>}` to
    /// `[network] allow`, so a test can point the plugin at a
    /// [`crate::TestServer`] instead of the service.
    ///
    /// Only test packages carry the setting. The host refuses settings that
    /// the package's schema does not name, so a published plugin never
    /// receives one and keeps to its fixed hosts. The manifest must list
    /// `allow = [` on one line under `[network]`.
    pub fn with_test_server_setting(mut self, key: &str) -> PluginFiles {
        let entry = format!("\"${{settings.{key}}}\"");
        let mut found = false;
        let lines: Vec<String> = self
            .manifest
            .lines()
            .map(|line| match line.trim_start().strip_prefix("allow = [") {
                Some(rest) if !found => {
                    found = true;
                    let rest = rest.trim_start();
                    if rest.starts_with(']') {
                        format!("allow = [{entry}{rest}")
                    } else {
                        format!("allow = [{entry}, {rest}")
                    }
                }
                _ => line.to_string(),
            })
            .collect();
        assert!(found, "the manifest has no `allow = [` line under [network]");
        self.manifest = lines.join("\n") + "\n";
        let (name, text) = self.schema.take().expect("the manifest names a settings schema");
        let mut schema: serde_json::Value = serde_json::from_str(&text).expect("the settings schema is JSON");
        let properties = schema
            .as_object_mut()
            .and_then(|o| {
                o.entry("properties")
                    .or_insert_with(|| serde_json::json!({}))
                    .as_object_mut()
            })
            .expect("the settings schema is an object");
        properties.insert(
            key.to_string(),
            serde_json::json!({ "type": "string", "format": "uri", "title": "Test Server" }),
        );
        self.schema = Some((name, schema.to_string()));
        self
    }

    /// The plugin id from the manifest.
    pub fn id(&self) -> String {
        jf_plugin_host::manifest::parse(&self.manifest)
            .expect("a valid manifest.toml")
            .id
    }

    /// A `.jfpkg` with `wasm`, signed with the test key when `signed`.
    pub fn package(&self, wasm: Vec<u8>, signed: bool) -> Vec<u8> {
        let mut builder = PackageBuilder::new().manifest(&self.manifest).wasm(wasm);
        if let Some((name, text)) = &self.schema {
            builder = builder.file(name, text.as_bytes().to_vec());
        }
        if let Some(icon) = &self.icon {
            builder = builder.icon(icon.clone());
        }
        if signed {
            builder = builder.sign(test_key_seed());
        }
        builder.build().expect("building the package")
    }
}

/// A plugin host in a temporary folder that trusts the test key.
pub struct Env {
    /// The temporary folder (the host's files live in `plugins/`).
    pub dir: tempfile::TempDir,
    /// The host.
    pub host: PluginHost,
    /// The keystore stand-in.
    pub secrets: Arc<MemorySecretStore>,
}

impl std::fmt::Debug for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env").field("dir", &self.dir.path()).finish()
    }
}

impl Default for Env {
    fn default() -> Env {
        Env::new()
    }
}

impl Env {
    /// A fresh host with production limits.
    pub fn new() -> Env {
        let dir = tempfile::tempdir().expect("temporary folder");
        let secrets = Arc::new(MemorySecretStore::new());
        let host = Env::open(dir.path(), &secrets);
        Env { dir, host, secrets }
    }

    fn open(dir: &Path, secrets: &Arc<MemorySecretStore>) -> PluginHost {
        let config = HostConfig {
            dir: dir.join("plugins"),
            cache_dir: dir.join("cache"),
            secrets: secrets.clone(),
            allow_community: false,
        };
        let options = HostOptions {
            trust: TrustList::builtin().with_test_key(),
            limits: Limits::default(),
            ..HostOptions::default()
        };
        PluginHost::with_options(config, options).expect("opening the plugin host")
    }

    /// Another host on the same folder and keystore (an app restart).
    pub fn reopen(&self) -> PluginHost {
        Env::open(self.dir.path(), &self.secrets)
    }

    /// Builds the plugin crate in `crate_dir` (cargo package `package`),
    /// packs and signs it with the test key, and installs it.
    pub fn install_crate(&self, crate_dir: &Path, package: &str) -> PluginInfo {
        let files = PluginFiles::read(crate_dir);
        let bytes = files.package(component_in(crate_dir, package), true);
        let path = self.dir.path().join(format!("{}.jfpkg", files.id()));
        std::fs::write(&path, bytes).expect("writing the package");
        self.host
            .install(&path)
            .unwrap_or_else(|e| panic!("installing {package}: {e}"))
    }

    /// Packs `files` (for example [`PluginFiles::with_test_server_setting`])
    /// with the component of the crate in `crate_dir` (cargo package
    /// `package`), signs it with the test key and installs it.
    pub fn install_files(&self, crate_dir: &Path, package: &str, files: &PluginFiles) -> PluginInfo {
        let bytes = files.package(component_in(crate_dir, package), true);
        let path = self.dir.path().join(format!("{}.jfpkg", files.id()));
        std::fs::write(&path, bytes).expect("writing the package");
        self.host
            .install(&path)
            .unwrap_or_else(|e| panic!("installing {package}: {e}"))
    }

    /// Sets the plugin's settings.
    pub fn settings(&self, id: &str, settings: serde_json::Value) {
        self.host
            .set_settings(id, &settings.to_string())
            .unwrap_or_else(|e| panic!("settings {settings}: {e}"));
    }

    /// Every file under the host's folder whose bytes contain `needle`.
    pub fn files_containing(&self, needle: &str) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut stack = vec![self.dir.path().to_path_buf()];
        while let Some(path) = stack.pop() {
            if path.is_dir() {
                for entry in std::fs::read_dir(&path).into_iter().flatten().flatten() {
                    stack.push(entry.path());
                }
            } else if let Ok(bytes) = std::fs::read(&path)
                && bytes.windows(needle.len()).any(|w| w == needle.as_bytes())
            {
                found.push(path);
            }
        }
        found
    }
}

/// Waits for a call's answer (at most 30 s).
pub fn recv<T>(reply: Reply<T>) -> Result<T, CallError> {
    reply
        .recv_timeout(Duration::from_secs(30))
        .expect("no answer within 30 s")
}

/// The first page of `limit` items.
pub fn page(limit: u32) -> PageRequest {
    PageRequest { cursor: None, limit }
}

/// The page after `cursor`.
pub fn next_page(cursor: Option<String>, limit: u32) -> PageRequest {
    PageRequest { cursor, limit }
}

/// Search filters for these kinds (none: every kind).
pub fn kinds(kinds: &[ItemKind]) -> SearchFilters {
    SearchFilters {
        kinds: kinds.to_vec(),
    }
}

/// Starts a login of `kind` and returns the flow.
pub fn login(host: &PluginHost, id: &str, kind: AuthKind) -> AuthFlow {
    host.begin_auth(id, kind)
        .unwrap_or_else(|e| panic!("starting a {kind} login: {e}"))
}

/// The next step of a login flow (at most 30 s).
pub fn next_step(flow: &AuthFlow) -> AuthStep {
    flow.steps()
        .recv_timeout(Duration::from_secs(30))
        .expect("no login step within 30 s")
}

/// The last step of a login flow, skipping the ones before it.
pub fn final_step(flow: &AuthFlow) -> AuthStep {
    loop {
        let step = next_step(flow);
        if step.is_final() {
            return step;
        }
    }
}

/// Form values.
pub fn fields(pairs: &[(&str, &str)]) -> Vec<FieldValue> {
    pairs
        .iter()
        .map(|(name, value)| FieldValue {
            name: name.to_string(),
            value: value.to_string(),
        })
        .collect()
}

/// An item of `kind` with only an id and a title, as the host passes back
/// what it stored.
pub fn item(kind: ItemKind, id: &str, title: &str) -> Item {
    Item {
        id: id.into(),
        kind,
        title: title.into(),
        artist: None,
        album: None,
        album_id: None,
        artist_id: None,
        duration_ms: None,
        track_number: None,
        disc_number: None,
        year: None,
        child_count: None,
        isrc: None,
        artwork: None,
        explicit: false,
        playable: true,
        unplayable: None,
    }
}
