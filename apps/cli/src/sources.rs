//! Online sources: `jf plugin …`, `jf search` (the library and every source)
//! and `jf play --source`.
//!
//! Like the rest of `jf`, this only sends commands and prints what the core
//! reports: the core installs and checks plugins, keeps their sign-ins in the
//! system keystore, merges search results and resolves streams. Commands that
//! change something are sent, then the core is shut down, which lets an
//! install, uninstall or logout finish and still reports its result.

use std::collections::BTreeMap;
use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use clap::{Subcommand, ValueEnum};
use crossbeam_channel::{Receiver, RecvTimeoutError};
use jf_core::{
    AuthField, AuthMethod, AuthStep, Collection, Command, CommandId, Core, Event, ItemKind, PluginBadge,
    PluginSummary, SearchScope, SearchUpdate, SecretStorage, SourceItem, SourcesSnapshot, TrackRef,
};
use serde_json::{Value, json};

use crate::util::{bold, dim, fmt_ms};

/// The `source` of library results.
const LIBRARY: &str = "library";
/// How long a search may take in all (each source has its own, shorter
/// deadline in the core).
const SEARCH_WAIT: Duration = Duration::from_secs(60);
/// How long a sign-in may take in all (the core ends a flow after 15 minutes).
const LOGIN_WAIT: Duration = Duration::from_secs(16 * 60);

#[derive(Subcommand, Debug)]
pub enum PluginCmd {
    /// List the installed plugins (the default)
    List {
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Install or update a plugin from a .jfpkg file
    Install {
        file: PathBuf,
        /// Allow community (unofficial) plugins first; one still starts off
        /// until you accept its disclosure with `jf plugin enable`
        #[arg(long)]
        allow_community: bool,
    },
    /// Uninstall a plugin with its settings, data and sign-in
    Remove {
        /// Plugin id or name
        plugin: String,
    },
    /// Turn a plugin on; a community plugin shows its disclosure first
    Enable {
        /// Plugin id or name
        plugin: String,
        /// Accept a community plugin's disclosure without asking
        #[arg(long)]
        yes: bool,
    },
    /// Turn a plugin off
    Disable {
        /// Plugin id or name
        plugin: String,
    },
    /// Show a plugin's settings, or change them
    Settings {
        /// Plugin id or name
        plugin: String,
        /// Values to set as name=value (name= goes back to the default)
        values: Vec<String>,
        /// Replace all settings with this JSON object
        #[arg(long, conflicts_with = "values")]
        json: Option<String>,
    },
    /// Sign in to a source
    Login {
        /// Plugin id or name
        plugin: String,
        /// How to sign in [default: the plugin's first way]
        #[arg(long, value_enum)]
        method: Option<Method>,
        /// Answer a sign-in field as name=value instead of being asked
        /// (better typed at the prompt for passwords)
        #[arg(long = "field", value_name = "NAME=VALUE")]
        fields: Vec<String>,
    },
    /// Sign out of a source: its tokens and cookies are deleted
    Logout {
        /// Plugin id or name
        plugin: String,
    },
}

/// A way to sign in.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Qr,
    Oauth,
    CookieImport,
    PasswordExchange,
    ApiKey,
}

impl From<Method> for AuthMethod {
    fn from(method: Method) -> Self {
        match method {
            Method::Qr => AuthMethod::Qr,
            Method::Oauth => AuthMethod::Oauth,
            Method::CookieImport => AuthMethod::CookieImport,
            Method::PasswordExchange => AuthMethod::PasswordExchange,
            Method::ApiKey => AuthMethod::ApiKey,
        }
    }
}

fn method_name(method: AuthMethod) -> &'static str {
    match method {
        AuthMethod::Qr => "qr",
        AuthMethod::Oauth => "oauth",
        AuthMethod::CookieImport => "cookie-import",
        AuthMethod::PasswordExchange => "password-exchange",
        AuthMethod::ApiKey => "api-key",
    }
}

