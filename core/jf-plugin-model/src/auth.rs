//! Login flows as a state machine without I/O (ADR-0004).
//!
//! A [`Machine`] holds the rules of a flow over the plugin's `auth.begin`,
//! `auth.poll` and `auth.submit`; the host drives it with its own threads,
//! timers and windows. `jf-plugin-host` drives it on a thread with
//! channels and a loopback listener; Jellyfesh Web (`core/jf-web`) drives it
//! from JavaScript with timers and a plugin worker. Each call returns
//! [`Next`], what the host does next:
//!
//! * [`Next::Call`]: call the plugin with [`Machine::flow`] and hand the
//!   answer (or the reason there is none) to [`Machine::reply`];
//! * [`Next::Poll`]: show the step (a QR code, a code to type elsewhere,
//!   progress text, or a page opened without redirect capture), wait the
//!   plugin's interval (0.2–30 s), then call [`Machine::wake`];
//! * [`Next::Form`]: show the form and pass what the person typed to
//!   [`Machine::submit`];
//! * [`Next::Redirect`]: open the page and pass the authorisation code the
//!   host caught on its redirect URI to [`Machine::redirect`] (oauth);
//! * [`Next::Finish`]: the flow is over with `Done` or `Failed`.
//!
//! What the machine checks: forms (1–16 fields, unique names of
//! `a-z 0-9 _ -`, cookie fields only for cookie-import, always secret),
//! required fields, field sizes, pages to open (on the allowlist, http or
//! https), code expiry, the plugin's state (at most 16 KiB), and the text of
//! every step (cut and stripped of control characters). For each submit it
//! names the values the host must protect: secret fields and the oauth code
//! are refused by `kv` (and by the keystore for password-exchange) and masked
//! in logs while the call runs; after a password-exchange submit the host
//! restarts the plugin's instance. Cookie fields are imported into the
//! plugin's jar by the host ([`AuthHost::import_cookies`]) and replaced by the
//! imported names. A flow lives at most [`FLOW_LIFETIME`]; the host waits at
//! most [`INPUT_TIMEOUT`] for the person or the browser.

use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL;
use sha2::{Digest, Sha256};
use url::Url;
use zeroize::{Zeroize, Zeroizing};

use crate::sanitize::text;
use crate::types::{
    AuthKind, AuthStep, FieldKind, FieldSpec, FieldValue, Flow, FlowStep, OauthRequest, RedirectCapture, Step,
};

/// Longest a flow may take from start to finish.
pub const FLOW_LIFETIME: Duration = Duration::from_secs(15 * 60);
/// Longest the host waits for the person to fill in a form or finish in the
/// browser.
pub const INPUT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Most login state a plugin may keep in [`FlowStep::state`].
pub const MAX_STATE: usize = 16 * 1024;
/// Most fields in a form.
pub const MAX_FIELDS: usize = 16;
/// Longest submitted value.
pub const MAX_FIELD_VALUE: usize = 64 * 1024;
/// Poll interval when the plugin names none.
pub const DEFAULT_POLL_MS: u32 = 2000;
/// Shortest and longest poll interval.
pub const POLL_RANGE_MS: (u32, u32) = (200, 30_000);

/// What the machine needs from the host.
pub trait AuthHost {
    /// Whether the plugin's allowlist covers `url` (pages a flow opens).
    fn allows(&self, url: &Url) -> bool;
    /// Imports the cookies the manifest's `[cookie_import]` names from a
    /// pasted `Cookie:` header into the plugin's jar, and returns the names
    /// imported; `Err` carries the message for the person.
    fn import_cookies(&self, header: &str) -> Result<Vec<String>, String>;
    /// Wall-clock time in milliseconds since the Unix epoch (code expiry).
    fn now_ms(&self) -> u64;
}

/// The oauth parts the host prepared before `begin`: where it listens for
/// the redirect, and the PKCE verifier and `state` it made.
#[derive(Debug)]
pub struct Oauth {
    /// What the plugin receives in `flow.oauth`.
    pub request: OauthRequest,
    /// The PKCE verifier, sent to the plugin with the code.
    pub verifier: Zeroizing<String>,
}

