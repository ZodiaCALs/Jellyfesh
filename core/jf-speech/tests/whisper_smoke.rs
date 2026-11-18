//! Recognition with a real model and real speech, through whisper.cpp.
//!
//! Ignored by default; CI runs it with `--features whisper -- --ignored`
//! (the `speech` job: ggml-tiny and a sentence spoken by espeak-ng). Set:
//!
//! * `JF_WHISPER_MODEL`: a ggml model file;
//! * `JF_WHISPER_MODEL_ID`: its catalogue id (default `tiny`);
//! * `JF_WHISPER_WAV`: 16 kHz mono WAV;
//! * `JF_WHISPER_EXPECT`: comma-separated words that must be recognised;
//! * `JF_WHISPER_LANGUAGE`: a language hint (without it, the language is detected).
#![cfg(feature = "whisper")]

use std::env;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use jf_speech::{Options, SpeechError, Transcript};

struct Setup {
    model: &'static jf_speech::ModelInfo,
    path: PathBuf,
    pcm: Vec<f32>,
}

/// The model and audio from the environment, or `None` (the test is skipped).
fn setup() -> Option<Setup> {
    let (Some(path), Some(wav)) = (env::var_os("JF_WHISPER_MODEL"), env::var_os("JF_WHISPER_WAV")) else {
        eprintln!("skipped: set JF_WHISPER_MODEL and JF_WHISPER_WAV to run this test");
        return None;
    };
    let id = env::var("JF_WHISPER_MODEL_ID").unwrap_or_else(|_| "tiny".into());
    let model = jf_speech::model(&id).unwrap_or_else(|| panic!("{id} is not in the catalogue"));
    Some(Setup {
        model,
        path: path.into(),
        pcm: read_wav(Path::new(&wav)),
    })
}

fn read_wav(path: &Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("reading JF_WHISPER_WAV");
    let spec = reader.spec();
    assert_eq!(
        (spec.sample_rate, spec.channels),
        (16_000, 1),
        "JF_WHISPER_WAV must be 16 kHz mono"
    );
    match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let scale = (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 / scale)
                .collect()
        }
    }
}

fn normalised(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[test]
#[ignore = "needs JF_WHISPER_MODEL and JF_WHISPER_WAV"]
fn recognises_and_times_speech() {
    let Some(setup) = setup() else { return };
    let recognizer = jf_speech::recognizer().expect("built with the feature whisper");
    let duration_ms = u32::try_from(setup.pcm.len() * 1000 / 16_000).unwrap();
    let options = Options {
        language: env::var("JF_WHISPER_LANGUAGE").ok(),
        ..Options::default()
    };
    let mut fractions = Vec::new();
    let transcript = recognizer
        .transcribe(
            setup.model,
            &setup.path,
            &setup.pcm,
            &options,
            &mut |f| fractions.push(f),
            &AtomicBool::new(false),
        )
        .expect("recognition");
    eprintln!("{duration_ms} ms of audio: {transcript:#?}");

    let language = transcript.language.as_deref().expect("a language");
    if let Some(hint) = &options.language {
        assert!(
            hint.to_ascii_lowercase().starts_with(language),
            "{hint} → {language}"
        );
    }

    let words: Vec<String> = transcript
        .segments
        .iter()
        .flat_map(|s| &s.words)
        .map(|w| normalised(&w.text))
        .collect();
    let expected = env::var("JF_WHISPER_EXPECT").unwrap_or_default();
    for word in expected.split(',').map(normalised).filter(|w| !w.is_empty()) {
        assert!(words.contains(&word), "{word:?} not recognised in {words:?}");
    }

    check_times(&transcript, duration_ms);
    assert!(fractions.iter().all(|f| (0.0..=1.0).contains(f)), "{fractions:?}");
    assert!(fractions.windows(2).all(|w| w[0] <= w[1]), "{fractions:?}");
    assert_eq!(fractions.last(), Some(&1.0));
}

fn check_times(transcript: &Transcript, duration_ms: u32) {
    assert!(!transcript.segments.is_empty(), "nothing recognised");
    let mut last_start = 0;
    for segment in &transcript.segments {
        assert!(
            segment.start_ms <= segment.end_ms && segment.end_ms <= duration_ms,
            "{segment:?}"
        );
        assert!(segment.start_ms >= last_start, "{segment:?}");
        assert!(!segment.words.is_empty(), "{segment:?}");
        for word in &segment.words {
            assert!(
                word.start_ms >= last_start,
                "{word:?} starts before the word before it"
            );
            assert!(word.start_ms <= word.end_ms, "{word:?}");
            assert!(
                segment.start_ms <= word.start_ms && word.end_ms <= segment.end_ms,
                "{word:?}"
            );
            assert!((0.0..=1.0).contains(&word.confidence), "{word:?}");
            last_start = word.start_ms;
        }
    }
}

#[test]
#[ignore = "needs JF_WHISPER_MODEL and JF_WHISPER_WAV"]
fn cancels() {
    let Some(setup) = setup() else { return };
    let recognizer = jf_speech::recognizer().expect("built with the feature whisper");
    let options = Options {
        language: Some("en".into()),
        ..Options::default()
    };
    let cancel = AtomicBool::new(false);
    let result = recognizer.transcribe(
        setup.model,
        &setup.path,
        &setup.pcm,
        &options,
        &mut |_| cancel.store(true, Ordering::Relaxed),
        &cancel,
    );
    assert_eq!(result, Err(SpeechError::Cancelled));

    // A missing model file.
    let missing = setup.path.with_extension("missing");
    let result = recognizer.transcribe(
        setup.model,
        &missing,
        &setup.pcm,
        &options,
        &mut |_| {},
        &AtomicBool::new(false),
    );
    assert_eq!(result, Err(SpeechError::ModelMissing(setup.model.id.into())));
}
