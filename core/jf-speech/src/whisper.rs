//! Recognition with whisper.cpp, through whisper-rs (feature `whisper`).
//!
//! CPU only. The model loaded last stays loaded for the next recognition,
//! and one recognition runs at a time. whisper.cpp works on a thread of its
//! own while the caller's thread reports progress and watches the cancel
//! flag: whisper-rs's callbacks must be `'static`, so they reach the job
//! through atomics shared with the caller (see [`Whisper::params`]).
//! whisper.cpp 1.8.3 asks the abort callback after each encoder pass and
//! decoder step, so a cancel takes effect within one encoder pass (about a
//! second for Tiny on a computer, several for Large Turbo on a phone).

use std::ffi::c_int;
use std::fmt;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime};

use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext,
    WhisperContextParameters, WhisperError, WhisperState, WhisperTokenId,
};

use crate::words::{self, RawSegment, Token};
use crate::{ModelInfo, Options, Recognizer, SpeechError, Transcript};

/// Samples per second whisper.cpp expects.
const SAMPLE_RATE: usize = 16_000;
/// Length of the audio window Whisper hears at once, in milliseconds.
const WINDOW_MS: usize = 30_000;

/// whisper.cpp's recogniser.
pub(crate) struct Whisper {
    /// The model loaded last. Locked for a whole recognition, which keeps
    /// recognitions one at a time.
    loaded: Mutex<Option<Loaded>>,
    /// What whisper.cpp's callbacks share with the running job.
    shared: Arc<Shared>,
}

struct Loaded {
    path: PathBuf,
    /// Size and modification time when loaded: a model downloaded again is loaded again.
    file: (u64, Option<SystemTime>),
    context: WhisperContext,
}

#[derive(Debug, Default)]
struct Shared {
    /// Read by the abort callback; set when the caller cancels.
    abort: AtomicBool,
    /// Written by the progress callback: whisper.cpp's percentage done.
    percent: AtomicI32,
}

impl fmt::Debug for Whisper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Whisper")
            .field("version", &whisper_rs::WHISPER_CPP_VERSION)
            .finish()
    }
}

impl Whisper {
    pub(crate) fn new() -> Whisper {
        // whisper.cpp's and ggml's log lines go to `tracing` instead of stderr.
        whisper_rs::install_logging_hooks();
        Whisper {
            loaded: Mutex::new(None),
            shared: Arc::default(),
        }
    }

    /// Decoding settings for songs.
    fn params<'a, 'b>(
        &self,
        language: &'a str,
        prompt: &'b [WhisperTokenId],
        threads: usize,
    ) -> FullParams<'a, 'b> {
        // Greedy decoding at temperature 0, and 5 sampled candidates when it falls
        // back to a higher temperature: whisper.cpp's and OpenAI's defaults. Beam
        // search would cost several times the decoding for little gain on singing.
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });
        params.set_n_threads(c_int::try_from(threads).unwrap_or(4));
        params.set_language(Some(language));
        params.set_translate(false);
        // Nothing carries over from an earlier recognition.
        params.set_no_context(true);
        // No blank first token, and no non-speech symbols ("♪", "[Music]",
        // brackets): instrumental passages yield no text instead of symbols.
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        // Temperature fallback: a window whose text repeats itself (entropy below
        // 2.4) or is improbable (mean log probability below -1) is decoded again
        // at 0.2, 0.4, … 1.0. Repetition loops are the usual failure on music.
        // From 0.5 on, whisper.cpp also stops conditioning on the text before,
        // which breaks a loop that the context keeps alive. A window that is
        // likely silence (no-speech probability over 0.6 with a low log
        // probability) gives no text.
        params.set_temperature(0.0);
        params.set_temperature_inc(0.2);
        params.set_entropy_thold(2.4);
        params.set_logprob_thold(-1.0);
        params.set_no_speech_thold(0.6);
        // Timestamp-token estimates for each token (t0/t1), besides DTW's.
        params.set_token_timestamps(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        if !prompt.is_empty() {
            params.set_tokens(prompt);
        }
        let shared = Arc::clone(&self.shared);
        params
            .set_progress_callback_safe(move |percent: i32| shared.percent.store(percent, Ordering::Relaxed));
        // whisper-rs 0.16 calls the abort closure as the closure's own type but
        // stores it as a `Box<dyn FnMut() -> bool>`; passing that boxed type
        // makes the two the same. Like the progress closure, it is never freed:
        // a few bytes per recognition.
        let shared = Arc::clone(&self.shared);
        let abort: Box<dyn FnMut() -> bool> = Box::new(move || shared.abort.load(Ordering::Relaxed));
        params.set_abort_callback_safe::<_, Box<dyn FnMut() -> bool>>(abort);
        params
    }
}

