//! AI Lyrics (ADR-0019): the `AiLyrics` slice, lyrics jobs, consent to send
//! audio, and on-device speech models.
//!
//! The AI Lyrics plugin (the first enabled plugin that declares
//! `make_lyrics`) decides how lyrics are made; its `maker.plan` says what
//! that needs first. The core runs one job at a time: the plugin's
//! `maker.make` call reads the song through a [`SongAccess`] (the engine's
//! decoders, 16 kHz mono) and recognises it with the app's recogniser and a
//! downloaded model, or sends it to the origin the person allowed. What it
//! makes is kept in `<data>/made-lyrics/` ([`MadeLyrics`]) and shown when
//! the song has no synced lyrics of its own.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::Sender;
use jf_engine::{AudioInput, MonoAudio, SPEECH_RATE};
use jf_plugin_host::{self as host, CallError, MakeJob, MakeStage, MakerAccess, PluginHost};

use crate::actor::{Msg, Shared};
use crate::made::{MadeLyrics, MadeMeta};
use crate::types::{
    AiLyricsMode, AiLyricsNeed, AiLyricsPlan, AiLyricsSnapshot, AudioConsent, Event, LyricsJob,
    LyricsJobStage, SpeechModelState, SpeechModelStatus,
};

/// How long the plugin's plan may take.
const PLAN_WAIT: Duration = Duration::from_secs(25);
/// Finished jobs listed.
const FINISHED_KEPT: usize = 20;
/// The longest song decoded for recognition.
const MAX_SONG_MS: u64 = 20 * 60 * 1000;
/// Recognition progress is reported in steps of this fraction.
const PROGRESS_STEP: f32 = 0.01;
/// Where the consents are kept, in the data folder.
const CONSENTS_FILE: &str = "ai-lyrics-consents.json";

/// What the AI Lyrics helpers tell the actor.
#[derive(Debug)]
pub(crate) enum AiMsg {
    /// The plugin's plan (for the request `generation`).
    Plan {
        generation: u64,
        result: Result<host::MakerPlan, CallError>,
    },
    /// A running job reached a stage.
    Stage { job: u64, stage: LyricsJobStage },
    /// A job's `maker.make` call ended.
    Finished {
        job: u64,
        result: Result<host::Made, CallError>,
    },
    /// Which models are downloaded and verified (checked at start).
    Models(Vec<(String, bool)>),
    /// A model download moved on or ended.
    Download { id: String, status: SpeechModelStatus },
}

/// Where a job's audio comes from.
#[derive(Debug, Clone)]
pub(crate) enum SongAudio {
    File(PathBuf),
    /// A source track: its stream is resolved when the job starts.
    Source {
        plugin: String,
        item: Box<host::Item>,
    },
}

/// A song to make lyrics for, resolved by the actor.
#[derive(Debug, Clone)]
pub(crate) struct Song {
    /// What identifies it in the made-lyrics folder.
    pub identity: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// Plain lyrics the song has, for the plugin to time.
    pub text: Option<String>,
    pub language: Option<String>,
    pub audio: SongAudio,
    /// The queue entry, when it is the song playing.
    pub uid: Option<u64>,
    pub library_id: Option<i64>,
}

#[derive(Debug)]
struct Job {
    info: LyricsJob,
    song: Song,
    cancel: Arc<AtomicBool>,
}

/// What the plugin's plan is.
#[derive(Debug, Clone, PartialEq)]
enum PlanState {
    /// Not asked yet, or the answer is on its way.
    Asking,
    Ready(host::MakerPlan),
    Failed(String),
}

/// What happened that the actor must act on.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Outcome {
    /// Made lyrics for this song identity changed: show them if it plays.
    pub made: Option<String>,
}

/// The AI Lyrics state the actor holds.
pub(crate) struct AiLyrics {
    tx: Sender<Msg>,
    data_dir: PathBuf,
    made: MadeLyrics,
    store: jf_speech::ModelStore,
    recognizer: Option<Arc<dyn jf_speech::Recognizer>>,
    host: Option<PluginHost>,
    /// The AI Lyrics plugin and what its plan depends on (version, account, enabled).
    plugin: Option<(String, String)>,
    plan: PlanState,
    plan_generation: u64,
    pro: bool,
    consents: BTreeSet<AudioConsent>,
    waiting: VecDeque<Job>,
    running: Option<Job>,
    finished: VecDeque<LyricsJob>,
    next_job: u64,
    models: Vec<SpeechModelState>,
    downloads: HashMap<String, Arc<AtomicBool>>,
    version: u64,
}