pub fn plugin(core: Core, cmd: PluginCmd) -> Result<()> {
    match cmd {
        PluginCmd::List { json } => list(&core.snapshot().sources, json),
        PluginCmd::Install {
            file,
            allow_community,
        } => install(core, file, allow_community),
        PluginCmd::Remove { plugin } => {
            let p = find(&core, &plugin)?;
            let sources = finish(core, Command::UninstallPlugin { id: p.id.clone() })?;
            if sources.plugins.iter().any(|q| q.id == p.id) {
                bail!("{} is still installed", p.name);
            }
            outln!("removed {}", bold(&p.name));
            Ok(())
        }
        PluginCmd::Enable { plugin, yes } => enable(core, &plugin, yes),
        PluginCmd::Disable { plugin } => {
            let p = find(&core, &plugin)?;
            finish(
                core,
                Command::SetPluginEnabled {
                    id: p.id.clone(),
                    enabled: false,
                },
            )?;
            outln!("{} is off", bold(&p.name));
            Ok(())
        }
        PluginCmd::Settings { plugin, values, json } => settings(core, &plugin, &values, json),
        PluginCmd::Login {
            plugin,
            method,
            fields,
        } => login(core, &plugin, method, &fields),
        PluginCmd::Logout { plugin } => {
            let p = find(&core, &plugin)?;
            finish(core, Command::Logout { plugin: p.id.clone() })?;
            outln!("signed out of {}", bold(&p.name));
            Ok(())
        }
    }
}

