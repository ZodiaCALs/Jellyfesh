//! Translations through gettext (`po/*.po`, compiled by build.rs).
//!
//! The catalogue folder is, in order: `$JELLYFESH_LOCALEDIR`, the installed
//! `share/locale` next to the binary (`/app/share/locale` in the Flatpak,
//! `<prefix>/share/locale` for the tarball), or the build folder for
//! development runs.

use std::path::PathBuf;

use gettextrs::{bind_textdomain_codeset, bindtextdomain, textdomain};

pub const DOMAIN: &str = "jellyfesh";

/// Binds the catalogue. GTK sets the locale from the environment when it
/// starts (before any message is translated).
pub fn init() {
    if let Some(dir) = locale_dir() {
        let _ = bindtextdomain(DOMAIN, dir);
    }
    let _ = bind_textdomain_codeset(DOMAIN, "UTF-8");
    let _ = textdomain(DOMAIN);
}

fn locale_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("JELLYFESH_LOCALEDIR").filter(|d| !d.is_empty()) {
        return Some(dir.into());
    }
    let installed = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent()?.parent().map(|p| p.join("share/locale")))
        .filter(|dir| dir.join("ja/LC_MESSAGES").join(format!("{DOMAIN}.mo")).is_file());
    installed.or_else(|| {
        let dev = PathBuf::from(concat!(env!("OUT_DIR"), "/locale"));
        dev.is_dir().then_some(dev)
    })
}

/// A translated message.
pub fn tr(msgid: &str) -> String {
    gettextrs::gettext(msgid)
}

/// A translated message with a context, for short words that translate
/// differently by use.
pub fn trc(context: &str, msgid: &str) -> String {
    gettextrs::pgettext(context, msgid)
}

/// A translated message for a count; `{n}` in it becomes the count, with
/// its thousands grouped ("1,204 songs").
pub fn trn(singular: &str, plural: &str, n: u64) -> String {
    let n32 = u32::try_from(n).unwrap_or(u32::MAX);
    gettextrs::ngettext(singular, plural, n32).replace("{n}", &crate::logic::format::count(n))
}

/// `tr` with `{name}` placeholders filled in.
pub fn trf(msgid: &str, args: &[(&str, &str)]) -> String {
    let mut s = tr(msgid);
    for (key, value) in args {
        s = s.replace(&format!("{{{key}}}"), value);
    }
    s
}
