//! Plain, unsynced text.

use crate::build::{DocBuilder, RawLine};
use crate::model::{Format, LyricDoc};
use crate::text::{split_lines, strip_bom};

/// Parses unsynced text: one [`crate::Line`] per input line, without times.
///
/// Lines are trimmed; blank lines inside the text are kept (stanza breaks),
/// blank lines before the first and after the last text line are dropped.
/// This function cannot fail and applies no size limits (its output is
/// never larger than its input); [`crate::parse`] checks the limits first.
pub fn parse_plain(input: &str) -> LyricDoc {
    let lines: Vec<&str> = split_lines(strip_bom(input)).map(str::trim).collect();
    let mut builder = DocBuilder::new(Format::Plain);
    if let (Some(first), Some(last)) = (
        lines.iter().position(|l| !l.is_empty()),
        lines.iter().rposition(|l| !l.is_empty()),
    ) {
        builder.lines = lines[first..=last]
            .iter()
            .map(|text| RawLine {
                text: (*text).to_owned(),
                ..RawLine::default()
            })
            .collect();
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Timing;

    #[test]
    fn keeps_inner_blank_lines_only() {
        let doc = parse_plain("\u{feff}\n\n  First  \n\nSecond\n\n");
        let texts: Vec<&str> = doc.lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["First", "", "Second"]);
        assert_eq!(doc.timing, Timing::Unsynced);
        assert!(
            doc.lines
                .iter()
                .all(|l| l.start_ms.is_none() && l.end_ms.is_none())
        );
    }

    #[test]
    fn empty_input_gives_an_empty_doc() {
        assert_eq!(parse_plain(""), LyricDoc::empty(Format::Plain));
        assert_eq!(parse_plain(" \n \r\n"), LyricDoc::empty(Format::Plain));
    }
}