impl Recognizer for Whisper {
    fn transcribe(
        &self,
        model: &ModelInfo,
        model_path: &Path,
        pcm: &[f32],
        options: &Options,
        progress: &mut dyn FnMut(f32),
        cancel: &AtomicBool,
    ) -> Result<Transcript, SpeechError> {
        let mut loaded = self.loaded.lock().unwrap_or_else(PoisonError::into_inner);
        if cancel.load(Ordering::Relaxed) {
            return Err(SpeechError::Cancelled);
        }
        let context = load(&mut loaded, model, model_path)?;
        progress(0.0);
        if c_int::try_from(pcm.len()).is_err() {
            return Err(SpeechError::Failed("the audio is too long".into()));
        }
        let languages = language_count(context);
        let hint = options
            .language
            .as_deref()
            .and_then(|tag| whisper_language(tag, languages));
        // whisper.cpp hears nothing in less than 100 ms.
        if pcm.len() < SAMPLE_RATE / 10 {
            return Ok(Transcript {
                language: hint.map(str::to_owned),
                segments: Vec::new(),
            });
        }
        let threads = options.threads.filter(|&n| n > 0).unwrap_or_else(default_threads);
        let mut state = context.create_state().map_err(failed)?;
        let language = match hint {
            Some(language) => language,
            None => detect_language(&mut state, pcm, threads, languages, cancel)?,
        };
        let prompt = options
            .prompt
            .as_deref()
            .map(|text| prompt_tokens(context, text))
            .unwrap_or_default();

        self.shared.abort.store(false, Ordering::Relaxed);
        self.shared.percent.store(0, Ordering::Relaxed);
        let params = self.params(language, &prompt, threads);
        let shared = &*self.shared;
        let outcome = thread::scope(|scope| {
            let caller = thread::current();
            let worker = scope.spawn(move || {
                let result = state.full(params, pcm);
                caller.unpark();
                (state, result)
            });
            let mut reported = 0;
            while !worker.is_finished() {
                if cancel.load(Ordering::Relaxed) {
                    shared.abort.store(true, Ordering::Relaxed);
                }
                let percent = shared.percent.load(Ordering::Relaxed).clamp(0, 100);
                if percent > reported {
                    reported = percent;
                    progress(percent as f32 / 100.0);
                }
                thread::park_timeout(Duration::from_millis(100));
            }
            worker.join()
        });
        let Ok((state, result)) = outcome else {
            return Err(SpeechError::Failed("whisper.cpp stopped unexpectedly".into()));
        };
        if cancel.load(Ordering::Relaxed) {
            return Err(SpeechError::Cancelled);
        }
        result.map_err(failed)?;
        let duration_ms = u32::try_from(pcm.len() * 1000 / SAMPLE_RATE).unwrap_or(u32::MAX);
        let segments = collect(&state, context, duration_ms);
        let transcript = Transcript {
            language: Some(language.to_owned()),
            segments,
        };
        progress(1.0);
        Ok(transcript)
    }
}

/// The model at `path`, loaded unless it is the one loaded last.
fn load<'a>(
    slot: &'a mut Option<Loaded>,
    model: &ModelInfo,
    path: &Path,
) -> Result<&'a WhisperContext, SpeechError> {
    let meta = fs::metadata(path).map_err(|_| SpeechError::ModelMissing(model.id.to_owned()))?;
    let file = (meta.len(), meta.modified().ok());
    if let Some(loaded) = slot.take_if(|loaded| loaded.path != path || loaded.file != file) {
        // Free the old model before loading the next.
        drop(loaded);
    }
    if slot.is_none() {
        let mut params = WhisperContextParameters::default();
        // DTW needs flash attention off.
        params
            .use_gpu(false)
            .flash_attn(false)
            .dtw_parameters(DtwParameters {
                mode: dtw_preset(model.id).map_or(DtwMode::None, |model_preset| DtwMode::ModelPreset {
                    model_preset,
                }),
                ..DtwParameters::default()
            });
        let context = WhisperContext::new_with_params(path, params)
            .map_err(|e| SpeechError::Failed(format!("loading the {} model: {e}", model.name)))?;
        tracing::info!(
            "loaded speech model {} (whisper.cpp {})",
            model.id,
            whisper_rs::WHISPER_CPP_VERSION
        );
        *slot = Some(Loaded {
            path: path.to_owned(),
            file,
            context,
        });
    }
    match slot {
        Some(loaded) => Ok(&loaded.context),
        None => Err(SpeechError::Failed("the model did not load".into())),
    }
}

