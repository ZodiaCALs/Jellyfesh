//! Sources → Install from File… refuses the packages the native host refuses, with its words,
//! and remote sources, which Jellyfesh Web cannot play (ADR-0018).

#![cfg(not(target_arch = "wasm32"))]

use jf_web::{bundled_updates, install_refusal, max_package_bytes};

fn package(signature: &str) -> String {
    format!(r#"{{"id":"org.example.echo","name":"Echo","signature":{signature}}}"#)
}

const UNSIGNED: &str = r#""Unsigned""#;
const RELEASE: &str = r#"{"Trusted":{"key_id":"release","label":"Jellyfesh"}}"#;
const AUTHOR: &str = r#"{"Community":{"key_id":"author"}}"#;

#[test]
fn community_packages_wait_for_community_plugins() {
    assert_eq!(install_refusal(&package(RELEASE), None, false).unwrap(), None);
    assert_eq!(
        install_refusal(&package(UNSIGNED), None, false)
            .unwrap()
            .as_deref(),
        Some(
            "Echo (org.example.echo) is a community plugin, and community plugins are turned off: \
             allow them in Sources settings first"
        )
    );
    assert_eq!(install_refusal(&package(AUTHOR), None, true).unwrap(), None);
}

#[test]
fn updates_need_the_same_key() {
    let same = install_refusal(&package(AUTHOR), Some(AUTHOR.into()), true).unwrap();
    assert_eq!(same, None);
    let other = install_refusal(&package(AUTHOR), Some(UNSIGNED.into()), true).unwrap();
    assert_eq!(
        other.as_deref(),
        Some(
            "org.example.echo was installed signed by key nobody (unsigned); uninstall it before \
             installing a package signed differently"
        )
    );
}

fn with_playback(signature: &str, mode: &str) -> String {
    format!(
        r#"{{"id":"org.example.remote","name":"Remote Example","signature":{signature},"playback_mode":"{mode}"}}"#
    )
}

const REMOTE_WORDS: &str = "Remote Example plays on your own devices, which Jellyfesh Web cannot do yet. \
                            Use the Jellyfesh app on your computer or phone.";

#[test]
fn remote_sources_are_refused_whoever_signed_them() {
    for signature in [RELEASE, AUTHOR, UNSIGNED] {
        for allow_community in [false, true] {
            let refusal =
                install_refusal(&with_playback(signature, "Remote"), None, allow_community).unwrap();
            assert_eq!(
                refusal.as_deref(),
                Some(REMOTE_WORDS),
                "{signature} {allow_community}"
            );
        }
        // Updating an installed copy is refused the same way.
        let refusal =
            install_refusal(&with_playback(signature, "Remote"), Some(signature.into()), true).unwrap();
        assert_eq!(refusal.as_deref(), Some(REMOTE_WORDS));
    }
    // Streaming sources go on to the usual checks.
    assert_eq!(
        install_refusal(&with_playback(RELEASE, "Stream"), None, false).unwrap(),
        None
    );
    assert_eq!(
        install_refusal(&with_playback(AUTHOR, "Vendor"), None, true).unwrap(),
        None
    );
    assert!(
        install_refusal(&with_playback(AUTHOR, "Stream"), None, false)
            .unwrap()
            .unwrap()
            .contains("community plugins are turned off")
    );
}

#[test]
fn only_newer_bundled_packages_replace_installed_ones() {
    assert!(bundled_updates("0.0.3", "0.0.4"));
    assert!(!bundled_updates("0.0.4", "0.0.4"));
    assert!(!bundled_updates("0.0.5", "0.0.4"));
    assert!(!bundled_updates("0.0.3", "next"));
    assert_eq!(max_package_bytes(), 40.0 * 1024.0 * 1024.0);
}