impl Oauth {
    /// The request for a verifier and `state` the host made, with an S256
    /// challenge.
    pub fn new(redirect_uri: String, verifier: Zeroizing<String>, state: String) -> Oauth {
        Oauth {
            request: OauthRequest {
                redirect_uri,
                code_challenge: pkce_challenge(&verifier),
                code_challenge_method: "S256".into(),
                state,
            },
            verifier,
        }
    }
}

/// The PKCE S256 code challenge for `verifier` (RFC 7636).
pub fn pkce_challenge(verifier: &str) -> String {
    BASE64URL.encode(Sha256::digest(verifier.as_bytes()))
}

/// A plugin call the host makes for the machine.
#[derive(Debug)]
pub enum Request {
    /// `auth.begin`.
    Begin,
    /// `auth.poll`.
    Poll,
    /// `auth.submit` with `fields`.
    Submit {
        /// The values to deliver.
        fields: Vec<FieldValue>,
        /// Values the host refuses to store (`kv.set`) and masks in the
        /// plugin's logs while this call runs. Wiped when dropped.
        protect: Vec<Zeroizing<String>>,
        /// Also refuse them in `kv.set-secret` (password-exchange).
        keystore: bool,
        /// Drop the plugin's instance after the call, so a password does not
        /// linger in its memory (password-exchange).
        restart: bool,
    },
}

/// What the host does next.
#[derive(Debug)]
pub enum Next {
    /// Make this call with [`Machine::flow`], then [`Machine::reply`].
    Call(Request),
    /// Show `step`, wait `after`, then [`Machine::wake`].
    Poll {
        /// What to show meanwhile.
        step: AuthStep,
        /// How long to wait before polling.
        after: Duration,
    },
    /// Show this `AskFields` step and wait for [`Machine::submit`].
    Form(AuthStep),
    /// Show this `OpenUrl` step, then wait for the browser's redirect and
    /// [`Machine::redirect`].
    Redirect(AuthStep),
    /// The flow is over.
    Finish(AuthStep),
}

/// A failed step with this message.
pub fn failed(message: impl Into<String>) -> AuthStep {
    AuthStep::Failed {
        message: message.into(),
    }
}

/// The message when a shown code expired.
const EXPIRED: &str = "The code expired. Start again.";

/// One login flow. See the module documentation.
#[derive(Debug)]
pub struct Machine {
    flow: Flow,
    verifier: Option<Zeroizing<String>>,
    /// When the code being shown stops working.
    expires_at_ms: Option<u64>,
    /// The form being shown.
    specs: Vec<FieldSpec>,
}

impl Machine {
    /// A flow of `kind` with the host's `flow_id` (unique per flow) and, for
    /// oauth, what it prepared.
    pub fn new(kind: AuthKind, flow_id: String, oauth: Option<Oauth>) -> Machine {
        let (request, verifier) = match oauth {
            Some(Oauth { request, verifier }) => (Some(request), Some(verifier)),
            None => (None, None),
        };
        Machine {
            flow: Flow {
                id: flow_id,
                kind,
                state: String::new(),
                oauth: request,
            },
            verifier,
            expires_at_ms: None,
            specs: Vec::new(),
        }
    }

    /// The flow record every plugin call takes.
    pub fn flow(&self) -> &Flow {
        &self.flow
    }

    /// The kind of login.
    pub fn kind(&self) -> AuthKind {
        self.flow.kind
    }

    /// The first call: `auth.begin`.
    pub fn start(&mut self) -> Next {
        Next::Call(Request::Begin)
    }

