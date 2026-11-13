//! Login flows: the host drives the state machine of
//! `jf_plugin_model::auth` over the plugin's `auth.begin`, `auth.poll` and
//! `auth.submit` (ADR-0004).
//!
//! [`PluginHost::begin_auth`] starts a flow on its own thread and returns an
//! [`AuthFlow`]; the UI renders every [`AuthStep`] that arrives on
//! [`AuthFlow::steps`] and answers forms with [`AuthFlow::submit`]. The
//! machine holds the rules (forms, required fields, poll intervals, code
//! expiry, which values to protect); this driver adds what needs threads and
//! sockets:
//!
//! * **qr** (also used for codes to type elsewhere): shows the code and polls
//!   at the plugin's interval (0.2–30 s) until done, failed or expired;
//! * **oauth**: before `begin` it binds a listener on `127.0.0.1` (random
//!   port), makes a PKCE verifier and S256 challenge and a `state` value, and
//!   passes them in `flow.oauth`; when the plugin returns `open-url` with
//!   loopback capture, the UI opens the URL in the system browser and the
//!   driver waits for the redirect, checks `state`, answers the browser and
//!   calls `submit` with `code`, `code-verifier` and `redirect-uri`;
//! * **cookie-import**: keeps only the cookies the manifest's
//!   `[cookie_import]` names, stores them in the plugin's jar for its URL and
//!   passes the plugin the imported names instead of the values;
//! * **password-exchange**: the password reaches the plugin in exactly one
//!   `submit` call; while it runs the host refuses to store it (kv or
//!   keystore) and masks it in logs; afterwards the field buffers are wiped
//!   and the instance is restarted so it does not linger in plugin memory;
//! * **api-key**: like a form, with the key masked in logs and kept out of
//!   plain kv (the plugin stores it with `kv.set-secret`).
//!
//! Every flow ends with [`AuthStep::Done`] (the account name is remembered
//! and shown in [`crate::PluginInfo::account`]) or [`AuthStep::Failed`],
//! after which the channel closes. A flow lives at most 15 minutes; dropping
//! the [`AuthFlow`] cancels it.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL;
use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, select};
use jf_plugin_model::auth::{AuthHost, FLOW_LIFETIME, INPUT_TIMEOUT, Machine, Next, Oauth, Request, failed};
use url::Url;
use zeroize::{Zeroize, Zeroizing};

use crate::bindings::exports::jellyfesh::plugin::auth as wit;
use crate::error::CallError;
use crate::host::PluginHost;
use crate::manifest::valid_cookie_name;
use crate::plugin::{Job, JobContext, Plugin};
use crate::pool::Scheduler;
use crate::runtime::{Budget, Protected};
use crate::sanitize::text;
use crate::types::{AuthKind, AuthStep, Capability, FieldValue, Flow, FlowStep};

/// A login flow in progress. Dropping it cancels the flow.
#[derive(Debug)]
pub struct AuthFlow {
    plugin: String,
    kind: AuthKind,
    steps: Receiver<AuthStep>,
    commands: Sender<Command>,
}

#[derive(Debug)]
enum Command {
    Submit(Vec<FieldValue>),
    Cancel,
}

impl AuthFlow {
    /// The plugin's id.
    pub fn plugin(&self) -> &str {
        &self.plugin
    }

    /// The kind of login.
    pub fn kind(&self) -> AuthKind {
        self.kind
    }

    /// Steps to render, in order. The last one is `Done` or `Failed`; then
    /// the channel closes.
    pub fn steps(&self) -> &Receiver<AuthStep> {
        &self.steps
    }

    /// Answers the current `AskFields` step. Values are moved into the flow
    /// and wiped after the plugin's `submit` returns.
    pub fn submit(&self, fields: Vec<FieldValue>) {
        let _ = self.commands.send(Command::Submit(fields));
    }

    /// Cancels the flow; a `Failed` step follows.
    pub fn cancel(&self) {
        let _ = self.commands.send(Command::Cancel);
    }
}

