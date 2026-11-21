//! NetEase lyrics to a payload.
//!
//! The lyric answer has the lyrics as LRC (`lrc.lyric`) and, when someone
//! translated them, the translation as LRC with the same time stamps
//! (`tlyric.lyric`). Credits at the top may come as JSON lines
//! (`{"t":0,"c":[{"tx":"Lyrics: "},{"tx":"Someone"}]}`), which become plain
//! timed lines here. When the answer also has word-timed lyrics (`yrc`,
//! with its own translation `ytlrc`), they win: [`crate::yrc`] turns them
//! into enhanced LRC, which the apps show word by word.

use jf_community_common::lrc;
use jf_plugin_sdk::prelude::*;
use serde::Deserialize;

use crate::api;
use crate::model::{LyricText, Lyrics};
use crate::yrc;

/// What Jellyfesh shows as the source.
pub const ATTRIBUTION: &str = "Lyrics from NetEase Cloud Music";

/// One JSON credit line.
#[derive(Debug, Deserialize)]
struct CreditLine {
    /// Start in ms.
    t: u64,
    /// Text pieces.
    #[serde(default)]
    c: Vec<CreditPiece>,
}

#[derive(Debug, Deserialize)]
struct CreditPiece {
    #[serde(default)]
    tx: String,
}

