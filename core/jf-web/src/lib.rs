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
//! * [`search`]: search by source, the library first (ADR-0024 §4);
//! * [`collections`]: hearts, playlists and plays, kept in the browser with
//!   `jf-library`'s rules;
//! * [`lyrics`]: lyric documents and timelines (`jf-lyrics`) from plugins,
//!   tags and lyrics files;
//! * [`relay`]: which services refuse browsers, and the relay's address;
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

pub mod ai_lyrics;
pub mod collections;
pub mod details;
pub mod discover;
pub mod library;
pub mod look;
pub mod lyrics;
pub mod pro;
pub mod qr;
pub mod relay;
pub mod rules;
pub mod search;
pub mod session;
pub mod settings;
pub mod sources;

use jf_plugin_model::links::Links;
use jf_plugin_model::manifest::AllowRule;
use jf_plugin_model::{
    AuthKind, ContentPolicy, FieldValue, FlowStep, ImageRef, Item, ItemKind, ItemPage, LyricPayload,
    PlaybackMode, Quality, StreamInfo,
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

/// The website, with a trailing slash, as the apps' About finds it: `JF_WEBSITE` when the build
/// set it, else the GitHub Pages site of the repository the build ran for; `None` when the build
/// knows neither, or the address is not `https`.
pub fn website_from(explicit: Option<&str>, repository: Option<&str>) -> Option<String> {
    let site = match explicit.map(str::trim).filter(|s| !s.is_empty()) {
        Some(site) => site.to_owned(),
        None => {
            let (owner, name) = repository?.trim().split_once('/')?;
            if owner.is_empty() || name.is_empty() || name.contains('/') {
                return None;
            }
            format!("https://{}.github.io/{name}/", owner.to_lowercase())
        }
    };
    if !site.starts_with("https://") || site.len() <= "https://".len() {
        return None;
    }
    Some(if site.ends_with('/') {
        site
    } else {
        format!("{site}/")
    })
}

/// This build's website (About, the Pro sheet's legal links), or `undefined`.
#[wasm_bindgen]
pub fn website() -> Option<String> {
    website_from(option_env!("JF_WEBSITE"), option_env!("GITHUB_REPOSITORY"))
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

    /// Adds many files at once (`[FileMeta]`): the library's songs, so they can be queued.
    pub fn add_files(&mut self, metas: &str) -> Result<(), JsError> {
        for meta in parse::<Vec<FileMeta>>(metas, "files")? {
            self.session.add_file(meta);
        }
        Ok(())
    }

    /// The ids of queued files without tags (library files of a restored queue): `[id]`. Hand
    /// each back with `add_file`.
    pub fn missing_files(&self) -> String {
        json(&self.session.missing_files())
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

    /// The play in progress ended: `{"uid", "counted", "started_ms", "saved"}` or `null`.
    /// `saved` is what the entry is for hearts and plays (`Saved`), `null` for a dropped file.
    pub fn finish(&mut self, played_ms: f64, natural: bool) -> String {
        #[derive(Serialize)]
        struct Ended {
            #[serde(flatten)]
            finished: session::Finished,
            saved: Option<collections::Saved>,
        }
        json(
            &self
                .session
                .finish(played_ms.max(0.0) as u64, natural)
                .map(|finished| Ended {
                    saved: self.session.saved(finished.uid),
                    finished,
                }),
        )
    }

    /// What an entry is for hearts, playlists and plays (`Saved`), or `null` (a dropped file).
    pub fn saved(&self, uid: f64) -> String {
        json(&self.session.saved(uid as u64))
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

/// The services offered in Sources → Add a Source (`jf_plugin_model::presets`, the list the apps
/// share), in order: `[{"id", "name", "plugin", "address_setting", "address_example"}, …]`. The
/// page offers each while its official plugin is bundled.
#[wasm_bindgen]
pub fn source_presets() -> String {
    let list: Vec<Value> = jf_plugin_model::presets::PRESETS
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "plugin": p.plugin,
                "address_setting": p.address_setting,
                "address_example": p.address_example,
            })
        })
        .collect();
    json(&list)
}