    /// The plugin's answer to the last call, or why there is none (for the
    /// person: the plugin's error text, a crash, a timeout).
    pub fn reply(&mut self, answer: Result<FlowStep, String>, host: &dyn AuthHost) -> Next {
        let mut reply = match answer {
            Ok(reply) => reply,
            Err(message) => return Next::Finish(failed(message)),
        };
        if reply.state.len() > MAX_STATE {
            return Next::Finish(failed("The plugin kept too much login state."));
        }
        self.flow.state = std::mem::take(&mut reply.state);
        let after = poll_interval(reply.poll_after_ms);
        self.expires_at_ms = None;
        match reply.step {
            Step::Done(account) => {
                let account = text(&account, 256);
                Next::Finish(AuthStep::Done {
                    account: if account.is_empty() {
                        "Account".into()
                    } else {
                        account
                    },
                })
            }
            Step::Failed(message) => Next::Finish(failed(text(&message, 1024))),
            Step::ShowQr(qr) => {
                if expired(qr.expires_at_ms, host) {
                    return Next::Finish(failed(EXPIRED));
                }
                self.expires_at_ms = qr.expires_at_ms;
                Next::Poll {
                    step: AuthStep::ShowQr {
                        data: text(&qr.data, 4096),
                        hint: qr.hint.map(|h| text(&h, 512)),
                        expires_at_ms: qr.expires_at_ms,
                    },
                    after,
                }
            }
            Step::ShowCode(code) => {
                if expired(code.expires_at_ms, host) {
                    return Next::Finish(failed(EXPIRED));
                }
                self.expires_at_ms = code.expires_at_ms;
                Next::Poll {
                    step: AuthStep::ShowCode {
                        code: text(&code.code, 64),
                        hint: text(&code.hint, 512),
                        expires_at_ms: code.expires_at_ms,
                    },
                    after,
                }
            }
            Step::Waiting(message) => Next::Poll {
                step: AuthStep::Waiting {
                    message: text(&message, 512),
                },
                after,
            },
            Step::OpenUrl(open) => {
                let Some(url) = Url::parse(&open.url)
                    .ok()
                    .filter(|u| matches!(u.scheme(), "https" | "http") && host.allows(u))
                else {
                    return Next::Finish(failed("The plugin asked to open a page outside its allowlist."));
                };
                let step = AuthStep::OpenUrl { url: url.into() };
                match open.capture {
                    RedirectCapture::None => Next::Poll { step, after },
                    RedirectCapture::Loopback if self.verifier.is_some() => Next::Redirect(step),
                    RedirectCapture::Loopback => {
                        Next::Finish(failed("Loopback capture is only for oauth logins."))
                    }
                }
            }
            Step::AskFields(specs) => match check_specs(specs, self.flow.kind) {
                Ok(specs) => {
                    self.specs = specs.clone();
                    Next::Form(AuthStep::AskFields { fields: specs })
                }
                Err(step) => Next::Finish(step),
            },
        }
    }

    /// The wait after [`Next::Poll`] is over: poll the plugin, unless the
    /// code expired meanwhile.
    pub fn wake(&mut self, host: &dyn AuthHost) -> Next {
        if expired(self.expires_at_ms, host) {
            return Next::Finish(failed(EXPIRED));
        }
        Next::Call(Request::Poll)
    }

    /// What the person typed into the form of [`Next::Form`]. Fields the form
    /// does not name are dropped; the values are moved into the call and
    /// wiped with it.
    pub fn submit(&mut self, fields: Vec<FieldValue>, host: &dyn AuthHost) -> Next {
        let specs = std::mem::take(&mut self.specs);
        if specs.is_empty() {
            return Next::Finish(failed("The plugin asked for no form."));
        }
        match prepare_fields(&specs, fields, host) {
            Ok((fields, protect)) => Next::Call(Request::Submit {
                fields,
                protect,
                keystore: self.flow.kind == AuthKind::PasswordExchange,
                restart: self.flow.kind == AuthKind::PasswordExchange,
            }),
            Err(step) => Next::Finish(step),
        }
    }

    /// The authorisation code the host caught on the redirect of
    /// [`Next::Redirect`].
    pub fn redirect(&mut self, code: Zeroizing<String>) -> Next {
        let (Some(verifier), Some(request)) = (&self.verifier, &self.flow.oauth) else {
            return Next::Finish(failed("Loopback capture is only for oauth logins."));
        };
        let fields = vec![
            FieldValue {
                name: "code".into(),
                value: code.to_string(),
            },
            FieldValue {
                name: "code-verifier".into(),
                value: verifier.to_string(),
            },
            FieldValue {
                name: "redirect-uri".into(),
                value: request.redirect_uri.clone(),
            },
        ];
        Next::Call(Request::Submit {
            fields,
            protect: vec![code],
            keystore: false,
            restart: false,
        })
    }
}