/// A plugin by id, by the last part of its id (`subsonic` for
/// `org.jellyfesh.subsonic`) or by name, ignoring case.
fn find(core: &Core, query: &str) -> Result<PluginSummary> {
    let sources = core.snapshot().sources;
    if let Some(reason) = &sources.unavailable {
        bail!("{reason}");
    }
    let q = query.trim().to_lowercase();
    let matches = |p: &&PluginSummary| {
        p.id.to_lowercase() == q
            || p.id
                .rsplit('.')
                .next()
                .is_some_and(|last| last.to_lowercase() == q)
            || p.name.to_lowercase() == q
    };
    let found: Vec<&PluginSummary> = sources.plugins.iter().filter(matches).collect();
    match found.as_slice() {
        [one] => Ok((*one).clone()),
        [] => bail!("no plugin {query:?} is installed (see `jf plugin list`)"),
        _ => bail!(
            "{query:?} names several plugins: {}",
            found.iter().map(|p| p.id.as_str()).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// Sends `command`, shuts the core down (which finishes installs,
/// uninstalls and logouts) and returns the `Sources` slice it ended with;
/// fails with the command's error. Other problems the core reported are
/// printed.
fn finish(core: Core, command: Command) -> Result<Arc<SourcesSnapshot>> {
    let events = core.subscribe();
    let mut sources = core.snapshot().sources;
    let id = core.send(command);
    core.shutdown();
    let mut failed = None;
    for event in events.try_iter() {
        match event {
            Event::CommandFailed { id: failed_id, error } if failed_id == id => failed = Some(error),
            Event::Error { message } => eprintln!("! {message}"),
            Event::Sources(snapshot) => sources = snapshot,
            _ => {}
        }
    }
    match failed {
        Some(error) => bail!("{error}"),
        None => Ok(sources),
    }
}

fn badge(p: &PluginSummary) -> &'static str {
    match p.badge {
        PluginBadge::Official => "official",
        PluginBadge::Community => "community",
    }
}

fn list(sources: &SourcesSnapshot, json: bool) -> Result<()> {
    if json {
        let plugins: Vec<Value> = sources.plugins.iter().map(plugin_json).collect();
        let out = json!({
            "plugins": plugins,
            "allow_community": sources.allow_community,
            "secret_storage": match sources.secret_storage {
                SecretStorage::Keystore => "keystore",
                SecretStorage::SessionOnly => "session_only",
            },
            "unavailable": sources.unavailable,
        });
        outln!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }
    if let Some(reason) = &sources.unavailable {
        bail!("{reason}");
    }
    if sources.plugins.is_empty() {
        outln!(
            "{}",
            dim("no plugins installed: `jf plugin install <file.jfpkg>`")
        );
    }
    for p in &sources.plugins {
        let state = match (&p.disabled_reason, p.enabled) {
            (_, true) => "on".to_string(),
            (Some(reason), false) => format!("off: {reason}"),
            (None, false) if p.needs_disclosure => "off: its disclosure is not accepted yet".to_string(),
            (None, false) => "off".to_string(),
        };
        outln!(
            "{}  {}  {}",
            bold(&p.name),
            dim(&format!("{} {} · {}", p.id, p.version, badge(p))),
            state
        );
        if let Some(description) = &p.description {
            outln!("  {}", dim(description));
        }
        let mut facts = vec![format!("can: {}", p.capabilities.join(", "))];
        if !p.auth_methods.is_empty() {
            let ways: Vec<&str> = p.auth_methods.iter().map(|m| method_name(*m)).collect();
            facts.push(format!("sign in: {}", ways.join(", ")));
            facts.push(match &p.account {
                Some(account) => format!("signed in as {account}"),
                None => "not signed in".into(),
            });
        }
        outln!("  {}", dim(&facts.join(" · ")));
    }
    if sources.secret_storage == SecretStorage::SessionOnly {
        eprintln!("! no keystore can be reached: sign-ins last only while jf runs");
    }
    Ok(())
}

fn plugin_json(p: &PluginSummary) -> Value {
    json!({
        "id": p.id,
        "name": p.name,
        "version": p.version,
        "author": p.author,
        "description": p.description,
        "badge": badge(p),
        "signing_key": p.signing_key,
        "enabled": p.enabled,
        "disabled_reason": p.disabled_reason,
        "needs_disclosure": p.needs_disclosure,
        "capabilities": p.capabilities,
        "auth_methods": p.auth_methods.iter().map(|m| method_name(*m)).collect::<Vec<_>>(),
        "account": p.account,
        "attribution": p.attribution,
        "settings": serde_json::from_str::<Value>(&p.settings_json).unwrap_or(Value::Null),
        "icon": p.icon,
    })
}

fn install(core: Core, file: PathBuf, allow_community: bool) -> Result<()> {
    if !file.is_file() {
        bail!("{} is not a file", file.display());
    }
    let file = std::path::absolute(&file).unwrap_or(file);
    let before = core.snapshot();
    if allow_community && !before.settings.allow_community_plugins {
        let settings = jf_core::Settings {
            allow_community_plugins: true,
            ..(*before.settings).clone()
        };
        core.send(Command::SetSettings { settings });
    }
    let sources = match finish(core, Command::InstallPlugin { path: file }) {
        Err(e) if !allow_community && e.to_string().contains("community plugins are turned off") => {
            bail!("{e}\n  to install it anyway: jf plugin install --allow-community <file>")
        }
        result => result?,
    };
    // The plugin that is new, or whose version changed.
    let old: BTreeMap<&str, &str> = before
        .sources
        .plugins
        .iter()
        .map(|p| (p.id.as_str(), p.version.as_str()))
        .collect();
    let changed: Vec<&PluginSummary> = sources
        .plugins
        .iter()
        .filter(|p| old.get(p.id.as_str()) != Some(&p.version.as_str()))
        .collect();
    if changed.is_empty() {
        outln!("installed again; nothing changed");
    }
    for p in changed {
        let verb = if old.contains_key(p.id.as_str()) {
            "updated"
        } else {
            "installed"
        };
        outln!(
            "{verb} {} {}",
            bold(&p.name),
            dim(&format!("{} {} · {}", p.id, p.version, badge(p)))
        );
        if p.needs_disclosure {
            outln!(
                "{}",
                dim(&format!(
                    "  a community plugin stays off until you accept its disclosure: jf plugin enable {}",
                    p.id
                ))
            );
        }
        if p.capabilities.iter().any(|c| c == "auth") && p.account.is_none() {
            outln!("{}", dim(&format!("  sign in with: jf plugin login {}", p.id)));
        }
    }
    Ok(())
}

fn enable(core: Core, query: &str, yes: bool) -> Result<()> {
    let p = find(&core, query)?;
    let community = p.badge == PluginBadge::Community;
    let allowed = core.snapshot().settings.allow_community_plugins;
    if community && (p.needs_disclosure || !allowed) {
        // ADR-0012: the person accepts the disclosure for each community
        // plugin; accepting allows community plugins too.
        let signer = match &p.signing_key {
            Some(key) => format!("signed by the key {key}"),
            None => "not signed".to_string(),
        };
        outln!(
            "{} is a community plugin by {} ({signer}), not made or checked by Jellyfesh.",
            bold(&p.name),
            p.author
        );
        outln!(
            "It may violate the service's terms, it may break at any time, and it is for personal use only."
        );
        if !allowed {
            outln!("Turning it on also allows community plugins.");
        }
        if !yes && !confirm("Turn it on?")? {
            bail!("left off");
        }
        finish(core, Command::AcknowledgeUnofficial { id: p.id.clone() })?;
    } else {
        finish(
            core,
            Command::SetPluginEnabled {
                id: p.id.clone(),
                enabled: true,
            },
        )?;
    }
    outln!("{} is on", bold(&p.name));
    Ok(())
}

/// Asks a yes/no question on the terminal; no terminal means no.
fn confirm(question: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        bail!("not confirmed: run it in a terminal, or pass --yes");
    }
    print!("{question} [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn settings(core: Core, query: &str, values: &[String], replace: Option<String>) -> Result<()> {
    let p = find(&core, query)?;
    let schema: Value = serde_json::from_str(&p.settings_schema_json).unwrap_or(Value::Null);
    let properties = schema["properties"].as_object().cloned().unwrap_or_default();
    if values.is_empty() && replace.is_none() {
        let current: Value = serde_json::from_str(&p.settings_json).unwrap_or(Value::Null);
        if properties.is_empty() {
            outln!("{}", dim(&format!("{} has no settings", p.name)));
        }
        for (name, spec) in &properties {
            let value = match current.get(name) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => String::new(),
                Some(other) => other.to_string(),
            };
            let title = spec["title"].as_str().unwrap_or(name);
            let kind = spec["type"].as_str().unwrap_or("string");
            outln!("{name} = {value}  {}", dim(&format!("{title} ({kind})")));
            if let Some(description) = spec["description"].as_str() {
                outln!("  {}", dim(description));
            }
            if let Some(choices) = spec["enum"].as_array() {
                let choices: Vec<&str> = choices.iter().filter_map(Value::as_str).collect();
                outln!("  {}", dim(&format!("one of: {}", choices.join(", "))));
            }
        }
        return Ok(());
    }
    let settings_json = match replace {
        Some(json) => json,
        None => {
            let mut current: serde_json::Map<String, Value> =
                serde_json::from_str(&p.settings_json).unwrap_or_default();
            for pair in values {
                let (name, text) = pair
                    .split_once('=')
                    .ok_or_else(|| anyhow!("expected name=value, got {pair:?}"))?;
                let spec = properties
                    .get(name)
                    .ok_or_else(|| anyhow!("{} has no setting {name:?}", p.name))?;
                if text.is_empty() {
                    current.remove(name);
                } else {
                    current.insert(name.to_owned(), typed(name, text, spec["type"].as_str())?);
                }
            }
            Value::Object(current).to_string()
        }
    };
    let id = p.id.clone();
    finish(core, Command::SetPluginSettings { id, settings_json })?;
    outln!("saved the settings of {}", bold(&p.name));
    Ok(())
}

/// A setting's value from text, by its type in the schema.
fn typed(name: &str, text: &str, kind: Option<&str>) -> Result<Value> {
    let bad = || anyhow!("{name} takes a {}, not {text:?}", kind.unwrap_or("value"));
    Ok(match kind {
        Some("integer") => Value::from(text.parse::<i64>().map_err(|_| bad())?),
        Some("number") => serde_json::Number::from_f64(text.parse::<f64>().map_err(|_| bad())?)
            .map(Value::Number)
            .ok_or_else(bad)?,
        Some("boolean") => match text.to_lowercase().as_str() {
            "true" | "yes" | "on" | "1" => Value::Bool(true),
            "false" | "no" | "off" | "0" => Value::Bool(false),
            _ => return Err(bad()),
        },
        _ => Value::String(text.to_owned()),
    })
}

fn login(core: Core, query: &str, method: Option<Method>, given: &[String]) -> Result<()> {
    let p = find(&core, query)?;
    let method = match method {
        Some(m) => m.into(),
        None => *p
            .auth_methods
            .first()
            .ok_or_else(|| anyhow!("{} has no sign-in", p.name))?,
    };
    let mut answers: BTreeMap<String, String> = BTreeMap::new();
    for pair in given {
        let (name, value) = pair
            .split_once('=')
            .ok_or_else(|| anyhow!("expected --field name=value, got {pair:?}"))?;
        answers.insert(name.to_owned(), value.to_owned());
    }
    let events = core.subscribe();
    core.send(Command::BeginLogin {
        plugin: p.id.clone(),
        method,
    });
    let end = Instant::now() + LOGIN_WAIT;
    loop {
        let left = end
            .checked_duration_since(Instant::now())
            .ok_or_else(|| anyhow!("the sign-in took too long"))?;
        let event = match events.recv_timeout(left) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => bail!("the sign-in took too long"),
            Err(RecvTimeoutError::Disconnected) => bail!("the core stopped"),
        };
        let update = match event {
            // Starting the login or answering it failed: it is over.
            Event::CommandFailed { error, .. } => bail!("{error}"),
            Event::Error { message } => {
                eprintln!("! {message}");
                continue;
            }
            Event::Auth(update) if update.plugin == p.id => update,
            _ => continue,
        };
        match update.step {
            AuthStep::ShowQr { data, hint, .. } => {
                print_qr(&data);
                outln!(
                    "{}",
                    hint.unwrap_or_else(|| format!("Scan the code with the {} app.", p.name))
                );
                outln!("{}", dim(&data));
                outln!("{}", dim("waiting…"));
            }
            AuthStep::ShowCode { code, hint, .. } => {
                outln!("\n    {}\n", bold(&code));
                outln!("{hint}");
                outln!("{}", dim("waiting…"));
            }
            AuthStep::OpenUrl { url } => {
                outln!("Continue in your browser:\n  {url}");
                open_browser(&url);
                outln!("{}", dim("waiting for the browser…"));
            }
            AuthStep::AskFields { fields } => {
                let mut values = Vec::new();
                for field in &fields {
                    let value = match answers.remove(&field.name) {
                        Some(value) => value,
                        None => ask(field)?,
                    };
                    if field.required && value.is_empty() {
                        bail!("{} is required", field.label);
                    }
                    values.push((field.name.clone(), value));
                }
                core.send(Command::SubmitLogin {
                    plugin: p.id.clone(),
                    flow: update.flow,
                    fields: values,
                });
            }
            AuthStep::Waiting { message } => outln!("{}", dim(&message)),
            AuthStep::Done { account } => {
                outln!("signed in to {} as {}", bold(&p.name), bold(&account));
                let storage = core.snapshot().sources.secret_storage;
                core.shutdown();
                if storage == SecretStorage::SessionOnly {
                    eprintln!("! no keystore can be reached: this sign-in ended when jf exited");
                }
                return Ok(());
            }
            AuthStep::Failed { message } => bail!("{message}"),
        }
    }
}

