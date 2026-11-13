//! Online sources in the browser: settings, the checks every plugin answer
//! passes, unified search sections and sign-in flows. The plugins run in Web
//! Workers (platforms/web), each of which reads and verifies its own package
//! first (`jf-web-sandbox`); what decides anything is here, shared with the
//! apps through `jf-plugin-model`.

use jf_plugin_model::allow::Allowlist;
use jf_plugin_model::auth::{AuthHost, Machine, Next, Request};
use jf_plugin_model::manifest::AllowRule;
use jf_plugin_model::merge::{self, MergedItem};
use jf_plugin_model::sanitize::{self, Checker};
use jf_plugin_model::schema::SettingsSchema;
use jf_plugin_model::{
    AuthKind, AuthStep, CallError, ContentPolicy, FieldValue, Flow, FlowStep, ImageRef, Item, ItemPage,
    LyricPayload, StreamInfo,
};
use serde::Serialize;
use serde_json::Value;
use url::Url;
use zeroize::Zeroizing;

use crate::lyrics::{self, Lyrics};

/// The settings to use: `stored` made to fit the schema (values that no
/// longer validate fall back to their defaults), or the defaults.
pub fn effective_settings(schema: &Value, stored: Option<&Value>) -> Result<Value, String> {
    let schema = parse_schema(schema)?;
    Ok(match stored {
        Some(value) => schema.fit(value),
        None => schema.defaults(),
    })
}

/// Validates new settings against the schema, as the core does for
/// `SetPluginSettings`; returns them with defaults filled in.
pub fn validate_settings(schema: &Value, values: &Value) -> Result<Value, String> {
    parse_schema(schema)?.validate(values)
}

fn parse_schema(schema: &Value) -> Result<SettingsSchema, String> {
    if schema.as_object().is_some_and(|o| o.is_empty()) {
        return Ok(SettingsSchema::empty());
    }
    SettingsSchema::parse(&schema.to_string())
}

/// One installed source as the browser's host uses it: its id, name,
/// effective allowlist and content policy.
#[derive(Debug, Clone)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub allowlist: Allowlist,
    pub content_policy: ContentPolicy,
}

impl Source {
    /// The source for these rules and settings.
    pub fn new(
        id: &str,
        name: &str,
        rules: &[AllowRule],
        settings: &Value,
        content_policy: ContentPolicy,
    ) -> Source {
        Source {
            id: id.to_owned(),
            name: name.to_owned(),
            allowlist: Allowlist::new(rules, settings),
            content_policy,
        }
    }

    /// Whether the allowlist covers `url`.
    pub fn allows(&self, url: &str) -> bool {
        Url::parse(url).is_ok_and(|u| self.allowlist.check(&u).is_ok())
    }

    fn with_checker<T>(&self, f: impl FnOnce(&Checker<'_>) -> T) -> T {
        let allows = |url: &Url| self.allowlist.check(url).is_ok();
        f(&Checker::new(&self.id, &allows))
    }

    /// A page from the plugin, checked: at most `limit` items, invalid ones
    /// dropped, images only from the allowlist.
    pub fn check_page(&self, page: ItemPage, limit: u32) -> ItemPage {
        self.with_checker(|c| sanitize::page(page, limit, c))
    }

    /// A stream the player may fetch.
    pub fn check_stream(&self, info: StreamInfo) -> Result<StreamInfo, CallError> {
        self.with_checker(|c| sanitize::stream(info, c))
    }

    /// An artwork answer the page may fetch.
    pub fn check_artwork(&self, image: ImageRef) -> Result<ImageRef, CallError> {
        self.with_checker(|c| sanitize::artwork(image, c))
    }

    /// The plugin's lyrics, checked and parsed (unsynced when the content
    /// policy says so).
    pub fn lyrics(&self, payload: LyricPayload) -> Result<Lyrics, String> {
        let payload = sanitize::lyrics(payload).map_err(|e| e.to_string())?;
        lyrics::from_payload(payload, &self.name, self.content_policy.lyrics_sync)
    }
}

/// One row of a search section: the item as its source lists it, gaps
/// filled from the other copies, and the other sources that have it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SectionRow {
    pub item: Item,
    pub also_in: Vec<String>,
}