// ---- the music library -------------------------------------------------------------------------

/// The catalogue of the person's music folders (`library`): the page loads the tracks it keeps,
/// tells it what a scan found, and asks it for lists.
#[wasm_bindgen]
#[derive(Debug, Default)]
pub struct Library(library::Library);

#[wasm_bindgen]
impl Library {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Library {
        Library::default()
    }

    /// Replaces every track (`[LibTrack]`).
    pub fn load(&mut self, tracks: &str) -> Result<(), JsError> {
        self.0.load(parse(tracks, "tracks")?);
        Ok(())
    }

    /// Adds tracks, replacing those with the same key.
    pub fn upsert(&mut self, tracks: &str) -> Result<(), JsError> {
        self.0.upsert(parse(tracks, "tracks")?);
        Ok(())
    }

    /// Removes tracks by key (`[key]`).
    pub fn remove(&mut self, keys: &str) -> Result<(), JsError> {
        let keys: Vec<String> = parse(keys, "keys")?;
        self.0.remove(&keys);
        Ok(())
    }

    pub fn remove_folder(&mut self, folder: u32) {
        self.0.remove_folder(folder);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `{tracks, albums, artists, duration_ms}`.
    pub fn stats(&mut self) -> String {
        json(&self.0.stats())
    }

    /// A page of albums: `sort` is `title`, `artist`, `year` or `added`.
    pub fn albums(
        &mut self,
        sort: &str,
        descending: bool,
        offset: usize,
        limit: usize,
    ) -> Result<String, JsError> {
        let sort: library::AlbumSort = parse(&json(&sort), "sort")?;
        Ok(json(&self.0.albums(sort, descending, offset, limit)))
    }

    /// `{album, tracks}`, or `null` when there is no such album.
    pub fn album(&mut self, id: &str) -> String {
        json(
            &self
                .0
                .album(id)
                .map(|(album, tracks)| serde_json::json!({ "album": album, "tracks": tracks })),
        )
    }

    /// A page of artists, by name.
    pub fn artists(&mut self, offset: usize, limit: usize) -> String {
        json(&self.0.artists(offset, limit))
    }

    /// `{artist, albums, tracks}`, or `null`.
    pub fn artist(&mut self, id: &str) -> String {
        json(&self.0.artist(id).map(|(artist, albums, tracks)| {
            serde_json::json!({ "artist": artist, "albums": albums, "tracks": tracks })
        }))
    }

    /// A page of tracks: `sort` is `title`, `artist`, `album`, `year`, `duration`, `path` or `added`.
    pub fn tracks(
        &mut self,
        sort: &str,
        descending: bool,
        offset: usize,
        limit: usize,
    ) -> Result<String, JsError> {
        let sort: library::TrackSort = parse(&json(&sort), "sort")?;
        Ok(json(&self.0.tracks(sort, descending, offset, limit)))
    }

    /// Every track's key in that order (to play the whole list).
    pub fn track_keys(&mut self, sort: &str, descending: bool) -> Result<String, JsError> {
        let sort: library::TrackSort = parse(&json(&sort), "sort")?;
        Ok(json(&self.0.track_keys(sort, descending)))
    }

    /// `{tracks, albums, artists}` matching `text`, at most `limit` of each.
    pub fn search(&mut self, text: &str, limit: usize) -> String {
        json(&self.0.search(text, limit))
    }
}

/// The artists a track is credited to (`library::artists_of`), for the page to show.
#[wasm_bindgen]
pub fn track_artists(artist_values: &str, explicit_artists: &str) -> Result<String, JsError> {
    let values: Vec<String> = parse(artist_values, "artist values")?;
    let explicit: Vec<String> = parse(explicit_artists, "artists")?;
    Ok(json(&library::artists_of(&values, &explicit)))
}

// ---- installing plugins ------------------------------------------------------------------------

/// The largest package Sources → Install from File… reads, in bytes (as the native host).
#[wasm_bindgen]
pub fn max_package_bytes() -> f64 {
    jf_plugin_model::install::MAX_PACKAGE_FILE as f64
}

/// What Jellyfesh Web says when a package plays on the person's own devices (`playback_mode =
/// "remote"`, ADR-0018): the page has no remote sessions in this version.
pub fn remote_refusal(name: &str) -> String {
    format!(
        "{name} plays on your own devices, which Jellyfesh Web cannot do yet. Use the Jellyfesh app on your computer or phone."
    )
}

/// Why a package cannot be installed now, in the native host's words, or `None`.
///
/// `package` is `{"id", "name", "signature", "playback_mode"}` from the package
/// (`jf_web_sandbox::PackageInfo`; `playback_mode` may be left out); `installed` is the
/// `signature` of the version installed now, left out when nothing with this id is installed.
/// Remote sources are refused first, whoever signed them: Jellyfesh Web has no remote sessions
/// (ADR-0018). Community packages are refused while community plugins are off, and an update
/// must be signed by the same key as the installed version.
#[wasm_bindgen]
pub fn install_refusal(
    package: &str,
    installed: Option<String>,
    allow_community: bool,
) -> Result<Option<String>, JsError> {
    #[derive(serde::Deserialize)]
    struct Package {
        id: String,
        name: String,
        signature: jf_plugin_model::SignatureStatus,
        #[serde(default)]
        playback_mode: Option<PlaybackMode>,
    }
    let package: Package = parse(package, "package")?;
    if package.playback_mode == Some(PlaybackMode::Remote) {
        return Ok(Some(remote_refusal(&package.name)));
    }
    let installed: Option<jf_plugin_model::SignatureStatus> =
        installed.map(|text| parse(&text, "installed")).transpose()?;
    let remembered = installed.as_ref().map(jf_plugin_model::install::signer);
    Ok(
        match jf_plugin_model::install::check(
            &package.id,
            &package.signature,
            remembered.as_ref().map(Option::as_deref),
            allow_community,
        ) {
            Ok(()) => None,
            Err(jf_plugin_model::HostError::CommunityDisabled) => Some(
                jf_plugin_model::install::community_refusal(Some((&package.name, &package.id))),
            ),
            Err(other) => Some(other.to_string()),
        },
    )
}

/// The service a plugin reaches that does not let browsers connect to it directly
/// ([`relay::refusing_service`]: by the plugin's id or the hosts of its allow rules, an
/// `AllowRule` list), or `undefined`. Jellyfesh Web says so before signing in, unless a relay is
/// set up (ADR-0022 §7).
#[wasm_bindgen]
pub fn refusing_service(plugin: &str, rules: &str) -> Result<Option<String>, JsError> {
    let rules: Vec<AllowRule> = parse(rules, "allow rules")?;
    Ok(relay::refusing_service(plugin, &rules).map(str::to_owned))
}

/// The relay address from Settings › Sources › Relay, normalised (no trailing slash); throws
/// why it cannot be used from this page as a code for the page's words: `invalid`,
/// `credentials`, `query` or `mixed-content` (on an https page only an https relay or one on
/// this computer).
#[wasm_bindgen]
pub fn relay_address(address: &str, page_is_https: bool) -> Result<String, JsError> {
    relay::relay_address(address, page_is_https).map_err(|problem| JsError::new(problem.code()))
}

/// Whether a bundled package of version `bundled` replaces the installed version `installed`
/// (only a newer one does).
#[wasm_bindgen]
pub fn bundled_updates(installed: &str, bundled: &str) -> bool {
    semver::Version::parse(bundled)
        .is_ok_and(|bundled| jf_plugin_model::install::bundled_updates(installed, &bundled))
}

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

/// Stored settings made to fit a settings schema, or its defaults. `api_version` is the
/// package's ("1.2" reads the schema with profile 1.2's keywords; left out, 1.1's).
#[wasm_bindgen]
pub fn effective_settings(
    schema: &str,
    stored: Option<String>,
    api_version: Option<String>,
) -> Result<String, JsError> {
    let schema: Value = parse(schema, "schema")?;
    let stored: Option<Value> = match stored {
        Some(text) => serde_json::from_str(&text).ok(),
        None => None,
    };
    let profile = sources::profile(api_version.as_deref());
    Ok(json(
        &sources::effective_settings(&schema, stored.as_ref(), profile).map_err(error)?,
    ))
}

/// New settings checked against the schema, defaults filled in (`api_version` as above).
#[wasm_bindgen]
pub fn validate_settings(schema: &str, values: &str, api_version: Option<String>) -> Result<String, JsError> {
    let schema: Value = parse(schema, "schema")?;
    let values: Value = parse(values, "settings")?;
    let profile = sources::profile(api_version.as_deref());
    Ok(json(
        &sources::validate_settings(&schema, &values, profile).map_err(error)?,
    ))
}

/// The best of a package's translations (`["zh-Hans", "ja"]` as JSON) for a language tag
/// (`zh-TW`, `ja-JP`), or `undefined` for English (API 1.2, `jf_plugin_model::l10n`).
#[wasm_bindgen]
pub fn best_language(available: &str, wanted: &str) -> Result<Option<String>, JsError> {
    let available: Vec<String> = parse(available, "languages")?;
    Ok(jf_plugin_model::l10n::best_language(&available, wanted).map(str::to_owned))
}

/// An appearance plugin's look (`jf_plugin_model::Look`), checked: numbers within their ranges,
/// lists without duplicates (API 1.2).
#[wasm_bindgen]
pub fn check_look(look: &str) -> Result<String, JsError> {
    let look: jf_plugin_model::Look = parse(look, "look")?;
    Ok(json(&jf_plugin_model::sanitize::look(look)))
}

/// Discover's limits: `{"call_ms":…,"shelf_items":…,"root_items":…}` (as the core's).
#[wasm_bindgen]
pub fn discover_limits() -> String {
    json(&serde_json::json!({
        "call_ms": discover::CALL_MS,
        "shelf_items": discover::SHELF_ITEMS,
        "root_items": discover::ROOT_ITEMS,
    }))
}

/// A plugin's own Discover shelves (checked by `Source::check_shelves`) as a page: `{shelves,
/// error}`, without empty shelves.
#[wasm_bindgen]
pub fn discover_own_page(shelves: &str) -> Result<String, JsError> {
    let shelves: Vec<jf_plugin_model::Shelf> = parse(shelves, "shelves")?;
    Ok(json(&discover::own_page(shelves)))
}

/// The folders of a browse root (`Item` list) that become Discover shelves.
#[wasm_bindgen]
pub fn discover_root_folders(root: &str) -> Result<String, JsError> {
    let root: Vec<Item> = parse(root, "root")?;
    Ok(json(&discover::root_folders(&root)))
}

/// A Discover page made from a source's answers (`discover::Answers`: `for_you`, `root` and
/// `folders`, each answer `{"ok": [Item…]}` or `{"err": "…"}`): `{shelves, error}`.
#[wasm_bindgen]
pub fn discover_made_page(name: &str, answers: &str) -> Result<String, JsError> {
    let answers: discover::Answers = parse(answers, "answers")?;
    Ok(json(&discover::made_page(name, answers)))
}

/// An audio format with the core's quality label (`details::AudioFormat`), from what is known.
#[wasm_bindgen]
pub fn audio_format(
    codec: Option<String>,
    sample_rate_hz: Option<u32>,
    bit_depth: Option<u32>,
    channels: Option<u32>,
    bitrate_kbps: Option<u32>,
) -> String {
    json(&details::format(
        codec.as_deref(),
        sample_rate_hz,
        bit_depth,
        channels,
        bitrate_kbps,
    ))
}

/// Get Info's rows for a song (`{about, audio}`, each a list of `{key, value}`): the plugin's
/// details (`ItemDetails`, or `undefined`), what the page knows (`details::Known`) and the format
/// of what plays (`AudioFormat`, or `undefined`).
#[wasm_bindgen]
pub fn song_rows(details: Option<String>, known: &str, format: Option<String>) -> Result<String, JsError> {
    let details: Option<jf_plugin_model::ItemDetails> = match details {
        Some(text) => Some(parse(&text, "details")?),
        None => None,
    };
    let known: details::Known = parse(known, "known")?;
    let format: Option<details::AudioFormat> = match format {
        Some(text) => Some(parse(&text, "format")?),
        None => None,
    };
    Ok(json(&details::song_rows(
        details.as_ref(),
        &known,
        format.as_ref(),
    )))
}

/// Get Info's rows for an album or an artist (`{key, value}` list).
#[wasm_bindgen]
pub fn item_rows(details: &str) -> Result<String, JsError> {
    let details: jf_plugin_model::ItemDetails = parse(details, "details")?;
    Ok(json(&details::item_rows(&details)))
}

// ---- AI Lyrics -------------------------------------------------------------------------------

/// What making lyrics needs first (`ai_lyrics::AiLyricsNeed`): the AI Lyrics plugin (or
/// `undefined`), whether Pro is active, its plan (`"asking"`, `{"failed": "…"}` or `{"ready":
/// MakerPlan}`) and the consents (`[{plugin, origin}]`).
#[wasm_bindgen]
pub fn ai_lyrics_need(
    plugin: Option<String>,
    pro: bool,
    plan: &str,
    consents: &str,
) -> Result<String, JsError> {
    let plan: ai_lyrics::PlanState = parse(plan, "plan")?;
    let consents: Vec<ai_lyrics::AudioConsent> = parse(consents, "consents")?;
    Ok(json(&ai_lyrics::need(plugin.as_deref(), pro, &plan, &consents)))
}

// ---- the look --------------------------------------------------------------------------------

/// The look to apply (`look::LookSnapshot` plus `density_scale`): an appearance plugin's answer
/// (`jf_plugin_model::Look`) checked and with the Luminous defaults filled in, or the defaults
/// when there is none (no plugin, or Pro off).
#[wasm_bindgen]
pub fn resolve_look(look: Option<String>, plugin: Option<String>) -> Result<String, JsError> {
    let look: Option<jf_plugin_model::Look> = match look {
        Some(text) => Some(parse(&text, "look")?),
        None => None,
    };
    let resolved = look::resolve(look, plugin.as_deref());
    let mut value = serde_json::to_value(&resolved).map_err(error)?;
    value["density_scale"] = serde_json::json!(look::density_scale(resolved.density));
    Ok(json(&value))
}

// ---- Jellyfesh Pro ---------------------------------------------------------------------------

/// Jellyfesh Pro for this release (ADR-0021): Pro Preview builds, or a licence key checked offline
/// against the keys built in. The page keeps the key (`activate` returns the text to keep) and
/// hands it back at the next visit, when it is checked again.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Pro {
    inner: pro::Pro,
}