/// Asks for one sign-in field on the terminal; secret ones are not echoed
/// where `stty` can turn echo off.
fn ask(field: &AuthField) -> Result<String> {
    if !std::io::stdin().is_terminal() {
        bail!("{} is needed: pass --field {}=…", field.label, field.name);
    }
    if let Some(help) = &field.help {
        outln!("{}", dim(help));
    }
    let hint = field
        .placeholder
        .as_deref()
        .map(|p| format!(" ({p})"))
        .unwrap_or_default();
    print!("{}{hint}: ", field.label);
    std::io::stdout().flush()?;
    let hidden = field.secret && echo(false);
    let mut value = String::new();
    let read = std::io::stdin().lock().read_line(&mut value);
    if hidden {
        echo(true);
        println!();
    }
    read.context("reading the answer")?;
    Ok(value.trim_end_matches(['\r', '\n']).to_owned())
}

/// Turns terminal echo on or off with `stty`; false when it could not.
fn echo(on: bool) -> bool {
    std::process::Command::new("stty")
        .arg(if on { "echo" } else { "-echo" })
        .stdin(Stdio::inherit())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Opens a URL in the default browser, if the platform has a way.
fn open_browser(url: &str) {
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/c", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    let opened = command
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !opened {
        outln!("{}", dim("(open the address yourself)"));
    }
}

/// Draws a QR code with half-block characters (two modules per row).
fn print_qr(data: &str) {
    match qrcode::QrCode::new(data.as_bytes()) {
        Ok(code) => {
            let art = code
                .render::<qrcode::render::unicode::Dense1x2>()
                .dark_color(qrcode::render::unicode::Dense1x2::Light)
                .light_color(qrcode::render::unicode::Dense1x2::Dark)
                .quiet_zone(true)
                .build();
            outln!("{art}");
        }
        Err(e) => outln!("{}", dim(&format!("(cannot draw the code: {e})"))),
    }
}

// ---- search ------------------------------------------------------------------

/// The latest section of each source for one search, library first, the
/// others in the order they first answered.
pub struct Sections {
    pub sections: Vec<Arc<SearchUpdate>>,
}

impl Sections {
    /// Takes in one update: it replaces the earlier one from its source.
    fn apply(&mut self, update: Arc<SearchUpdate>) {
        match self.sections.iter_mut().find(|s| s.source == update.source) {
            Some(old) => *old = update,
            None if update.source == LIBRARY => self.sections.insert(0, update),
            None => self.sections.push(update),
        }
    }
}

/// Sends a search and collects every source's section until the last one.
pub fn search_sources(core: &Core, query: &str, scope: SearchScope) -> Result<Sections> {
    let events = core.subscribe();
    let request = core.send(Command::Search {
        query: query.to_owned(),
        scope,
    });
    collect(&events, request)
}

fn collect(events: &Receiver<Event>, request: CommandId) -> Result<Sections> {
    let mut found = Sections { sections: Vec::new() };
    let end = Instant::now() + SEARCH_WAIT;
    loop {
        let left = end
            .checked_duration_since(Instant::now())
            .ok_or_else(|| anyhow!("the search took too long"))?;
        match events.recv_timeout(left) {
            Ok(Event::SearchResults(update)) if update.request == request => {
                let done = update.done;
                found.apply(update);
                if done {
                    return Ok(found);
                }
            }
            Ok(Event::CommandFailed { id, error }) if id == request => bail!("{error}"),
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => bail!("the search took too long"),
            Err(RecvTimeoutError::Disconnected) => bail!("the core stopped"),
        }
    }
}

/// `jf search`: the library and the sources (or one of them).
pub fn search(
    core: &Core,
    query: &str,
    source: Option<&str>,
    library: bool,
    limit: usize,
    json: bool,
) -> Result<()> {
    let scope = match (source, library) {
        (Some(plugin), _) => SearchScope::Plugin {
            id: find(core, plugin)?.id,
        },
        (None, true) => SearchScope::Library,
        (None, false) => SearchScope::All,
    };
    let found = search_sources(core, query, scope)?;
    let names: BTreeMap<String, String> = core
        .snapshot()
        .sources
        .plugins
        .iter()
        .map(|p| (p.id.clone(), p.name.clone()))
        .collect();
    let name = |source: &str| -> String {
        if source == LIBRARY {
            "Library".into()
        } else {
            names.get(source).cloned().unwrap_or_else(|| source.to_owned())
        }
    };
    if json {
        let sections: Vec<Value> = found
            .sections
            .iter()
            .map(|s| {
                json!({
                    "source": s.source,
                    "name": name(&s.source),
                    "error": s.error,
                    "items": s.items.iter().take(limit).map(item_json).collect::<Vec<_>>(),
                })
            })
            .collect();
        outln!("{}", serde_json::to_string_pretty(&sections)?);
        return Ok(());
    }
    let mut any = false;
    for section in &found.sections {
        if section.items.is_empty() && section.error.is_none() {
            continue;
        }
        outln!("{}", bold(&name(&section.source)));
        if let Some(error) = &section.error {
            outln!("  {}", dim(&format!("! {error}")));
        }
        for kind in [
            ItemKind::Artist,
            ItemKind::Album,
            ItemKind::Playlist,
            ItemKind::Folder,
            ItemKind::Track,
        ] {
            let items: Vec<&SourceItem> = section
                .items
                .iter()
                .filter(|i| i.kind == kind)
                .take(limit)
                .collect();
            if items.is_empty() {
                continue;
            }
            any = true;
            outln!("  {}", dim(kind_title(kind)));
            for item in items {
                outln!("    {}", item_line(item, &name));
            }
        }
    }
    if !any {
        outln!("{}", dim("no matches"));
    }
    Ok(())
}

fn kind_title(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Track => "Tracks",
        ItemKind::Album => "Albums",
        ItemKind::Artist => "Artists",
        ItemKind::Playlist => "Playlists",
        ItemKind::Folder => "Folders",
    }
}

