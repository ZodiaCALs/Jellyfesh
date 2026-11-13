//! The Rust side of Jellyfesh Web's plugin workers.
//!
//! A plugin's WIT imports are synchronous, so a worker cannot await
//! WebCrypto or `fetch` inside one. The worker implements `http` with a
//! synchronous request (allowed in dedicated workers) and asks this module,
//! synchronously, what the native host decides in Rust:
//!
//! * [`Sandbox`]: the request rules of `jf_plugin_model::http` (headers the
//!   host sets, sizes, user information), the plugin's effective allowlist
//!   (`jf_plugin_model::allow`, also for the URL a redirect ended at) and
//!   its `max_rps` token bucket;
//! * the `crypto` import (`jf_plugin_model::crypto`): the same RustCrypto
//!   code as the apps, so MD5, AES-ECB and RSA PKCS #1 v1.5 (which WebCrypto
//!   lacks) behave identically;
//! * [`Package`]: reading a `.jfpkg` and checking its signature against the
//!   built-in trust list (the Jellyfesh release key), as the native host
//!   installs packages. A worker verifies its plugin's package before it runs
//!   it; the build reads the packages with it too.
//!
//! Errors are thrown as JavaScript errors whose message is the WIT error as
//! JSON (`{"NotAllowed":"…"}`, `{"InvalidKey":"…"}`), which the worker turns
//! into the component's error variants.

use jf_plugin_model::allow::{Access, Allowlist};
use jf_plugin_model::crypto;
use jf_plugin_model::http::{self as rules, TokenBucket};
use jf_plugin_model::manifest::AllowRule;
use jf_plugin_model::package;
use jf_plugin_model::trust::TrustList;
use jf_plugin_model::{
    AesMode, AuthKind, Capability, ContentPolicy, CryptoError, HashAlgorithm, Header, PlaybackMode,
    RsaPadding, RsaPublicKey, SignatureStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;
use wasm_bindgen::prelude::*;

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}

/// A WIT `http-error`, as the worker turns it into the component's variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum HttpError {
    NotAllowed(String),
    InvalidRequest(String),
}

fn thrown(error: &impl Serialize) -> JsError {
    JsError::new(&json(error))
}

/// One plugin's network rules.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Sandbox {
    allowlist: Allowlist,
    bucket: TokenBucket,
}

/// A request as the worker describes it.
#[derive(Debug, Deserialize)]
struct RequestHead {
    method: String,
    url: String,
    headers: Vec<Header>,
    body_len: usize,
}

impl Sandbox {
    /// The checked URL to send, or why not.
    pub fn check(
        &self,
        method: &str,
        url: &str,
        headers: &[Header],
        body_len: usize,
    ) -> Result<Url, HttpError> {
        let url = rules::check_request(url, headers, body_len, method.eq_ignore_ascii_case("head"))
            .map_err(HttpError::InvalidRequest)?;
        self.allowlist.check(&url).map_err(HttpError::NotAllowed)?;
        Ok(url)
    }
}

#[wasm_bindgen]
impl Sandbox {
    /// The rules for a plugin's allow rules (`AllowRule` list as JSON), its
    /// settings and its manifest's `max_rps`.
    #[wasm_bindgen(constructor)]
    pub fn new(rules: &str, settings: &str, max_rps: u32) -> Result<Sandbox, JsError> {
        let rules: Vec<AllowRule> = serde_json::from_str(rules).map_err(|e| JsError::new(&e.to_string()))?;
        let settings: Value = serde_json::from_str(settings).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Sandbox {
            allowlist: Allowlist::new(&rules, &settings),
            bucket: TokenBucket::new(max_rps),
        })
    }

    /// Checks a request (`{method, url, headers, body_len}`): returns the URL
    /// to send (fragment dropped), or throws `NotAllowed`/`InvalidRequest`.
    pub fn check_request(&self, head: &str) -> Result<String, JsError> {
        let head: RequestHead = serde_json::from_str(head).map_err(|e| JsError::new(&e.to_string()))?;
        self.check(&head.method, &head.url, &head.headers, head.body_len)
            .map(String::from)
            .map_err(|e| thrown(&e))
    }

    /// Whether the URL a response came from (after redirects) is on the
    /// allowlist; `user` (the person's own server) or `fixed`, else `null`.
    pub fn access(&self, url: &str) -> Option<String> {
        let url = Url::parse(url).ok()?;
        Some(
            match self.allowlist.check(&url).ok()? {
                Access::UserServer => "user",
                Access::Fixed => "fixed",
            }
            .to_owned(),
        )
    }

    /// Takes a request token at `now_ms`; `None` when there is one, else how
    /// many milliseconds to wait.
    pub fn take_token(&mut self, now_ms: f64) -> Option<f64> {
        self.bucket.take(now_ms)
    }

    /// The limits: `{"max_response":…,"max_per_call":…,"max_redirects":…,"max_timeout_ms":…}`.
    pub fn limits() -> String {
        json(&serde_json::json!({
            "max_response": rules::MAX_RESPONSE,
            "max_per_call": rules::MAX_PER_CALL,
            "max_redirects": rules::MAX_REDIRECTS,
            "max_timeout_ms": rules::MAX_TIMEOUT.as_millis() as u64,
        }))
    }
}