/// The alignment heads whisper.cpp has for a model, by whisper.cpp's model
/// name (a quantised model has its original's).
fn dtw_preset(id: &str) -> Option<DtwModelPreset> {
    let name = id.split_once("-q").map_or(id, |(name, _)| name);
    Some(match name {
        "tiny" => DtwModelPreset::Tiny,
        "tiny.en" => DtwModelPreset::TinyEn,
        "base" => DtwModelPreset::Base,
        "base.en" => DtwModelPreset::BaseEn,
        "small" => DtwModelPreset::Small,
        "small.en" => DtwModelPreset::SmallEn,
        "medium" => DtwModelPreset::Medium,
        "medium.en" => DtwModelPreset::MediumEn,
        "large-v1" => DtwModelPreset::LargeV1,
        "large-v2" => DtwModelPreset::LargeV2,
        "large-v3" => DtwModelPreset::LargeV3,
        "large-v3-turbo" => DtwModelPreset::LargeV3Turbo,
        _ => return None,
    })
}

/// `min(4, cores − 1)`, at least 1.
fn default_threads() -> usize {
    let cores = thread::available_parallelism().map_or(1, NonZeroUsize::get);
    cores.saturating_sub(1).clamp(1, 4)
}

/// How many languages the model knows: 99, 100 from large-v3 on (Cantonese),
/// 0 for English-only models.
fn language_count(context: &WhisperContext) -> i32 {
    if context.is_multilingual() {
        context.n_vocab() - 51765 - 1
    } else {
        0
    }
}

/// Whisper's code for a BCP 47 tag ("zh-Hant" → "zh"), when the model knows the language.
fn whisper_language(tag: &str, languages: i32) -> Option<&'static str> {
    if languages == 0 {
        return Some("en");
    }
    let primary = tag.trim().split(['-', '_']).next()?.to_ascii_lowercase();
    if !(2..=3).contains(&primary.len()) || !primary.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    let code = match primary.as_str() {
        "und" | "mul" | "zxx" => return None,
        // Whisper's own codes, and deprecated ISO 639 codes.
        "jv" => "jw",
        "nb" => "no",
        "fil" => "tl",
        "cmn" => "zh",
        "iw" => "he",
        "in" => "id",
        "ji" => "yi",
        code => code,
    };
    let id = whisper_rs::get_lang_id(code)?;
    if id < languages {
        whisper_rs::get_lang_str(id)
    } else if code == "yue" {
        // Before large-v3, Cantonese is recognised as Chinese.
        Some("zh")
    } else {
        None
    }
}

/// Detects the language over up to three windows across the song: songs often
/// open without singing, and whisper.cpp alone listens to the first 30 s.
fn detect_language(
    state: &mut WhisperState,
    pcm: &[f32],
    threads: usize,
    languages: i32,
    cancel: &AtomicBool,
) -> Result<&'static str, SpeechError> {
    if languages == 0 {
        return Ok("en");
    }
    state.pcm_to_mel(pcm, threads).map_err(failed)?;
    let duration_ms = pcm.len() * 1000 / SAMPLE_RATE;
    let mut total = vec![0.0f32; usize::try_from(languages).unwrap_or(0)];
    for offset in detection_offsets(duration_ms) {
        if cancel.load(Ordering::Relaxed) {
            return Err(SpeechError::Cancelled);
        }
        let (_, probabilities) = state.lang_detect(offset, threads).map_err(failed)?;
        for (sum, p) in total.iter_mut().zip(probabilities) {
            *sum += p;
        }
    }
    let best = total
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(id, _)| id);
    best.and_then(|id| whisper_rs::get_lang_str(i32::try_from(id).ok()?))
        .ok_or_else(|| SpeechError::Failed("no language detected".into()))
}

/// Starts of the 30 s windows centred at a quarter, half and three quarters
/// of the audio; the start alone for 30 s or less.
fn detection_offsets(duration_ms: usize) -> Vec<usize> {
    let last = duration_ms.saturating_sub(WINDOW_MS);
    let mut offsets: Vec<usize> = [1, 2, 3]
        .iter()
        .map(|quarter| {
            (duration_ms * quarter / 4)
                .saturating_sub(WINDOW_MS / 2)
                .min(last)
        })
        .collect();
    offsets.dedup();
    offsets
}

