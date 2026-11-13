//! `cargo xtask plugin build <package> [--release] [--out <file>]`: build a
//! plugin crate into a WebAssembly component with no WASI imports
//! (`target/plugins/<package>.wasm`). The steps live in `jf-plugin-build`,
//! shared with `jf-plugin build` and the plugin tests.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use jf_plugin_build::Build;

const USAGE: &str = "usage: cargo xtask plugin build <package> [--release] [--out <file>]";

/// Runs `cargo xtask plugin …`.
pub fn run(args: &[&str]) -> Result<()> {
    match args {
        ["build", rest @ ..] => {
            let mut package = None;
            let mut release = false;
            let mut out = None;
            let mut rest = rest.iter();
            while let Some(arg) = rest.next() {
                match *arg {
                    "--release" => release = true,
                    "--out" => out = Some(PathBuf::from(rest.next().context("--out needs a file")?)),
                    name if !name.starts_with('-') && package.is_none() => package = Some(name.to_string()),
                    other => bail!("unexpected argument {other:?}\n{USAGE}"),
                }
            }
            let package = package.context(USAGE)?;
            let built = jf_plugin_build::build(&Build {
                release,
                out,
                ..Build::new(workspace_root(), package)
            })?;
            println!(
                "wrote {} ({} KiB); imports: {}",
                built.path.display(),
                built.component.len() / 1024,
                built.imports.join(", ")
            );
            Ok(())
        }
        _ => bail!("{USAGE}"),
    }
}

fn workspace_root() -> PathBuf {
    let xtask = Path::new(env!("CARGO_MANIFEST_DIR"));
    xtask.parent().unwrap_or(xtask).to_path_buf()
}