impl Drop for AuthFlow {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl PluginHost {
    /// Starts a login of `kind`, which the plugin's manifest must list.
    pub fn begin_auth(&self, id: &str, kind: AuthKind) -> Result<AuthFlow, CallError> {
        let plugin = self.callable(id, Some(Capability::Auth))?;
        if !plugin.manifest.auth.contains(&kind) {
            return Err(CallError::NotDeclared(format!("auth kind {kind}")));
        }
        let (steps_tx, steps_rx) = crossbeam_channel::unbounded();
        let (commands_tx, commands_rx) = crossbeam_channel::unbounded();
        let driver = Driver {
            scheduler: self.inner.scheduler.clone(),
            plugin,
            kind,
            steps: steps_tx,
            commands: commands_rx,
            started: Instant::now(),
        };
        std::thread::Builder::new()
            .name("jf-plugin-auth".into())
            .spawn(move || driver.run())
            .map_err(|e| CallError::Load(format!("cannot start the login: {e}")))?;
        Ok(AuthFlow {
            plugin: id.to_string(),
            kind,
            steps: steps_rx,
            commands: commands_tx,
        })
    }
}

/// The loopback listener of an oauth flow and the `state` it expects.
struct Loopback {
    listener: TcpListener,
    state: String,
}

struct Driver {
    scheduler: Scheduler,
    plugin: Arc<Plugin>,
    kind: AuthKind,
    steps: Sender<AuthStep>,
    commands: Receiver<Command>,
    started: Instant,
}

fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    // getrandom only fails on exotic platforms; a zero token still works
    // as an identifier, and PKCE/state then fail closed at the service.
    let _ = getrandom::fill(&mut buf);
    BASE64URL.encode(buf)
}

impl AuthHost for Driver {
    fn allows(&self, url: &Url) -> bool {
        self.plugin.services.allows(url)
    }

    /// Imports the declared cookies from a pasted `Cookie:` header into the
    /// plugin's jar; returns the names imported.
    fn import_cookies(&self, header: &str) -> Result<Vec<String>, String> {
        let services = &self.plugin.services;
        let rules = services
            .cookie_import
            .as_ref()
            .ok_or_else(|| "This plugin does not accept cookies.".to_string())?;
        let url = services
            .cookie_import_url()
            .filter(|u| services.allows(u))
            .ok_or_else(|| "Set the server address in the plugin's settings first.".to_string())?;
        let header = header
            .trim()
            .trim_start_matches("Cookie:")
            .trim_start_matches("cookie:");
        let mut pairs = Vec::new();
        for part in header.split(';') {
            let Some((name, value)) = part.trim().split_once('=') else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim());
            if valid_cookie_name(name)
                && rules.names.iter().any(|n| n == name)
                && !value.is_empty()
                && value
                    .bytes()
                    .all(|b| (0x21..0x7f).contains(&b) && !matches!(b, b'"' | b',' | b';' | b'\\'))
                && !pairs.iter().any(|(n, _): &(String, String)| n == name)
            {
                pairs.push((name.to_string(), value.to_string()));
            }
        }
        if pairs.is_empty() {
            return Err(format!(
                "None of the expected cookies ({}) were found.",
                rules.names.join(", ")
            ));
        }
        services.cookies.import(&url, &pairs);
        Ok(pairs
            .into_iter()
            .map(|(name, mut value)| {
                value.zeroize();
                name
            })
            .collect())
    }

    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64)
    }
}

impl Driver {
    fn run(self) {
        let id = self.plugin.manifest.id.clone();
        let last = self.drive();
        if let AuthStep::Done { account } = &last {
            let account = account.clone();
            self.plugin.services.update_state(|s| s.account = Some(account));
            tracing::info!(plugin = %id, "logged in");
        }
        let _ = self.steps.send(last);
    }

    fn emit(&self, step: AuthStep) -> Result<(), AuthStep> {
        self.steps.send(step).map_err(|_| failed("cancelled"))
    }

    fn remaining(&self) -> Result<Duration, AuthStep> {
        FLOW_LIFETIME
            .checked_sub(self.started.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| failed("The login took too long. Start again."))
    }

