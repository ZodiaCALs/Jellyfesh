//! `jf-plugin`: scaffold, build, pack, sign, verify and inspect Jellyfesh
//! plugins. See the crate docs (`src/lib.rs`) and `docs/plugins/authoring.md`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use jf_plugin::{PluginFiles, Scaffold, SigningKey, describe_signature};
use jf_plugin_build::Build;

#[derive(Debug, Parser)]
#[command(
    name = "jf-plugin",
    version,
    about = "Scaffold, build, pack, sign, verify and inspect Jellyfesh plugins",
    after_help = "Signing keys come from --key <file> or the JF_PLUGIN_SIGNING_KEY environment variable \
                  (64 hex characters: the ed25519 seed)."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create a plugin crate from the template.
    New {
        /// Cargo package name (lowercase letters, digits, '-', '_').
        name: String,
        /// Folder to create [default: ./<name>].
        #[arg(long)]
        path: Option<PathBuf>,
        /// Plugin id, reverse DNS [default: org.example.<name>].
        #[arg(long)]
        id: Option<String>,
        /// Author for the manifest.
        #[arg(long)]
        author: Option<String>,
        /// The plugins/sdk folder of a Jellyfesh checkout [default: found from the new folder, or the checkout this tool was built from].
        #[arg(long)]
        sdk: Option<PathBuf>,
    },
    /// Build the plugin crate into a WebAssembly component.
    Build {
        /// Optimised build (smaller, faster component).
        #[arg(long)]
        release: bool,
        /// The plugin crate's folder [default: the current folder].
        #[arg(long)]
        path: Option<PathBuf>,
        /// Where to write the component [default: <target>/plugins/<package>.wasm].
        #[arg(long)]
        out: Option<PathBuf>,
        /// Cargo's target folder.
        #[arg(long)]
        target_dir: Option<PathBuf>,
    },
    /// Pack manifest.toml, the component, the settings schema and icon.png into a .jfpkg.
    Pack {
        /// Build the component optimised (ignored with --wasm).
        #[arg(long)]
        release: bool,
        /// The plugin crate's folder [default: the current folder].
        #[arg(long)]
        path: Option<PathBuf>,
        /// Use this component instead of building the crate.
        #[arg(long)]
        wasm: Option<PathBuf>,
        /// Where to write the package [default: next to the component, <id>-<version>.jfpkg].
        #[arg(long)]
        out: Option<PathBuf>,
        /// Cargo's target folder.
        #[arg(long)]
        target_dir: Option<PathBuf>,
        /// Also sign the package with the key in this file.
        #[arg(long)]
        key: Option<PathBuf>,
    },
    /// Write a new ed25519 signing key to FILE (mode 0600). The key is never printed.
    Keygen {
        /// The key file to create.
        file: PathBuf,
        /// Replace FILE if it exists.
        #[arg(long)]
        force: bool,
    },
    /// Sign a package (replacing any signature) with --key FILE or JF_PLUGIN_SIGNING_KEY.
    Sign {
        /// The .jfpkg to sign.
        package: PathBuf,
        /// Key file [default: the JF_PLUGIN_SIGNING_KEY environment variable].
        #[arg(long)]
        key: Option<PathBuf>,
        /// Where to write the signed package [default: replace PACKAGE].
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Check a package as the app does on install, and the component's imports.
    Verify {
        /// The .jfpkg to check.
        package: PathBuf,
        /// Fail unless the package is signed.
        #[arg(long)]
        require_signature: bool,
        /// Fail unless a trusted key signed it.
        #[arg(long)]
        require_trusted: bool,
        #[command(flatten)]
        trust: Trust,
    },
    /// Show a package's manifest, capabilities, signature, key id, entries and imports.
    Inspect {
        /// The .jfpkg to show.
        package: PathBuf,
        /// Print JSON.
        #[arg(long)]
        json: bool,
        #[command(flatten)]
        trust: Trust,
    },
}

