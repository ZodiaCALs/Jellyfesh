//! Errors returned by the parsers.

use serde::{Deserialize, Serialize};

/// Why a lyric document could not be parsed.
///
/// Line numbers are 1-based and count lines of the input as given.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LyricsError {
    /// The input is larger than [`crate::MAX_INPUT_BYTES`].
    #[error("lyrics input is {bytes} bytes, more than the limit of {limit} bytes")]
    InputTooLarge {
        /// Size of the input.
        bytes: usize,
        /// The limit.
        limit: usize,
    },
    /// The input, or the lines it expands to, exceeds [`crate::MAX_LINES`].
    #[error("lyrics have {lines} lines, more than the limit of {limit} lines")]
    TooManyLines {
        /// Number of lines found (counting stops shortly after the limit).
        lines: usize,
        /// The limit.
        limit: usize,
    },
    /// Repeated LRC timestamps expand to more text than [`crate::MAX_EXPANDED_BYTES`].
    #[error("repeated timestamps expand the lyrics to more than {limit} bytes of text")]
    ExpandedTooLarge {
        /// The limit.
        limit: usize,
    },
    /// The XML of a TTML document is not well-formed.
    #[error("invalid XML at line {line}: {message}")]
    Xml {
        /// Line of the error.
        line: usize,
        /// What the XML reader reported.
        message: String,
    },
    /// The document is well-formed XML but not TTML.
    #[error("not a TTML document (line {line}): {message}")]
    NotTtml {
        /// Line of the offending element.
        line: usize,
        /// What was found instead.
        message: String,
    },
    /// A TTML time expression could not be parsed.
    #[error("invalid time expression {value:?} in attribute `{attribute}` at line {line}")]
    InvalidTime {
        /// Line of the element carrying the attribute.
        line: usize,
        /// Attribute name (`begin`, `end` or `dur`).
        attribute: String,
        /// The attribute value as written.
        value: String,
    },
    /// The input does not start with the `WEBVTT` signature.
    #[error("not a WebVTT file: line 1 must start with \"WEBVTT\"")]
    MissingVttHeader,
}
