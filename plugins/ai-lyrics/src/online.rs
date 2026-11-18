//! The online mode: OpenAI's speech-to-text API as OpenAI, Groq and
//! compatible servers offer it.
//!
//! `POST <base>/audio/transcriptions` with a `multipart/form-data` body:
//! `file` (16 kHz mono WAV), `model`, `response_format=verbose_json`,
//! `timestamp_granularities[]=word` and `=segment`, `temperature=0`, and
//! `language` and `prompt` when there are any; the key as a bearer token.
//! The answer's `words` and `segments` carry times in seconds. Facts as
//! ADR-0019 records them (checked 2026-11-17): OpenAI gives word times only
//! for `whisper-1`, Groq for its Whisper models; uploads are limited to
//! 25 MB.
//!
//! A song is sent in parts of at most eight minutes (shorter where the
//! provider needs longer to answer: [`crate::settings::Provider::part_ms`]),
//! each overlapping the next by two seconds; where two parts overlap, each
//! keeps the words that start in its half of the overlap.

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;

use crate::model::{Segment, Transcript, Word};
use crate::settings::{Provider, Settings, host};

/// How much consecutive parts share.
pub const OVERLAP_MS: u64 = 2000;
/// How long one request may take: the host's limit.
const TIMEOUT_MS: u32 = 30_000;
/// The longest error text kept from a provider.
const MAX_MESSAGE: usize = 300;

/// The shortest last part: a few seconds alone make Whisper guess.
const MIN_LAST_MS: u64 = 10_000;

/// The parts of a song of `duration_ms` sent one by one: (start, length),
/// each at most `part_ms`, consecutive ones sharing [`OVERLAP_MS`] (the
/// last one more when it would otherwise be shorter than ten seconds).
pub fn parts(duration_ms: u64, part_ms: u64) -> Vec<(u64, u64)> {
    let part_ms = part_ms.clamp(MIN_LAST_MS.max(OVERLAP_MS * 2), jf_plugin_sdk::audio::MAX_WAV_MS);
    if duration_ms <= part_ms {
        return vec![(0, duration_ms.max(1))];
    }
    let step = part_ms - OVERLAP_MS;
    let mut out: Vec<(u64, u64)> = Vec::new();
    let mut start = 0;
    loop {
        if start + part_ms >= duration_ms {
            let after_last = out.last().map_or(0, |(s, _)| s + 1);
            let start = if duration_ms - start < MIN_LAST_MS {
                duration_ms.saturating_sub(MIN_LAST_MS).max(after_last)
            } else {
                start
            };
            out.push((start, duration_ms - start));
            break;
        }
        out.push((start, part_ms));
        start += step;
    }
    out
}

/// The window of each part whose words are kept: from the middle of its
/// overlap with the part before to the middle of its overlap with the next.
pub fn windows(parts: &[(u64, u64)]) -> Vec<(u64, u64)> {
    (0..parts.len())
        .map(|i| {
            let from = if i == 0 {
                0
            } else {
                let (start, _) = parts[i];
                let (before, length) = parts[i - 1];
                (start + before + length) / 2
            };
            let to = match parts.get(i + 1) {
                Some((next, _)) => (parts[i].0 + parts[i].1 + next) / 2,
                None => u64::MAX,
            };
            (from, to)
        })
        .collect()
}

/// A word in an answer.
#[derive(Debug, Clone, Deserialize)]
struct ApiWord {
    #[serde(default, alias = "text")]
    word: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
}

/// A segment in an answer.
#[derive(Debug, Clone, Deserialize)]
struct ApiSegment {
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    text: String,
    #[serde(default)]
    words: Option<Vec<ApiWord>>,
    #[serde(default)]
    no_speech_prob: Option<f64>,
    #[serde(default)]
    avg_logprob: Option<f64>,
}

/// A `verbose_json` answer.
#[derive(Debug, Clone, Deserialize)]
struct Answer {
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    words: Option<Vec<ApiWord>>,
    #[serde(default)]
    segments: Option<Vec<ApiSegment>>,
}

fn ms(seconds: f64, offset: u64) -> u32 {
    let ms = if seconds.is_finite() && seconds > 0.0 {
        (seconds * 1000.0).round() as u64
    } else {
        0
    };
    u32::try_from(offset + ms).unwrap_or(u32::MAX)
}