fn item_line(item: &SourceItem, name: &dyn Fn(&str) -> String) -> String {
    let mut line = bold(&item.title);
    if let Some(subtitle) = &item.subtitle {
        line.push_str(&format!(" — {subtitle}"));
    }
    let mut extra = Vec::new();
    if let Some(album) = &item.album {
        extra.push(album.clone());
    }
    if let Some(ms) = item.duration_ms {
        extra.push(fmt_ms(ms));
    }
    if !item.also_in.is_empty() {
        let others: Vec<String> = item.also_in.iter().map(|s| name(s)).collect();
        extra.push(format!("also in {}", others.join(", ")));
    }
    if !item.playable {
        extra.push(
            item.unplayable_reason
                .clone()
                .unwrap_or_else(|| "not playable".into()),
        );
    }
    if !extra.is_empty() {
        line.push_str(&format!("  {}", dim(&extra.join(" · "))));
    }
    line
}

fn item_json(item: &SourceItem) -> Value {
    json!({
        "source": item.source,
        "kind": kind_title(item.kind).trim_end_matches('s').to_lowercase(),
        "id": item.id,
        "title": item.title,
        "subtitle": item.subtitle,
        "album": item.album,
        "duration_ms": item.duration_ms,
        "artwork": item.artwork.as_ref().map(|a| match a {
            jf_core::ArtworkRef::File(path) => path.display().to_string(),
            jf_core::ArtworkRef::Url(url) => url.clone(),
        }),
        "playable": item.playable,
        "unplayable_reason": item.unplayable_reason,
        "also_in": item.also_in,
    })
}

