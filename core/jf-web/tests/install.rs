//! Sources → Install from File… refuses the packages the native host refuses, with its words.

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

#[test]
fn only_newer_bundled_packages_replace_installed_ones() {
    assert!(bundled_updates("0.0.3", "0.0.4"));
    assert!(!bundled_updates("0.0.4", "0.0.4"));
    assert!(!bundled_updates("0.0.5", "0.0.4"));
    assert!(!bundled_updates("0.0.3", "next"));
    assert_eq!(max_package_bytes(), 40.0 * 1024.0 * 1024.0);
}