/// A package as the browser shows and runs it.
#[derive(Debug, Clone, Serialize)]
pub struct PackageInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: Option<String>,
    /// Signed by a key in the built-in trust list (the release key).
    pub official: bool,
    pub signature: SignatureStatus,
    pub api_version: String,
    pub capabilities: Vec<Capability>,
    pub auth_kinds: Vec<AuthKind>,
    pub playback_mode: PlaybackMode,
    pub allow: Vec<AllowRule>,
    pub max_rps: u32,
    pub settings_schema: Value,
    pub settings_defaults: Value,
    pub content_policy: ContentPolicy,
    /// Lowercase hex SHA-256 of `plugin.wasm`, to match the transpiled
    /// component the page loads.
    pub wasm_sha256: String,
    pub has_icon: bool,
}

/// Reads and checks a `.jfpkg` against the built-in trust list (the
/// Jellyfesh release key), as `jf-plugin-host` installs packages. Returns
/// the package's description and its component.
pub fn read_package(bytes: &[u8]) -> Result<(PackageInfo, Vec<u8>), String> {
    let package = package::read(bytes, &TrustList::builtin()).map_err(|e| e.to_string())?;
    let manifest = &package.manifest;
    let info = PackageInfo {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.to_string(),
        author: manifest.author.clone(),
        description: manifest.description.clone(),
        official: matches!(package.signature, SignatureStatus::Trusted { .. }),
        signature: package.signature.clone(),
        api_version: manifest.api_version.to_string(),
        capabilities: manifest.capabilities.clone(),
        auth_kinds: manifest.auth.clone(),
        playback_mode: manifest.playback_mode,
        allow: manifest.allow.clone(),
        max_rps: manifest.max_rps,
        settings_schema: package.settings_schema.json().clone(),
        settings_defaults: package.settings_schema.defaults(),
        content_policy: manifest.content_policy.clone(),
        wasm_sha256: sha256_hex(&package.wasm),
        has_icon: package.icon.is_some(),
    };
    Ok((info, package.wasm))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A package read and checked against the built-in trust list.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Package {
    info: String,
    wasm: Vec<u8>,
}

#[wasm_bindgen]
impl Package {
    /// Reads and checks a `.jfpkg`; throws why it is not a valid package.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<Package, JsError> {
        let (info, wasm) = read_package(bytes).map_err(|e| JsError::new(&e))?;
        Ok(Package {
            info: json(&info),
            wasm,
        })
    }

    /// `PackageInfo` as JSON.
    pub fn info(&self) -> String {
        self.info.clone()
    }

    /// The component (`plugin.wasm`).
    pub fn component(&self) -> Vec<u8> {
        self.wasm.clone()
    }
}

fn algorithm(name: &str) -> Result<HashAlgorithm, JsError> {
    serde_json::from_str(&format!("\"{name}\""))
        .map_err(|_| thrown(&CryptoError::InvalidInput(format!("unknown hash {name}"))))
}

fn crypto_result(result: Result<Vec<u8>, CryptoError>) -> Result<Vec<u8>, JsError> {
    result.map_err(|e| thrown(&e))
}

/// `crypto.digest`: `algorithm` is `Md5`, `Sha1`, `Sha256` or `Sha512`.
#[wasm_bindgen]
pub fn digest(algorithm_name: &str, data: &[u8]) -> Result<Vec<u8>, JsError> {
    crypto_result(crypto::digest(algorithm(algorithm_name)?, data))
}

/// `crypto.hmac`.
#[wasm_bindgen]
pub fn hmac(algorithm_name: &str, key: &[u8], data: &[u8]) -> Result<Vec<u8>, JsError> {
    crypto_result(crypto::hmac(algorithm(algorithm_name)?, key, data))
}

fn aes_mode(mode: &str) -> Result<AesMode, JsError> {
    serde_json::from_str(mode).map_err(|e| thrown(&CryptoError::InvalidKey(format!("bad AES mode: {e}"))))
}

