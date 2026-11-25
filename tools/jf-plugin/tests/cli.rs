//! `jf-plugin` end to end, run as a plugin author runs it: a crate from the
//! template is built into a real component, then packed, signed, verified,
//! inspected and tampered with.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use jf_plugin::{KEY_ENV, SigningKey, read_entries};
use jf_plugin_host::package::{MANIFEST, PackageBuilder, WASM};

fn jf_plugin() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jf-plugin"));
    command.env_remove(KEY_ENV);
    command
}

fn run(command: &mut Command) -> Output {
    command.output().expect("running jf-plugin")
}

/// Standard output of a command that must succeed.
fn ok(command: &mut Command) -> String {
    let output = run(command);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(output.status.success(), "{command:?} failed:\n{stdout}\n{stderr}");
    stdout + &stderr
}

/// Standard error of a command that must fail.
fn fails(command: &mut Command) -> String {
    let output = run(command);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "{command:?} succeeded:\n{stdout}\n{stderr}"
    );
    stdout + &stderr
}

/// The cargo target folder this test was built in.
fn target_dir() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.ancestors().nth(3).unwrap().to_path_buf()
}

/// A crate made by `jf-plugin new`, and its component built by
/// `jf-plugin build`: once per test binary.
fn template_plugin() -> &'static (PathBuf, PathBuf) {
    static BUILT: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();
    BUILT.get_or_init(|| {
        let root = target_dir().join("jf-plugin-tests");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let dir = root.join("radio-demo");
        let out = ok(jf_plugin()
            .args([
                "new",
                "radio-demo",
                "--id",
                "org.example.radio-demo",
                "--author",
                "Tests",
            ])
            .arg("--path")
            .arg(&dir));
        assert!(out.contains("created"), "{out}");
        // The workspace's lock file pins the same dependency versions the
        // rest of the tests built, so no index or download is needed.
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap();
        std::fs::copy(workspace.join("Cargo.lock"), dir.join("Cargo.lock")).unwrap();
        let wasm = root.join("radio-demo.wasm");
        let out = ok(jf_plugin()
            .env("CARGO_NET_OFFLINE", "true")
            .arg("build")
            .arg("--path")
            .arg(&dir)
            .arg("--target-dir")
            .arg(target_dir())
            .arg("--out")
            .arg(&wasm));
        assert!(out.contains("jellyfesh:plugin/http@1.3.0"), "{out}");
        (dir, wasm)
    })
}

/// A fresh unsigned package of the template plugin in `dir`.
fn package(dir: &Path) -> PathBuf {
    let (crate_dir, wasm) = template_plugin();
    let out = dir.join("radio.jfpkg");
    let printed = ok(jf_plugin()
        .arg("pack")
        .arg("--path")
        .arg(crate_dir)
        .arg("--wasm")
        .arg(wasm)
        .arg("--out")
        .arg(&out));
    assert!(
        printed.contains("org.example.radio-demo 0.1.0, unsigned"),
        "{printed}"
    );
    out
}

