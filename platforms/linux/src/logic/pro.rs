//! Jellyfesh Pro (ADR-0021, docs/ui.md "AI Lyrics, Customize and Pro" and
//! "Settings"): what Settings › Extensions › Jellyfesh Pro says for the
//! `Pro` slice, and which message a refused licence key shows. The core
//! decides whether Pro is active and checks the keys; this picks the words.

use jf_core::{LicenceError, ProSnapshot, ProSource};

/// The Jellyfesh Pro group's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// A Pro Preview build: "Pro features are free during the public beta."
    Preview,
    /// "Jellyfesh Pro · Purchased".
    Purchased,
    /// "Jellyfesh Pro · Licence …ABCD", with Remove Licence….
    Licence { id_suffix: String },
    /// "AI Lyrics and Customize are part of Jellyfesh Pro.", with Enter
    /// Licence Key… where the build takes keys.
    Inactive,
}

pub fn status(pro: &ProSnapshot) -> Status {
    match &pro.source {
        ProSource::Preview => Status::Preview,
        ProSource::Store { .. } => Status::Purchased,
        ProSource::Licence { id_suffix, .. } => Status::Licence {
            id_suffix: id_suffix.clone(),
        },
        ProSource::None => Status::Inactive,
    }
}

/// What a refused key shows (docs/ui.md): an unknown key and a bad
/// signature read the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyProblem {
    /// "That doesn't look like a Jellyfesh licence key."
    Malformed,
    /// "This licence key isn't valid."
    Invalid,
    /// "This key is for another product."
    OtherProduct,
    /// "This licence key has been withdrawn."
    Withdrawn,
}

pub fn key_problem(error: LicenceError) -> KeyProblem {
    match error {
        LicenceError::Malformed => KeyProblem::Malformed,
        LicenceError::UnknownKey | LicenceError::BadSignature => KeyProblem::Invalid,
        LicenceError::WrongProduct => KeyProblem::OtherProduct,
        LicenceError::Revoked => KeyProblem::Withdrawn,
    }
}

/// Whether Activate can send what was typed (the core checks the rest).
pub fn can_activate(key: &str) -> bool {
    !key.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use jf_core::Store;

    use super::*;

    fn pro(source: ProSource) -> ProSnapshot {
        ProSnapshot {
            version: 1,
            active: source != ProSource::None,
            source,
            preview: false,
            licence_error: None,
            stores: Vec::new(),
            licence_keys: true,
        }
    }

    #[test]
    fn the_status_follows_the_source() {
        assert_eq!(status(&pro(ProSource::None)), Status::Inactive);
        assert_eq!(status(&pro(ProSource::Preview)), Status::Preview);
        assert_eq!(
            status(&pro(ProSource::Store {
                store: Store::MicrosoftStore
            })),
            Status::Purchased
        );
        assert_eq!(
            status(&pro(ProSource::Licence {
                id_suffix: "ABCD".into(),
                issued_at_ms: 0,
            })),
            Status::Licence {
                id_suffix: "ABCD".into()
            }
        );
    }

    #[test]
    fn refused_keys_read_as_the_ui_reference_words_them() {
        assert_eq!(key_problem(LicenceError::Malformed), KeyProblem::Malformed);
        assert_eq!(key_problem(LicenceError::UnknownKey), KeyProblem::Invalid);
        assert_eq!(key_problem(LicenceError::BadSignature), KeyProblem::Invalid);
        assert_eq!(key_problem(LicenceError::WrongProduct), KeyProblem::OtherProduct);
        assert_eq!(key_problem(LicenceError::Revoked), KeyProblem::Withdrawn);
    }

    #[test]
    fn a_blank_key_is_not_sent() {
        assert!(!can_activate("  "));
        assert!(can_activate("JFP1-ABCD"));
    }
}