/// Merges the answers so far (`(source, items)` in the order they came) and
/// sorts the entries into one section per answer, as the core does: an
/// entry shows in the section of its first playable copy (else its first
/// copy), at that copy's place, listing the other sources in `also_in`.
pub fn sections(answers: &[(String, Vec<Item>)]) -> Vec<Vec<SectionRow>> {
    let lists: Vec<(&str, &[Item])> = answers
        .iter()
        .map(|(source, items)| (source.as_str(), items.as_slice()))
        .collect();
    let mut used: Vec<Vec<bool>> = answers
        .iter()
        .map(|(_, items)| vec![false; items.len()])
        .collect();
    let mut sections: Vec<Vec<(usize, SectionRow)>> = vec![Vec::new(); answers.len()];
    for merged in merge::merge(&lists) {
        if let Some((answer, index, row)) = entry(answers, &mut used, &merged) {
            sections[answer].push((index, row));
        }
    }
    sections
        .into_iter()
        .map(|mut section| {
            section.sort_by_key(|(index, _)| *index);
            section.into_iter().map(|(_, row)| row).collect()
        })
        .collect()
}

fn entry(
    answers: &[(String, Vec<Item>)],
    used: &mut [Vec<bool>],
    merged: &MergedItem,
) -> Option<(usize, usize, SectionRow)> {
    let mut copies: Vec<(usize, usize, bool)> = Vec::new();
    for copy in &merged.sources {
        let Some(answer) = answers.iter().position(|(source, _)| *source == copy.source) else {
            continue;
        };
        let found =
            answers[answer].1.iter().enumerate().position(|(i, item)| {
                !used[answer][i] && item.id == copy.id && item.kind == merged.item.kind
            });
        if let Some(index) = found {
            used[answer][index] = true;
            copies.push((answer, index, copy.playable));
        }
    }
    copies.sort_by_key(|(answer, _, _)| *answer);
    let &(answer, index, _) = copies.iter().find(|c| c.2).or_else(|| copies.first())?;
    let mut item = answers[answer].1.get(index)?.clone();
    let mut also_in = Vec::new();
    for &(other, _, _) in &copies {
        let source = &answers[other].0;
        if other != answer && !also_in.contains(source) {
            also_in.push(source.clone());
        }
    }
    if item.album.is_none() {
        item.album.clone_from(&merged.item.album);
    }
    if item.duration_ms.is_none() {
        item.duration_ms = merged.item.duration_ms;
    }
    if item.artwork.is_none() {
        item.artwork.clone_from(&merged.item.artwork);
    }
    Some((answer, index, SectionRow { item, also_in }))
}

/// What a sign-in flow needs from the page: the source's allowlist (pages
/// a flow opens) and the time. Cookie import needs a cookie jar the browser
/// does not give pages, so it is refused.
struct BrowserAuthHost<'a> {
    allowlist: &'a Allowlist,
    now_ms: u64,
}

impl AuthHost for BrowserAuthHost<'_> {
    fn allows(&self, url: &Url) -> bool {
        self.allowlist.check(url).is_ok()
    }

    fn import_cookies(&self, _header: &str) -> Result<Vec<String>, String> {
        Err("Signing in with cookies is not available in the browser.".into())
    }

    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

/// What the page does next in a sign-in flow, as data for JavaScript.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "next", rename_all = "snake_case")]
pub enum FlowAction {
    /// Call the plugin's `auth.begin`, `auth.poll` or `auth.submit` with
    /// `flow` (and `fields`); then `reply`. For a submit, `protect` lists
    /// the values the worker must refuse to store and mask in logs, and
    /// `restart` asks to restart the worker afterwards.
    Call {
        call: &'static str,
        flow: Flow,
        fields: Vec<FieldValue>,
        protect: Vec<String>,
        keystore: bool,
        restart: bool,
    },
    /// Show `step`, wait `after_ms`, then `wake`.
    Poll { step: AuthStep, after_ms: u64 },
    /// Show the form and `submit` what the person types.
    Form { step: AuthStep },
    /// Open the page; the browser cannot catch a loopback redirect, so the
    /// flow cannot go on.
    Redirect { step: AuthStep },
    /// The flow is over.
    Finish { step: AuthStep },
}

