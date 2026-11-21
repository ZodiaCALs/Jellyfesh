//! Jellyfesh Pro in Jellyfesh Web (ADR-0021), as the core's `pro.rs`: from a Pro Preview build
//! (`JF_PRO_PREVIEW=1` when jf-web was built) or a licence key the person entered, checked offline
//! with jf-licence against the keys built into this release, and checked again at every visit.
//! Jellyfesh Web is sold in no store, so licence keys always work here and there are no store
//! entitlements; Pro Preview comes first, as in the core.

use jf_licence::{Licence, PublicKey};
use serde::Serialize;

/// Whether this build is a Pro Preview build: only `JF_PRO_PREVIEW=1` counts.
pub fn preview_build() -> bool {
    option_env!("JF_PRO_PREVIEW") == Some("1")
}

/// Where Pro comes from (the core's `ProSource`, without stores).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ProSource {
    None,
    Preview,
    Licence {
        /// "Licence …ABCD".
        id_suffix: String,
        issued_at_ms: i64,
    },
}

/// Why a licence key was refused (the core's `LicenceError`); the page words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LicenceError {
    Malformed,
    UnknownKey,
    BadSignature,
    WrongProduct,
    Revoked,
}

fn licence_error(error: jf_licence::LicenceError) -> LicenceError {
    match error {
        jf_licence::LicenceError::Malformed => LicenceError::Malformed,
        jf_licence::LicenceError::UnknownKey => LicenceError::UnknownKey,
        jf_licence::LicenceError::BadSignature => LicenceError::BadSignature,
        jf_licence::LicenceError::WrongProduct => LicenceError::WrongProduct,
        jf_licence::LicenceError::Revoked => LicenceError::Revoked,
    }
}

/// The Pro slice (the core's `ProSnapshot`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProSnapshot {
    pub active: bool,
    pub source: ProSource,
    pub preview: bool,
    pub licence_error: Option<LicenceError>,
    /// Whether Enter Licence Key… is offered (always on the web).
    pub licence_keys: bool,
}

/// Pro as the page holds it.
#[derive(Debug)]
pub struct Pro {
    preview: bool,
    keys: Vec<PublicKey>,
    licence: Option<(String, Licence)>,
    error: Option<LicenceError>,
}

/// A key as it is kept: the canonical text (upper case, trimmed).
fn canonical(key: &str) -> String {
    key.trim().to_ascii_uppercase()
}

impl Pro {
    /// Pro for this build and these keys, with the key kept from an earlier visit (checked again:
    /// a key withdrawn in an update says why).
    pub fn new(preview: bool, keys: Vec<PublicKey>, stored_key: Option<&str>) -> Pro {
        let mut pro = Pro {
            preview,
            keys,
            licence: None,
            error: None,
        };
        if let Some(key) = stored_key.filter(|k| !k.trim().is_empty()) {
            match jf_licence::verify(key, &pro.keys) {
                Ok(licence) => pro.licence = Some((canonical(key), licence)),
                Err(error) => pro.error = Some(licence_error(error)),
            }
        }
        pro
    }

    /// Pro for this release: its preview setting and its keys.
    pub fn for_release(stored_key: Option<&str>) -> Pro {
        Pro::new(preview_build(), jf_licence::release_keys(), stored_key)
    }

    fn source(&self) -> ProSource {
        if self.preview {
            return ProSource::Preview;
        }
        match &self.licence {
            Some((_, licence)) => ProSource::Licence {
                id_suffix: licence.id_suffix(),
                issued_at_ms: licence.issued_at_ms(),
            },
            None => ProSource::None,
        }
    }

    /// Whether Pro features work now.
    pub fn active(&self) -> bool {
        self.source() != ProSource::None
    }

    pub fn snapshot(&self) -> ProSnapshot {
        ProSnapshot {
            active: self.active(),
            source: self.source(),
            preview: self.preview,
            licence_error: self.error,
            licence_keys: true,
        }
    }

