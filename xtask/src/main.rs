//! Repository tasks.
//!
//! * `cargo xtask tokens` regenerates `core/jf-tokens/src/generated.rs` from `design/tokens.json`.
//! * `cargo xtask tokens --check` fails when that file is stale (CI runs it).
//! * `cargo xtask tokens --target rust|swift|kotlin|csharp|css|all` writes one platform's tokens.
//! * `cargo xtask bindings [--lang swift|kotlin|csharp|all] [--release]` builds `jf-ffi` and
//!   writes the language bindings (see `docs/ffi.md`).
//! * `cargo xtask plugin build <package> [--release] [--out <file>]` builds a plugin crate into a
//!   WebAssembly component with no WASI imports (`target/plugins/<package>.wasm`).

mod ffi;
mod plugin;
mod tokens;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask tokens [--check]\n       \
                     cargo xtask tokens --target rust|swift|kotlin|csharp|css|all\n       \
                     cargo xtask bindings [--lang swift|kotlin|csharp|all] [--release]\n       \
                     cargo xtask plugin build <package> [--release] [--out <file>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["tokens"] => tokens::run(false),
        ["tokens", "--check"] => tokens::run(true),
        ["plugin", rest @ ..] => plugin::run(rest),
        ["bindings" | "tokens", ..] => ffi::run(&args), // see ffi::USAGE
        ["help" | "-h" | "--help"] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