/// A sign-in flow driven from JavaScript.
#[derive(Debug)]
pub struct SignIn {
    machine: Machine,
    allowlist: Allowlist,
}

impl SignIn {
    /// A flow of `kind` with the page's flow id. OAuth needs a loopback
    /// listener the browser does not have, so its pages open without
    /// redirect capture.
    pub fn new(kind: AuthKind, flow_id: String, allowlist: Allowlist) -> SignIn {
        SignIn {
            machine: Machine::new(kind, flow_id, None),
            allowlist,
        }
    }

    fn action(&self, next: Next) -> FlowAction {
        match next {
            Next::Call(request) => {
                let flow = self.machine.flow().clone();
                match request {
                    Request::Begin => FlowAction::Call {
                        call: "begin",
                        flow,
                        fields: Vec::new(),
                        protect: Vec::new(),
                        keystore: false,
                        restart: false,
                    },
                    Request::Poll => FlowAction::Call {
                        call: "poll",
                        flow,
                        fields: Vec::new(),
                        protect: Vec::new(),
                        keystore: false,
                        restart: false,
                    },
                    Request::Submit {
                        fields,
                        protect,
                        keystore,
                        restart,
                    } => FlowAction::Call {
                        call: "submit",
                        flow,
                        fields,
                        protect: protect.iter().map(|p| p.to_string()).collect(),
                        keystore,
                        restart,
                    },
                }
            }
            Next::Poll { step, after } => FlowAction::Poll {
                step,
                after_ms: after.as_millis() as u64,
            },
            Next::Form(step) => FlowAction::Form { step },
            Next::Redirect(step) => FlowAction::Redirect { step },
            Next::Finish(step) => FlowAction::Finish { step },
        }
    }

    pub fn start(&mut self) -> FlowAction {
        let next = self.machine.start();
        self.action(next)
    }

    /// The plugin's answer, or the reason there is none.
    pub fn reply(&mut self, answer: Result<FlowStep, String>, now_ms: u64) -> FlowAction {
        let host = BrowserAuthHost {
            allowlist: &self.allowlist,
            now_ms,
        };
        let next = self.machine.reply(answer, &host);
        self.action(next)
    }

    pub fn wake(&mut self, now_ms: u64) -> FlowAction {
        let host = BrowserAuthHost {
            allowlist: &self.allowlist,
            now_ms,
        };
        let next = self.machine.wake(&host);
        self.action(next)
    }

    pub fn submit(&mut self, fields: Vec<FieldValue>, now_ms: u64) -> FlowAction {
        let host = BrowserAuthHost {
            allowlist: &self.allowlist,
            now_ms,
        };
        let next = self.machine.submit(fields, &host);
        self.action(next)
    }

    /// An authorisation code caught some other way (not used in the
    /// browser today).
    pub fn redirect(&mut self, code: String) -> FlowAction {
        let next = self.machine.redirect(Zeroizing::new(code));
        self.action(next)
    }