/// The prompt's first tokens. whisper.cpp reads at most half its text context
/// (223 tokens) and would keep the last; the start of a song's lyrics is what
/// its first window needs. Nothing for a prompt it cannot tokenise.
fn prompt_tokens(context: &WhisperContext, text: &str) -> Vec<WhisperTokenId> {
    let text: String = text.trim().chars().filter(|&c| c != '\0').collect();
    if text.is_empty() {
        return Vec::new();
    }
    let limit = usize::try_from(context.n_text_ctx() / 2 - 1).unwrap_or(0);
    // Every token stands for at least one byte, so this many always fit (whisper-rs
    // must not be given fewer than the text needs).
    match context.tokenize(&text, text.len() + 1) {
        Ok(mut tokens) => {
            tokens.truncate(limit);
            tokens
        }
        Err(e) => {
            tracing::debug!("the speech prompt could not be tokenised: {e}");
            Vec::new()
        }
    }
}

/// The recognised segments and their words, within the audio's duration.
fn collect(state: &WhisperState, context: &WhisperContext, duration_ms: u32) -> Vec<crate::Segment> {
    let eot = context.token_eot();
    let raw: Vec<RawSegment> = state
        .as_iter()
        .map(|segment| {
            let tokens = (0..segment.n_tokens())
                .filter_map(|i| segment.get_token(i))
                .filter_map(|token| {
                    let data = token.token_data();
                    // Special tokens (timestamps, language, end of text) come after
                    // the text tokens in the vocabulary.
                    if data.id >= eot {
                        return None;
                    }
                    let bytes = token.to_bytes().ok()?.to_vec();
                    Some(Token {
                        bytes,
                        p: data.p,
                        t_dtw: data.t_dtw,
                        t0: data.t0,
                        t1: data.t1,
                    })
                })
                .collect();
            RawSegment {
                text: segment.to_str_lossy().map(|t| t.into_owned()).unwrap_or_default(),
                t0: segment.start_timestamp(),
                t1: segment.end_timestamp(),
                tokens,
            }
        })
        .collect();
    words::segments(&raw, duration_ms)
}

fn failed(e: WhisperError) -> SpeechError {
    SpeechError::Failed(format!("whisper.cpp: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_for_the_catalogue() {
        let preset = |id| dtw_preset(id).map(|p| format!("{p:?}"));
        assert_eq!(preset("tiny").as_deref(), Some("Tiny"));
        assert_eq!(preset("base").as_deref(), Some("Base"));
        assert_eq!(preset("small").as_deref(), Some("Small"));
        assert_eq!(preset("large-v3-turbo-q5_0").as_deref(), Some("LargeV3Turbo"));
        assert_eq!(preset("base.en-q5_1").as_deref(), Some("BaseEn"));
        assert_eq!(preset("distil-large-v3"), None);
        for model in crate::catalogue() {
            assert!(dtw_preset(model.id).is_some(), "{}", model.id);
        }
    }

    #[test]
    fn languages_from_bcp47() {
        assert_eq!(whisper_language("en", 99), Some("en"));
        assert_eq!(whisper_language("zh-Hant-TW", 99), Some("zh"));
        assert_eq!(whisper_language("ja_JP", 99), Some("ja"));
        assert_eq!(whisper_language(" PT-br ", 99), Some("pt"));
        assert_eq!(whisper_language("jv", 99), Some("jw"));
        assert_eq!(whisper_language("nb-NO", 99), Some("no"));
        assert_eq!(whisper_language("yue", 100), Some("yue"));
        assert_eq!(whisper_language("yue-HK", 99), Some("zh"));
        assert_eq!(whisper_language("haw", 99), Some("haw"));
        for unknown in ["", "auto", "und", "x-klingon", "tlh", "e\0n", "english"] {
            assert_eq!(whisper_language(unknown, 99), None, "{unknown:?}");
        }
        // English-only models hear English.
        assert_eq!(whisper_language("fr", 0), Some("en"));
    }

    #[test]
    fn detection_windows() {
        assert_eq!(detection_offsets(10_000), [0]);
        assert_eq!(detection_offsets(30_000), [0]);
        assert_eq!(detection_offsets(60_000), [0, 15_000, 30_000]);
        assert_eq!(detection_offsets(240_000), [45_000, 105_000, 165_000]);
    }

    #[test]
    fn thread_count() {
        assert!((1..=4).contains(&default_threads()));
    }
}