fn word(w: &ApiWord, offset: u64) -> Word {
    let start_ms = ms(w.start, offset);
    Word {
        text: w.word.trim().to_owned(),
        start_ms,
        end_ms: ms(w.end, offset).max(start_ms),
    }
}

/// What one answer says, with times from the song's start (`offset`).
/// Words the server sent on their own go to the segment they start in (or
/// make one segment without segments); segments Whisper itself would call
/// silence are dropped. `None` when the answer has no times at all.
fn transcript(answer: Answer, offset: u64) -> Option<Transcript> {
    let segments: Vec<ApiSegment> = answer
        .segments
        .unwrap_or_default()
        .into_iter()
        // Whisper's own rule for silence: likely no speech, and unsure.
        .filter(|s| !(s.no_speech_prob.unwrap_or(0.0) > 0.6 && s.avg_logprob.unwrap_or(0.0) < -1.0))
        .collect();
    let loose: Vec<Word> = answer
        .words
        .unwrap_or_default()
        .iter()
        .map(|w| word(w, offset))
        .filter(|w| !w.text.is_empty())
        .collect();
    let mut out: Vec<Segment> = segments
        .iter()
        .map(|s| Segment {
            text: s.text.trim().to_owned(),
            start_ms: ms(s.start, offset),
            end_ms: ms(s.end, offset),
            words: s
                .words
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|w| word(w, offset))
                .filter(|w| !w.text.is_empty())
                .collect(),
        })
        .collect();
    let segment_words = out.iter().any(|s| !s.words.is_empty());
    if !loose.is_empty() && !segment_words {
        if out.is_empty() {
            out.push(Segment {
                text: String::new(),
                start_ms: loose[0].start_ms,
                end_ms: loose.last().map_or(0, |w| w.end_ms),
                words: loose,
            });
        } else {
            let starts: Vec<u32> = out.iter().map(|s| s.start_ms).collect();
            for w in loose {
                let at = starts.partition_point(|s| *s <= w.start_ms).saturating_sub(1);
                out[at].words.push(w);
            }
        }
    }
    if out.is_empty() && answer.text.is_some_and(|t| !t.trim().is_empty()) {
        return None;
    }
    Some(Transcript {
        language: answer.language.as_deref().and_then(language_code),
        segments: out,
    })
}

/// A language as OpenAI-compatible answers name it ("english", or a code)
/// as a BCP 47 tag.
pub fn language_code(name: &str) -> Option<String> {
    let name = name.trim().to_ascii_lowercase();
    if (2..=3).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_lowercase()) {
        return Some(name);
    }
    const NAMES: [(&str, &str); 32] = [
        ("english", "en"),
        ("chinese", "zh"),
        ("japanese", "ja"),
        ("korean", "ko"),
        ("spanish", "es"),
        ("french", "fr"),
        ("german", "de"),
        ("italian", "it"),
        ("portuguese", "pt"),
        ("russian", "ru"),
        ("ukrainian", "uk"),
        ("polish", "pl"),
        ("dutch", "nl"),
        ("swedish", "sv"),
        ("norwegian", "no"),
        ("danish", "da"),
        ("finnish", "fi"),
        ("czech", "cs"),
        ("hungarian", "hu"),
        ("romanian", "ro"),
        ("greek", "el"),
        ("turkish", "tr"),
        ("arabic", "ar"),
        ("hebrew", "he"),
        ("hindi", "hi"),
        ("thai", "th"),
        ("vietnamese", "vi"),
        ("indonesian", "id"),
        ("malay", "ms"),
        ("tagalog", "tl"),
        ("cantonese", "yue"),
        ("catalan", "ca"),
    ];
    NAMES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, code)| (*code).to_owned())
}

/// The parts' transcripts as one: each part keeps the words that start in
/// its window; segments left without words go.
pub fn merge(parts: Vec<(Transcript, (u64, u64))>) -> Transcript {
    let mut language = None;
    let mut segments = Vec::new();
    for (transcript, (from, to)) in parts {
        language = language.or(transcript.language);
        for segment in transcript.segments {
            let words: Vec<Word> = crate::lines::segment_words(&segment)
                .into_iter()
                .filter(|w| (from..to).contains(&u64::from(w.start_ms)))
                .collect();
            if let (Some(first), Some(last)) = (words.first(), words.last()) {
                segments.push(Segment {
                    text: segment.text.clone(),
                    start_ms: first.start_ms,
                    end_ms: last.end_ms,
                    words,
                });
            }
        }
    }
    segments.sort_by_key(|s| s.start_ms);
    Transcript { language, segments }
}

