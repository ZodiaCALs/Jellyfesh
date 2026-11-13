//! Jellyfesh Web's core, compiled to WebAssembly.
//!
//! The browser shell (`platforms/web`) holds views, browser APIs and wiring;
//! everything that decides something is here (ADR-0001: shells hold no
//! business logic), on the same crates as the apps:
//!
//! * [`session`]: the play queue (`jf-session`) and the playback rules
//!   ([`rules`], mirrored from `jf-core` and `jf-engine`, which do not build
//!   for the browser): what plays next, Previous, play counting, gapless or
//!   crossfade, the equaliser bands, ReplayGain and the limiter;
//! * [`sources`]: settings, the checks on plugin answers, unified search
//!   sections and sign-in flows (`jf-plugin-model`);
//! * [`lyrics`]: lyric documents and timelines (`jf-lyrics`) from plugins,
//!   tags and lyrics files;
//! * [`settings`], [`qr`], and the themes of a fixed hue (`jf-palette`).
//!
//! Two modules sit beside it: `jf-web-media` (artwork themes and backdrops,
//! the tags of dragged-in files), loaded on demand in a worker, and
//! `jf-web-sandbox` (in every plugin worker: package reading and signature
//! checks, the crypto import and the network rules).
//!
//! The JavaScript API below passes structured values as JSON text (shapes
//! documented in `platforms/web/src/lib/core/types.ts`) and bytes as
//! `Uint8Array`. Nothing here runs per frame: drawing time is the shell's
//! port of `DisplayClock` and the lyric cursor.

pub mod lyrics;
pub mod qr;
pub mod rules;
pub mod session;
pub mod settings;
pub mod sources;

use jf_plugin_model::manifest::AllowRule;
use jf_plugin_model::{
    AuthKind, ContentPolicy, FieldValue, FlowStep, ImageRef, Item, ItemPage, LyricPayload, Quality,
    StreamInfo,
};
use jf_session::{Advance, QueueState, RepeatMode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use wasm_bindgen::prelude::*;

use crate::rules::ReplayGainInfo;
use crate::session::{FileMeta, NewEntry, Session};
use crate::settings::WebSettings;

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}

fn parse<T: DeserializeOwned>(text: &str, what: &str) -> Result<T, JsError> {
    serde_json::from_str(text).map_err(|e| JsError::new(&format!("{what}: {e}")))
}

fn error(message: impl std::fmt::Display) -> JsError {
    JsError::new(&message.to_string())
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn console_error(message: &str);
}

/// Reports Rust panics on the console instead of an opaque trap.
#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| console_error(&format!("jf-web: {info}"))));
}

/// The version of the core (the workspace version).
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

// ---- the session -----------------------------------------------------------------------------

/// The queue, its rules and the settings.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Core {
    session: Session,
    settings: WebSettings,
}

impl Default for Core {
    fn default() -> Self {
        Core::new(0, 0)
    }
}

#[wasm_bindgen]
impl Core {
    /// An empty core; shuffles derive from the 64-bit seed `hi:lo`.
    #[wasm_bindgen(constructor)]
    pub fn new(seed_hi: u32, seed_lo: u32) -> Core {
        Core {
            session: Session::new((u64::from(seed_hi) << 32) | u64::from(seed_lo)),
            settings: WebSettings::default(),
        }
    }

    /// Restores a queue saved with [`Core::save`]; false when it cannot be
    /// read (the queue stays as it is).
    pub fn restore(&mut self, state: &str) -> bool {
        match serde_json::from_str::<QueueState>(state).map(Session::restore) {
            Ok(Ok(session)) => {
                self.session = session;
                true
            }
            _ => false,
        }
    }

    /// The queue to save (without dragged-in files).
    pub fn save(&self) -> String {
        json(&self.session.state())
    }

    /// The settings.
    pub fn settings(&self) -> String {
        json(&self.settings)
    }