    /// Waits `duration`, or less if the flow is cancelled.
    fn pause(&self, duration: Duration) -> Result<(), AuthStep> {
        let wait = duration.min(self.remaining()?);
        match self.commands.recv_timeout(wait) {
            Ok(Command::Cancel) | Err(RecvTimeoutError::Disconnected) => Err(failed("cancelled")),
            Ok(Command::Submit(_)) | Err(RecvTimeoutError::Timeout) => Ok(()),
        }
    }

    /// Runs one plugin call on the plugin's worker and waits for it. `Ok`
    /// carries the plugin's answer or the reason there is none (for the
    /// machine); `Err` ends the flow at once (cancelled, out of time).
    fn call(&self, flow: &Flow, request: Request) -> Result<Result<FlowStep, String>, AuthStep> {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let flow = wit::Flow::from(flow.clone());
        let job: Job = Box::new(move |ctx: &mut JobContext<'_>| {
            let limits = &ctx.runtime.limits;
            let result = match request {
                Request::Begin => ctx.call(
                    Budget::auth_begin(limits),
                    Protected::default(),
                    |source, store| source.jellyfesh_plugin_auth().call_begin(store, &flow),
                ),
                Request::Poll => ctx.call(
                    Budget::auth_step(limits),
                    Protected::default(),
                    |source, store| source.jellyfesh_plugin_auth().call_poll(store, &flow),
                ),
                Request::Submit {
                    fields,
                    protect,
                    keystore,
                    restart,
                } => {
                    let mut fields: Vec<wit::FieldValue> = fields.into_iter().map(Into::into).collect();
                    let protected = Protected {
                        values: protect,
                        keystore,
                    };
                    let result = ctx.call(Budget::auth_step(limits), protected, |source, store| {
                        source.jellyfesh_plugin_auth().call_submit(store, &flow, &fields)
                    });
                    for field in &mut fields {
                        field.value.zeroize();
                    }
                    if restart {
                        ctx.restart();
                    }
                    result
                }
            };
            let _ = tx.send(result);
        });
        if !self.scheduler.submit(&self.plugin, job) {
            return Ok(Err("The plugin is not available.".into()));
        }
        let deadline = self.remaining()?;
        loop {
            select! {
                recv(rx) -> reply => {
                    return Ok(match reply {
                        Ok(Ok(Ok(step))) => Ok(step.into()),
                        Ok(Ok(Err(error))) => Err(crate::error::plugin_error_text(&error.into())),
                        Ok(Err(error)) => Err(error.to_string()),
                        Err(_) => Err("The plugin is not available.".into()),
                    });
                }
                recv(self.commands) -> command => {
                    if matches!(command, Ok(Command::Cancel) | Err(_)) {
                        return Err(failed("cancelled"));
                    }
                }
                default(deadline) => return Err(failed("The login took too long. Start again.")),
            }
        }
    }

    fn drive(&self) -> AuthStep {
        match self.drive_inner() {
            Ok(step) | Err(step) => step,
        }
    }

    fn drive_inner(&self) -> Result<AuthStep, AuthStep> {
        let (loopback, oauth) = if self.kind == AuthKind::Oauth {
            let listener = TcpListener::bind("127.0.0.1:0")
                .and_then(|l| l.set_nonblocking(true).map(|()| l))
                .map_err(|e| failed(format!("cannot listen for the browser: {e}")))?;
            let port = listener.local_addr().map_err(|e| failed(e.to_string()))?.port();
            let state = random_token(16);
            let oauth = Oauth::new(
                format!("http://127.0.0.1:{port}/callback"),
                Zeroizing::new(random_token(32)),
                state.clone(),
            );
            (Some(Loopback { listener, state }), Some(oauth))
        } else {
            (None, None)
        };
        let mut machine = Machine::new(self.kind, random_token(12), oauth);
        let mut next = machine.start();
        loop {
            next = match next {
                Next::Call(request) => {
                    let answer = self.call(machine.flow(), request)?;
                    machine.reply(answer, self)
                }
                Next::Poll { step, after } => {
                    self.emit(step)?;
                    self.pause(after)?;
                    machine.wake(self)
                }
                Next::Form(step) => {
                    self.emit(step)?;
                    let fields = self.wait_for_fields()?;
                    machine.submit(fields, self)
                }
                Next::Redirect(step) => {
                    self.emit(step)?;
                    let Some(loopback) = &loopback else {
                        return Err(failed("Loopback capture is only for oauth logins."));
                    };
                    let code = Zeroizing::new(self.wait_for_redirect(loopback)?);
                    machine.redirect(code)
                }
                Next::Finish(step) => return Ok(step),
            };
        }
    }

    fn wait_for_fields(&self) -> Result<Vec<FieldValue>, AuthStep> {
        let wait = INPUT_TIMEOUT.min(self.remaining()?);
        match self.commands.recv_timeout(wait) {
            Ok(Command::Submit(fields)) => Ok(fields),
            Ok(Command::Cancel) | Err(RecvTimeoutError::Disconnected) => Err(failed("cancelled")),
            Err(RecvTimeoutError::Timeout) => Err(failed("The login took too long. Start again.")),
        }
    }

    /// Waits for the browser to come back to the loopback listener with the
    /// expected `state`; returns the authorisation code.
    fn wait_for_redirect(&self, loopback: &Loopback) -> Result<String, AuthStep> {
        let deadline = Instant::now() + INPUT_TIMEOUT.min(self.remaining()?);
        loop {
            match loopback.listener.accept() {
                Ok((stream, _)) => {
                    if let Some(result) = handle_redirect(stream, &loopback.state) {
                        return result;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() > deadline {
                        return Err(failed("The browser did not come back in time."));
                    }
                    match self.commands.recv_timeout(Duration::from_millis(50)) {
                        Ok(Command::Cancel) | Err(RecvTimeoutError::Disconnected) => {
                            return Err(failed("cancelled"));
                        }
                        _ => {}
                    }
                }
                Err(e) => return Err(failed(format!("the browser connection failed: {e}"))),
            }
        }
    }
}

