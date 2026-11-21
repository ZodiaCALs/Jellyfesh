//! About (docs/ui.md, "Settings"): the website, the third-party licences
//! and the legal pages (legal/README.md: the Licence Agreement, the Privacy
//! Policy and the Terms of Sale) where the build knows where they are
//! published, derived as the macOS,
//! Windows and Android builds do: the website the build was given
//! (`JF_WEBSITE`), else, in GitHub Actions, the repository's GitHub Pages
//! address, where the website is deployed (docs/github.md); the licences are
//! its `THIRD_PARTY_NOTICES.html` (tools/release/build_site.py). Without a
//! website, the notices file installed next to the app
//! (`share/doc/jellyfesh/THIRD_PARTY_NOTICES.md`, packaging/install.sh).

use std::path::{Path, PathBuf};

/// The page the website publishes the third-party notices on.
pub const NOTICES_PAGE: &str = "THIRD_PARTY_NOTICES.html";

/// The legal pages, relative to the website (tools/release/legal_pages.py
/// writes them).
pub const LICENCE_AGREEMENT_PAGE: &str = "legal/eula.html";
pub const PRIVACY_POLICY_PAGE: &str = "legal/privacy.html";
pub const TERMS_OF_SALE_PAGE: &str = "legal/terms-of-sale.html";

/// The website and the pages it publishes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Links {
    pub website: String,
    pub licences: String,
    pub licence_agreement: String,
    pub privacy_policy: String,
    pub terms_of_sale: String,
}

/// The website a build knows: `explicit` (the `JF_WEBSITE` build
/// variable) when set, else the GitHub Pages address of `repository`
/// (`GITHUB_REPOSITORY`, `owner/name`).
pub fn website(explicit: Option<&str>, repository: Option<&str>) -> Option<String> {
    if let Some(site) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        return Some(site.to_owned());
    }
    let (owner, name) = repository?.trim().split_once('/')?;
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some(format!("https://{}.github.io/{name}/", owner.to_lowercase()))
}

/// The website (with a trailing slash) and the pages it publishes; `None`
/// unless `website` is an `https` address.
pub fn links(website: Option<&str>) -> Option<Links> {
    let site = website?.trim();
    if !site.starts_with("https://") || site.len() <= "https://".len() {
        return None;
    }
    let base = if site.ends_with('/') {
        site.to_owned()
    } else {
        format!("{site}/")
    };
    Some(Links {
        licences: format!("{base}{NOTICES_PAGE}"),
        licence_agreement: format!("{base}{LICENCE_AGREEMENT_PAGE}"),
        privacy_policy: format!("{base}{PRIVACY_POLICY_PAGE}"),
        terms_of_sale: format!("{base}{TERMS_OF_SALE_PAGE}"),
        website: base,
    })
}

/// Where the notices file is installed for the app at `exe`
/// (`<prefix>/bin/jellyfesh` → `<prefix>/share/doc/jellyfesh/…`).
pub fn notices_file(exe: &Path) -> Option<PathBuf> {
    Some(
        exe.parent()?
            .parent()?
            .join("share/doc/jellyfesh/THIRD_PARTY_NOTICES.md"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_given_website_wins() {
        assert_eq!(
            website(Some(" https://jellyfesh.example/ "), Some("Owner/jellyfesh")).as_deref(),
            Some("https://jellyfesh.example/")
        );
        assert_eq!(
            website(Some(""), Some("Owner/jellyfesh")).as_deref(),
            Some("https://owner.github.io/jellyfesh/")
        );
    }

    #[test]
    fn github_pages_from_the_repository() {
        assert_eq!(
            website(None, Some("ZodiaCALs/jellyfesh")).as_deref(),
            Some("https://zodiacals.github.io/jellyfesh/")
        );
        assert_eq!(website(None, Some("no-slash")), None);
        assert_eq!(website(None, Some("a/b/c")), None);
        assert_eq!(website(None, Some("/name")), None);
        assert_eq!(website(None, None), None);
    }

    #[test]
    fn the_notices_page_is_on_the_website() {
        assert_eq!(
            links(Some("https://example.github.io/jellyfesh")),
            Some(Links {
                website: "https://example.github.io/jellyfesh/".into(),
                licences: "https://example.github.io/jellyfesh/THIRD_PARTY_NOTICES.html".into(),
                licence_agreement: "https://example.github.io/jellyfesh/legal/eula.html".into(),
                privacy_policy: "https://example.github.io/jellyfesh/legal/privacy.html".into(),
                terms_of_sale: "https://example.github.io/jellyfesh/legal/terms-of-sale.html".into(),
            })
        );
        assert_eq!(
            links(Some("https://jellyfesh.example/")).map(|l| l.licences),
            Some("https://jellyfesh.example/THIRD_PARTY_NOTICES.html".into())
        );
        // Not an https address: no links.
        assert_eq!(links(Some("jellyfesh.example")), None);
        assert_eq!(links(Some("http://jellyfesh.example/")), None);
        assert_eq!(links(Some("https://")), None);
        assert_eq!(links(Some("  ")), None);
        assert_eq!(links(None), None);
    }

    #[test]
    fn the_notices_file_sits_beside_the_app() {
        assert_eq!(
            notices_file(Path::new("/app/bin/jellyfesh")),
            Some(PathBuf::from("/app/share/doc/jellyfesh/THIRD_PARTY_NOTICES.md"))
        );
        assert_eq!(notices_file(Path::new("jellyfesh")), None);
    }
}