#[wasm_bindgen]
impl Pro {
    #[wasm_bindgen(constructor)]
    pub fn new(stored_key: Option<String>) -> Pro {
        Pro {
            inner: pro::Pro::for_release(stored_key.as_deref()),
        }
    }

    /// The Pro slice: `{active, source, preview, licence_error, licence_keys}`.
    pub fn snapshot(&self) -> String {
        json(&self.inner.snapshot())
    }

    /// Checks a licence key: `{"ok": "<the key to keep>"}` or `{"err": "Malformed" | "UnknownKey" |
    /// "BadSignature" | "WrongProduct" | "Revoked"}`.
    pub fn activate(&mut self, key: &str) -> String {
        match self.inner.activate(key) {
            Ok(text) => json(&serde_json::json!({ "ok": text })),
            Err(error) => json(&serde_json::json!({ "err": error })),
        }
    }

    /// Remove Licence….
    pub fn remove_licence(&mut self) {
        self.inner.remove_licence();
    }
}

// ---- hearts, playlists and plays ----------------------------------------------------------

/// Hearts, playlists and plays (`collections`), kept by the page in IndexedDB. Items are
/// `Saved` (`{"plugin", "item"}`); times are milliseconds since the epoch. Refusals are errors
/// whose message is a code: `empty-name`, `no-playlist`, `no-entry` or `not-a-song`.
#[wasm_bindgen]
#[derive(Debug, Default)]
pub struct Collections {
    inner: collections::Collections,
}

