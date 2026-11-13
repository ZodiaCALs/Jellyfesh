//! Build-time generation for the Linux app:
//!
//! * the stylesheets: `data/css/*.css` are templates whose `{{group.token}}`
//!   placeholders are filled from `design/tokens.json` (Misty tokens plus the
//!   Jellyfesh groups), written to `$OUT_DIR/css/`;
//! * the GResource bundle (icons and the formal wordmark);
//! * the gettext catalogues (`po/*.po` → `$OUT_DIR/locale/<lang>/LC_MESSAGES/jellyfesh.mo`),
//!   when `msgfmt` is installed.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "src/logic/tokens_css.rs"]
mod tokens_css;

const STYLESHEETS: [&str; 3] = ["style.css", "controls-d.css", "glass-live.css"];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let repo = manifest.join("../..");

    stylesheets(&manifest, &repo, &out);

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        resources(&manifest, &repo);
        catalogues(&manifest, &out);
    }
}

fn stylesheets(manifest: &Path, repo: &Path, out: &Path) {
    let tokens_path = repo.join("design/tokens.json");
    println!("cargo:rerun-if-changed={}", tokens_path.display());
    let text = fs::read_to_string(&tokens_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", tokens_path.display()));
    let tokens: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not JSON: {e}", tokens_path.display()));
    let dir = out.join("css");
    fs::create_dir_all(&dir).expect("create $OUT_DIR/css");
    for name in STYLESHEETS {
        let src = manifest.join("data/css").join(name);
        println!("cargo:rerun-if-changed={}", src.display());
        let template =
            fs::read_to_string(&src).unwrap_or_else(|e| panic!("cannot read {}: {e}", src.display()));
        let css = tokens_css::expand(&template, &tokens).unwrap_or_else(|e| panic!("{name}: {e}"));
        let header = format!(
            "/* Generated from data/css/{name} and design/tokens.json by build.rs. Do not edit. */\n"
        );
        fs::write(dir.join(name), header + &css).expect("write generated CSS");
    }
}

fn resources(manifest: &Path, repo: &Path) {
    let data = manifest.join("data");
    glib_build_tools::compile_resources(
        &[data.as_path(), repo],
        data.join("resources.gresource.xml").to_str().expect("UTF-8 path"),
        "jellyfesh.gresource",
    );
}

/// Compiles every language in `po/LINGUAS` with `msgfmt`. Without `msgfmt`
/// the app still builds and runs in English.
fn catalogues(manifest: &Path, out: &Path) {
    let po = manifest.join("po");
    let linguas = po.join("LINGUAS");
    println!("cargo:rerun-if-changed={}", linguas.display());
    let Ok(list) = fs::read_to_string(&linguas) else {
        return;
    };
    let langs = list
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default())
        .flat_map(str::split_whitespace);
    for lang in langs {
        let src = po.join(format!("{lang}.po"));
        println!("cargo:rerun-if-changed={}", src.display());
        let dir = out.join("locale").join(lang).join("LC_MESSAGES");
        fs::create_dir_all(&dir).expect("create locale dir");
        let result = Command::new("msgfmt")
            .arg("--check")
            .arg("-o")
            .arg(dir.join("jellyfesh.mo"))
            .arg(&src)
            .status();
        match result {
            Ok(status) if status.success() => {}
            Ok(status) => panic!("msgfmt failed for {}: {status}", src.display()),
            Err(e) => {
                println!("cargo:warning=msgfmt not available ({e}); the app builds without translations");
                return;
            }
        }
    }
}