/// A form field of a multipart body.
pub enum Field<'a> {
    Text(&'a str, &'a str),
    File {
        name: &'a str,
        file_name: &'a str,
        content_type: &'a str,
        data: &'a [u8],
    },
}

/// A `multipart/form-data` body with `boundary`.
pub fn multipart(boundary: &str, fields: &[Field<'_>]) -> Vec<u8> {
    let mut body = Vec::new();
    for field in fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match field {
            Field::Text(name, value) => {
                body.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes(),
                );
            }
            Field::File {
                name,
                file_name,
                content_type,
                data,
            } => {
                body.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{name}\"; filename=\"{file_name}\"\r\n\
                         Content-Type: {content_type}\r\n\r\n"
                    )
                    .as_bytes(),
                );
                body.extend_from_slice(data);
                body.extend_from_slice(b"\r\n");
            }
        }
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// The provider's own words about an error: `error.message` (OpenAI,
/// Groq), `detail` (many servers) or the text.
fn message(body: &[u8]) -> Option<String> {
    let value: Option<serde_json::Value> = serde_json::from_slice(body).ok();
    let text = match &value {
        Some(value) => value
            .pointer("/error/message")
            .or_else(|| value.get("detail"))
            .or_else(|| value.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        None => Some(String::from_utf8_lossy(body).into_owned()),
    }?;
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() || text.starts_with('<') {
        return None;
    }
    Some(match text.char_indices().nth(MAX_MESSAGE) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text,
    })
}

/// An answer that is not a transcript, as an error for people.
pub fn failure(status: u16, body: &[u8], provider: Provider, host: &str) -> PluginError {
    let said = message(body);
    let with = |text: String| match &said {
        Some(said) => format!("{text} ({host} says: {said})"),
        None => text,
    };
    let code = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/error/code")
                .and_then(|c| c.as_str().map(str::to_owned))
        });
    match status {
        401 => PluginError::AuthRequired,
        403 => PluginError::service(with(format!("{host} refused the request (HTTP 403)."))),
        404 if provider == Provider::Custom => PluginError::service(with(format!(
            "{host} has no speech-to-text API at this address. Check the Server Address in AI Lyrics' settings."
        ))),
        404 => PluginError::service(with(format!("{host} does not offer this model."))),
        413 => PluginError::service(with(format!(
            "{host} found a part of the song too large to take."
        ))),
        429 if code.as_deref() == Some("insufficient_quota") => PluginError::service(with(format!(
            "Your {} account has no credit left for this.",
            provider.name()
        ))),
        429 => PluginError::RateLimited,
        400..=499 => PluginError::service(with(format!(
            "{host} did not accept the request (HTTP {status})."
        ))),
        _ => PluginError::service(with(format!(
            "{host} had a problem (HTTP {status}). Try again later."
        ))),
    }
}

/// Reads a successful answer.
pub fn read(body: &[u8], offset: u64, host: &str, model: &str) -> Result<Transcript> {
    let answer: Answer = serde_json::from_slice(body)
        .map_err(|e| PluginError::service(format!("{host} sent an answer AI Lyrics cannot read: {e}")))?;
    transcript(answer, offset).ok_or_else(|| {
        PluginError::service(format!(
            "{model} at {host} gave no times. Choose a model with word timing, such as whisper-1 at OpenAI."
        ))
    })
}

/// A request's error for people.
fn network(error: http::HttpError, host: &str) -> PluginError {
    match error {
        http::HttpError::Timeout => PluginError::Network(format!(
            "{host} did not answer within 30 seconds. Try again, or a faster provider or model."
        )),
        other => PluginError::from(other),
    }
}