#[wasm_bindgen]
impl Collections {
    /// The collections saved with `state`, or empty ones.
    #[wasm_bindgen(constructor)]
    pub fn new(state: Option<String>) -> Collections {
        Collections {
            inner: state
                .as_deref()
                .map(collections::Collections::from_state)
                .unwrap_or_default(),
        }
    }

    /// Everything, for the page to keep.
    pub fn state(&self) -> String {
        self.inner.state()
    }

    /// A new playlist; returns its id.
    pub fn create_playlist(&mut self, name: &str, now_ms: f64) -> Result<f64, JsError> {
        Ok(self.inner.create_playlist(name, now_ms as i64).map_err(error)? as f64)
    }

    pub fn rename_playlist(&mut self, id: f64, name: &str, now_ms: f64) -> Result<(), JsError> {
        self.inner
            .rename_playlist(id as i64, name, now_ms as i64)
            .map_err(error)
    }

    pub fn delete_playlist(&mut self, id: f64) -> Result<(), JsError> {
        self.inner.delete_playlist(id as i64).map_err(error)
    }

    /// Every playlist by name (`PlaylistSummary` list).
    pub fn playlists(&self) -> String {
        json(&self.inner.playlists())
    }

    /// One playlist (`PlaylistSummary`) or `null`.
    pub fn playlist(&self, id: f64) -> String {
        json(&self.inner.playlist(id as i64))
    }