// ---- play ----------------------------------------------------------------------

/// What `jf play --source` plays: a whole album, playlist or artist, or tracks.
pub enum SourcePlay {
    Collection(Collection),
    Tracks(Vec<TrackRef>),
}

/// Searches one source for `query`. A result titled exactly like the
/// query plays whole (an album or playlist, then an artist); otherwise the
/// playable tracks play in result order. Items the service will not play
/// here are never played; when nothing else matched, their reason is the
/// error.
pub fn source_play(core: &Core, plugin: &str, query: &str) -> Result<SourcePlay> {
    let p = find(core, plugin)?;
    let found = search_sources(core, query, SearchScope::Plugin { id: p.id.clone() })?;
    let items: Vec<&SourceItem> = found.sections.iter().flat_map(|s| s.items.iter()).collect();
    let wanted = query.trim().to_lowercase();
    for kinds in [
        &[ItemKind::Album, ItemKind::Playlist][..],
        &[ItemKind::Artist][..],
    ] {
        if let Some(whole) = items
            .iter()
            .find(|i| kinds.contains(&i.kind) && i.title.to_lowercase() == wanted)
        {
            return Ok(SourcePlay::Collection(Collection::Source {
                plugin: p.id.clone(),
                id: whole.id.clone(),
                item_kind: whole.kind,
            }));
        }
    }
    let tracks: Vec<TrackRef> = items.iter().filter_map(|i| i.track.clone()).collect();
    if !tracks.is_empty() {
        return Ok(SourcePlay::Tracks(tracks));
    }
    let locked = items
        .iter()
        .find(|i| i.kind == ItemKind::Track && !i.playable)
        .and_then(|i| i.unplayable_reason.clone());
    match locked {
        Some(reason) => bail!("nothing in {} matching {query:?} can play here: {reason}", p.name),
        None => bail!("nothing in {} matches {query:?}", p.name),
    }
}