    /// Replaces the settings; returns them normalised.
    pub fn set_settings(&mut self, settings: &str) -> Result<String, JsError> {
        let settings: WebSettings = parse(settings, "settings")?;
        self.settings = settings.normalized();
        Ok(json(&self.settings))
    }

    /// Adds a dragged-in file (`FileMeta`) so it can be queued.
    pub fn add_file(&mut self, meta: &str) -> Result<(), JsError> {
        self.session.add_file(parse::<FileMeta>(meta, "file")?);
        Ok(())
    }

    /// Remembers items a source listed (an `Item` list).
    pub fn remember(&mut self, plugin: &str, items: &str) -> Result<(), JsError> {
        self.session
            .remember(plugin, &parse::<Vec<Item>>(items, "items")?);
        Ok(())
    }

    /// Replaces the queue (`NewEntry` list) and makes `start` current.
    pub fn play(&mut self, entries: &str, start: usize, shuffle: bool) -> Result<String, JsError> {
        let entries: Vec<NewEntry> = parse(entries, "entries")?;
        self.session.play(entries, start, shuffle).map_err(error)?;
        Ok(self.snapshot())
    }

    /// Play Next (`next`) or Play Last; returns the snapshot.
    pub fn enqueue(&mut self, entries: &str, next: bool) -> Result<String, JsError> {
        let entries: Vec<NewEntry> = parse(entries, "entries")?;
        self.session.enqueue(entries, next);
        Ok(self.snapshot())
    }

    /// Removes an entry; true when it was the current one.
    pub fn remove(&mut self, uid: f64) -> Result<bool, JsError> {
        self.session.remove(uid as u64).map_err(error)
    }

    pub fn move_item(&mut self, uid: f64, to: usize) -> Result<(), JsError> {
        self.session.move_item(uid as u64, to).map_err(error)
    }

    /// Makes an entry current; returns its row.
    pub fn jump_to(&mut self, uid: f64) -> Result<String, JsError> {
        Ok(json(&self.session.jump_to(uid as u64).map_err(error)?))
    }

    pub fn clear(&mut self) {
        self.session.clear();
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.session.set_shuffle(on);
    }

    /// `off`, `all` or `one`.
    pub fn set_repeat(&mut self, mode: &str) -> Result<(), JsError> {
        let mode: RepeatMode = parse(&format!("\"{mode}\""), "repeat mode")?;
        self.session.set_repeat(mode);
        Ok(())
    }

    /// Next (`manual`) or the end of a track: the row to play, or `null` at
    /// the end of the queue (stop; the queue rewinds to its first entry).
    pub fn next(&mut self, manual: bool) -> String {
        json(
            &self
                .session
                .next(if manual { Advance::Manual } else { Advance::Auto }),
        )
    }

    /// Previous at `position_ms`: `{"action":"restart"}`, `{"action":"play","row":…}` or `null`.
    pub fn previous(&mut self, position_ms: f64, active: bool) -> String {
        json(&self.session.previous(position_ms.max(0.0) as u64, active))
    }

    pub fn current(&self) -> String {
        json(&self.session.current())
    }

    /// The entry that plays next on its own, or `null`.
    pub fn upcoming(&self) -> String {
        json(&self.session.upcoming())
    }

    /// How the current entry hands over to `next_uid` on its own, with the
    /// crossfade settings: `{"kind":"gapless"}` or `{"kind":"crossfade","overlap_ms":…}`.
    pub fn transition(&self, next_uid: f64, current_ms: Option<f64>, next_ms: Option<f64>) -> String {
        let ms = |v: Option<f64>| v.filter(|v| v.is_finite() && *v > 0.0).map(|v| v as u64);
        json(&self.session.transition(
            self.settings.crossfade,
            next_uid as u64,
            ms(current_ms),
            ms(next_ms),
        ))
    }

