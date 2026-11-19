//! What the build says about itself (ADR-0021 §4): how it reached the
//! person, from the `JF_DISTRIBUTION` build flag the release workflow sets
//! (`direct` for the tarball and the Flatpak on the release page). The core
//! decides from it which stores sell Jellyfesh Pro and whether it checks for
//! updates; builds from source say nothing and are `Source`.

use jf_core::Distribution;

/// The distribution a build flag names (`direct`, `app-store`,
/// `microsoft-store`, `google-play`, `web`, `source`); anything else, or no
/// flag, is a build from source.
pub fn distribution(flag: Option<&str>) -> Distribution {
    match flag.map(|f| f.trim().to_ascii_lowercase()).as_deref() {
        Some("direct") => Distribution::Direct,
        Some("app-store") => Distribution::AppStore,
        Some("microsoft-store") => Distribution::MicrosoftStore,
        Some("google-play") => Distribution::GooglePlay,
        Some("web") => Distribution::Web,
        _ => Distribution::Source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_names_the_distribution() {
        assert_eq!(distribution(Some("direct")), Distribution::Direct);
        assert_eq!(distribution(Some(" Direct ")), Distribution::Direct);
        assert_eq!(distribution(Some("app-store")), Distribution::AppStore);
        assert_eq!(distribution(Some("microsoft-store")), Distribution::MicrosoftStore);
        assert_eq!(distribution(Some("google-play")), Distribution::GooglePlay);
        assert_eq!(distribution(Some("web")), Distribution::Web);
    }

    #[test]
    fn builds_from_source_say_nothing() {
        assert_eq!(distribution(None), Distribution::Source);
        assert_eq!(distribution(Some("")), Distribution::Source);
        assert_eq!(distribution(Some("source")), Distribution::Source);
        assert_eq!(distribution(Some("flathub")), Distribution::Source);
    }
}