fn seed_hex(key: &SigningKey) -> String {
    key.seed().iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn pack_sign_verify_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = package(dir.path());

    let out = ok(jf_plugin().arg("verify").arg(&pkg));
    assert!(
        out.contains("valid package org.example.radio-demo 0.1.0"),
        "{out}"
    );
    assert!(out.contains("signature: none: unsigned"), "{out}");
    assert!(fails(jf_plugin().arg("verify").arg(&pkg).arg("--require-signature")).contains("not signed"));

    // Sign with a key file.
    let key_file = dir.path().join("author.key");
    let out = ok(jf_plugin().arg("keygen").arg(&key_file));
    let key = SigningKey::from_file(&key_file).unwrap();
    assert!(
        out.contains(&key.key_id()) && out.contains(&key.public_hex()),
        "{out}"
    );
    let out = ok(jf_plugin().arg("sign").arg(&pkg).arg("--key").arg(&key_file));
    assert!(
        out.contains(&format!("signed {} with key id {}", pkg.display(), key.key_id())),
        "{out}"
    );

    let out = ok(jf_plugin().arg("verify").arg(&pkg).arg("--require-signature"));
    assert!(
        out.contains(&format!("valid, key id {}: not a trusted key", key.key_id())),
        "{out}"
    );
    let out = fails(jf_plugin().arg("verify").arg(&pkg).arg("--require-trusted"));
    assert!(out.contains("not a trusted key"), "{out}");
    let out =
        ok(jf_plugin()
            .arg("verify")
            .arg(&pkg)
            .args(["--require-trusted", "--trust-key", &key.public_hex()]));
    assert!(out.contains("the app shows it as Official"), "{out}");

    // Inspect shows the manifest, capabilities, signature, key id and imports.
    let out = ok(jf_plugin().arg("inspect").arg(&pkg));
    for expected in [
        "org.example.radio-demo 0.1.0: Radio Demo",
        "author         Tests",
        "capabilities   search",
        "${settings.server_url}",
        "server_url (text, URL)",
        &format!("key id {}", key.key_id()),
        "plugin.wasm",
        "SIGNATURE",
        "jellyfesh:plugin/http@1.3.0",
        "jellyfesh:plugin/catalog@1.3.0",
    ] {
        assert!(out.contains(expected), "{expected:?} missing from\n{out}");
    }
    let json: serde_json::Value =
        serde_json::from_str(&ok(jf_plugin().arg("inspect").arg("--json").arg(&pkg))).unwrap();
    assert_eq!(json["signature"]["status"], "community");
    assert_eq!(json["signature"]["key_id"], key.key_id());
    assert_eq!(json["manifest"]["capabilities"], serde_json::json!(["search"]));

    // Signing again replaces the signature.
    let other = dir.path().join("other.key");
    ok(jf_plugin().arg("keygen").arg(&other));
    ok(jf_plugin().arg("sign").arg(&pkg).arg("--key").arg(&other));
    let other_id = SigningKey::from_file(&other).unwrap().key_id();
    assert!(ok(jf_plugin().arg("verify").arg(&pkg)).contains(&other_id));
}

#[test]
fn signs_with_the_key_in_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = package(dir.path());
    let test_key = SigningKey::test_key();
    let secret = seed_hex(&test_key);

    // The CI secret: 64 hex characters, here with a trailing newline.
    let out = ok(jf_plugin()
        .env(KEY_ENV, format!("{secret}\n"))
        .arg("sign")
        .arg(&pkg));
    assert!(out.contains(&test_key.key_id()), "{out}");
    assert!(!out.contains(&secret), "the key was printed");
    let out = ok(jf_plugin()
        .arg("verify")
        .arg(&pkg)
        .args(["--trust-test-key", "--require-trusted"]));
    assert!(
        out.contains("Jellyfesh test key, the app shows it as Official"),
        "{out}"
    );
    // The app's own trust list does not include the test key.
    assert!(ok(jf_plugin().arg("verify").arg(&pkg)).contains("not a trusted key"));

    // No key at all, or a malformed one.
    let out = fails(jf_plugin().arg("sign").arg(&pkg));
    assert!(out.contains(KEY_ENV), "{out}");
    let out = fails(jf_plugin().env(KEY_ENV, "").arg("sign").arg(&pkg));
    assert!(out.contains("no signing key"), "{out}");
    let wrong = format!("{}zz", &secret[..62]);
    let out = fails(jf_plugin().env(KEY_ENV, &wrong).arg("sign").arg(&pkg));
    assert!(out.contains("64 hex characters"), "{out}");
    assert!(
        !out.contains(&wrong[..40]),
        "the malformed key was printed: {out}"
    );
}