/// Which keys count as trusted, beyond the app's own list.
#[derive(Debug, clap::Args)]
struct Trust {
    /// Also trust the published test key (for tests only).
    #[arg(long)]
    trust_test_key: bool,
    /// Also trust this public key (64 hex characters); may be repeated.
    #[arg(long = "trust-key", value_name = "PUBLIC_KEY")]
    trust_keys: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::New {
            name,
            path,
            id,
            author,
            sdk,
        } => {
            let dir = path.unwrap_or_else(|| PathBuf::from(&name));
            let sdk = match sdk {
                Some(sdk) => sdk,
                None => {
                    let near = dir
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or(Path::new("."));
                    jf_plugin::find_sdk(near).context(
                        "cannot find the plugin SDK; pass --sdk <path to plugins/sdk in a Jellyfesh checkout>",
                    )?
                }
            };
            let written = jf_plugin::scaffold(&Scaffold {
                name,
                dir: dir.clone(),
                id,
                author,
                sdk,
            })?;
            println!("created {} ({} files)", dir.display(), written.len());
            println!(
                "next: edit manifest.toml and src/lib.rs, then `jf-plugin build --path {}`",
                dir.display()
            );
            Ok(())
        }
        Command::Build {
            release,
            path,
            out,
            target_dir,
        } => {
            let built = build(path, release, out, target_dir)?;
            println!(
                "wrote {} ({} KiB); imports: {}",
                built.path.display(),
                built.component.len().div_ceil(1024),
                built.imports.join(", ")
            );
            Ok(())
        }
        Command::Pack {
            release,
            path,
            wasm,
            out,
            target_dir,
            key,
        } => {
            let dir = crate_dir(path.clone())?;
            let files = PluginFiles::read(&dir)?;
            let manifest = files.manifest()?;
            let key = key.as_deref().map(SigningKey::from_file).transpose()?;
            let (component, folder) = match wasm {
                Some(file) => {
                    let bytes =
                        std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
                    (bytes, file.parent().map(Path::to_path_buf).unwrap_or_default())
                }
                None => {
                    let built = build(path, release, None, target_dir)?;
                    let folder = built.path.parent().map(Path::to_path_buf).unwrap_or_default();
                    (built.component, folder)
                }
            };
            let bytes = jf_plugin::pack(&files, component, key.as_ref())?;
            let out =
                out.unwrap_or_else(|| folder.join(format!("{}-{}.jfpkg", manifest.id, manifest.version)));
            write(&out, &bytes)?;
            println!(
                "wrote {} ({} KiB): {} {}, {}",
                out.display(),
                bytes.len().div_ceil(1024),
                manifest.id,
                manifest.version,
                match &key {
                    Some(key) => format!("signed with key id {}", key.key_id()),
                    None => "unsigned".to_string(),
                }
            );
            Ok(())
        }
        Command::Keygen { file, force } => {
            let key = jf_plugin::keygen(&file, force)?;
            println!(
                "wrote a new signing key to {} (only you can read it; keep it secret and out of version control)",
                file.display()
            );
            println!("public key: {}", key.public_hex());
            println!("key id:     {}", key.key_id());
            Ok(())
        }
        Command::Sign { package, key, out } => {
            let key = SigningKey::from_file_or_env(key.as_deref())?;
            let bytes = std::fs::read(&package).with_context(|| format!("reading {}", package.display()))?;
            let signed = jf_plugin::sign(&bytes, &key)?;
            let out = out.unwrap_or(package);
            write(&out, &signed)?;
            println!("signed {} with key id {}", out.display(), key.key_id());
            Ok(())
        }
        Command::Verify {
            package,
            require_signature,
            require_trusted,
            trust,
        } => {
            let trust = jf_plugin::trust_list(trust.trust_test_key, &trust.trust_keys)?;
            let bytes = std::fs::read(&package).with_context(|| format!("reading {}", package.display()))?;
            let verified = jf_plugin::verify(&bytes, &trust)
                .with_context(|| format!("{} is not a valid plugin package", package.display()))?;
            let manifest = &verified.package.manifest;
            println!(
                "{}: valid package {} {}",
                package.display(),
                manifest.id,
                manifest.version
            );
            println!("signature: {}", describe_signature(&verified.package.signature));
            jf_plugin::require(&verified.package.signature, require_signature, require_trusted)
        }
        Command::Inspect { package, json, trust } => {
            let trust = jf_plugin::trust_list(trust.trust_test_key, &trust.trust_keys)?;
            let bytes = std::fs::read(&package).with_context(|| format!("reading {}", package.display()))?;
            let report = jf_plugin::inspect(&bytes, &trust)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report.json())?);
            } else {
                print!("{report}");
            }
            Ok(())
        }
    }
}

fn crate_dir(path: Option<PathBuf>) -> Result<PathBuf> {
    match path {
        Some(path) => Ok(path),
        None => std::env::current_dir().context("the current folder"),
    }
}

fn build(
    path: Option<PathBuf>,
    release: bool,
    out: Option<PathBuf>,
    target_dir: Option<PathBuf>,
) -> Result<jf_plugin_build::Built> {
    let dir = crate_dir(path)?;
    let package = jf_plugin_build::package_name(&dir.join("Cargo.toml"))?;
    jf_plugin_build::build(&Build {
        release,
        out,
        target_dir,
        ..Build::new(dir, package)
    })
}

/// Writes `bytes` to `path` through a temporary file, so a failed write never
/// leaves half a package.
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    std::fs::write(&temporary, bytes).with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, path).with_context(|| format!("writing {}", path.display()))
}