/// `crypto.aes-encrypt`; `mode` is an `AesMode` as JSON.
#[wasm_bindgen]
pub fn aes_encrypt(mode: &str, key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, JsError> {
    crypto_result(crypto::aes_encrypt(&aes_mode(mode)?, key, plaintext))
}

/// `crypto.aes-decrypt`; `mode` is an `AesMode` as JSON.
#[wasm_bindgen]
pub fn aes_decrypt(mode: &str, key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, JsError> {
    crypto_result(crypto::aes_decrypt(&aes_mode(mode)?, key, ciphertext))
}

/// `crypto.rsa-encrypt`; `key` is an `RsaPublicKey` as JSON, `padding`
/// `Pkcs1v15`, `OaepSha1` or `OaepSha256`.
#[wasm_bindgen]
pub fn rsa_encrypt(key: &str, padding: &str, data: &[u8]) -> Result<Vec<u8>, JsError> {
    let key: RsaPublicKey = serde_json::from_str(key)
        .map_err(|e| thrown(&CryptoError::InvalidKey(format!("bad RSA key: {e}"))))?;
    let padding: RsaPadding = serde_json::from_str(&format!("\"{padding}\""))
        .map_err(|_| thrown(&CryptoError::InvalidInput(format!("unknown padding {padding}"))))?;
    crypto_result(crypto::rsa_encrypt(&key, padding, data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_model::package::PackageBuilder;
    use serde_json::json;

    const MANIFEST: &str = r#"
id = "org.example.music"
name = "Example"
version = "1.0.0"
api_version = "1.0"
author = "Tests"
capabilities = ["auth", "search"]
playback_mode = "stream"
auth = ["qr", "password-exchange"]

[network]
allow = ["${settings.server_url}"]
max_rps = 20

[settings]
schema = "settings.schema.json"
"#;
    const SCHEMA: &str = r#"{"type": "object", "properties": {
        "server_url": {"type": "string", "format": "uri"},
        "quality": {"type": "string", "enum": ["app", "low"], "default": "app"}},
        "required": ["server_url"]}"#;

    fn package() -> Vec<u8> {
        PackageBuilder::new()
            .manifest(MANIFEST)
            .wasm(b"\0asm-test".to_vec())
            .file("settings.schema.json", SCHEMA.as_bytes().to_vec())
            .build()
            .unwrap()
    }

    #[test]
    fn reads_packages_against_the_trust_list() {
        let (info, wasm) = read_package(&package()).unwrap();
        assert_eq!(info.id, "org.example.music");
        assert!(!info.official, "unsigned");
        assert_eq!(info.signature, SignatureStatus::Unsigned);
        assert_eq!(info.allow, vec![AllowRule::Setting("server_url".into())]);
        assert_eq!(info.settings_defaults, json!({"quality": "app"}));
        assert_eq!(wasm, b"\0asm-test");
        assert_eq!(info.wasm_sha256.len(), 64);
        assert!(read_package(b"junk").is_err());
        // Signed with the published test key: valid, but not the release key.
        let signed = PackageBuilder::new()
            .manifest(MANIFEST)
            .wasm(b"\0asm-test".to_vec())
            .file("settings.schema.json", SCHEMA.as_bytes().to_vec())
            .sign(jf_plugin_model::trust::test_key_seed())
            .build()
            .unwrap();
        let (info, _) = read_package(&signed).unwrap();
        assert!(!info.official);
        assert!(matches!(info.signature, SignatureStatus::Community { .. }));
    }

    fn sandbox() -> Sandbox {
        Sandbox {
            allowlist: Allowlist::new(
                &[AllowRule::Setting("server_url".into())],
                &json!({"server_url": "http://192.168.1.20:4533"}),
            ),
            bucket: TokenBucket::new(20),
        }
    }

    fn header(name: &str) -> Header {
        Header {
            name: name.into(),
            value: "x".into(),
        }
    }

    #[test]
    fn requests_follow_the_native_rules() {
        let s = sandbox();
        let url = s
            .check("GET", "http://192.168.1.20:4533/rest/ping?f=json#x", &[], 0)
            .unwrap();
        assert_eq!(url.as_str(), "http://192.168.1.20:4533/rest/ping?f=json");
        assert!(matches!(
            s.check("GET", "https://other.example.org/", &[], 0),
            Err(HttpError::NotAllowed(_))
        ));
        assert!(matches!(
            s.check("GET", "http://192.168.1.20:4533/", &[header("Host")], 0),
            Err(HttpError::InvalidRequest(_))
        ));
        assert!(matches!(
            s.check("HEAD", "http://192.168.1.20:4533/", &[], 3),
            Err(HttpError::InvalidRequest(_))
        ));
        assert_eq!(s.access("http://192.168.1.20:4533/x").as_deref(), Some("user"));
        assert_eq!(s.access("http://192.168.1.20:9999/x"), None);
    }

    #[test]
    fn errors_serialise_as_wit_variants() {
        assert_eq!(json(&HttpError::NotAllowed("x".into())), "{\"NotAllowed\":\"x\"}");
        assert_eq!(json(&CryptoError::DecryptFailed), "\"DecryptFailed\"");
    }

    #[test]
    fn crypto_matches_known_vectors() {
        let md5 = crypto::digest(HashAlgorithm::Md5, b"abc").unwrap();
        assert_eq!(md5[..4], [0x90, 0x01, 0x50, 0x98]);
        let mode: AesMode =
            serde_json::from_str("{\"Gcm\":{\"nonce\":[0,0,0,0,0,0,0,0,0,0,0,0],\"aad\":[]}}").unwrap();
        let sealed = crypto::aes_encrypt(&mode, &[7; 32], b"secret").unwrap();
        assert_eq!(crypto::aes_decrypt(&mode, &[7; 32], &sealed).unwrap(), b"secret");
    }
}
