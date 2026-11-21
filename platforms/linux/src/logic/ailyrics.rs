//! AI Lyrics in the Lyrics pane, the song menu and Now Playing's More
//! (docs/ui.md "AI Lyrics, Customize and Pro"; ADR-0019). The core decides
//! what is needed first (`AiLyricsSnapshot::need`), runs the jobs and keeps
//! the made lyrics; this reads the slice for the views: what the pane
//! offers for the song playing, and what choosing Create Synced Lyrics
//! (Try Again, Make Again) does now.

use jf_core::{
    AiLyricsNeed, AiLyricsSnapshot, Command, LyricsJob, LyricsJobStage, LyricsSnapshot, LyricsSource,
    SongRef, SpeechModelStatus, TrackRef,
};

/// What the Lyrics pane offers about making lyrics.
#[derive(Debug, Clone, PartialEq)]
pub enum Offer {
    /// AI Lyrics is off or missing, or nothing is left to do.
    None,
    /// Create Synced Lyrics (Time These Lyrics over plain lyrics); Pro or
    /// the question about sending audio come when it is chosen.
    Create,
    /// The song's job: its stage, its progress when it can be measured
    /// (0–1), and Cancel.
    Running {
        job: u64,
        stage: LyricsJobStage,
        progress: Option<f32>,
    },
    /// "Couldn't make lyrics: {message}" and Try Again.
    Failed { message: String },
    /// "Download the {Model} model ({size}) to make lyrics on this device."
    /// and Download.
    DownloadModel { id: String, name: String, bytes: u64 },
    /// "Downloading {Model}… 35%" and Cancel.
    DownloadingModel { id: String, name: String, fraction: f32 },
    /// "Add your API key to make lyrics online." and Open AI Lyrics
    /// Settings.
    SignIn,
    /// Why lyrics cannot be made, in the core's words.
    Unavailable { reason: String },
}

/// What choosing Create Synced Lyrics (or Try Again, Make Again) does.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Send(Command),
    /// Jellyfesh Pro is inactive: the Pro sheet.
    ProSheet,
    /// "Send This Song to {origin}?" first; Allow sends `allow`.
    AskConsent {
        origin: String,
        allow: Vec<Command>,
    },
}

/// A job still waiting or running (Cancel stops it).
pub fn active(stage: &LyricsJobStage) -> bool {
    !matches!(
        stage,
        LyricsJobStage::Done | LyricsJobStage::Failed { .. } | LyricsJobStage::Cancelled
    )
}

/// The newest job for the queue entry `uid` (the song playing).
pub fn job_for(ai: &AiLyricsSnapshot, uid: Option<u64>) -> Option<&LyricsJob> {
    let uid = uid?;
    ai.jobs.iter().rev().find(|j| j.uid == Some(uid))
}

/// A running job's progress for a bar (Recognising only).
pub fn progress(stage: &LyricsJobStage) -> Option<f32> {
    match stage {
        LyricsJobStage::Recognising { fraction } if fraction.is_finite() => Some(fraction.clamp(0.0, 1.0)),
        _ => None,
    }
}

/// The pane's offer: the song's job first (running, or failed), then, while
/// AI Lyrics is available, what making lyrics needs.
pub fn offer(job: Option<&LyricsJob>, ai: &AiLyricsSnapshot) -> Offer {
    if let Some(job) = job {
        if active(&job.stage) {
            return Offer::Running {
                job: job.id,
                stage: job.stage.clone(),
                progress: progress(&job.stage),
            };
        }
        if let LyricsJobStage::Failed { message } = &job.stage {
            return Offer::Failed {
                message: message.clone(),
            };
        }
    }
    if !ai.available {
        return Offer::None;
    }
    match &ai.need {
        AiLyricsNeed::Nothing | AiLyricsNeed::Pro | AiLyricsNeed::Consent { .. } => Offer::Create,
        AiLyricsNeed::SignIn => Offer::SignIn,
        AiLyricsNeed::Model { id, name, bytes } => {
            let downloading = ai
                .models
                .iter()
                .find(|m| m.id == *id)
                .and_then(|m| match m.state {
                    SpeechModelStatus::Downloading { fraction } => Some(fraction),
                    _ => None,
                });
            match downloading {
                Some(fraction) => Offer::DownloadingModel {
                    id: id.clone(),
                    name: name.clone(),
                    fraction: fraction.clamp(0.0, 1.0),
                },
                None => Offer::DownloadModel {
                    id: id.clone(),
                    name: name.clone(),
                    bytes: *bytes,
                },
            }
        }
        AiLyricsNeed::Unavailable { reason } if !reason.trim().is_empty() => Offer::Unavailable {
            reason: reason.trim().to_owned(),
        },
        AiLyricsNeed::Unavailable { .. } => Offer::None,
    }
}

/// Whether the song has lyrics to show as lines, and whether they are
/// timed.
fn lines(lyrics: Option<&LyricsSnapshot>) -> Option<bool> {
    let doc = lyrics?.doc.as_ref()?;
    if doc.lines.is_empty() {
        return None;
    }
    Some(doc.is_synced())
}

/// Whether the pane shows `offer` with `lyrics`: always without lyrics,
/// below plain ones, and over synced ones only while their song's job runs
/// or after it failed.
pub fn shows(offer: &Offer, lyrics: Option<&LyricsSnapshot>) -> bool {
    match (offer, lines(lyrics)) {
        (Offer::None, _) => false,
        (_, None | Some(false)) => true,
        (o, Some(true)) => matches!(o, Offer::Running { .. } | Offer::Failed { .. }),
    }
}