    /// The plugin and item behind an entry: `{"plugin":…,"item":…}` or `null`.
    pub fn source_item(&self, uid: f64) -> String {
        #[derive(Serialize)]
        struct SourceItem {
            plugin: String,
            item: Item,
        }
        json(
            &self
                .session
                .source_item(uid as u64)
                .map(|(plugin, item)| SourceItem { plugin, item }),
        )
    }

    /// The dragged-in file behind an entry, or `null`.
    pub fn file_of(&self, uid: f64) -> String {
        json(&self.session.file_of(uid as u64))
    }

    /// An entry started playing (history, play timing).
    pub fn started(&mut self, uid: f64, now_ms: f64, duration_ms: Option<f64>) {
        self.session.started(
            uid as u64,
            now_ms as i64,
            duration_ms
                .filter(|d| d.is_finite() && *d > 0.0)
                .map(|d| d as u64),
        );
    }

    /// The play in progress ended: `{"uid":…,"counted":bool}` or `null`.
    pub fn finish(&mut self, played_ms: f64, natural: bool) -> String {
        #[derive(Serialize)]
        struct Finished {
            uid: u64,
            counted: bool,
        }
        json(
            &self
                .session
                .finish(played_ms.max(0.0) as u64, natural)
                .map(|(uid, counted)| Finished { uid, counted }),
        )
    }

    /// The queue for the views (`QueueSnapshot`).
    pub fn snapshot(&self) -> String {
        json(&self.session.snapshot())
    }

    pub fn window(&self, offset: usize, limit: usize) -> String {
        json(&self.session.window(offset, limit))
    }

    pub fn history(&self, limit: usize) -> String {
        json(&self.session.history(limit))
    }

    /// The ReplayGain scale for a track's tags (`ReplayGainInfo` or `null`)
    /// with the current settings.
    pub fn replaygain_scale(&self, info: &str) -> f32 {
        let info: Option<ReplayGainInfo> = serde_json::from_str(info).unwrap_or(None);
        rules::replaygain_scale(
            info.as_ref(),
            self.settings.replaygain_mode,
            self.settings.preamp_db,
        )
    }

    /// Whether the limiter is engaged for these ReplayGain scales.
    pub fn limiter_needed(&self, scales: Vec<f32>, crossfading: bool) -> bool {
        rules::limiter_needed(&scales, &self.settings.eq, crossfading)
    }
}

// ---- playback rules --------------------------------------------------------------------------

/// The equaliser presets: `[[name, EqSettings], …]`.
#[wasm_bindgen]
pub fn eq_presets() -> String {
    json(&rules::EqSettings::presets())
}

/// The limiter: `{"ceiling_db":…,"attack_s":…,"release_s":…}`.
#[wasm_bindgen]
pub fn limiter_params() -> String {
    json(&serde_json::json!({
        "ceiling_db": rules::LIMITER_CEILING_DB,
        "attack_s": rules::LIMITER_ATTACK_S,
        "release_s": rules::LIMITER_RELEASE_S,
    }))
}

/// Equal-power crossfade curves: `points` outgoing gains, then `points`
/// incoming gains.
#[wasm_bindgen]
pub fn fade_curves(points: usize) -> Vec<f32> {
    let [out, inc] = rules::fade_curves(points);
    out.into_iter().chain(inc).collect()
}

/// The quality to try after the browser could not play `failed` (`"Lossless"`, …), or `null`.
#[wasm_bindgen]
pub fn next_quality(failed: &str) -> String {
    let failed: Quality = match serde_json::from_str(&format!("\"{failed}\"")) {
        Ok(q) => q,
        Err(_) => return "null".into(),
    };
    json(&rules::next_quality(failed))
}

/// A MIME type for `canPlayType` from a codec hint.
#[wasm_bindgen]
pub fn codec_mime(codec: &str) -> Option<String> {
    rules::codec_mime(codec).map(str::to_owned)
}

// ---- sources ---------------------------------------------------------------------------------

