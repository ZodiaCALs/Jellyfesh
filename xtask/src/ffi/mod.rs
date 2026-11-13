//! FFI tasks (`docs/ffi.md`):
//!
//! * `cargo xtask bindings [--lang swift|kotlin|csharp|all] [--release]` builds `jf-ffi` and writes
//!   each platform's bindings into its git-ignored generated folder.
//! * `cargo xtask tokens --target rust|swift|kotlin|csharp|css|all` writes the design tokens for one
//!   platform (`rust` is the committed module that `cargo xtask tokens` writes; `css` is Jellyfesh
//!   Web's stylesheet of custom properties).

mod bindings;
mod native_tokens;

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

pub const USAGE: &str = "usage: cargo xtask bindings [--lang swift|kotlin|csharp|all] [--release]
       cargo xtask tokens --target rust|swift|kotlin|csharp|css|all";

/// A language the shells use: the FFI shells' languages, and CSS for Jellyfesh Web (tokens only;
/// the web shell reaches the core through wasm-bindgen, not these bindings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Swift,
    Kotlin,
    CSharp,
    Css,
}

impl Lang {
    /// The languages with FFI bindings.
    const BINDINGS: [Lang; 3] = [Lang::Swift, Lang::Kotlin, Lang::CSharp];
    /// The languages with token files.
    const TOKENS: [Lang; 4] = [Lang::Swift, Lang::Kotlin, Lang::CSharp, Lang::Css];

    /// The languages a `--lang`/`--target` value names among `all` (`all` names every one).
    fn parse(name: &str, all: &[Lang]) -> Result<Vec<Lang>> {
        let langs = match name {
            "swift" => vec![Lang::Swift],
            "kotlin" => vec![Lang::Kotlin],
            "csharp" | "c#" | "cs" => vec![Lang::CSharp],
            "css" => vec![Lang::Css],
            "all" => all.to_vec(),
            other => bail!("unknown language {other:?}\n{USAGE}"),
        };
        if let Some(lang) = langs.iter().find(|l| !all.contains(l)) {
            bail!("{lang:?} has no files of this kind\n{USAGE}");
        }
        Ok(langs)
    }

    /// Where this language's generated files go, relative to the workspace root.
    pub fn generated_dir(self) -> &'static str {
        match self {
            Lang::Swift => "platforms/macos/Generated",
            Lang::Kotlin => "platforms/android/core-bindings/src/main/kotlin",
            Lang::CSharp => "platforms/windows/Generated",
            Lang::Css => "platforms/web/generated",
        }
    }
}

/// Runs `bindings …` or `tokens --target …`.
pub fn run(args: &[&str]) -> Result<()> {
    match args {
        ["bindings", rest @ ..] => {
            let mut langs = Lang::BINDINGS.to_vec();
            let mut release = false;
            let mut rest = rest.iter();
            while let Some(arg) = rest.next() {
                match *arg {
                    "--lang" => match rest.next() {
                        Some(name) => langs = Lang::parse(name, &Lang::BINDINGS)?,
                        None => bail!("--lang needs a value\n{USAGE}"),
                    },
                    "--release" => release = true,
                    "-h" | "--help" => {
                        println!("{USAGE}");
                        return Ok(());
                    }
                    other => bail!("unknown argument {other:?}\n{USAGE}"),
                }
            }
            bindings::run(&workspace_root(), &langs, release)
        }
        ["tokens", "--target", "rust"] => crate::tokens::run(false),
        ["tokens", "--target", target] => {
            if *target == "all" {
                crate::tokens::run(false)?;
            }
            let root = workspace_root();
            for lang in Lang::parse(target, &Lang::TOKENS)? {
                native_tokens::run(&root, lang)?;
            }
            Ok(())
        }
        _ => bail!("{USAGE}"),
    }
}

fn workspace_root() -> PathBuf {
    let xtask = Path::new(env!("CARGO_MANIFEST_DIR"));
    xtask.parent().unwrap_or(xtask).to_path_buf()
}
