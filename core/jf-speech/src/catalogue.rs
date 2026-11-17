//! The Whisper models the app offers.
//!
//! The files are OpenAI's Whisper weights (MIT) converted to `ggml` by
//! whisper.cpp and published on Hugging Face. Ids, files and SHA-1 sums come
//! from whisper.cpp's `models/README.md` and `models/download-ggml-model.sh`
//! (checked 2026-11-17); sizes are that README's, rounded to MiB.

/// A downloadable speech model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    /// Stable id: whisper.cpp's model name ("base").
    pub id: &'static str,
    /// The name shown to people ("Base").
    pub name: &'static str,
    /// The file name, in the store and at the URL ("ggml-base.bin").
    pub file: &'static str,
    /// Where the file is downloaded from.
    pub url: &'static str,
    /// The size whisper.cpp states, in bytes; the exact size comes from the server.
    pub approx_bytes: u64,
    /// The SHA-1 whisper.cpp publishes, lowercase hex.
    pub sha1: &'static str,
    /// The SHA-256, lowercase hex, once CI has computed it from the published file.
    pub sha256: Option<&'static str>,
    /// Recognises languages other than English.
    pub multilingual: bool,
}

/// The model used unless the person picks another.
pub const DEFAULT_MODEL: &str = "base";

const MIB: u64 = 1024 * 1024;

static CATALOGUE: [ModelInfo; 4] = [
    ModelInfo {
        id: "tiny",
        name: "Tiny",
        file: "ggml-tiny.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
        approx_bytes: 75 * MIB,
        sha1: "bd577a113a864445d4c299885e0cb97d4ba92b5f",
        sha256: None,
        multilingual: true,
    },
    ModelInfo {
        id: "base",
        name: "Base",
        file: "ggml-base.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        approx_bytes: 142 * MIB,
        sha1: "465707469ff3a37a2b9b8d8f89f2f99de7299dac",
        sha256: None,
        multilingual: true,
    },
    ModelInfo {
        id: "small",
        name: "Small",
        file: "ggml-small.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        approx_bytes: 466 * MIB,
        sha1: "55356645c2b361a969dfd0ef2c5a50d530afd8d5",
        sha256: None,
        multilingual: true,
    },
    ModelInfo {
        id: "large-v3-turbo-q5_0",
        name: "Large Turbo",
        file: "ggml-large-v3-turbo-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        approx_bytes: 547 * MIB,
        sha1: "e050f7970618a659205450ad97eb95a18d69c9ee",
        sha256: None,
        multilingual: true,
    },
];

/// Every model the app offers, smallest first.
pub fn catalogue() -> &'static [ModelInfo] {
    &CATALOGUE
}

/// The model with this id.
pub fn model(id: &str) -> Option<&'static ModelInfo> {
    CATALOGUE.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_hex(s: &str, len: usize) -> bool {
        s.len() == len && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    }

    #[test]
    fn the_four_models_in_order() {
        let ids: Vec<_> = catalogue().iter().map(|m| m.id).collect();
        assert_eq!(ids, ["tiny", "base", "small", "large-v3-turbo-q5_0"]);
        let names: Vec<_> = catalogue().iter().map(|m| m.name).collect();
        assert_eq!(names, ["Tiny", "Base", "Small", "Large Turbo"]);
        assert!(
            catalogue()
                .windows(2)
                .all(|w| w[0].approx_bytes < w[1].approx_bytes)
        );
    }

    #[test]
    fn files_urls_and_sums_follow_whisper_cpp() {
        for m in catalogue() {
            assert_eq!(m.file, format!("ggml-{}.bin", m.id));
            assert_eq!(
                m.url,
                format!(
                    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
                    m.file
                )
            );
            assert!(is_hex(m.sha1, 40), "{}", m.id);
            assert!(m.sha256.is_none_or(|s| is_hex(s, 64)), "{}", m.id);
            assert!(m.multilingual);
        }
        assert_eq!(
            model("tiny").map(|m| m.sha1),
            Some("bd577a113a864445d4c299885e0cb97d4ba92b5f")
        );
        assert_eq!(model("small").map(|m| m.approx_bytes), Some(466 * MIB));
    }

    #[test]
    fn lookup() {
        assert_eq!(model(DEFAULT_MODEL).map(|m| m.name), Some("Base"));
        assert_eq!(model("large-v3-turbo-q5_0").map(|m| m.name), Some("Large Turbo"));
        assert!(model("medium").is_none());
        assert!(model("").is_none());
        assert!(model("Base").is_none());
    }
}