/// Stored settings made to fit a settings schema, or its defaults.
#[wasm_bindgen]
pub fn effective_settings(schema: &str, stored: Option<String>) -> Result<String, JsError> {
    let schema: Value = parse(schema, "schema")?;
    let stored: Option<Value> = match stored {
        Some(text) => serde_json::from_str(&text).ok(),
        None => None,
    };
    Ok(json(
        &sources::effective_settings(&schema, stored.as_ref()).map_err(error)?,
    ))
}

/// New settings checked against the schema, defaults filled in.
#[wasm_bindgen]
pub fn validate_settings(schema: &str, values: &str) -> Result<String, JsError> {
    let schema: Value = parse(schema, "schema")?;
    let values: Value = parse(values, "settings")?;
    Ok(json(
        &sources::validate_settings(&schema, &values).map_err(error)?,
    ))
}

/// Unified search sections for `[[source, [Item…]], …]` (`SectionRow` lists).
#[wasm_bindgen]
pub fn search_sections(answers: &str) -> Result<String, JsError> {
    let answers: Vec<(String, Vec<Item>)> = parse(answers, "answers")?;
    Ok(json(&sources::sections(&answers)))
}

/// One installed source: checks its answers and parses its lyrics.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Source {
    inner: sources::Source,
}

#[wasm_bindgen]
impl Source {
    /// The source for its allow rules (`AllowRule` list), settings and
    /// content policy.
    #[wasm_bindgen(constructor)]
    pub fn new(id: &str, name: &str, rules: &str, settings: &str, policy: &str) -> Result<Source, JsError> {
        let rules: Vec<AllowRule> = parse(rules, "allow rules")?;
        let settings: Value = parse(settings, "settings")?;
        let policy: ContentPolicy = parse(policy, "content policy")?;
        Ok(Source {
            inner: sources::Source::new(id, name, &rules, &settings, policy),
        })
    }

    /// Whether the allowlist covers `url`.
    pub fn allows(&self, url: &str) -> bool {
        self.inner.allows(url)
    }

    /// An `ItemPage` answer, checked and cut to `limit`.
    pub fn check_page(&self, page: &str, limit: u32) -> Result<String, JsError> {
        let page: ItemPage = parse(page, "page")?;
        Ok(json(&self.inner.check_page(page, limit)))
    }

    /// A `StreamInfo` answer, checked.
    pub fn check_stream(&self, info: &str) -> Result<String, JsError> {
        let info: StreamInfo = parse(info, "stream")?;
        Ok(json(&self.inner.check_stream(info).map_err(error)?))
    }

    /// An `ImageRef` answer, checked.
    pub fn check_artwork(&self, image: &str) -> Result<String, JsError> {
        let image: ImageRef = parse(image, "image")?;
        Ok(json(&self.inner.check_artwork(image).map_err(error)?))
    }

    /// A `LyricPayload` answer, parsed: `{doc, timeline, synced}`.
    pub fn lyrics(&self, payload: &str) -> Result<String, JsError> {
        let payload: LyricPayload = parse(payload, "lyrics")?;
        Ok(json(&self.inner.lyrics(payload).map_err(error)?))
    }
}

/// A sign-in flow; every method returns the next `FlowAction` as JSON.
#[wasm_bindgen]
#[derive(Debug)]
pub struct SignIn {
    inner: sources::SignIn,
}

#[wasm_bindgen]
impl SignIn {
    /// A flow of `kind` (`"Qr"`, `"PasswordExchange"`, …) for a source's
    /// allow rules and settings.
    #[wasm_bindgen(constructor)]
    pub fn new(kind: &str, flow_id: &str, rules: &str, settings: &str) -> Result<SignIn, JsError> {
        let kind: AuthKind = parse(&format!("\"{kind}\""), "sign-in kind")?;
        let rules: Vec<AllowRule> = parse(rules, "allow rules")?;
        let settings: Value = parse(settings, "settings")?;
        let allowlist = jf_plugin_model::allow::Allowlist::new(&rules, &settings);
        Ok(SignIn {
            inner: sources::SignIn::new(kind, flow_id.to_owned(), allowlist),
        })
    }