#[test]
fn tampered_packages_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = package(dir.path());
    let key = dir.path().join("author.key");
    ok(jf_plugin().arg("keygen").arg(&key));
    ok(jf_plugin().arg("sign").arg(&pkg).arg("--key").arg(&key));
    let key_id = SigningKey::from_file(&key).unwrap().key_id();
    let entries = read_entries(&std::fs::read(&pkg).unwrap()).unwrap();

    let tamper = |name: &str, change: &dyn Fn(&mut Vec<u8>)| {
        let mut entries = entries.clone();
        change(entries.get_mut(name).unwrap());
        let bytes = entries
            .into_iter()
            .fold(PackageBuilder::new(), |b, (n, c)| b.file(&n, c))
            .build()
            .unwrap();
        let path = dir.path().join(format!("tampered-{name}.jfpkg"));
        std::fs::write(&path, bytes).unwrap();
        path
    };
    // A changed manifest (wider network access) under the old signature.
    let manifest = tamper(MANIFEST, &|text| {
        let edited = String::from_utf8(text.clone())
            .unwrap()
            .replace("max_rps = 5", "max_rps = 20");
        *text = edited.into_bytes();
    });
    // One flipped byte in the component's custom sections at the end.
    let wasm = tamper(WASM, &|bytes| {
        let last = bytes.len() - 1;
        bytes[last] ^= 0x20;
    });
    for path in [&manifest, &wasm] {
        let out = fails(jf_plugin().arg("verify").arg(path));
        assert!(
            out.contains("the signature does not match the package contents"),
            "{out}"
        );
        // Inspect still shows what it can.
        let out = ok(jf_plugin().arg("inspect").arg(path));
        assert!(out.contains("signature      INVALID"), "{out}");
        assert!(out.contains(&format!("claims key id {key_id}")), "{out}");
    }

    // Not a package at all.
    let junk = dir.path().join("junk.jfpkg");
    std::fs::write(&junk, b"PK\x03\x04 but not really").unwrap();
    assert!(fails(jf_plugin().arg("verify").arg(&junk)).contains("not a valid plugin package"));
    // Re-signing a tampered package is fine: the new signature covers it.
    ok(jf_plugin().arg("sign").arg(&manifest).arg("--key").arg(&key));
    ok(jf_plugin().arg("verify").arg(&manifest));
}

#[test]
fn keygen_writes_a_private_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("signing.key");
    let out = ok(jf_plugin().arg("keygen").arg(&file));
    let text = std::fs::read_to_string(&file).unwrap();
    let seed = text.trim_end();
    assert_eq!(text.len(), 65, "64 hex characters and a newline");
    assert!(seed.bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(!out.contains(seed), "the key was printed: {out}");
    assert_eq!(
        SigningKey::parse(&text).unwrap().key_id(),
        SigningKey::from_file(&file).unwrap().key_id()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{mode:o}");
    }

    // Never over an existing key unless asked.
    assert!(fails(jf_plugin().arg("keygen").arg(&file)).contains("already exists"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
}

#[test]
#[cfg(unix)]
fn keygen_force_replaces_and_tightens_the_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("signing.key");
    std::fs::write(&file, "old\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    ok(jf_plugin().arg("keygen").arg(&file).arg("--force"));
    assert_ne!(std::fs::read_to_string(&file).unwrap(), "old\n");
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    // A key file others can read still works, with a warning.
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let pkg = package(dir.path());
    let out = ok(jf_plugin().arg("sign").arg(&pkg).arg("--key").arg(&file));
    assert!(out.contains("other users can read the key file"), "{out}");
}

#[test]
fn pack_checks_the_crate() {
    let dir = tempfile::tempdir().unwrap();
    let (crate_dir, wasm) = template_plugin();
    let copy = dir.path().join("crate");
    std::fs::create_dir_all(&copy).unwrap();
    for file in ["manifest.toml", "settings.schema.json"] {
        std::fs::copy(crate_dir.join(file), copy.join(file)).unwrap();
    }
    let manifest = std::fs::read_to_string(copy.join("manifest.toml")).unwrap();
    let pack = |copy: &Path| {
        let mut command = jf_plugin();
        command
            .arg("pack")
            .arg("--path")
            .arg(copy)
            .arg("--wasm")
            .arg(wasm)
            .arg("--out")
            .arg(copy.join("out.jfpkg"));
        command
    };
    ok(&mut pack(&copy));

    // A typo in the manifest is an error, not a silently weaker policy.
    std::fs::write(copy.join("manifest.toml"), manifest.replace("max_rps", "max_rsp")).unwrap();
    assert!(fails(&mut pack(&copy)).contains("unknown field"));
    // The allowlist's setting must be a URL property of the schema.
    std::fs::write(copy.join("manifest.toml"), &manifest).unwrap();
    std::fs::write(
        copy.join("settings.schema.json"),
        r#"{"type": "object", "properties": {"server_url": {"type": "string"}}}"#,
    )
    .unwrap();
    assert!(fails(&mut pack(&copy)).contains("format"));
    // A core module is not a component.
    std::fs::copy(
        crate_dir.join("settings.schema.json"),
        copy.join("settings.schema.json"),
    )
    .unwrap();
    std::fs::write(copy.join("module.wasm"), b"\0asm\x01\0\0\0").unwrap();
    let out = fails(
        jf_plugin()
            .arg("pack")
            .arg("--path")
            .arg(&copy)
            .arg("--wasm")
            .arg(copy.join("module.wasm")),
    );
    assert!(out.contains("checking the component"), "{out}");
}
