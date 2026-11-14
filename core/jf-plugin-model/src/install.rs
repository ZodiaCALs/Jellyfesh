//! The rules for installing a package, shared by the native host (`jf-plugin-host`, `jf-core`)
//! and Jellyfesh Web, so every app refuses the same packages with the same words.

use crate::{HostError, SignatureStatus};

/// Largest `.jfpkg` a host reads.
pub const MAX_PACKAGE_FILE: u64 = 40 * 1024 * 1024;

/// Who signed a package: the key id, or `None` when it is unsigned.
pub fn signer(signature: &SignatureStatus) -> Option<String> {
    match signature {
        SignatureStatus::Unsigned => None,
        SignatureStatus::Trusted { key_id, .. } | SignatureStatus::Community { key_id } => {
            Some(key_id.clone())
        }
    }
}

/// Whether the package of plugin `id`, signed as `signature`, may be installed now.
///
/// `installed` is the signer remembered for the installed version, when one is installed
/// (`Some(None)`: it was unsigned). A package not signed by a trusted key is refused while
/// community plugins are off; an update must be signed by the same key as the installed version.
pub fn check(
    id: &str,
    signature: &SignatureStatus,
    installed: Option<Option<&str>>,
    allow_community: bool,
) -> Result<(), HostError> {
    let official = matches!(signature, SignatureStatus::Trusted { .. });
    if !official && !allow_community {
        return Err(HostError::CommunityDisabled);
    }
    if let Some(remembered) = installed
        && remembered != signer(signature).as_deref()
    {
        return Err(HostError::KeyChanged(
            id.to_owned(),
            remembered.unwrap_or("nobody (unsigned)").to_owned(),
        ));
    }
    Ok(())
}

/// What the apps say when a community package is refused because community plugins are off,
/// naming it (`(name, id)`) when the package could be read.
pub fn community_refusal(named: Option<(&str, &str)>) -> String {
    match named {
        Some((name, id)) => format!(
            "{name} ({id}) is a community plugin, and community plugins are turned off: allow them in Sources settings first"
        ),
        None => "community plugins are turned off: allow them in Sources settings first".into(),
    }
}

/// Whether a package the app ships (version `bundled`) updates the installed version
/// `installed`: only a newer version does, so a package someone installed themselves stays
/// until the app brings a newer one.
pub fn bundled_updates(installed: &str, bundled: &semver::Version) -> bool {
    semver::Version::parse(installed).is_ok_and(|v| v < *bundled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trusted() -> SignatureStatus {
        SignatureStatus::Trusted {
            key_id: "release".into(),
            label: "Jellyfesh".into(),
        }
    }

    fn community(key: &str) -> SignatureStatus {
        SignatureStatus::Community { key_id: key.into() }
    }

    #[test]
    fn community_packages_need_community_plugins_on() {
        assert!(check("a.b", &trusted(), None, false).is_ok());
        for signature in [community("k"), SignatureStatus::Unsigned] {
            assert!(matches!(
                check("a.b", &signature, None, false),
                Err(HostError::CommunityDisabled)
            ));
            assert!(check("a.b", &signature, None, true).is_ok());
        }
    }

    #[test]
    fn updates_keep_the_key() {
        assert!(check("a.b", &community("k"), Some(Some("k")), true).is_ok());
        assert!(check("a.b", &SignatureStatus::Unsigned, Some(None), true).is_ok());
        assert!(check("a.b", &trusted(), Some(Some("release")), false).is_ok());
        let other = check("a.b", &community("other"), Some(Some("k")), true).unwrap_err();
        assert_eq!(
            other.to_string(),
            "a.b was installed signed by key k; uninstall it before installing a package signed differently"
        );
        let unsigned = check("a.b", &community("k"), Some(None), true).unwrap_err();
        assert!(unsigned.to_string().contains("by key nobody (unsigned);"));
    }

    #[test]
    fn refusals_name_the_package() {
        assert_eq!(
            community_refusal(Some(("Echo", "org.example.echo"))),
            "Echo (org.example.echo) is a community plugin, and community plugins are turned off: allow them in Sources settings first"
        );
        assert!(community_refusal(None).starts_with("community plugins are turned off"));
    }

    #[test]
    fn only_newer_bundled_packages_update() {
        let v = |s: &str| semver::Version::parse(s).unwrap();
        assert!(bundled_updates("0.0.3", &v("0.0.4")));
        assert!(!bundled_updates("0.0.4", &v("0.0.4")));
        assert!(!bundled_updates("0.1.0", &v("0.0.4")));
        assert!(bundled_updates("0.0.4-beta.1", &v("0.0.4")));
        assert!(!bundled_updates("not a version", &v("0.0.4")));
    }
}