    pub fn start(&mut self) -> String {
        json(&self.inner.start())
    }

    /// The plugin's `FlowStep` (JSON), or `error` when there is none.
    pub fn reply(
        &mut self,
        step: Option<String>,
        error: Option<String>,
        now_ms: f64,
    ) -> Result<String, JsError> {
        let answer = match (step, error) {
            (Some(step), _) => Ok(parse::<FlowStep>(&step, "flow step")?),
            (None, Some(error)) => Err(error),
            (None, None) => Err("The plugin did not answer.".to_owned()),
        };
        Ok(json(&self.inner.reply(answer, now_ms as u64)))
    }

    pub fn wake(&mut self, now_ms: f64) -> String {
        json(&self.inner.wake(now_ms as u64))
    }

    /// What the person typed (`FieldValue` list).
    pub fn submit(&mut self, fields: &str, now_ms: f64) -> Result<String, JsError> {
        let fields: Vec<FieldValue> = parse(fields, "fields")?;
        Ok(json(&self.inner.submit(fields, now_ms as u64)))
    }
}

// ---- lyrics, files, theme --------------------------------------------------------------------

/// Lyrics from a lyrics file: `{doc, timeline, synced}`.
#[wasm_bindgen]
pub fn lyrics_from_file(bytes: &[u8], name: &str) -> Result<String, JsError> {
    Ok(json(&lyrics::from_file(bytes, name).map_err(error)?))
}

/// Lyrics embedded in tags: `{doc, timeline, synced}`.
#[wasm_bindgen]
pub fn lyrics_from_tags(text: &str) -> Result<String, JsError> {
    Ok(json(&lyrics::from_tags(text).map_err(error)?))
}

/// `audio`, `lyrics` or `other`.
#[wasm_bindgen]
pub fn classify_file(name: &str, mime: &str) -> String {
    json(&lyrics::classify(name, mime)).trim_matches('"').to_owned()
}

/// Pairs dragged-in files (`[[name, mime], …]`) with their lyrics files:
/// `[[audio index, lyrics index or null], …]`.
#[wasm_bindgen]
pub fn pair_sidecars(files: &str) -> Result<String, JsError> {
    let files: Vec<(String, String)> = parse(files, "files")?;
    Ok(json(&lyrics::pair_sidecars(&files)))
}

/// The themes of a fixed hue: `{hue, from_artwork, light, dark}` (artwork
/// themes come from `jf-web-media`).
#[wasm_bindgen]
pub fn theme_for_hue(hue: f32) -> String {
    #[derive(Serialize)]
    struct Themes {
        hue: f32,
        from_artwork: bool,
        light: jf_palette::Theme,
        dark: jf_palette::Theme,
    }
    json(&Themes {
        hue,
        from_artwork: false,
        light: jf_palette::resolve_theme(hue, jf_palette::Appearance::Light),
        dark: jf_palette::resolve_theme(hue, jf_palette::Appearance::Dark),
    })
}

/// The alpha of a glass grade whose token alpha is `alpha` at the
/// transparency setting (`jf_palette::glass_alpha`: 0 solid, 0.5 the
/// token's alpha, 1 half of it).
#[wasm_bindgen]
pub fn glass_alpha(alpha: f32, transparency: f32) -> f32 {
    jf_palette::glass_alpha(alpha, transparency)
}

/// A QR code as `{"path": <SVG path>, "size": <modules>}`, or `null`.
#[wasm_bindgen]
pub fn qr_svg(data: &str) -> String {
    #[derive(Serialize)]
    struct Qr {
        path: String,
        size: usize,
    }
    json(&qr::svg_path(data).map(|(path, size)| Qr { path, size }))
}