/// Turns JSON credit lines into LRC lines; other lines are kept as they are.
pub fn credits_to_lrc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('{')
            && let Ok(credit) = serde_json::from_str::<CreditLine>(trimmed)
        {
            let words: String = credit.c.iter().map(|p| p.tx.as_str()).collect();
            out.push_str(&format!("[{}]{}\n", lrc::format_time(credit.t), words.trim()));
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The lyric answer for song `id`: `song/lyric/v1`, which can add the
/// word-timed lyrics (`yv`, `ytv`), else the older `song/lyric` when that
/// fails (the web interface's support for the newer endpoint is not
/// confirmed). The sign-in, the rate limit and the network are not retried.
pub fn fetch(id: u64) -> Result<Lyrics> {
    let newer = api::call_as::<Lyrics>(
        "/song/lyric/v1",
        serde_json::json!({ "id": id, "cp": false, "lv": -1, "tv": -1, "yv": -1, "ytv": -1 }),
    );
    match newer {
        Ok(answer) => Ok(answer),
        Err(error @ (PluginError::AuthRequired | PluginError::RateLimited | PluginError::Network(_))) => {
            Err(error)
        }
        Err(error) => {
            jf_plugin_sdk::info!("song/lyric/v1 did not answer ({error:?}); asking song/lyric");
            api::call_as("/song/lyric", serde_json::json!({ "id": id, "lv": -1, "tv": -1 }))
        }
    }
}

/// NetEase marks instrumentals with one line far in the future ("纯音乐，请欣赏",
/// "pure music, enjoy" at 99:00 or later).
fn instrumental(lyrics: &lrc::Lrc) -> bool {
    let texts: Vec<&lrc::Line> = lyrics
        .lines
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .collect();
    texts.len() == 1 && texts[0].time_ms >= 99 * 60_000 && texts[0].text.contains("纯音乐")
}

fn text(lyric: Option<&LyricText>) -> Option<&str> {
    lyric
        .and_then(|l| l.lyric.as_deref())
        .filter(|t| !t.trim().is_empty())
}

/// Word-timed lyrics as enhanced LRC, when the answer has some.
fn word_timed(answer: &Lyrics, translations: bool) -> Option<LyricPayload> {
    let words = text(answer.yrc.as_ref())?;
    let translation = translations
        .then(|| text(answer.ytlrc.as_ref()).or_else(|| text(answer.tlyric.as_ref())))
        .flatten();
    let mut payload = LyricPayload::new(LyricFormat::Elrc, yrc::to_elrc(words, translation)?);
    payload.attribution = Some(ATTRIBUTION.into());
    Some(payload)
}

/// The payload for an answer, or `None` when there are no lyrics.
pub fn payload(answer: &Lyrics, translations: bool) -> Option<LyricPayload> {
    if answer.nolyric {
        return None;
    }
    if let Some(payload) = word_timed(answer, translations) {
        return Some(payload);
    }
    let original = text(answer.lrc.as_ref()).unwrap_or_default();
    if original.trim().is_empty() {
        return None;
    }
    let mut parsed = lrc::parse(&credits_to_lrc(original));
    if parsed.is_empty() || instrumental(&parsed) {
        return None;
    }
    if translations
        && parsed.is_synced()
        && let Some(text) = answer.tlyric.as_ref().and_then(|l| l.lyric.as_deref())
        && !text.trim().is_empty()
    {
        parsed = lrc::with_translation(&parsed, &lrc::parse(&credits_to_lrc(text)));
    }
    let format = if parsed.is_synced() {
        LyricFormat::Lrc
    } else {
        LyricFormat::Plain
    };
    let mut payload = LyricPayload::new(format, lrc::render(&parsed));
    payload.attribution = Some(ATTRIBUTION.into());
    payload.language = None;
    Some(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn answer(value: serde_json::Value) -> Lyrics {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn converts_credit_lines() {
        let text = "{\"t\":0,\"c\":[{\"tx\":\"作词: \"},{\"tx\":\"Mock Writer\",\"li\":\"x\"}]}\n\
                    {\"t\":1500,\"c\":[{\"tx\":\"作曲: \"},{\"tx\":\"Mock Composer\"}]}\n[00:12.00]Line\n";
        assert_eq!(
            credits_to_lrc(text),
            "[00:00.00]作词: Mock Writer\n[00:01.50]作曲: Mock Composer\n[00:12.00]Line\n"
        );
        assert_eq!(credits_to_lrc("{not json}\n"), "{not json}\n");
    }

    #[test]
    fn makes_a_bilingual_lrc() {
        let got = payload(
            &answer(json!({
                "lrc": { "lyric": "[by:mock]\n[00:01.00]第一行\n[00:03.00]第二行\n" },
                "tlyric": { "lyric": "[00:01.00]First line\n[00:03.00]Second line\n" }
            })),
            true,
        )
        .unwrap();
        assert_eq!(got.format, LyricFormat::Lrc);
        assert_eq!(
            got.text,
            "[by:mock]\n[00:01.00]第一行\n[00:01.00]First line\n[00:03.00]第二行\n[00:03.00]Second line\n"
        );
        assert_eq!(got.attribution.as_deref(), Some(ATTRIBUTION));
        let without = payload(
            &answer(json!({
                "lrc": { "lyric": "[00:01.00]第一行\n" },
                "tlyric": { "lyric": "[00:01.00]First line\n" }
            })),
            false,
        )
        .unwrap();
        assert_eq!(without.text, "[00:01.00]第一行\n");
    }

    #[test]
    fn no_lyrics_for_instrumentals_or_empty_answers() {
        assert!(payload(&answer(json!({ "nolyric": true })), true).is_none());
        assert!(payload(&answer(json!({ "lrc": { "lyric": "" } })), true).is_none());
        assert!(
            payload(
                &answer(json!({ "lrc": { "lyric": "[99:00.00]纯音乐，请欣赏\n" } })),
                true
            )
            .is_none()
        );
        assert!(payload(&answer(json!({ "uncollected": true })), true).is_none());
    }

    #[test]
    fn word_timed_lyrics_win_with_their_own_translation() {
        let both = json!({
            "lrc": { "lyric": "[00:01.00]Lantern light\n" },
            "tlyric": { "lyric": "[00:01.00]灯笼的光（LRC 的翻译）\n" },
            "yrc": { "lyric": "[1000,1000](1000,500,0)Lantern (1500,500,0)light\n" },
            "ytlrc": { "lyric": "[00:01.00]灯笼的光\n" }
        });
        let got = payload(&answer(both.clone()), true).unwrap();
        assert_eq!(got.format, LyricFormat::Elrc);
        assert_eq!(
            got.text,
            "[00:01.00]<00:01.00>Lantern <00:01.50>light<00:02.00>\n[00:01.00]灯笼的光\n"
        );
        assert_eq!(got.attribution.as_deref(), Some(ATTRIBUTION));
        let without = payload(&answer(both), false).unwrap();
        assert_eq!(
            without.text,
            "[00:01.00]<00:01.00>Lantern <00:01.50>light<00:02.00>\n"
        );
        // Without its own translation, the LRC's is used.
        let lrc_translation = json!({
            "yrc": { "lyric": "[1000,1000](1000,500,0)Lantern (1500,500,0)light\n" },
            "tlyric": { "lyric": "[00:01.00]灯笼的光\n" }
        });
        assert!(
            payload(&answer(lrc_translation), true)
                .unwrap()
                .text
                .ends_with("[00:01.00]灯笼的光\n")
        );
        // YRC that times nothing leaves the LRC.
        let empty =
            json!({ "lrc": { "lyric": "[00:01.00]Line\n" }, "yrc": { "lyric": "{\"t\":0,\"c\":[]}\n" } });
        assert_eq!(payload(&answer(empty), true).unwrap().format, LyricFormat::Lrc);
    }

    #[test]
    fn unsynced_lyrics_are_plain() {
        let got = payload(
            &answer(json!({ "lrc": { "lyric": "Words only\nMore words" } })),
            true,
        )
        .unwrap();
        assert_eq!(got.format, LyricFormat::Plain);
        assert_eq!(got.text, "Words only\nMore words\n");
    }
}