/// Recognises the song of `track` with the provider: its parts one by one.
/// `key` is the provider's API key (none for a server that takes requests
/// without one).
pub fn transcribe(
    track: TrackHandle,
    settings: &Settings,
    key: Option<&str>,
    language: Option<&str>,
    prompt: Option<&str>,
) -> Result<Transcript> {
    let base = settings.base().ok_or_else(|| {
        PluginError::service("Enter your server's address (Server Address) in AI Lyrics' settings.")
    })?;
    let host = host(&base).unwrap_or_default();
    let model = settings.online_model();
    // Online providers take ISO 639-1 codes.
    let language = language.filter(|l| l.len() == 2);
    let boundary = format!(
        "jellyfesh-{}",
        crypto::random_bytes(12)?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let duration = audio::duration_ms(track)?;
    let parts = parts(duration, settings.provider.part_ms());
    let windows = windows(&parts);
    let mut answers = Vec::with_capacity(parts.len());
    for (&(start, length), window) in parts.iter().zip(windows) {
        let wav = audio::wav(track, start, length)?;
        let mut fields = vec![
            Field::File {
                name: "file",
                file_name: "song.wav",
                content_type: "audio/wav",
                data: &wav,
            },
            Field::Text("model", &model),
            Field::Text("response_format", "verbose_json"),
            Field::Text("timestamp_granularities[]", "word"),
            Field::Text("timestamp_granularities[]", "segment"),
            Field::Text("temperature", "0"),
        ];
        if let Some(language) = language {
            fields.push(Field::Text("language", language));
        }
        if let Some(prompt) = prompt {
            fields.push(Field::Text("prompt", prompt));
        }
        let body = multipart(&boundary, &fields);
        drop(fields);
        drop(wav);
        let mut request = http::Request::post(format!("{base}/audio/transcriptions"))
            .body(body, &format!("multipart/form-data; boundary={boundary}"))
            .header("accept", "application/json")
            .timeout_ms(TIMEOUT_MS);
        if let Some(key) = key {
            request = request.bearer(key);
        }
        let response = request.send().map_err(|e| network(e, &host))?;
        if !response.is_success() {
            return Err(failure(response.status, &response.body, settings.provider, &host));
        }
        answers.push((read(&response.body, start, &host, &model)?, window));
    }
    Ok(merge(answers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_split_into_overlapping_parts() {
        assert_eq!(parts(200_000, 480_000), [(0, 200_000)]);
        assert_eq!(parts(0, 480_000), [(0, 1)]);
        assert_eq!(
            parts(1_000_000, 480_000),
            [(0, 480_000), (478_000, 480_000), (956_000, 44_000)]
        );
        assert_eq!(
            parts(500_000, 180_000),
            [(0, 180_000), (178_000, 180_000), (356_000, 144_000)]
        );
        assert_eq!(
            parts(360_001, 180_000),
            [(0, 180_000), (178_000, 180_000), (350_001, 10_000)],
            "a last part of four seconds starts earlier"
        );
        for (duration, part) in [(1_000_000, 480_000), (7_777_777, 120_000), (360_001, 180_000)] {
            let list = parts(duration, part);
            assert!(list.iter().all(|(_, l)| *l <= part));
            assert_eq!(list.last().map(|(s, l)| s + l), Some(duration), "{list:?}");
            for pair in list.windows(2) {
                assert!(pair[1].0 <= pair[0].0 + pair[0].1 - OVERLAP_MS, "{pair:?}");
            }
        }
        let w = windows(&parts(1_000_000, 480_000));
        assert_eq!(w, [(0, 479_000), (479_000, 957_000), (957_000, u64::MAX)]);
    }

    #[test]
    fn answers_with_words_segments_or_both() {
        let both: Answer = serde_json::from_str(
            r#"{"task":"transcribe","language":"english","duration":4.5,"text":"Hello there. Again",
                "words":[{"word":"Hello","start":0.5,"end":0.9},{"word":"there.","start":1.0,"end":1.4},
                         {"word":"Again","start":3.0,"end":3.5}],
                "segments":[{"id":0,"start":0.4,"end":1.5,"text":" Hello there.","no_speech_prob":0.01,"avg_logprob":-0.2},
                            {"id":1,"start":2.9,"end":3.6,"text":" Again","no_speech_prob":0.02,"avg_logprob":-0.3}]}"#,
        )
        .unwrap();
        let t = transcript(both, 10_000).unwrap();
        assert_eq!(t.language.as_deref(), Some("en"));
        assert_eq!(t.segments.len(), 2);
        assert_eq!(t.segments[0].words.len(), 2);
        assert_eq!(t.segments[0].words[0].start_ms, 10_500);
        assert_eq!(t.segments[1].words[0].text, "Again");

        let segments_only: Answer = serde_json::from_str(
            r#"{"language":"ja","segments":[{"start":1.0,"end":2.0,"text":"こんにちは"},
               {"start":30.0,"end":31.0,"text":"Thanks for watching","no_speech_prob":0.9,"avg_logprob":-1.5}]}"#,
        )
        .unwrap();
        let t = transcript(segments_only, 0).unwrap();
        assert_eq!(t.segments.len(), 1, "Whisper's silence rule drops the second");
        assert!(t.segments[0].words.is_empty());

        let text_only: Answer = serde_json::from_str(r#"{"text":"Words without times"}"#).unwrap();
        assert_eq!(transcript(text_only, 0), None);
        let nothing: Answer = serde_json::from_str(r#"{"text":""}"#).unwrap();
        assert_eq!(transcript(nothing, 0).map(|t| t.segments.len()), Some(0));
    }

    #[test]
    fn overlapping_words_are_kept_once() {
        let part = |words: &[(&str, u32)]| Transcript {
            language: Some("en".into()),
            segments: vec![Segment {
                text: String::new(),
                start_ms: words[0].1,
                end_ms: words.last().unwrap().1 + 300,
                words: words
                    .iter()
                    .map(|(t, s)| Word {
                        text: (*t).into(),
                        start_ms: *s,
                        end_ms: s + 300,
                    })
                    .collect(),
            }],
        };
        let list = parts(300_000, 180_000);
        let windows = windows(&list);
        let merged = merge(vec![
            (
                part(&[("one", 170_000), ("two", 178_500), ("three", 179_500)]),
                windows[0],
            ),
            (
                part(&[("two", 178_520), ("three", 179_520), ("four", 181_000)]),
                windows[1],
            ),
        ]);
        let words: Vec<(String, u32)> = merged
            .segments
            .iter()
            .flat_map(|s| s.words.iter().map(|w| (w.text.clone(), w.start_ms)))
            .collect();
        assert_eq!(
            words,
            [
                ("one".to_string(), 170_000),
                ("two".into(), 178_500),
                ("three".into(), 179_520),
                ("four".into(), 181_000)
            ]
        );
    }

    #[test]
    fn multipart_bodies() {
        let body = multipart(
            "b0",
            &[
                Field::Text("model", "whisper-1"),
                Field::File {
                    name: "file",
                    file_name: "song.wav",
                    content_type: "audio/wav",
                    data: b"RIFF",
                },
            ],
        );
        assert_eq!(
            String::from_utf8(body).unwrap(),
            "--b0\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\nwhisper-1\r\n\
             --b0\r\nContent-Disposition: form-data; name=\"file\"; filename=\"song.wav\"\r\n\
             Content-Type: audio/wav\r\n\r\nRIFF\r\n--b0--\r\n"
        );
    }

    #[test]
    fn errors_in_the_providers_words() {
        let body = br#"{"error":{"message":"Incorrect API key provided: sk-...abcd.","type":"invalid_request_error","code":"invalid_api_key"}}"#;
        assert_eq!(
            failure(401, body, Provider::OpenAi, "api.openai.com"),
            PluginError::AuthRequired
        );
        let quota =
            br#"{"error":{"message":"You exceeded your current quota.","code":"insufficient_quota"}}"#;
        assert!(matches!(
            failure(429, quota, Provider::OpenAi, "api.openai.com"),
            PluginError::Service(t) if t.contains("no credit") && t.contains("exceeded")
        ));
        assert_eq!(
            failure(429, b"{}", Provider::Groq, "api.groq.com"),
            PluginError::RateLimited
        );
        assert!(matches!(
            failure(404, br#"{"detail":"Not Found"}"#, Provider::Custom, "10.0.0.2:8000"),
            PluginError::Service(t) if t.contains("Server Address") && t.contains("Not Found")
        ));
        assert!(matches!(
            failure(502, b"<html>bad gateway</html>", Provider::Groq, "api.groq.com"),
            PluginError::Service(t) if t == "api.groq.com had a problem (HTTP 502). Try again later."
        ));
        assert_eq!(language_code("English").as_deref(), Some("en"));
        assert_eq!(language_code("ja").as_deref(), Some("ja"));
        assert_eq!(language_code("klingon"), None);
    }
}