/// The song has plain lyrics: Create Synced Lyrics reads Time These Lyrics.
pub fn times_existing(lyrics: Option<&LyricsSnapshot>) -> bool {
    lines(lyrics) == Some(false)
}

/// What making lyrics for `song` does now: the job, the Pro sheet, or the
/// question about sending audio.
pub fn make(ai: &AiLyricsSnapshot, song: SongRef) -> Step {
    match (&ai.need, &ai.plugin) {
        (AiLyricsNeed::Pro, _) => Step::ProSheet,
        (AiLyricsNeed::Consent { origin }, Some(plugin)) => Step::AskConsent {
            origin: origin.clone(),
            allow: vec![
                Command::AllowAudioUpload {
                    plugin: plugin.clone(),
                    origin: origin.clone(),
                },
                Command::MakeLyrics { song },
            ],
        },
        // Otherwise the core makes them, or answers with what is missing.
        _ => Step::Send(Command::MakeLyrics { song }),
    }
}

/// The song menu's Create Synced Lyrics: library songs, while AI Lyrics is
/// available.
pub fn offers_for(ai: &AiLyricsSnapshot, track: &TrackRef) -> bool {
    ai.available && matches!(track, TrackRef::Library(_))
}

/// The engine of lyrics AI Lyrics made ("Made by AI Lyrics · {engine}").
pub fn made_with(lyrics: Option<&LyricsSnapshot>) -> Option<&str> {
    match &lyrics?.source {
        Some(LyricsSource::Made { engine, .. }) => Some(engine.as_str()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use jf_core::{AudioConsent, SpeechModelState};

    use super::*;

    fn ai(need: AiLyricsNeed) -> AiLyricsSnapshot {
        AiLyricsSnapshot {
            version: 1,
            available: true,
            plugin: Some("org.jellyfesh.ai-lyrics".into()),
            plan: None,
            need,
            jobs: Vec::new(),
            models: Vec::new(),
            consents: Vec::<AudioConsent>::new(),
        }
    }

    fn job(uid: u64, stage: LyricsJobStage) -> LyricsJob {
        LyricsJob {
            id: uid * 10,
            title: "Paper Boats".into(),
            artist: None,
            uid: Some(uid),
            library_id: Some(4),
            stage,
        }
    }

    #[test]
    fn the_songs_job_comes_first() {
        let mut state = ai(AiLyricsNeed::Nothing);
        state.jobs = vec![
            job(1, LyricsJobStage::Done),
            job(2, LyricsJobStage::Recognising { fraction: 0.42 }),
        ];
        assert_eq!(
            offer(job_for(&state, Some(2)), &state),
            Offer::Running {
                job: 20,
                stage: LyricsJobStage::Recognising { fraction: 0.42 },
                progress: Some(0.42)
            }
        );
        // A finished job leaves the offer to the slice.
        assert_eq!(offer(job_for(&state, Some(1)), &state), Offer::Create);
        state.jobs.push(job(
            3,
            LyricsJobStage::Failed {
                message: "busy".into(),
            },
        ));
        assert_eq!(
            offer(job_for(&state, Some(3)), &state),
            Offer::Failed {
                message: "busy".into()
            }
        );
    }

    #[test]
    fn what_is_needed_first() {
        assert_eq!(offer(None, &ai(AiLyricsNeed::Pro)), Offer::Create);
        assert_eq!(offer(None, &ai(AiLyricsNeed::SignIn)), Offer::SignIn);
        let mut state = ai(AiLyricsNeed::Model {
            id: "base".into(),
            name: "Base".into(),
            bytes: 142_000_000,
        });
        assert!(matches!(
            offer(None, &state),
            Offer::DownloadModel {
                bytes: 142_000_000,
                ..
            }
        ));
        state.models = vec![SpeechModelState {
            id: "base".into(),
            name: "Base".into(),
            bytes: 142_000_000,
            state: SpeechModelStatus::Downloading { fraction: 0.35 },
        }];
        assert!(matches!(offer(None, &state), Offer::DownloadingModel { fraction, .. } if fraction == 0.35));
        assert_eq!(
            offer(None, &ai(AiLyricsNeed::Unavailable { reason: "  ".into() })),
            Offer::None
        );
        let mut off = ai(AiLyricsNeed::Nothing);
        off.available = false;
        assert_eq!(offer(None, &off), Offer::None);
    }

    #[test]
    fn making_asks_for_pro_or_consent_first() {
        assert_eq!(make(&ai(AiLyricsNeed::Pro), SongRef::Current), Step::ProSheet);
        let step = make(
            &ai(AiLyricsNeed::Consent {
                origin: "https://api.example".into(),
            }),
            SongRef::Current,
        );
        assert!(
            matches!(&step, Step::AskConsent { origin, allow } if origin == "https://api.example" && allow.len() == 2)
        );
        assert_eq!(
            make(&ai(AiLyricsNeed::Nothing), SongRef::Library { id: 4 }),
            Step::Send(Command::MakeLyrics {
                song: SongRef::Library { id: 4 }
            })
        );
    }

    #[test]
    fn the_song_menu_offers_it_for_library_songs() {
        let state = ai(AiLyricsNeed::Nothing);
        assert!(offers_for(&state, &TrackRef::Library(3)));
        assert!(!offers_for(&state, &TrackRef::File("/a.flac".into())));
    }
}