    /// The machine's host for tests.
    #[cfg(test)]
    fn host_at(&self, now_ms: u64) -> BrowserAuthHost<'_> {
        BrowserAuthHost {
            allowlist: &self.allowlist,
            now_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jf_plugin_model::{CodeDisplay, ItemKind, Step};
    use serde_json::json;

    const SCHEMA: &str = r#"{"type": "object", "properties": {
        "server_url": {"type": "string", "format": "uri"},
        "quality": {"type": "string", "enum": ["app", "low"], "default": "app"}},
        "required": ["server_url"]}"#;

    #[test]
    fn settings_are_checked_against_the_schema() {
        let schema: Value = serde_json::from_str(SCHEMA).unwrap();
        let ok = validate_settings(&schema, &json!({"server_url": "http://192.168.1.20:4533"})).unwrap();
        assert_eq!(ok["quality"], "app");
        assert!(validate_settings(&schema, &json!({"server_url": "ftp://x"})).is_err());
        assert!(validate_settings(&schema, &json!({})).is_err(), "required");
        let fitted = effective_settings(&schema, Some(&json!({"quality": "nope", "x": 1}))).unwrap();
        assert_eq!(fitted, json!({"quality": "app"}));
        assert_eq!(effective_settings(&json!({}), None).unwrap(), json!({}));
    }

    fn item(id: &str, title: &str, isrc: Option<&str>) -> Item {
        Item {
            id: id.into(),
            kind: ItemKind::Track,
            title: title.into(),
            artist: Some("Artist".into()),
            album: None,
            album_id: None,
            artist_id: None,
            duration_ms: Some(200_000),
            track_number: None,
            disc_number: None,
            year: None,
            child_count: None,
            isrc: isrc.map(str::to_owned),
            artwork: None,
            explicit: false,
            playable: true,
            unplayable: None,
        }
    }

    #[test]
    fn sections_show_each_entry_once() {
        let a = (
            "a".to_string(),
            vec![item("1", "Song", None), item("2", "Other", None)],
        );
        let mut b_song = item("x", "Song", Some("USRC17607839"));
        b_song.album = Some("Album".into());
        let b = ("b".to_string(), vec![b_song, item("y", "New", None)]);
        let sections = sections(&[a, b]);
        assert_eq!(sections[0].len(), 2);
        assert_eq!(sections[0][0].also_in, vec!["b".to_string()]);
        assert_eq!(sections[0][0].item.album.as_deref(), Some("Album"), "gaps filled");
        assert_eq!(sections[1].len(), 1);
        assert_eq!(sections[1][0].item.id, "y");
    }

    #[test]
    fn answers_are_checked_against_the_allowlist() {
        let rules = [AllowRule::Setting("server_url".into())];
        let source = Source::new(
            "org.example.music",
            "Example",
            &rules,
            &json!({"server_url": "http://192.168.1.20:4533"}),
            ContentPolicy::default(),
        );
        assert!(source.allows("http://192.168.1.20:4533/rest/ping"));
        assert!(!source.allows("http://192.168.1.21:4533/"));
        let stream = StreamInfo {
            url: "https://evil.example.org/a.mp3".into(),
            headers: Vec::new(),
            expires_at_ms: None,
            codec: None,
            bitrate_kbps: None,
            cacheable: false,
        };
        assert!(source.check_stream(stream).is_err());
        let mut art = item("1", "Song", None);
        art.artwork = Some(ImageRef {
            url: "https://evil.example.org/a.jpg".into(),
            headers: Vec::new(),
            width: None,
            height: None,
        });
        let page = source.check_page(
            ItemPage {
                items: vec![art, item("", "No id", None)],
                next: None,
                total: None,
            },
            50,
        );
        assert_eq!(page.items.len(), 1);
        assert!(page.items[0].artwork.is_none());
    }

    #[test]
    fn sign_in_flows_run_from_javascript() {
        let allowlist = Allowlist::new(
            &[AllowRule::Setting("server_url".into())],
            &json!({"server_url": "http://192.168.1.20:8096"}),
        );
        let mut flow = SignIn::new(AuthKind::Qr, "f1".into(), allowlist);
        assert!(matches!(flow.start(), FlowAction::Call { call: "begin", .. }));
        let code = FlowStep {
            step: Step::ShowCode(CodeDisplay {
                code: "482913".into(),
                hint: "Open Quick Connect".into(),
                expires_at_ms: Some(10_000),
            }),
            state: "secret-state".into(),
            poll_after_ms: Some(1000),
        };
        let action = flow.reply(Ok(code), 1_000);
        assert!(matches!(action, FlowAction::Poll { after_ms: 1000, .. }));
        let FlowAction::Call {
            call, flow: record, ..
        } = flow.wake(2_000)
        else {
            panic!("expected a poll call");
        };
        assert_eq!((call, record.state.as_str()), ("poll", "secret-state"));
        assert!(matches!(flow.wake(10_000), FlowAction::Finish { .. }));
        assert!(flow.host_at(0).import_cookies("a=b").is_err());
    }
}