    /// Entries from position `offset` (`PlaylistEntry` list).
    pub fn entries(&self, id: f64, offset: usize, limit: usize) -> String {
        json(&self.inner.entries(id as i64, offset, limit))
    }

    /// Adds songs (`Saved` list) at `at` (appended without it); returns their entry ids.
    pub fn add_to_playlist(
        &mut self,
        id: f64,
        items: &str,
        at: Option<usize>,
        now_ms: f64,
    ) -> Result<String, JsError> {
        let items: Vec<collections::Saved> = parse(items, "items")?;
        Ok(json(
            &self
                .inner
                .add_to_playlist(id as i64, items, at, now_ms as i64)
                .map_err(error)?,
        ))
    }

    /// Removes entries (an id list).
    pub fn remove_from_playlist(&mut self, id: f64, entry_ids: &str, now_ms: f64) -> Result<(), JsError> {
        let entry_ids: Vec<i64> = parse(entry_ids, "entries")?;
        self.inner
            .remove_from_playlist(id as i64, &entry_ids, now_ms as i64)
            .map_err(error)
    }

    pub fn move_playlist_entry(
        &mut self,
        id: f64,
        entry_id: f64,
        to: usize,
        now_ms: f64,
    ) -> Result<(), JsError> {
        self.inner
            .move_playlist_entry(id as i64, entry_id as i64, to, now_ms as i64)
            .map_err(error)
    }

