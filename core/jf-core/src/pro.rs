//! Jellyfesh Pro (ADR-0021): where it comes from and the `Pro` slice.
//!
//! Sources, strongest first: a Pro Preview build (`JF_PRO_PREVIEW=1` when
//! the core was built), an active store entitlement the app reported, a
//! licence key the person entered (kept with the settings and checked again
//! at every start against the keys built into this release). The actor tells
//! the plugin host whether Pro is active, so Pro capabilities answer
//! `pro-required` while it is not.

use std::collections::BTreeMap;

use jf_licence::{Licence, PublicKey};

use crate::types::{Distribution, LicenceError, ProConfig, ProSnapshot, ProSource, Store, StoreEntitlement};

/// Whether this build is a Pro Preview build: only `JF_PRO_PREVIEW=1`
/// counts (the release workflow sends `0` when it is off).
pub fn preview_build() -> bool {
    option_env!("JF_PRO_PREVIEW") == Some("1")
}

/// The product id a store sells Pro as.
fn product(store: Store) -> &'static str {
    match store {
        Store::GooglePlay => "pro",
        Store::AppStore | Store::MicrosoftStore => "org.jellyfesh.pro",
    }
}

/// Pro as the actor holds it.
#[derive(Debug)]
pub(crate) struct Pro {
    preview: bool,
    distribution: Distribution,
    keys: Vec<PublicKey>,
    /// What each store said last.
    entitlements: BTreeMap<StoreKey, bool>,
    /// The licence key kept, and what it says.
    licence: Option<(String, Licence)>,
    error: Option<LicenceError>,
    version: u64,
}

/// `Store` as a map key.
type StoreKey = u8;

fn store_key(store: Store) -> StoreKey {
    match store {
        Store::AppStore => 0,
        Store::MicrosoftStore => 1,
        Store::GooglePlay => 2,
    }
}

impl Pro {
    /// Pro for this build, with the licence key kept from an earlier run (checked again).
    pub(crate) fn new(config: &ProConfig, distribution: Distribution, stored_key: Option<&str>) -> Pro {
        let mut pro = Pro {
            preview: config.preview,
            distribution,
            keys: config.licence_keys.clone(),
            entitlements: BTreeMap::new(),
            licence: None,
            error: None,
            version: 1,
        };
        if let Some(key) = stored_key.filter(|_| pro.licence_keys()) {
            match jf_licence::verify(key, &pro.keys) {
                Ok(licence) => pro.licence = Some((key.to_owned(), licence)),
                // A key that stopped working (revoked in an update): say why.
                Err(error) => pro.error = Some(licence_error(error)),
            }
        }
        pro
    }

    /// Whether licence keys work in this build (not in App Store and Google
    /// Play builds, which sell Pro only through their store).
    pub(crate) fn licence_keys(&self) -> bool {
        !matches!(
            self.distribution,
            Distribution::AppStore | Distribution::GooglePlay
        )
    }

    /// The store this build sells Pro in, if any.
    fn store(&self) -> Option<Store> {
        self.distribution.store()
    }

    /// Whether Pro features work now.
    pub(crate) fn active(&self) -> bool {
        !matches!(self.source(), ProSource::None)
    }

    fn source(&self) -> ProSource {
        if self.preview {
            return ProSource::Preview;
        }
        if let Some(store) = self.store()
            && self.entitlements.get(&store_key(store)) == Some(&true)
        {
            return ProSource::Store { store };
        }
        match &self.licence {
            Some((_, licence)) => ProSource::Licence {
                id_suffix: licence.id_suffix(),
                issued_at_ms: licence.issued_at_ms(),
            },
            None => ProSource::None,
        }
    }

    /// The `Pro` slice.
    pub(crate) fn snapshot(&self) -> ProSnapshot {
        ProSnapshot {
            version: self.version,
            active: self.active(),
            source: self.source(),
            preview: self.preview,
            licence_error: self.error,
            stores: self.store().into_iter().collect(),
            licence_keys: self.licence_keys(),
        }
    }

    /// Checks and keeps a licence key; returns the text to store. A bad key
    /// keeps the licence there was and sets the error.
    pub(crate) fn activate(&mut self, key: &str) -> Result<String, LicenceError> {
        self.version += 1;
        match jf_licence::verify(key, &self.keys) {
            Ok(licence) => {
                let text = jf_licence_text(key);
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
    pub(crate) fn remove_licence(&mut self) {
        self.version += 1;
        self.licence = None;
        self.error = None;
    }

    /// What a store reported. Returns whether anything changed; stores the
    /// build does not sell in, and other products, are ignored.
    pub(crate) fn set_entitlement(&mut self, entitlement: &StoreEntitlement) -> bool {
        if self.store() != Some(entitlement.store) || entitlement.product != product(entitlement.store) {
            tracing::info!(
                store = ?entitlement.store,
                product = %entitlement.product,
                "ignoring an entitlement this build does not sell"
            );
            return false;
        }
        let key = store_key(entitlement.store);
        if self.entitlements.get(&key) == Some(&entitlement.active) {
            return false;
        }
        self.entitlements.insert(key, entitlement.active);
        self.version += 1;
        true
    }
}

/// A key as it is stored: the canonical text (upper case, grouped).
fn jf_licence_text(key: &str) -> String {
    key.trim().to_ascii_uppercase()
}

/// Why a licence key was refused, for `CommandFailed` (the apps word the
/// `licence_error` themselves).
pub(crate) fn licence_error_text(error: LicenceError) -> &'static str {
    match error {
        LicenceError::Malformed => "that does not look like a Jellyfesh licence key",
        LicenceError::UnknownKey | LicenceError::BadSignature => "this licence key is not valid",
        LicenceError::WrongProduct => "this licence key is for another product",
        LicenceError::Revoked => "this licence key has been withdrawn",
    }
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