impl std::fmt::Debug for AiLyrics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiLyrics")
            .field("plugin", &self.plugin)
            .field("plan", &self.plan)
            .field(
                "jobs",
                &(self.waiting.len() + usize::from(self.running.is_some())),
            )
            .finish_non_exhaustive()
    }
}

impl AiLyrics {
    pub(crate) fn new(
        tx: Sender<Msg>,
        data_dir: &Path,
        store: jf_speech::ModelStore,
        recognizer: Option<Arc<dyn jf_speech::Recognizer>>,
        host: Option<PluginHost>,
    ) -> AiLyrics {
        let consents: Vec<AudioConsent> = std::fs::read(data_dir.join(CONSENTS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let models = store
            .models()
            .iter()
            .map(|m| SpeechModelState {
                id: m.id.to_owned(),
                name: m.name.to_owned(),
                bytes: m.approx_bytes,
                state: SpeechModelStatus::NotDownloaded,
            })
            .collect();
        let ai = AiLyrics {
            tx,
            data_dir: data_dir.to_owned(),
            made: MadeLyrics::new(data_dir),
            store,
            recognizer,
            host,
            plugin: None,
            plan: PlanState::Asking,
            plan_generation: 0,
            pro: false,
            consents: consents.into_iter().collect(),
            waiting: VecDeque::new(),
            running: None,
            finished: VecDeque::new(),
            next_job: 1,
            models,
            downloads: HashMap::new(),
            version: 0,
        };
        ai.check_models();
        ai
    }

    /// The made-lyrics folder.
    pub(crate) fn made(&self) -> &MadeLyrics {
        &self.made
    }

    /// Checks which models are downloaded, on a thread of its own (the
    /// first look at a model hashes it).
    fn check_models(&self) {
        let store = self.store.clone();
        let tx = self.tx.clone();
        let spawned = std::thread::Builder::new()
            .name("jf-models".into())
            .spawn(move || {
                let ready = store
                    .models()
                    .iter()
                    .map(|m| (m.id.to_owned(), store.path(m.id).is_some()))
                    .collect();
                let _ = tx.send(Msg::AiLyrics(AiMsg::Models(ready)));
            });
        if let Err(e) = spawned {
            tracing::warn!("could not check the speech models: {e}");
        }
    }

    /// The plugins changed (or Pro did): finds the AI Lyrics plugin and asks
    /// for its plan again when anything it depends on changed.
    pub(crate) fn plugins_changed(&mut self, pro: bool, force: bool) -> bool {
        let found = self.host.as_ref().and_then(|host| {
            host.plugins()
                .into_iter()
                .find(|p| p.enabled && p.capabilities.contains(&host::Capability::MakeLyrics))
                .map(|p| {
                    let key = format!(
                        "{}\n{}\n{}",
                        p.version,
                        p.account.unwrap_or_default(),
                        p.api_version
                    );
                    (p.id, key)
                })
        });
        if !force && found == self.plugin && pro == self.pro {
            return false;
        }
        if self.pro && !pro {
            // Turning Pro off stops lyrics jobs; nothing made is deleted.
            self.stop_all();
        }
        self.plugin = found;
        self.pro = pro;
        self.refresh_plan();
        true
    }

    /// Asks the plugin for its plan again (its settings changed, a model
    /// arrived).
    pub(crate) fn refresh_plan(&mut self) {
        self.plan_generation += 1;
        let (Some(host), Some((plugin, _)), true) = (self.host.clone(), self.plugin.clone(), self.pro) else {
            self.plan = PlanState::Asking;
            return;
        };
        self.plan = PlanState::Asking;
        let generation = self.plan_generation;
        let tx = self.tx.clone();
        let spawned = std::thread::Builder::new()
            .name("jf-maker-plan".into())
            .spawn(move || {
                let result = match host.maker_plan(&plugin).recv_timeout(PLAN_WAIT) {
                    Ok(result) => result,
                    Err(_) => Err(CallError::Timeout),
                };
                let _ = tx.send(Msg::AiLyrics(AiMsg::Plan { generation, result }));
            });
        if let Err(e) = spawned {
            self.plan = PlanState::Failed(format!("could not ask the plugin: {e}"));
        }
    }

    /// The AI Lyrics plugin, if one is on.
    pub(crate) fn plugin(&self) -> Option<&str> {
        self.plugin.as_ref().map(|(id, _)| id.as_str())
    }

    /// Whether the plugin makes lyrics by itself for songs without synced
    /// lyrics, and could now.
    pub(crate) fn automatic(&self) -> bool {
        matches!(&self.plan, PlanState::Ready(plan) if plan.automatic) && self.need() == AiLyricsNeed::Nothing
    }

    /// What making lyrics needs first.
    pub(crate) fn need(&self) -> AiLyricsNeed {
        need(
            self.plugin(),
            self.pro,
            &self.plan,
            &self.consents,
            self.recognizer.is_some(),
            &self.models,
        )
    }

    /// Queues a job for `song`; a song already waiting or being made keeps
    /// its job. Fails while something is needed first.
    pub(crate) fn make(&mut self, song: Song) -> Result<u64, String> {
        match self.need() {
            AiLyricsNeed::Nothing => {}
            need => return Err(need_text(&need)),
        }
        if let Some(job) = self
            .running
            .iter()
            .chain(self.waiting.iter())
            .find(|job| job.song.identity == song.identity)
        {
            return Ok(job.info.id);
        }
        let id = self.next_job;
        self.next_job += 1;
        self.waiting.push_back(Job {
            info: LyricsJob {
                id,
                title: song.title.clone(),
                artist: song.artist.clone(),
                uid: song.uid,
                library_id: song.library_id,
                stage: LyricsJobStage::Waiting,
            },
            song,
            cancel: Arc::new(AtomicBool::new(false)),
        });
        self.start_next();
        Ok(id)
    }

    /// Whether `identity` has a job waiting or running.
    pub(crate) fn busy_with(&self, identity: &str) -> bool {
        self.running
            .iter()
            .chain(self.waiting.iter())
            .any(|job| job.song.identity == identity)
    }

    /// Stops a waiting or running job.
    pub(crate) fn cancel(&mut self, job: u64) -> Result<(), String> {
        if let Some(running) = self.running.as_ref().filter(|j| j.info.id == job) {
            running.cancel.store(true, Ordering::Relaxed);
            return Ok(());
        }
        let Some(index) = self.waiting.iter().position(|j| j.info.id == job) else {
            return Err(format!("there is no lyrics job {job} waiting or running"));
        };
        if let Some(mut job) = self.waiting.remove(index) {
            job.info.stage = LyricsJobStage::Cancelled;
            self.finish(job.info);
        }
        Ok(())
    }

    /// Stops every waiting and running job.
    fn stop_all(&mut self) {
        while let Some(mut job) = self.waiting.pop_front() {
            job.info.stage = LyricsJobStage::Cancelled;
            self.finish(job.info);
        }
        if let Some(job) = &self.running {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Deletes a song's made lyrics.
    pub(crate) fn delete_made(&mut self, identity: &str) -> Result<Outcome, String> {
        match self.made.delete(identity) {
            Ok(true) => Ok(Outcome {
                made: Some(identity.to_owned()),
            }),
            Ok(false) => Err("the song has no lyrics made by AI Lyrics".into()),
            Err(e) => Err(format!("could not delete the made lyrics: {e}")),
        }
    }

    /// The person allowed (or stopped allowing) a plugin to send audio to
    /// an origin.
    pub(crate) fn set_consent(&mut self, plugin: &str, origin: &str, allow: bool) -> Result<(), String> {
        let origin = host::sanitize::origin(origin).ok_or_else(|| format!("{origin} is not an origin"))?;
        let consent = AudioConsent {
            plugin: plugin.to_owned(),
            origin,
        };
        let changed = if allow {
            self.consents.insert(consent)
        } else {
            self.consents.remove(&consent)
        };
        if changed {
            let list: Vec<&AudioConsent> = self.consents.iter().collect();
            let json = serde_json::to_vec_pretty(&list).map_err(|e| e.to_string())?;
            crate::sources::write_atomic(&self.data_dir.join(CONSENTS_FILE), &json)
                .map_err(|e| format!("could not save the consent: {e}"))?;
            if !allow && let Some(job) = &self.running {
                // Audio already on its way stays sent; nothing more goes.
                job.cancel.store(true, Ordering::Relaxed);
            }
        }
        Ok(())
    }

    /// Downloads a model on a thread of its own.
    pub(crate) fn download(&mut self, id: &str) -> Result<(), String> {
        let model = self
            .store
            .model(id)
            .ok_or_else(|| format!("there is no speech model {id:?}"))?;
        if self.downloads.contains_key(id) {
            return Ok(());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.downloads.insert(id.to_owned(), cancel.clone());
        self.set_model(id, SpeechModelStatus::Downloading { fraction: 0.0 });
        let (store, tx, model_id) = (self.store.clone(), self.tx.clone(), model.id.to_owned());
        let spawned = std::thread::Builder::new()
            .name("jf-model-download".into())
            .spawn(move || {
                let mut last = -1.0_f32;
                let mut progress = |done: u64, total: u64| {
                    let fraction = if total == 0 {
                        0.0
                    } else {
                        (done as f64 / total as f64).min(1.0) as f32
                    };
                    if fraction - last >= PROGRESS_STEP {
                        last = fraction;
                        let status = SpeechModelStatus::Downloading { fraction };
                        let _ = tx.send(Msg::AiLyrics(AiMsg::Download {
                            id: model_id.clone(),
                            status,
                        }));
                    }
                };
                let status = match store.download(&model_id, &mut progress, &cancel) {
                    Ok(_) => SpeechModelStatus::Ready,
                    Err(jf_speech::SpeechError::Cancelled) => SpeechModelStatus::NotDownloaded,
                    Err(e) => SpeechModelStatus::Failed {
                        message: e.to_string(),
                    },
                };
                let _ = tx.send(Msg::AiLyrics(AiMsg::Download { id: model_id, status }));
            });
        if let Err(e) = spawned {
            self.downloads.remove(id);
            self.set_model(id, SpeechModelStatus::NotDownloaded);
            return Err(format!("could not start the download: {e}"));
        }
        Ok(())
    }

    /// Stops a model download; what came so far is kept.
    pub(crate) fn cancel_download(&mut self, id: &str) -> Result<(), String> {
        match self.downloads.get(id) {
            Some(cancel) => {
                cancel.store(true, Ordering::Relaxed);
                Ok(())
            }
            None => Err(format!("the speech model {id:?} is not downloading")),
        }
    }

    /// Deletes a downloaded model.
    pub(crate) fn delete_model(&mut self, id: &str) -> Result<(), String> {
        if self.downloads.contains_key(id) {
            return Err("the model is downloading; cancel the download first".into());
        }
        self.store
            .delete(id)
            .map_err(|e| format!("could not delete the speech model: {e}"))?;
        self.set_model(id, SpeechModelStatus::NotDownloaded);
        self.models_changed();
        Ok(())
    }

    fn set_model(&mut self, id: &str, status: SpeechModelStatus) {
        if let Some(model) = self.models.iter_mut().find(|m| m.id == id) {
            model.state = status;
        }
    }

    /// Tells the plugin host which models are ready and asks for the plan
    /// again (the plugin's need may have changed).
    fn models_changed(&mut self) {
        if let Some(host) = &self.host {
            host.set_speech_models(
                self.models
                    .iter()
                    .map(|m| host::SpeechModel {
                        id: m.id.clone(),
                        name: m.name.clone(),
                        bytes: m.bytes,
                        ready: m.state == SpeechModelStatus::Ready,
                    })
                    .collect(),
            );
        }
        self.refresh_plan();
    }

    /// Handles what a helper sent; returns what the actor must do next.
    pub(crate) fn on_message(&mut self, msg: AiMsg) -> Outcome {
        let mut outcome = Outcome::default();
        match msg {
            AiMsg::Plan { generation, result } => {
                if generation != self.plan_generation {
                    return outcome;
                }
                self.plan = match result {
                    Ok(plan) => PlanState::Ready(plan),
                    Err(CallError::ProRequired) => PlanState::Asking,
                    Err(e) => PlanState::Failed(e.to_string()),
                };
            }
            AiMsg::Stage { job, stage } => {
                if let Some(running) = self.running.as_mut().filter(|j| j.info.id == job)
                    && !running.cancel.load(Ordering::Relaxed)
                {
                    running.info.stage = stage;
                }
            }
            AiMsg::Finished { job, result } => {
                let Some(mut running) = self.running.take_if(|j| j.info.id == job) else {
                    return outcome;
                };
                running.info.stage = match result {
                    Ok(made) => match self.keep(&running.song, made) {
                        Ok(()) => {
                            outcome.made = Some(running.song.identity.clone());
                            LyricsJobStage::Done
                        }
                        Err(message) => LyricsJobStage::Failed { message },
                    },
                    Err(CallError::Cancelled) => LyricsJobStage::Cancelled,
                    Err(_) if running.cancel.load(Ordering::Relaxed) => LyricsJobStage::Cancelled,
                    Err(e) => LyricsJobStage::Failed {
                        message: e.to_string(),
                    },
                };
                self.finish(running.info);
                self.start_next();
            }
            AiMsg::Models(ready) => {
                for (id, ready) in ready {
                    if !self.downloads.contains_key(&id) {
                        let status = if ready {
                            SpeechModelStatus::Ready
                        } else {
                            SpeechModelStatus::NotDownloaded
                        };
                        self.set_model(&id, status);
                    }
                }
                self.models_changed();
            }
            AiMsg::Download { id, status } => {
                let ended = !matches!(status, SpeechModelStatus::Downloading { .. });
                if ended {
                    self.downloads.remove(&id);
                } else if !self.downloads.contains_key(&id) {
                    return outcome; // progress after the end
                }
                self.set_model(&id, status);
                if ended {
                    self.models_changed();
                }
            }
        }
        outcome
    }

    /// Keeps what the plugin made, if it reads as lyrics.
    fn keep(&self, song: &Song, made: host::Made) -> Result<(), String> {
        let format = crate::sources::lyrics_format(made.lyrics.format)
            .ok_or("the plugin made lyrics in a format Jellyfesh does not read")?;
        let doc = jf_lyrics::parse(&made.lyrics.text, Some(format))
            .map_err(|e| format!("the plugin's lyrics could not be read: {e}"))?;
        if doc.lines.is_empty() {
            return Err("no words were recognised in the song".into());
        }
        let meta = MadeMeta {
            plugin: self.plugin().unwrap_or_default().to_owned(),
            engine: made.engine,
            title: song.title.clone(),
            artist: song.artist.clone(),
            format: crate::made::format_name(format).to_owned(),
            made_at_ms: crate::actor::unix_ms(),
        };
        self.made
            .put(&song.identity, &made.lyrics.text, &meta)
            .map_err(|e| format!("could not keep the lyrics: {e}"))
    }

    fn finish(&mut self, job: LyricsJob) {
        self.finished.push_back(job);
        while self.finished.len() > FINISHED_KEPT {
            self.finished.pop_front();
        }
    }

    /// Starts the next waiting job when none runs.
    fn start_next(&mut self) {
        if self.running.is_some() {
            return;
        }
        let Some(mut job) = self.waiting.pop_front() else {
            return;
        };
        let (Some(host), Some(plugin)) = (self.host.clone(), self.plugin().map(str::to_owned)) else {
            job.info.stage = LyricsJobStage::Failed {
                message: "no AI Lyrics plugin is on".into(),
            };
            self.finish(job.info);
            return self.start_next();
        };
        let upload_to = match &self.plan {
            PlanState::Ready(plan) => plan.upload_to.clone(),
            _ => None,
        };
        let allowed_origins: Vec<String> = self
            .consents
            .iter()
            .filter(|c| c.plugin == plugin)
            .map(|c| c.origin.clone())
            .collect();
        job.info.stage = LyricsJobStage::ReadingAudio;
        let id = job.info.id;
        let runner = Runner {
            job: id,
            song: job.song.clone(),
            plugin,
            host,
            recognizer: self.recognizer.clone(),
            store: self.store.clone(),
            tx: self.tx.clone(),
            cancel: job.cancel.clone(),
            allowed_origins,
            upload_to,
        };
        self.running = Some(job);
        let tx = self.tx.clone();
        let spawned = std::thread::Builder::new()
            .name("jf-lyrics-job".into())
            .spawn(move || {
                let result = runner.run();
                let _ = tx.send(Msg::AiLyrics(AiMsg::Finished { job: id, result }));
            });
        if let Err(e) = spawned {
            let _ = self.tx.send(Msg::AiLyrics(AiMsg::Finished {
                job: id,
                result: Err(CallError::Load(format!("could not start the job: {e}"))),
            }));
        }
    }

    /// Publishes the slice.
    pub(crate) fn publish(&mut self, shared: &Shared) {
        self.version += 1;
        let plan = match (&self.plan, self.pro) {
            (PlanState::Ready(plan), true) => Some(AiLyricsPlan {
                mode: match plan.mode {
                    host::MakerMode::OnDevice => AiLyricsMode::OnDevice,
                    host::MakerMode::Online => AiLyricsMode::Online,
                },
                engine: plan.engine.clone(),
                upload_to: plan.upload_to.clone(),
                automatic: plan.automatic,
            }),
            _ => None,
        };
        let mut jobs: Vec<LyricsJob> = self
            .finished
            .iter()
            .cloned()
            .chain(self.running.iter().map(|j| j.info.clone()))
            .chain(self.waiting.iter().map(|j| j.info.clone()))
            .collect();
        // Oldest first: ids grow with each job.
        jobs.sort_by_key(|j| j.id);
        let snapshot = Arc::new(AiLyricsSnapshot {
            version: self.version,
            available: self.plugin.is_some(),
            plugin: self.plugin().map(str::to_owned),
            plan,
            need: self.need(),
            jobs,
            models: self.models.clone(),
            consents: self.consents.iter().cloned().collect(),
        });
        shared.update(|s| s.ai_lyrics = snapshot.clone());
        shared.emit(Event::AiLyrics(snapshot));
    }
}

/// What making lyrics needs first.
fn need(
    plugin: Option<&str>,
    pro: bool,
    plan: &PlanState,
    consents: &BTreeSet<AudioConsent>,
    on_device: bool,
    models: &[SpeechModelState],
) -> AiLyricsNeed {
    let Some(plugin) = plugin else {
        return AiLyricsNeed::Unavailable {
            reason: "no AI Lyrics plugin is installed and on".into(),
        };
    };
    if !pro {
        return AiLyricsNeed::Pro;
    }
    let plan = match plan {
        PlanState::Asking => {
            return AiLyricsNeed::Unavailable {
                reason: "AI Lyrics is getting ready".into(),
            };
        }
        PlanState::Failed(reason) => {
            return AiLyricsNeed::Unavailable {
                reason: reason.clone(),
            };
        }
        PlanState::Ready(plan) => plan,
    };
    match &plan.need {
        host::MakerNeed::SignIn => AiLyricsNeed::SignIn,
        host::MakerNeed::Unavailable(reason) => AiLyricsNeed::Unavailable {
            reason: reason.clone(),
        },
        host::MakerNeed::Model(id) => match models.iter().find(|m| &m.id == id) {
            Some(model) => AiLyricsNeed::Model {
                id: model.id.clone(),
                name: model.name.clone(),
                bytes: model.bytes,
            },
            None => AiLyricsNeed::Unavailable {
                reason: format!("the plugin asks for a speech model Jellyfesh does not offer ({id})"),
            },
        },
        host::MakerNeed::Nothing => match (plan.mode, &plan.upload_to) {
            (host::MakerMode::OnDevice, _) if !on_device => AiLyricsNeed::Unavailable {
                reason: "on-device recognition is not part of this build".into(),
            },
            (host::MakerMode::Online, Some(origin))
                if !consents.iter().any(|c| c.plugin == plugin && &c.origin == origin) =>
            {
                AiLyricsNeed::Consent {
                    origin: origin.clone(),
                }
            }
            _ => AiLyricsNeed::Nothing,
        },
    }
}

/// Why `MakeLyrics` fails while `need` is not `Nothing`.
fn need_text(need: &AiLyricsNeed) -> String {
    match need {
        AiLyricsNeed::Nothing => String::new(),
        AiLyricsNeed::Pro => "AI Lyrics needs Jellyfesh Pro".into(),
        AiLyricsNeed::SignIn => "AI Lyrics needs your API key first (AI Lyrics settings)".into(),
        AiLyricsNeed::Consent { origin } => format!("allow sending songs to {origin} first"),
        AiLyricsNeed::Model { name, .. } => format!("download the {name} speech model first"),
        AiLyricsNeed::Unavailable { reason } => format!("AI Lyrics is unavailable: {reason}"),
    }
}

/// One job's `maker.make` call, on its own thread.
struct Runner {
    job: u64,
    song: Song,
    plugin: String,
    host: PluginHost,
    recognizer: Option<Arc<dyn jf_speech::Recognizer>>,
    store: jf_speech::ModelStore,
    tx: Sender<Msg>,
    cancel: Arc<AtomicBool>,
    allowed_origins: Vec<String>,
    upload_to: Option<String>,
}

impl Runner {
    fn run(self) -> Result<host::Made, CallError> {
        let input = match &self.song.audio {
            SongAudio::File(path) => AudioInput::File(path.clone()),
            SongAudio::Source { plugin, item } => {
                let reply = self
                    .host
                    .resolve_stream(plugin, (**item).clone(), host::Quality::High);
                let info = reply.recv_timeout(PLAN_WAIT).map_err(|_| CallError::Timeout)??;
                AudioInput::Http(crate::sources::http_source(info))
            }
        };
        let access = Arc::new(SongAccess {
            job: self.job,
            input,
            duration_ms: self.song.duration_ms.filter(|&d| d > 0),
            audio: Mutex::new(None),
            recognizer: self.recognizer,
            store: self.store,
            tx: self.tx,
            cancel: self.cancel.clone(),
        });
        let song = host::MakeSong {
            title: self.song.title.clone(),
            artist: self.song.artist.clone(),
            album: self.song.album.clone(),
            duration_ms: access.duration_ms().unwrap_or(0),
            text: self.song.text.clone(),
            language: self.song.language.clone(),
        };
        let job = MakeJob {
            song,
            access,
            allowed_origins: self.allowed_origins,
            upload_to: self.upload_to,
            cancel: self.cancel.clone(),
        };
        let result = self
            .host
            .make_lyrics(&self.plugin, job)
            .recv()
            .unwrap_or(Err(CallError::Shutdown));
        if self.cancel.load(Ordering::Relaxed) && result.is_err() {
            return Err(CallError::Cancelled);
        }
        result
    }
}

/// A job's song as the plugin's `audio` and `speech` imports see it: decoded
/// once, when first asked for, to 16 kHz mono.
struct SongAccess {
    job: u64,
    input: AudioInput,
    duration_ms: Option<u64>,
    audio: Mutex<Option<Arc<MonoAudio>>>,
    recognizer: Option<Arc<dyn jf_speech::Recognizer>>,
    store: jf_speech::ModelStore,
    tx: Sender<Msg>,
    cancel: Arc<AtomicBool>,
}

impl SongAccess {
    fn audio(&self) -> Result<Arc<MonoAudio>, jf_engine::EngineError> {
        let mut audio = self.audio.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(audio) = audio.as_ref() {
            return Ok(audio.clone());
        }
        let decoded = Arc::new(jf_engine::decode_mono(
            &self.input,
            SPEECH_RATE,
            MAX_SONG_MS,
            &self.cancel,
        )?);
        *audio = Some(decoded.clone());
        Ok(decoded)
    }

    fn send(&self, stage: LyricsJobStage) {
        let _ = self.tx.send(Msg::AiLyrics(AiMsg::Stage { job: self.job, stage }));
    }
}

fn audio_error(error: jf_engine::EngineError) -> host::AudioError {
    match error {
        jf_engine::EngineError::Cancelled => host::AudioError::Cancelled,
        other => host::AudioError::Unavailable(format!("the song could not be read: {other}")),
    }
}

impl MakerAccess for SongAccess {
    fn duration_ms(&self) -> Result<u64, host::AudioError> {
        match self.duration_ms {
            Some(duration) => Ok(duration),
            None => self.audio().map(|a| a.duration_ms()).map_err(audio_error),
        }
    }

    fn wav(&self, start_ms: u64, length_ms: u64) -> Result<Vec<u8>, host::AudioError> {
        let audio = self.audio().map_err(audio_error)?;
        audio.wav(start_ms, length_ms).map_err(audio_error)
    }

    fn transcribe(
        &self,
        options: &host::SpeechOptions,
        cancel: &AtomicBool,
    ) -> Result<host::Transcript, host::SpeechError> {
        let recognizer = self.recognizer.as_ref().ok_or_else(|| {
            host::SpeechError::Unavailable("on-device recognition is not part of this build".into())
        })?;
        let model = self
            .store
            .model(&options.model)
            .ok_or_else(|| host::SpeechError::ModelMissing(options.model.clone()))?;
        let path = self
            .store
            .path(&options.model)
            .ok_or_else(|| host::SpeechError::ModelMissing(options.model.clone()))?;
        self.send(LyricsJobStage::Recognising { fraction: 0.0 });
        let audio = self.audio().map_err(|e| match e {
            jf_engine::EngineError::Cancelled => host::SpeechError::Cancelled,
            other => host::SpeechError::Failed(format!("the song could not be read: {other}")),
        })?;
        let speech_options = jf_speech::Options {
            language: options.language.clone(),
            prompt: options.prompt.clone(),
            threads: None,
        };
        let mut last = 0.0_f32;
        let mut progress = |fraction: f32| {
            if fraction - last >= PROGRESS_STEP {
                last = fraction;
                self.send(LyricsJobStage::Recognising {
                    fraction: fraction.clamp(0.0, 1.0),
                });
            }
        };
        let transcript = recognizer
            .transcribe(
                model,
                &path,
                &audio.samples,
                &speech_options,
                &mut progress,
                cancel,
            )
            .map_err(|e| match e {
                jf_speech::SpeechError::Cancelled => host::SpeechError::Cancelled,
                jf_speech::SpeechError::ModelMissing(id) => host::SpeechError::ModelMissing(id),
                jf_speech::SpeechError::Unavailable(why) => host::SpeechError::Unavailable(why),
                other => host::SpeechError::Failed(other.to_string()),
            })?;
        Ok(host::Transcript {
            language: transcript.language,
            segments: transcript
                .segments
                .into_iter()
                .map(|s| host::SpeechSegment {
                    text: s.text,
                    start_ms: s.start_ms,
                    end_ms: s.end_ms,
                    words: s
                        .words
                        .into_iter()
                        .map(|w| host::SpeechWord {
                            text: w.text,
                            start_ms: w.start_ms,
                            end_ms: w.end_ms,
                            confidence: w.confidence,
                        })
                        .collect(),
                })
                .collect(),
        })
    }

    fn stage(&self, stage: MakeStage) {
        self.send(match stage {
            MakeStage::ReadingAudio => LyricsJobStage::ReadingAudio,
            MakeStage::Sending { origin } => LyricsJobStage::Sending { origin },
            MakeStage::Recognising => LyricsJobStage::Recognising { fraction: 0.0 },
            MakeStage::TimingWords => LyricsJobStage::TimingWords,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(mode: host::MakerMode, need: host::MakerNeed, upload_to: Option<&str>) -> PlanState {
        PlanState::Ready(host::MakerPlan {
            mode,
            engine: "Test Engine".into(),
            upload_to: upload_to.map(str::to_owned),
            need,
            automatic: false,
        })
    }

    fn models() -> Vec<SpeechModelState> {
        vec![SpeechModelState {
            id: "base".into(),
            name: "Base".into(),
            bytes: 142,
            state: SpeechModelStatus::NotDownloaded,
        }]
    }

    #[test]
    fn needs_follow_the_plan_pro_and_consent() {
        let none = BTreeSet::new();
        let online = plan(
            host::MakerMode::Online,
            host::MakerNeed::Nothing,
            Some("https://api.example.com"),
        );
        let local = plan(host::MakerMode::OnDevice, host::MakerNeed::Nothing, None);
        let p = Some("org.example.maker");
        assert!(matches!(
            need(None, true, &local, &none, true, &models()),
            AiLyricsNeed::Unavailable { .. }
        ));
        assert_eq!(need(p, false, &local, &none, true, &models()), AiLyricsNeed::Pro);
        assert!(matches!(
            need(p, true, &PlanState::Asking, &none, true, &models()),
            AiLyricsNeed::Unavailable { .. }
        ));
        assert_eq!(
            need(p, true, &local, &none, true, &models()),
            AiLyricsNeed::Nothing
        );
        assert!(matches!(
            need(p, true, &local, &none, false, &models()),
            AiLyricsNeed::Unavailable { .. }
        ));
        assert_eq!(
            need(p, true, &online, &none, false, &models()),
            AiLyricsNeed::Consent {
                origin: "https://api.example.com".into()
            }
        );
        let allowed: BTreeSet<AudioConsent> = [AudioConsent {
            plugin: "org.example.maker".into(),
            origin: "https://api.example.com".into(),
        }]
        .into();
        assert_eq!(
            need(p, true, &online, &allowed, false, &models()),
            AiLyricsNeed::Nothing
        );
        // Consent is per plugin.
        assert!(matches!(
            need(Some("org.other"), true, &online, &allowed, false, &models()),
            AiLyricsNeed::Consent { .. }
        ));
        let sign_in = plan(host::MakerMode::Online, host::MakerNeed::SignIn, None);
        assert_eq!(
            need(p, true, &sign_in, &none, true, &models()),
            AiLyricsNeed::SignIn
        );
        let model = plan(
            host::MakerMode::OnDevice,
            host::MakerNeed::Model("base".into()),
            None,
        );
        assert_eq!(
            need(p, true, &model, &none, true, &models()),
            AiLyricsNeed::Model {
                id: "base".into(),
                name: "Base".into(),
                bytes: 142
            }
        );
        let unknown = plan(
            host::MakerMode::OnDevice,
            host::MakerNeed::Model("huge".into()),
            None,
        );
        assert!(matches!(
            need(p, true, &unknown, &none, true, &models()),
            AiLyricsNeed::Unavailable { .. }
        ));
        assert!(need_text(&AiLyricsNeed::Pro).contains("Pro"));
    }
}