    /// The heart on a song (`Saved`), or not; true when it changed.
    pub fn set_favorite(&mut self, saved: &str, favorite: bool, now_ms: f64) -> Result<bool, JsError> {
        let saved: collections::Saved = parse(saved, "item")?;
        self.inner
            .set_favorite(saved, favorite, now_ms as i64)
            .map_err(error)
    }

    pub fn is_favorite(&self, plugin: &str, id: &str) -> bool {
        self.inner.is_favorite(plugin, id)
    }

    /// The favourites, the latest first (`ItemStats` list).
    pub fn favorites(&self, offset: usize, limit: usize) -> String {
        json(&self.inner.favorites(offset, limit))
    }

    pub fn favorite_count(&self) -> usize {
        self.inner.favorite_count()
    }

    /// A play of a song (`Saved`) that started at `at_ms`, counted or a skip.
    pub fn record_play(&mut self, saved: &str, at_ms: f64, counted: bool) -> Result<(), JsError> {
        let saved: collections::Saved = parse(saved, "item")?;
        self.inner
            .record_play(saved, at_ms as i64, counted)
            .map_err(error)
    }

    /// Recently Played (`ItemStats` list).
    pub fn recently_played(&self, limit: usize) -> String {
        json(&self.inner.recently_played(limit))
    }

