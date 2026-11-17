//! QQ Music lyrics to an LRC payload.
//!
//! `music.musichallSong.PlayLyricInfo` `GetPlayLyricInfo` answers the
//! lyrics as LRC (`lyric`) and, when someone translated them, the
//! translation (`trans`), usually in base64 and sometimes with HTML
//! character references. The plugin asks for plain LRC only: word-timed QRC
//! lyrics come encrypted with a key from QQ Music's apps, which the plugin
//! does not have and must not use, so an encrypted answer (`crypt` 1) means
//! no lyrics.

use jf_community_common::{base64, entities, lrc};
use jf_plugin_sdk::prelude::*;
use serde_json::Value;

/// What Jellyfesh shows as the source.
pub const ATTRIBUTION: &str = "Lyrics from QQ Music";

/// The text of a lyrics field: as it is when it already looks like lyrics,
/// decoded from base64 otherwise, with character references resolved.
pub fn field_text(value: Option<&Value>) -> String {
    let raw = value.and_then(Value::as_str).unwrap_or_default().trim();
    if raw.is_empty() {
        return String::new();
    }
    let looks_like_lrc = raw.starts_with('[') || raw.contains('\n') || raw.contains("&#");
    let text = if looks_like_lrc {
        raw.to_string()
    } else {
        match base64::decode(raw) {
            Some(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            None => raw.to_string(),
        }
    };
    entities::decode(&text)
}

/// QQ Music marks instrumentals with a single line ("此歌曲为没有填词的纯音乐，请您欣赏").
fn instrumental(lyrics: &lrc::Lrc) -> bool {
    let lines: Vec<&str> = lyrics
        .lines
        .iter()
        .map(|l| l.text.trim())
        .chain(lyrics.plain.iter().map(|l| l.trim()))
        .filter(|t| !t.is_empty())
        .collect();
    lines.len() == 1 && lines[0].contains("纯音乐")
}

/// The payload for an answer's `data`, or `None` when there are no lyrics.
pub fn payload(data: &Value, translations: bool) -> Option<LyricPayload> {
    if data.get("crypt").and_then(Value::as_i64).unwrap_or(0) != 0 {
        return None;
    }
    let mut parsed = lrc::parse(&field_text(data.get("lyric")));
    if parsed.is_empty() || instrumental(&parsed) {
        return None;
    }
    if translations && parsed.is_synced() {
        let translation = field_text(data.get("trans"));
        if !translation.trim().is_empty() {
            parsed = lrc::with_translation(&parsed, &lrc::parse(&translation));
        }
    }
    let format = if parsed.is_synced() {
        LyricFormat::Lrc
    } else {
        LyricFormat::Plain
    };
    let mut payload = LyricPayload::new(format, lrc::render(&parsed));
    payload.attribution = Some(ATTRIBUTION.into());
    Some(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn b64(text: &str) -> String {
        base64::encode(text.as_bytes())
    }

    #[test]
    fn decodes_fields() {
        assert_eq!(field_text(Some(&json!(b64("[00:01.00]你好")))), "[00:01.00]你好");
        assert_eq!(field_text(Some(&json!("[00:01.00]It&apos;s"))), "[00:01.00]It's");
        assert_eq!(field_text(Some(&json!(""))), "");
        assert_eq!(field_text(None), "");
    }

    #[test]
    fn merges_the_translation() {
        let data = json!({
            "lyric": b64("[ti:Mock]\n[00:01.00]第一行\n[00:02.00]第二行\n"),
            "trans": b64("[00:01.00]Line one\n[00:02.00]//\n"),
            "crypt": 0
        });
        let got = payload(&data, true).unwrap();
        assert_eq!(got.format, LyricFormat::Lrc);
        assert_eq!(
            got.text,
            "[ti:Mock]\n[00:01.00]第一行\n[00:01.00]Line one\n[00:02.00]第二行\n"
        );
        assert_eq!(
            payload(&data, false).unwrap().text,
            "[ti:Mock]\n[00:01.00]第一行\n[00:02.00]第二行\n"
        );
    }

    #[test]
    fn no_lyrics_when_encrypted_empty_or_instrumental() {
        assert!(payload(&json!({ "lyric": b64("[00:01.00]x"), "crypt": 1 }), true).is_none());
        assert!(payload(&json!({ "lyric": "" }), true).is_none());
        let instrumental = json!({ "lyric": b64("[00:00.00]此歌曲为没有填词的纯音乐，请您欣赏\n") });
        assert!(payload(&instrumental, true).is_none());
    }
}