/// The plugin's poll interval, clamped.
fn poll_interval(ms: Option<u32>) -> Duration {
    let ms = ms
        .unwrap_or(DEFAULT_POLL_MS)
        .clamp(POLL_RANGE_MS.0, POLL_RANGE_MS.1);
    Duration::from_millis(u64::from(ms))
}

fn expired(expires_at_ms: Option<u64>, host: &dyn AuthHost) -> bool {
    expires_at_ms.is_some_and(|at| at <= host.now_ms())
}

/// Checks a form the plugin asks for.
pub fn check_specs(specs: Vec<FieldSpec>, kind: AuthKind) -> Result<Vec<FieldSpec>, AuthStep> {
    if specs.is_empty() || specs.len() > MAX_FIELDS {
        return Err(failed("The plugin asked for an invalid form."));
    }
    let mut out: Vec<FieldSpec> = Vec::with_capacity(specs.len());
    for spec in specs {
        let name_ok = !spec.name.is_empty()
            && spec.name.len() <= 64
            && spec
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'));
        if !name_ok || out.iter().any(|s| s.name == spec.name) {
            return Err(failed("The plugin asked for an invalid form."));
        }
        let cookie = spec.kind == FieldKind::CookieHeader;
        if cookie && kind != AuthKind::CookieImport {
            return Err(failed("Only cookie-import logins may ask for cookies."));
        }
        out.push(FieldSpec {
            name: spec.name,
            label: text(&spec.label, 256),
            kind: spec.kind,
            secret: spec.secret || cookie,
            required: spec.required,
            placeholder: spec.placeholder.map(|p| text(&p, 256)),
            help: spec.help.map(|h| text(&h, 512)),
        });
    }
    if kind == AuthKind::CookieImport && !out.iter().any(|s| s.kind == FieldKind::CookieHeader) {
        return Err(failed("A cookie-import login needs a cookie field."));
    }
    Ok(out)
}

/// Checks submitted fields against the form, imports cookies, and works out
/// which values to protect.
fn prepare_fields(
    specs: &[FieldSpec],
    mut fields: Vec<FieldValue>,
    host: &dyn AuthHost,
) -> Result<(Vec<FieldValue>, Vec<Zeroizing<String>>), AuthStep> {
    fields.retain_mut(|f| {
        let keep = specs.iter().any(|s| s.name == f.name);
        if !keep {
            f.value.zeroize();
        }
        keep
    });
    if fields.iter().any(|f| f.value.len() > MAX_FIELD_VALUE) {
        wipe(&mut fields);
        return Err(failed("A field is too long."));
    }
    for spec in specs.iter().filter(|s| s.required) {
        if !fields
            .iter()
            .any(|f| f.name == spec.name && !f.value.trim().is_empty())
        {
            wipe(&mut fields);
            return Err(failed(format!("{} is required.", spec.label)));
        }
    }
    let mut protect = Vec::new();
    for index in 0..fields.len() {
        let Some(spec) = specs.iter().find(|s| s.name == fields[index].name) else {
            continue;
        };
        if spec.kind == FieldKind::CookieHeader {
            match host.import_cookies(&fields[index].value) {
                Ok(imported) => {
                    fields[index].value.zeroize();
                    fields[index].value = imported.join(",");
                }
                Err(message) => {
                    wipe(&mut fields);
                    return Err(failed(message));
                }
            }
        } else if spec.secret {
            protect.push(Zeroizing::new(fields[index].value.clone()));
        }
    }
    Ok((fields, protect))
}