/// Handles one connection to the loopback listener. `None` means "not the
/// redirect we wait for; keep listening".
fn handle_redirect(mut stream: TcpStream, expected_state: &str) -> Option<Result<String, AuthStep>> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while head.len() < 8192 && !head.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => head.extend_from_slice(&buf[..n]),
        }
    }
    let head = String::from_utf8_lossy(&head);
    let target = head
        .lines()
        .next()?
        .strip_prefix("GET ")?
        .split(' ')
        .next()?
        .to_string();
    let url = url::Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    if url.path() != "/callback" {
        respond(&mut stream, "404 Not Found", "Not found.");
        return None;
    }
    let query: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    let get = |key: &str| query.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    let state_ok = get("state").is_some_and(|s| constant_time_eq(s.as_bytes(), expected_state.as_bytes()));
    if !state_ok {
        respond(&mut stream, "400 Bad Request", "This login link is not valid.");
        return None;
    }
    if let Some(error) = get("error") {
        respond(
            &mut stream,
            "200 OK",
            "The login was cancelled. You can close this window.",
        );
        let detail = get("error_description").unwrap_or(error);
        return Some(Err(failed(format!(
            "The service refused the login: {}",
            text(&detail, 256)
        ))));
    }
    match get("code") {
        Some(code) if !code.is_empty() && code.len() <= 4096 => {
            respond(
                &mut stream,
                "200 OK",
                "You are logged in to Jellyfesh. You can close this window.",
            );
            Some(Ok(code))
        }
        _ => {
            respond(&mut stream, "400 Bad Request", "The service sent no code.");
            None
        }
    }
}

fn respond(stream: &mut TcpStream, status: &str, message: &str) {
    let body = format!(
        "<!doctype html><meta charset=utf-8><title>Jellyfesh</title><p style=\"font:16px system-ui;margin:3em\">{message}</p>"
    );
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_url_safe() {
        assert_eq!(random_token(32).len(), 43);
        assert!(
            random_token(16)
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
    }

    #[test]
    fn compares_in_constant_time() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