    /// Most Played (`ItemStats` list).
    pub fn most_played(&self, limit: usize) -> String {
        json(&self.inner.most_played(limit))
    }
}

// ---- search ------------------------------------------------------------------------------------

/// What to ask each source: `{"kinds": [ItemKind…], "limit", "library_limit"}`.
#[wasm_bindgen]
pub fn search_plan() -> String {
    json(&search::plan())
}

/// One query's results by source (ADR-0024 §4; `search`).
#[wasm_bindgen]
#[derive(Debug)]
pub struct SearchResults {
    inner: search::Search,
}

#[wasm_bindgen]
impl SearchResults {
    /// A search with the library's answer (an `Item` list: its songs, albums and artists).
    #[wasm_bindgen(constructor)]
    pub fn new(library: &str) -> Result<SearchResults, JsError> {
        Ok(SearchResults {
            inner: search::Search::new(parse(library, "library")?),
        })
    }

    /// A source is being asked.
    pub fn asking(&mut self, source: &str, name: &str) {
        self.inner.asking(source, name);
    }

    /// A source's answer: its content policy (`ContentPolicy`) and `[[ItemKind, ItemPage], …]`.
    pub fn answer(&mut self, source: &str, policy: &str, pages: &str) -> Result<(), JsError> {
        let policy: ContentPolicy = parse(policy, "content policy")?;
        let pages: Vec<(ItemKind, ItemPage)> = parse(pages, "pages")?;
        self.inner.answer(source, &policy, pages);
        Ok(())
    }

    /// A source failed or ran out of time.
    pub fn fail(&mut self, source: &str, error: &str) {
        self.inner.fail(source, error);
    }

    /// The cursor for Show More of a source's kind (`"Track"`, `"Album"`, …), or `undefined`.
    pub fn cursor(&self, source: &str, kind: &str) -> Option<String> {
        let kind: ItemKind = serde_json::from_value(Value::String(kind.to_owned())).ok()?;
        self.inner.cursor(source, kind)
    }

    /// Show More: the next page (`ItemPage`) of a source's kind.
    pub fn more(&mut self, source: &str, kind: &str, page: &str) -> Result<(), JsError> {
        let kind: ItemKind = parse(&format!("\"{kind}\""), "kind")?;
        let page: ItemPage = parse(page, "page")?;
        self.inner.more(source, kind, page);
        Ok(())
    }

    /// The All and Library tabs and the list of tabs (`search::View`).
    pub fn view(&self) -> String {
        json(&self.inner.view())
    }

