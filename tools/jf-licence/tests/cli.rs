//! The jf-licence tool end to end: a key pair made here (never a release key), keys issued with
//! it and checked, and the refusals.

use std::path::Path;
use std::process::{Command, Output};

fn tool() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jf-licence"))
}

fn run(command: &mut Command) -> Output {
    command.output().expect("running jf-licence")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn makes_a_key_pair_issues_keys_and_checks_them() {
    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join("licence.key");
    let out = run(tool().args(["keygen", "--key-id", "9", "--out"]).arg(&secret));
    assert!(out.status.success(), "{}", text(&out.stderr));
    let public = text(&out.stdout).trim().to_string();
    assert!(public.starts_with("9:") && public.len() == 2 + 64, "{public}");
    let written = std::fs::read_to_string(&secret).unwrap();
    assert_eq!(written.trim().len(), 64);
    assert!(
        !text(&out.stdout).contains(written.trim()),
        "the secret is never printed"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&secret).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // Refuses to overwrite without --force.
    let again = run(tool().args(["keygen", "--key-id", "9", "--out"]).arg(&secret));
    assert!(!again.status.success());

    let out = run(tool()
        .args([
            "issue", "--key-id", "9", "--day", "20409", "--count", "2", "--secret",
        ])
        .arg(&secret));
    assert!(out.status.success(), "{}", text(&out.stderr));
    let keys: Vec<String> = text(&out.stdout).lines().map(str::to_string).collect();
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1], "every key has its own id");

    let out = run(tool().args(["verify", &keys[0], "--public", &public]));
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(text(&out.stdout).starts_with("valid: Jellyfesh Pro"));
    assert!(text(&out.stdout).contains("issued day  20409"));

    // Another key pair's public key: not valid, exit status 1.
    let other = dir.path().join("other.key");
    let other_public = text(&run(tool().args(["keygen", "--key-id", "9", "--out"]).arg(&other)).stdout)
        .trim()
        .to_string();
    let out = run(tool().args(["verify", &keys[0], "--public", &other_public]));
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stdout).contains("bad signature"));
    let out = run(tool().args(["verify", "JFP1-NOT-A-KEY", "--public", &public]));
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stdout).contains("malformed"));

    // The secret can come from the environment instead of a file.
    let out = run(tool()
        .args(["issue", "--key-id", "9"])
        .env("JF_LICENCE_SIGNING_KEY", written.trim()));
    assert!(out.status.success(), "{}", text(&out.stderr));
}

#[test]
fn refuses_to_write_the_secret_inside_a_repository() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = run(tool()
        .args(["keygen", "--key-id", "1", "--out"])
        .arg(repository.join("licence.key")));
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("git working tree"),
        "{}",
        text(&out.stderr)
    );
    assert!(!repository.join("licence.key").exists());
}

#[test]
fn checks_its_arguments() {
    let out = run(tool().args(["verify", "JFP1-X", "--public", "1:abc"]));
    assert!(!out.status.success());
    let out = run(tool()
        .args(["issue", "--key-id", "1"])
        .env("JF_LICENCE_SIGNING_KEY", "not hex"));
    assert!(!out.status.success());
}