    /// Checks and keeps a licence key; returns the text to keep. A bad key keeps the licence there
    /// was and sets the error.
    pub fn activate(&mut self, key: &str) -> Result<String, LicenceError> {
        match jf_licence::verify(key, &self.keys) {
            Ok(licence) => {
                let text = canonical(key);
                self.licence = Some((text.clone(), licence));
                self.error = None;
                Ok(text)
            }
            Err(error) => {
                let error = licence_error(error);
                self.error = Some(error);
                Err(error)
            }
        }
    }

    /// Removes the licence key (and any error about one).
    pub fn remove_licence(&mut self) {
        self.licence = None;
        self.error = None;
    }
}

#[cfg(test)]
mod tests {
    use jf_licence::Product;

    use super::*;

    const SECRET: [u8; 32] = [7; 32];

    fn key(day: u16) -> String {
        let licence = Licence {
            id: 0x0123_4567_89ab_cdef,
            key_id: 3,
            product: Product::Pro,
            issued_day: day,
            flags: 0,
        };
        jf_licence::issue(&SECRET, 3, &licence)
    }

    fn keys() -> Vec<PublicKey> {
        vec![jf_licence::public_key(&SECRET, 3)]
    }

    #[test]
    fn a_licence_key_turns_pro_on_and_shows_its_suffix() {
        let mut pro = Pro::new(false, keys(), None);
        assert!(!pro.active());
        assert_eq!(pro.snapshot().source, ProSource::None);
        let text = pro
            .activate(&format!("  {}  ", key(20_000).to_lowercase()))
            .unwrap();
        assert_eq!(text, key(20_000), "kept as canonical text");
        let snapshot = pro.snapshot();
        assert!(snapshot.active);
        let ProSource::Licence {
            id_suffix,
            issued_at_ms,
        } = snapshot.source
        else {
            panic!("a licence: {snapshot:?}");
        };
        assert_eq!(id_suffix.len(), 4);
        assert_eq!(issued_at_ms, 20_000 * 86_400_000);
        assert!(snapshot.licence_keys);
        pro.remove_licence();
        assert!(!pro.active());
    }

    #[test]
    fn a_bad_key_says_why_and_keeps_the_licence_there_was() {
        let mut pro = Pro::new(false, keys(), Some(&key(1)));
        assert!(pro.active());
        assert_eq!(pro.activate("JFP1-NOPE"), Err(LicenceError::Malformed));
        assert!(pro.active(), "the licence there was stays");
        assert_eq!(pro.snapshot().licence_error, Some(LicenceError::Malformed));
        // Signed by a key this build does not know.
        let other = jf_licence::issue(
            &[9; 32],
            4,
            &Licence {
                id: 1,
                key_id: 4,
                product: Product::Pro,
                issued_day: 1,
                flags: 0,
            },
        );
        assert_eq!(pro.activate(&other), Err(LicenceError::UnknownKey));
        // A mistyped key: one character changed.
        let good = key(2);
        let mut chars: Vec<char> = good.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'A' { 'B' } else { 'A' };
        let mistyped: String = chars.into_iter().collect();
        assert!(matches!(
            pro.activate(&mistyped),
            Err(LicenceError::BadSignature | LicenceError::Malformed)
        ));
    }

    #[test]
    fn a_kept_key_is_checked_again_at_every_visit() {
        let pro = Pro::new(false, keys(), Some(&key(5)));
        assert!(pro.active());
        // Not a key of this release (the keys changed): Pro is off, and the error says why.
        let pro = Pro::new(false, vec![jf_licence::public_key(&[1; 32], 3)], Some(&key(5)));
        assert!(!pro.active());
        assert_eq!(pro.snapshot().licence_error, Some(LicenceError::BadSignature));
    }

    #[test]
    fn preview_comes_first() {
        let pro = Pro::new(true, keys(), Some(&key(5)));
        assert_eq!(pro.snapshot().source, ProSource::Preview);
        assert!(pro.snapshot().preview);
    }
}