    /// A source's tab (`search::Tab`), or `null`.
    pub fn tab(&self, source: &str) -> String {
        json(&self.inner.tab(source))
    }
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
    /// The source for its allow rules (`AllowRule` list), settings, content
    /// policy and the manifest's link templates (`Links`, API 1.1; left out
    /// or `null` for none).
    #[wasm_bindgen(constructor)]
    pub fn new(
        id: &str,
        name: &str,
        rules: &str,
        settings: &str,
        policy: &str,
        links: Option<String>,
    ) -> Result<Source, JsError> {
        let rules: Vec<AllowRule> = parse(rules, "allow rules")?;
        let settings: Value = parse(settings, "settings")?;
        let policy: ContentPolicy = parse(policy, "content policy")?;
        let links: Links = match links {
            Some(text) => parse::<Option<Links>>(&text, "links")?.unwrap_or_default(),
            None => Links::default(),
        };
        Ok(Source {
            inner: sources::Source::new(id, name, &rules, &settings, policy).with_links(links),
        })
    }

    /// Whether the allowlist covers `url`.
    pub fn allows(&self, url: &str) -> bool {
        self.inner.allows(url)
    }

    /// The `https` link that opens an item of this source in its service (`kind` is `"Track"`,
    /// `"Album"`, `"Artist"`, `"Playlist"` or `"Folder"`), filled in from the manifest's
    /// templates, or `undefined` (Open in {Service}, docs/ui.md).
    pub fn link(&self, kind: &str, id: &str) -> Option<String> {
        let kind: ItemKind = serde_json::from_value(Value::String(kind.to_owned())).ok()?;
        self.inner.link(kind, id)
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

    /// A Discover answer (`Shelf` list), checked (API 1.2).
    pub fn check_shelves(&self, shelves: &str) -> Result<String, JsError> {
        let shelves: Vec<jf_plugin_model::Shelf> = parse(shelves, "shelves")?;
        Ok(json(&self.inner.check_shelves(shelves)))
    }

    /// A details answer (`ItemDetails`), checked (API 1.2).
    pub fn check_details(&self, details: &str) -> Result<String, JsError> {
        let details: jf_plugin_model::ItemDetails = parse(details, "details")?;
        Ok(json(&self.inner.check_details(details)))
    }

    /// A comments answer (`CommentPage`), checked (API 1.2).
    pub fn check_comments(&self, page: &str) -> Result<String, JsError> {
        let page: jf_plugin_model::CommentPage = parse(page, "comments")?;
        Ok(json(&self.inner.check_comments(page)))
    }

    /// A maker's plan (`MakerPlan`), checked: where audio goes must be on the allowlist.
    pub fn check_plan(&self, plan: &str) -> Result<String, JsError> {
        let plan: jf_plugin_model::MakerPlan = parse(plan, "plan")?;
        Ok(json(&self.inner.check_plan(plan).map_err(error)?))
    }

    /// Lyrics a maker made (`Made`), checked.
    pub fn check_made(&self, made: &str) -> Result<String, JsError> {
        let made: jf_plugin_model::Made = parse(made, "made")?;
        Ok(json(&self.inner.check_made(made).map_err(error)?))
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

#[cfg(test)]
mod tests {
    use super::website_from;

    #[test]
    fn the_website_comes_from_the_build() {
        assert_eq!(
            website_from(Some("https://jellyfesh.example"), Some("o/r")).as_deref(),
            Some("https://jellyfesh.example/")
        );
        assert_eq!(
            website_from(None, Some("ZodiaCALs/jellyfesh")).as_deref(),
            Some("https://zodiacals.github.io/jellyfesh/")
        );
        assert_eq!(website_from(Some("  "), None), None);
        assert_eq!(
            website_from(Some("http://plain.example"), None),
            None,
            "https only"
        );
        assert_eq!(website_from(None, Some("no-slash")), None);
        assert_eq!(website_from(None, Some("a/b/c")), None);
    }
}
