//! `jf-relay`: lets Jellyfesh Web reach services that refuse browsers. See `docs/web-relay.md`.

use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use jf_relay::{Allow, Config, MIN_TOKEN, Relay, new_token, normalize_origin};
use zeroize::Zeroizing;

#[derive(Debug, Parser)]
#[command(
    name = "jf-relay",
    version,
    about = "A relay you run yourself, so Jellyfesh Web can reach services that refuse browsers",
    after_help = "The token comes from the JF_RELAY_TOKEN environment variable or --token-file; \
                  enter the same token in Jellyfesh Web, Settings › Sources › Relay. \
                  `jf-relay token` makes a new one. Details: docs/web-relay.md."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Where to listen. Keep it on 127.0.0.1 unless a reverse proxy with https sits in front.
    #[arg(long, default_value = "127.0.0.1:8790")]
    listen: SocketAddr,
    /// The address Jellyfesh Web is opened at, as the browser shows it
    /// (https://you.github.io, http://localhost:4173); may be repeated.
    #[arg(long = "origin", value_name = "ORIGIN")]
    origins: Vec<String>,
    /// Also forward to this host (music.example.com, *.example.com, host:port; https only) or
    /// to your own server's origin (http://192.168.1.20:4533); may be repeated.
    #[arg(long = "allow", value_name = "HOST_OR_ORIGIN")]
    allow: Vec<String>,
    /// Read the token from this file instead of JF_RELAY_TOKEN.
    #[arg(long, value_name = "FILE")]
    token_file: Option<PathBuf>,
    /// Log each request's method, host and status (never its path, headers or body).
    #[arg(long)]
    verbose: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print a new random token.
    Token,
    /// List the hosts the relay forwards to.
    Hosts {
        /// Also these, as with --allow.
        #[arg(long = "allow", value_name = "HOST_OR_ORIGIN")]
        allow: Vec<String>,
    },
}

fn allowlist(extra: &[String]) -> Result<Allow, String> {
    let mut allow = Allow::bundled()?;
    for entry in extra {
        allow.add(entry)?;
    }
    Ok(allow)
}

fn token(file: Option<&PathBuf>) -> Result<Zeroizing<String>, String> {
    let token = match file {
        Some(path) => Zeroizing::new(
            std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read the token file {}: {e}", path.display()))?
                .trim()
                .to_owned(),
        ),
        None => Zeroizing::new(
            std::env::var("JF_RELAY_TOKEN")
                .map_err(|_| {
                    "set a token: JF_RELAY_TOKEN=… or --token-file FILE (`jf-relay token` makes one)"
                        .to_owned()
                })?
                .trim()
                .to_owned(),
        ),
    };
    if token.chars().count() < MIN_TOKEN || !token.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(format!(
            "the token must be at least {MIN_TOKEN} characters without spaces (`jf-relay token` makes one)"
        ));
    }
    Ok(token)
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Some(Command::Token) => {
            println!("{}", new_token()?);
            return Ok(());
        }
        Some(Command::Hosts { allow }) => {
            for entry in allowlist(&allow)?.entries() {
                println!("{entry}");
            }
            return Ok(());
        }
        None => {}
    }
    let mut origins = Vec::new();
    for origin in &cli.origins {
        origins.push(normalize_origin(origin).ok_or_else(|| {
            format!("--origin {origin}: give the address Jellyfesh Web is opened at, such as https://you.github.io")
        })?);
    }
    if origins.is_empty() {
        return Err("give the address you open Jellyfesh Web at: --origin https://… (the scheme, host and port, no path)".into());
    }
    let config = Config {
        token: token(cli.token_file.as_ref())?,
        origins,
        allow: allowlist(&cli.allow)?,
        verbose: cli.verbose,
    };
    let listener =
        TcpListener::bind(cli.listen).map_err(|e| format!("cannot listen on {}: {e}", cli.listen))?;
    let local = listener.local_addr().map_err(|e| e.to_string())?;
    eprintln!(
        "jf-relay {} listening on http://{local} for {}; {} hosts and origins allowed (`jf-relay hosts` lists them)",
        env!("CARGO_PKG_VERSION"),
        config.origins.join(", "),
        config.allow.entries().len()
    );
    if !local.ip().is_loopback() {
        eprintln!(
            "jf-relay: listening beyond this computer; put https in front (a reverse proxy) and keep the token secret"
        );
    }
    let relay = Relay::new(config, Arc::new(|line: &str| eprintln!("jf-relay: {line}")));
    relay.serve(listener).map_err(|e| format!("stopped: {e}"))
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("jf-relay: {error}");
            ExitCode::FAILURE
        }
    }
}