fn wipe(fields: &mut [FieldValue]) {
    for field in fields {
        field.value.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CodeDisplay, OpenUrl, QrCode};

    struct TestHost {
        now: u64,
    }

    impl AuthHost for TestHost {
        fn allows(&self, url: &Url) -> bool {
            url.host_str() == Some("music.example.com")
        }
        fn import_cookies(&self, header: &str) -> Result<Vec<String>, String> {
            if header.contains("session=") {
                Ok(vec!["session".into()])
            } else {
                Err("None of the expected cookies (session) were found.".into())
            }
        }
        fn now_ms(&self) -> u64 {
            self.now
        }
    }

    const HOST: TestHost = TestHost { now: 1_000 };

    fn spec(name: &str, kind: FieldKind) -> FieldSpec {
        FieldSpec {
            name: name.into(),
            label: name.into(),
            kind,
            secret: false,
            required: true,
            placeholder: None,
            help: None,
        }
    }

    fn reply(step: Step) -> FlowStep {
        FlowStep {
            step,
            state: "s".into(),
            poll_after_ms: None,
        }
    }

    fn value(name: &str, value: &str) -> FieldValue {
        FieldValue {
            name: name.into(),
            value: value.into(),
        }
    }

    #[test]
    fn forms_are_checked() {
        assert!(check_specs(vec![spec("user", FieldKind::Text)], AuthKind::PasswordExchange).is_ok());
        assert!(check_specs(vec![], AuthKind::ApiKey).is_err());
        assert!(check_specs(vec![spec("Bad Name", FieldKind::Text)], AuthKind::ApiKey).is_err());
        assert!(
            check_specs(
                vec![spec("a", FieldKind::Text), spec("a", FieldKind::Text)],
                AuthKind::ApiKey
            )
            .is_err()
        );
        assert!(check_specs(vec![spec("c", FieldKind::CookieHeader)], AuthKind::ApiKey).is_err());
        assert!(check_specs(vec![spec("c", FieldKind::Text)], AuthKind::CookieImport).is_err());
        let ok = check_specs(vec![spec("c", FieldKind::CookieHeader)], AuthKind::CookieImport).unwrap();
        assert!(ok[0].secret, "cookie fields are always secret");
    }

    #[test]
    fn pkce_challenge_is_s256() {
        // RFC 7636 appendix B.
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn a_password_form_protects_the_password_and_restarts() {
        let mut m = Machine::new(AuthKind::PasswordExchange, "f1".into(), None);
        assert!(matches!(m.start(), Next::Call(Request::Begin)));
        let mut password = spec("password", FieldKind::Text);
        password.secret = true;
        let next = m.reply(
            Ok(reply(Step::AskFields(vec![
                spec("username", FieldKind::Text),
                password,
            ]))),
            &HOST,
        );
        assert!(matches!(&next, Next::Form(AuthStep::AskFields { fields }) if fields.len() == 2));
        assert_eq!(m.flow().state, "s");
        let next = m.submit(
            vec![
                value("username", "ada"),
                value("password", "hunter22"),
                value("extra", "dropped"),
            ],
            &HOST,
        );
        let Next::Call(Request::Submit {
            fields,
            protect,
            keystore,
            restart,
        }) = next
        else {
            panic!("expected a submit, got {next:?}");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(protect.len(), 1);
        assert_eq!(protect[0].as_str(), "hunter22");
        assert!(keystore && restart);
        let done = m.reply(Ok(reply(Step::Done("  ".into()))), &HOST);
        assert!(matches!(done, Next::Finish(AuthStep::Done { account }) if account == "Account"));
    }

    #[test]
    fn required_fields_and_sizes() {
        let mut m = Machine::new(AuthKind::ApiKey, "f".into(), None);
        let _ = m.reply(
            Ok(reply(Step::AskFields(vec![spec("key", FieldKind::Text)]))),
            &HOST,
        );
        let next = m.submit(vec![value("key", "   ")], &HOST);
        assert!(matches!(next, Next::Finish(AuthStep::Failed { message }) if message == "key is required."));
        let mut m = Machine::new(AuthKind::ApiKey, "f".into(), None);
        let _ = m.reply(
            Ok(reply(Step::AskFields(vec![spec("key", FieldKind::Text)]))),
            &HOST,
        );
        let long = "k".repeat(MAX_FIELD_VALUE + 1);
        assert!(matches!(
            m.submit(vec![value("key", &long)], &HOST),
            Next::Finish(_)
        ));
    }

    #[test]
    fn codes_poll_until_they_expire() {
        let mut m = Machine::new(AuthKind::Qr, "f".into(), None);
        let code = Step::ShowCode(CodeDisplay {
            code: "482 913".into(),
            hint: "Enter it in Quick Connect".into(),
            expires_at_ms: Some(5_000),
        });
        let mut step = reply(code);
        step.poll_after_ms = Some(50);
        let next = m.reply(Ok(step), &HOST);
        let Next::Poll { step, after } = next else {
            panic!("expected to poll: {next:?}");
        };
        assert!(matches!(step, AuthStep::ShowCode { .. }));
        assert_eq!(after, Duration::from_millis(200), "clamped to 0.2 s");
        assert!(matches!(m.wake(&HOST), Next::Call(Request::Poll)));
        let later = TestHost { now: 5_000 };
        assert!(matches!(m.wake(&later), Next::Finish(AuthStep::Failed { message }) if message == EXPIRED));
        // Already expired when shown.
        let qr = Step::ShowQr(QrCode {
            data: "https://music.example.com/qr".into(),
            hint: None,
            expires_at_ms: Some(10),
        });
        assert!(matches!(m.reply(Ok(reply(qr)), &HOST), Next::Finish(_)));
    }

    #[test]
    fn pages_must_be_allowlisted_and_loopback_needs_oauth() {
        let open = |url: &str, capture| {
            reply(Step::OpenUrl(OpenUrl {
                url: url.into(),
                capture,
            }))
        };
        let mut m = Machine::new(AuthKind::Qr, "f".into(), None);
        let next = m.reply(
            Ok(open("https://evil.example.org/", RedirectCapture::None)),
            &HOST,
        );
        assert!(matches!(next, Next::Finish(AuthStep::Failed { .. })));
        let next = m.reply(
            Ok(open("https://music.example.com/link", RedirectCapture::None)),
            &HOST,
        );
        assert!(matches!(
            next,
            Next::Poll {
                step: AuthStep::OpenUrl { .. },
                ..
            }
        ));
        let next = m.reply(
            Ok(open("https://music.example.com/auth", RedirectCapture::Loopback)),
            &HOST,
        );
        assert!(matches!(next, Next::Finish(AuthStep::Failed { .. })));

        let oauth = Oauth::new(
            "http://127.0.0.1:4000/callback".into(),
            Zeroizing::new("verifier".into()),
            "st".into(),
        );
        let mut m = Machine::new(AuthKind::Oauth, "f".into(), Some(oauth));
        assert_eq!(m.flow().oauth.as_ref().unwrap().code_challenge_method, "S256");
        let next = m.reply(
            Ok(open("https://music.example.com/auth", RedirectCapture::Loopback)),
            &HOST,
        );
        assert!(matches!(next, Next::Redirect(AuthStep::OpenUrl { .. })));
        let Next::Call(Request::Submit { fields, protect, .. }) = m.redirect(Zeroizing::new("c0de".into()))
        else {
            panic!("expected a submit");
        };
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["code", "code-verifier", "redirect-uri"]);
        assert_eq!(protect[0].as_str(), "c0de");
    }

    #[test]
    fn cookies_are_imported_and_state_is_bounded() {
        let mut m = Machine::new(AuthKind::CookieImport, "f".into(), None);
        let _ = m.reply(
            Ok(reply(Step::AskFields(vec![spec(
                "cookies",
                FieldKind::CookieHeader,
            )]))),
            &HOST,
        );
        let next = m.submit(vec![value("cookies", "session=abc; other=1")], &HOST);
        assert!(
            matches!(&next, Next::Call(Request::Submit { fields, protect, .. }) if fields[0].value == "session" && protect.is_empty())
        );
        let mut m = Machine::new(AuthKind::CookieImport, "f".into(), None);
        let _ = m.reply(
            Ok(reply(Step::AskFields(vec![spec(
                "cookies",
                FieldKind::CookieHeader,
            )]))),
            &HOST,
        );
        assert!(matches!(
            m.submit(vec![value("cookies", "nope=1")], &HOST),
            Next::Finish(_)
        ));

        let mut big = reply(Step::Waiting("…".into()));
        big.state = "x".repeat(MAX_STATE + 1);
        assert!(matches!(m.reply(Ok(big), &HOST), Next::Finish(_)));
        assert!(matches!(
            m.reply(Err("the plugin crashed".into()), &HOST),
            Next::Finish(AuthStep::Failed { message }) if message == "the plugin crashed"
        ));
    }
}
