//! `cargo xtask bindings`: build `jf-ffi`, then
//!
//! * Swift: UniFFI's `JellyfeshFFI.swift`, `JellyfeshFFIFFI.h` and `JellyfeshFFIFFI.modulemap`
//!   into `platforms/macos/Generated/`;
//! * Kotlin: UniFFI's `org/jellyfesh/ffi/jellyfesh.kt` plus the hand-written `AndroidContext.kt`
//!   into `platforms/android/core-bindings/src/main/kotlin/`;
//! * C#: the hand-written `Jellyfesh.Ffi` library (`core/jf-ffi/csharp/`) and the native library
//!   built here into `platforms/windows/Generated/`.
//!
//! UniFFI runs through `jf-ffi-bindgen`, the `uniffi-bindgen` built from the same crates.io
//! release as the scaffolding, in library mode (it reads `core/jf-ffi/uniffi.toml`).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

use super::Lang;

/// Hand-written C# sources, relative to the workspace root.
const CSHARP_SOURCE: &str = "core/jf-ffi/csharp/Jellyfesh.Ffi";
/// Hand-written Kotlin sources that ship with the generated bindings.
const KOTLIN_SOURCE: &str = "core/jf-ffi/kotlin";

pub fn run(root: &Path, langs: &[Lang], release: bool) -> Result<()> {
    let profile = if release { "release" } else { "debug" };
    let mut build = vec!["build", "--package", "jf-ffi"];
    if release {
        build.push("--release");
    }
    cargo(root, &build)?;
    let library = target_dir(root).join(profile).join(library_file_name());
    ensure!(library.is_file(), "{} was not built", library.display());

    for &lang in langs {
        let out = root.join(lang.generated_dir());
        std::fs::create_dir_all(&out).with_context(|| format!("creating {}", out.display()))?;
        match lang {
            Lang::Css => bail!("Jellyfesh Web has no FFI bindings (it uses core/jf-web)"),
            Lang::Swift => uniffi(root, "swift", &library, &out)?,
            Lang::Kotlin => {
                uniffi(root, "kotlin", &library, &out)?;
                copy_tree(&root.join(KOTLIN_SOURCE), &out)?;
            }
            Lang::CSharp => {
                let project = out.join("Jellyfesh.Ffi");
                if project.exists() {
                    std::fs::remove_dir_all(&project)
                        .with_context(|| format!("removing {}", project.display()))?;
                }
                copy_tree(&root.join(CSHARP_SOURCE), &project)?;
                let native = out.join("native");
                std::fs::create_dir_all(&native)?;
                std::fs::copy(&library, native.join(library_file_name()))
                    .with_context(|| format!("copying {}", library.display()))?;
            }
        }
        println!("wrote {} bindings to {}", name(lang), lang.generated_dir());
    }
    Ok(())
}

fn name(lang: Lang) -> &'static str {
    match lang {
        Lang::Css => "CSS",
        Lang::Swift => "Swift",
        Lang::Kotlin => "Kotlin",
        Lang::CSharp => "C#",
    }
}

/// The shared library `cargo build` makes for `jf-ffi` on this platform.
pub fn library_file_name() -> &'static str {
    if cfg!(windows) {
        "jf_ffi.dll"
    } else if cfg!(target_os = "macos") {
        "libjf_ffi.dylib"
    } else {
        "libjf_ffi.so"
    }
}

fn target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|dir| if dir.is_absolute() { dir } else { root.join(dir) })
        .unwrap_or_else(|| root.join("target"))
}

fn cargo(root: &Path, args: &[&str]) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(&cargo)
        .current_dir(root)
        .args(args)
        .status()
        .with_context(|| format!("running cargo {}", args.join(" ")))?;
    ensure!(status.success(), "cargo {} failed", args.join(" "));
    Ok(())
}

/// `uniffi-bindgen generate` in library mode. `--no-format` keeps the output the same whether or
/// not ktlint or swiftformat is installed.
fn uniffi(root: &Path, language: &str, library: &Path, out: &Path) -> Result<()> {
    let library = library.to_str().context("the library path is not UTF-8")?;
    let out = out.to_str().context("the output path is not UTF-8")?;
    cargo(
        root,
        &[
            "run",
            "--quiet",
            "--package",
            "jf-ffi-bindgen",
            "--bin",
            "uniffi-bindgen",
            "--",
            "generate",
            "--language",
            language,
            "--no-format",
            "--metadata-no-deps",
            "--out-dir",
            out,
            library,
        ],
    )
}

/// Copies a folder's files, skipping .NET build output (`bin`, `obj`).
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to).with_context(|| format!("creating {}", to.display()))?;
    for entry in std::fs::read_dir(from).with_context(|| format!("reading {}", from.display()))? {
        let entry = entry?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            if matches!(entry.file_name().to_str(), Some("bin" | "obj")) {
                continue;
            }
            copy_tree(&path, &target)?;
        } else {
            std::fs::copy(&path, &target).with_context(|| format!("copying {}", path.display()))?;
        }
    }
    Ok(())
}
