//! TTML lyrics in the Apple style (`itunes:timing`, `ttm:agent`, `x-bg` spans,
//! `iTunesMetadata` translations) and its common variants (`x-translation` and
//! `x-roman` spans inside `<p>`).
//!
//! The XML is read with `quick-xml`, which never loads DTDs or external
//! entities. Only the five predefined entities and character references are
//! resolved; any other entity reference is kept literally as `&name;`.

use std::collections::BTreeMap;

use quick_xml::XmlVersion;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::reader::Reader;

use crate::build::{DocBuilder, RawAux, RawLine};
use crate::error::LyricsError;
use crate::model::{Agent, AgentKind, AuxKind, Format, LyricDoc, Timing};
use crate::text::{clean, strip_bom};
use crate::words::{Piece, RawWord, SpanTime, assemble};
use crate::{MAX_LINES, check_limits};

/// Parses a TTML document.
///
/// See the crate documentation for the supported subset. Errors: malformed
/// XML, a root element other than `<tt>`, and unparsable time expressions.
pub fn parse_ttml(input: &str) -> Result<LyricDoc, LyricsError> {
    check_limits(input)?;
    let src = strip_bom(input);
    let mut reader = Reader::from_str(src);
    reader.config_mut().expand_empty_elements = true;
    let mut parser = Parser::new(src);
    loop {
        let pos = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(|e| LyricsError::Xml {
            line: line_at(src, reader.error_position() as usize),
            message: e.to_string(),
        })?;
        match event {
            Event::Start(e) => parser.start(&e, pos)?,
            Event::End(_) => parser.end(),
            Event::Text(t) => parser.text(&t.xml10_content()),
            Event::CData(t) => parser.text(&t.xml10_content()),
            Event::GeneralRef(r) => parser.text(&resolve_reference(&r)),
            Event::Eof => break,
            // Declarations, comments, processing instructions and DOCTYPE are skipped;
            // `Empty` does not occur because empty elements are expanded.
            _ => {}
        }
    }
    parser.finish()
}

/// 1-based line of a byte position.
fn line_at(src: &str, pos: usize) -> usize {
    let end = pos.min(src.len());
    src.as_bytes()[..end].iter().filter(|&&b| b == b'\n').count() + 1
}

fn resolve_reference(r: &BytesRef<'_>) -> String {
    match r.resolve_char_ref() {
        Ok(Some(c)) => c.to_string(),
        Ok(None) => match resolve_predefined_entity(r) {
            Some(s) => s.to_owned(),
            None => format!("&{};", &**r),
        },
        Err(_) => '\u{FFFD}'.to_string(),
    }
}

/// Frame and tick rates for the `f` and `t` time metrics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Rates {
    pub frame: f64,
    pub tick: f64,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            frame: 30.0,
            tick: 1.0,
        }
    }
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// A decimal number `digits` or `digits.digits`.
fn decimal(s: &str) -> Option<f64> {
    let ok = match s.split_once('.') {
        Some((int, frac)) => digits(int) && digits(frac),
        None => digits(s),
    };
    if ok { s.parse().ok() } else { None }
}

/// Parses a TTML time expression into milliseconds, rounded to the nearest one.
///
/// Clock time: `hh:mm:ss.fff`, `mm:ss.fff` (Apple's short form), or
/// `hh:mm:ss:ff` with frames. Offset time: a number followed by `h`, `m`,
/// `s`, `ms`, `f` or `t`; a bare number means seconds (Apple writes `12.345`).
pub(crate) fn parse_time(value: &str, rates: Rates) -> Option<u32> {
    let v = value.trim();
    let int = |s: &str| if digits(s) { s.parse::<f64>().ok() } else { None };
    let ms = if v.contains(':') {
        match v.split(':').collect::<Vec<_>>().as_slice() {
            [m, s] => int(m)? * 60_000.0 + decimal(s)? * 1000.0,
            [h, m, s] => int(h)? * 3_600_000.0 + int(m)? * 60_000.0 + decimal(s)? * 1000.0,
            [h, m, s, f] => {
                int(h)? * 3_600_000.0
                    + int(m)? * 60_000.0
                    + int(s)? * 1000.0
                    + decimal(f)? * 1000.0 / rates.frame
            }
            _ => return None,
        }
    } else {
        let split = v
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(v.len());
        let (number, metric) = v.split_at(split);
        let unit = match metric {
            "" | "s" => 1000.0,
            "ms" => 1.0,
            "m" => 60_000.0,
            "h" => 3_600_000.0,
            "f" => 1000.0 / rates.frame,
            "t" => 1000.0 / rates.tick,
            _ => return None,
        };
        decimal(number)? * unit
    };
    (ms.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&ms)).then(|| ms.round() as u32)
}

/// Where text inside a `<p>` (or a sidecar `<text>`) goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Main,
    Background,
    Translation,
    Romanization,
}

#[derive(Debug)]
struct SpanFrame {
    role: Role,
    time: Option<SpanTime>,
    lang: Option<String>,
}

/// Collects the text pieces of one `<p>` or sidecar `<text>`, by role.
#[derive(Debug, Default)]
struct Collector {
    main: Vec<Piece>,
    background: Vec<Piece>,
    aux: Vec<(AuxKind, Option<String>, Vec<Piece>)>,
    spans: Vec<SpanFrame>,
}

impl Collector {
    /// The innermost role, the innermost span time within that role, and the role's language.
    fn context(&self) -> (Role, Option<SpanTime>, Option<String>) {
        let mut time = None;
        for frame in self.spans.iter().rev() {
            time = time.or(frame.time);
            if frame.role != Role::Main {
                return (frame.role, time, frame.lang.clone());
            }
        }
        (Role::Main, time, None)
    }

    fn push(&mut self, text: &str) {
        let (role, time, lang) = self.context();
        let piece = Piece::new(text, time);
        let kind = match role {
            Role::Main => return self.main.push(piece),
            Role::Background => return self.background.push(piece),
            Role::Translation => AuxKind::Translation,
            Role::Romanization => AuxKind::Romanization,
        };
        match self.aux.iter_mut().find(|(k, l, _)| *k == kind && *l == lang) {
            Some((_, _, pieces)) => pieces.push(piece),
            None => self.aux.push((kind, lang, vec![piece])),
        }
    }
}

/// What an open element means for the text and elements inside it.
#[derive(Debug)]
enum Frame {
    Root,
    /// `<head>` and any element in it without a meaning of its own.
    Head,
    AgentDef(usize),
    AgentName(usize),
    Title,
    Songwriter,
    /// `<translation>` or `<transliteration>` in Apple's `iTunesMetadata`.
    SidecarGroup(AuxKind, Option<String>),
    /// `<text for="…">` inside a sidecar group; its content is collected.
    SidecarText,
    /// `<body>`, `<div>` or another element in the body, with its inherited agent.
    Container(Option<String>),
    P,
    Span,
    /// Any other element inside collected content; its text flows through.
    Inline,
    /// Content that is skipped entirely.
    Ignored,
}

#[derive(Debug)]
struct PBuilder {
    begin: Option<u32>,
    end: Option<u32>,
    agent: Option<String>,
    key: Option<String>,
    collector: Collector,
}

#[derive(Debug)]
struct SidecarText {
    kind: AuxKind,
    lang: Option<String>,
    key: Option<String>,
    collector: Collector,
}

#[derive(Debug)]
struct SidecarEntry {
    kind: AuxKind,
    lang: Option<String>,
    key: String,
    text: String,
    words: Vec<RawWord>,
}

/// Attributes of one element: local name (prefix dropped) and unescaped value.
type Attrs = Vec<(String, String)>;

fn get<'a>(attrs: &'a Attrs, name: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
}

#[derive(Debug)]
struct Parser<'a> {
    src: &'a str,
    frames: Vec<Frame>,
    root_done: bool,
    builder: DocBuilder,
    declared: Option<Timing>,
    rates: Rates,
    lang: Option<String>,
    lang_fallback: Option<String>,
    p: Option<PBuilder>,
    sidecar: Option<SidecarText>,
    sidecar_entries: Vec<SidecarEntry>,
    /// `itunes:key` or `xml:id` of each line in `builder.lines`.
    keys: Vec<Option<String>>,
    buffer: String,
    songwriters: Vec<String>,
    next_span: u32,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            frames: Vec::new(),
            root_done: false,
            builder: DocBuilder::new(Format::Ttml),
            declared: None,
            rates: Rates::default(),
            lang: None,
            lang_fallback: None,
            p: None,
            sidecar: None,
            sidecar_entries: Vec::new(),
            keys: Vec::new(),
            buffer: String::new(),
            songwriters: Vec::new(),
            next_span: 0,
        }
    }

    fn attrs(&self, e: &BytesStart<'_>, pos: usize) -> Result<Attrs, LyricsError> {
        let xml_error = |message: String| LyricsError::Xml {
            line: line_at(self.src, pos),
            message,
        };
        let mut out = Vec::new();
        for attr in e.attributes() {
            let attr = attr.map_err(|err| xml_error(err.to_string()))?;
            if attr.key.as_namespace_binding().is_some() {
                continue;
            }
            let value = attr
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|err| xml_error(err.to_string()))?;
            out.push((attr.key.local_name().as_ref().to_owned(), value.into_owned()));
        }
        Ok(out)
    }

    fn time(&self, attrs: &Attrs, name: &str, pos: usize) -> Result<Option<u32>, LyricsError> {
        let Some(value) = get(attrs, name) else {
            return Ok(None);
        };
        parse_time(value, self.rates)
            .map(Some)
            .ok_or_else(|| LyricsError::InvalidTime {
                line: line_at(self.src, pos),
                attribute: name.to_owned(),
                value: value.to_owned(),
            })
    }

    /// `begin` plus `end`, or `begin` plus `dur`.
    fn interval(&self, attrs: &Attrs, pos: usize) -> Result<(Option<u32>, Option<u32>), LyricsError> {
        let begin = self.time(attrs, "begin", pos)?;
        let end = self.time(attrs, "end", pos)?;
        let dur = self.time(attrs, "dur", pos)?;
        let end = end.or_else(|| begin.zip(dur).map(|(b, d)| b.saturating_add(d)));
        Ok((begin, end))
    }

    fn collector(&mut self) -> Option<&mut Collector> {
        match (&mut self.p, &mut self.sidecar) {
            (Some(p), _) => Some(&mut p.collector),
            (None, Some(s)) => Some(&mut s.collector),
            (None, None) => None,
        }
    }

    fn inherited_agent(&self) -> Option<String> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Container(agent) => agent.clone(),
            _ => None,
        })
    }

    fn start(&mut self, e: &BytesStart<'_>, pos: usize) -> Result<(), LyricsError> {
        let qname = e.name();
        let local = e.local_name();
        let local = local.as_ref();
        let frame = match self.frames.last() {
            None => {
                if self.root_done || local != "tt" {
                    let message = if self.root_done {
                        format!("element <{}> after the root element", qname.as_ref())
                    } else {
                        format!("the root element is <{}>, not <tt>", qname.as_ref())
                    };
                    return Err(LyricsError::NotTtml {
                        line: line_at(self.src, pos),
                        message,
                    });
                }
                self.open_root(e, pos)?;
                Frame::Root
            }
            Some(Frame::Ignored | Frame::AgentName(_) | Frame::Title | Frame::Songwriter) => Frame::Ignored,
            Some(Frame::P | Frame::Span | Frame::Inline | Frame::SidecarText) => match local {
                "span" => {
                    self.open_span(e, pos)?;
                    Frame::Span
                }
                "br" => {
                    if let Some(c) = self.collector() {
                        c.push(" ");
                    }
                    Frame::Inline
                }
                "metadata" => Frame::Ignored,
                _ => Frame::Inline,
            },
            Some(Frame::Root) => match local {
                "head" => Frame::Head,
                "body" => {
                    let attrs = self.attrs(e, pos)?;
                    self.note_lang(&attrs);
                    Frame::Container(first_id(get(&attrs, "agent")))
                }
                _ => Frame::Ignored,
            },
            Some(Frame::Container(_)) => {
                let attrs = self.attrs(e, pos)?;
                match local {
                    "p" => self.open_p(&attrs, pos)?,
                    "metadata" | "head" | "styling" | "layout" => Frame::Ignored,
                    _ => {
                        self.note_lang(&attrs);
                        Frame::Container(first_id(get(&attrs, "agent")).or_else(|| self.inherited_agent()))
                    }
                }
            }
            Some(Frame::Head | Frame::AgentDef(_) | Frame::SidecarGroup(..)) => self.open_in_head(e, pos)?,
        };
        self.frames.push(frame);
        Ok(())
    }

    fn open_root(&mut self, e: &BytesStart<'_>, pos: usize) -> Result<(), LyricsError> {
        let attrs = self.attrs(e, pos)?;
        self.lang = non_empty(get(&attrs, "lang"));
        self.declared = get(&attrs, "timing").and_then(|t| match t.trim().to_ascii_lowercase().as_str() {
            "line" => Some(Timing::Line),
            "word" => Some(Timing::Word),
            "none" => Some(Timing::Unsynced),
            _ => None,
        });
        let rate = |name| get(&attrs, name).and_then(decimal).filter(|r| *r > 0.0);
        if let Some(frame) = rate("frameRate") {
            self.rates.frame = frame;
            self.rates.tick = frame * rate("subFrameRate").unwrap_or(1.0);
        }
        if let Some(tick) = rate("tickRate") {
            self.rates.tick = tick;
        }
        Ok(())
    }

    fn note_lang(&mut self, attrs: &Attrs) {
        if self.lang_fallback.is_none() {
            self.lang_fallback = non_empty(get(attrs, "lang"));
        }
    }

    fn open_in_head(&mut self, e: &BytesStart<'_>, pos: usize) -> Result<Frame, LyricsError> {
        let local = e.local_name();
        let attrs = self.attrs(e, pos)?;
        let frame = match (self.frames.last(), local.as_ref()) {
            (_, "agent") => match non_empty(get(&attrs, "id")) {
                Some(id) => {
                    let kind = match get(&attrs, "type")
                        .map(|t| t.trim().to_ascii_lowercase())
                        .as_deref()
                    {
                        Some("person" | "character") => AgentKind::Person,
                        Some("group" | "organization") => AgentKind::Group,
                        _ => AgentKind::Other,
                    };
                    let agents = &mut self.builder.agents;
                    let index = match agents.iter().position(|a| a.id == id) {
                        Some(i) => i,
                        None => {
                            agents.push(Agent { id, name: None, kind });
                            agents.len() - 1
                        }
                    };
                    Frame::AgentDef(index)
                }
                None => Frame::Head,
            },
            (Some(Frame::AgentDef(i)), "name") => {
                self.buffer.clear();
                Frame::AgentName(*i)
            }
            (_, "title") => {
                self.buffer.clear();
                Frame::Title
            }
            (_, "songwriter") => {
                self.buffer.clear();
                Frame::Songwriter
            }
            (_, "translation") => Frame::SidecarGroup(AuxKind::Translation, non_empty(get(&attrs, "lang"))),
            (_, "transliteration") => {
                Frame::SidecarGroup(AuxKind::Romanization, non_empty(get(&attrs, "lang")))
            }
            (Some(Frame::SidecarGroup(kind, lang)), "text") => {
                self.sidecar = Some(SidecarText {
                    kind: *kind,
                    lang: lang.clone(),
                    key: non_empty(get(&attrs, "for")),
                    collector: Collector::default(),
                });
                Frame::SidecarText
            }
            _ => Frame::Head,
        };
        Ok(frame)
    }

    fn open_p(&mut self, attrs: &Attrs, pos: usize) -> Result<Frame, LyricsError> {
        if self.builder.lines.len() >= MAX_LINES {
            return Err(LyricsError::TooManyLines {
                lines: MAX_LINES + 1,
                limit: MAX_LINES,
            });
        }
        let (begin, end) = self.interval(attrs, pos)?;
        self.note_lang(attrs);
        self.p = Some(PBuilder {
            begin,
            end,
            agent: first_id(get(attrs, "agent")).or_else(|| self.inherited_agent()),
            key: non_empty(get(attrs, "key")).or_else(|| non_empty(get(attrs, "id"))),
            collector: Collector::default(),
        });
        Ok(Frame::P)
    }

    fn open_span(&mut self, e: &BytesStart<'_>, pos: usize) -> Result<(), LyricsError> {
        let attrs = self.attrs(e, pos)?;
        let role = match get(&attrs, "role").map(str::trim) {
            Some("x-bg") => Role::Background,
            Some("x-translation") => Role::Translation,
            Some("x-roman" | "x-romanization" | "x-transliteration") => Role::Romanization,
            _ => Role::Main,
        };
        let (begin, end) = self.interval(&attrs, pos)?;
        let time = begin.map(|start| SpanTime {
            id: self.next_span,
            start,
            end,
        });
        self.next_span += 1;
        let lang = non_empty(get(&attrs, "lang"));
        if let Some(c) = self.collector() {
            c.spans.push(SpanFrame { role, time, lang });
        }
        Ok(())
    }

    fn text(&mut self, text: &str) {
        match self.frames.last() {
            Some(Frame::AgentName(_) | Frame::Title | Frame::Songwriter) => self.buffer.push_str(text),
            Some(Frame::P | Frame::Span | Frame::Inline | Frame::SidecarText) => {
                if let Some(c) = self.collector() {
                    c.push(text);
                }
            }
            _ => {}
        }
    }

    fn end(&mut self) {
        let Some(frame) = self.frames.pop() else { return };
        match frame {
            Frame::Root => self.root_done = true,
            Frame::P => self.close_p(),
            Frame::Span => {
                if let Some(c) = self.collector() {
                    c.spans.pop();
                }
            }
            Frame::SidecarText => self.close_sidecar_text(),
            Frame::AgentName(i) => {
                let name = clean(&self.buffer);
                if let Some(agent) = self.builder.agents.get_mut(i)
                    && agent.name.is_none()
                    && !name.is_empty()
                {
                    agent.name = Some(name);
                }
            }
            Frame::Title => {
                let title = clean(&self.buffer);
                if !title.is_empty() {
                    self.builder.metadata.insert("title".to_owned(), title);
                }
            }
            Frame::Songwriter => {
                let name = clean(&self.buffer);
                if !name.is_empty() {
                    self.songwriters.push(name);
                }
            }
            _ => {}
        }
    }

    fn close_p(&mut self) {
        let Some(p) = self.p.take() else { return };
        let Collector {
            main,
            background,
            aux,
            ..
        } = p.collector;
        let main = assemble(&main);
        let background = assemble(&background);
        let timed = main.words.iter().chain(&background.words);
        let start = p.begin.or_else(|| timed.clone().map(|w| w.start).min());
        let end = p.end.or_else(|| timed.filter_map(|w| w.end).max());
        let mut background_words = background.words;
        if background_words.is_empty() && !background.text.is_empty() {
            // Untimed background vocals last as long as the line.
            background_words.push(RawWord {
                start: start.unwrap_or(0),
                end,
                text: background.text,
                joins_next: false,
            });
        }
        let mut line = RawLine {
            start,
            end,
            agent: p.agent.map(|a| self.builder.agent(&a)),
            text: main.text,
            words: main.words,
            background: background_words,
            aux: Vec::new(),
        };
        for (kind, lang, pieces) in aux {
            let assembled = assemble(&pieces);
            line.aux.push(RawAux {
                track: self.builder.track(kind, lang),
                text: assembled.text,
                words: assembled.words,
            });
        }
        self.keys.push(p.key);
        self.builder.lines.push(line);
    }

    fn close_sidecar_text(&mut self) {
        let Some(sidecar) = self.sidecar.take() else {
            return;
        };
        let Some(key) = sidecar.key else { return };
        let assembled = assemble(&sidecar.collector.main);
        self.sidecar_entries.push(SidecarEntry {
            kind: sidecar.kind,
            lang: sidecar.lang,
            key,
            text: assembled.text,
            words: assembled.words,
        });
    }

    fn finish(mut self) -> Result<LyricDoc, LyricsError> {
        if !self.root_done {
            return Err(if self.frames.is_empty() {
                LyricsError::NotTtml {
                    line: 1,
                    message: "no <tt> root element".to_owned(),
                }
            } else {
                LyricsError::Xml {
                    line: line_at(self.src, self.src.len()),
                    message: "unexpected end of input: the document is truncated".to_owned(),
                }
            });
        }
        let mut by_key: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (i, key) in self.keys.iter().enumerate() {
            if let Some(key) = key {
                by_key.entry(key.as_str()).or_default().push(i);
            }
        }
        for entry in self.sidecar_entries {
            let Some(indices) = by_key.get(entry.key.as_str()) else {
                continue;
            };
            let track = self.builder.track(entry.kind, entry.lang);
            for &i in indices {
                self.builder.lines[i].aux.push(RawAux {
                    track,
                    text: entry.text.clone(),
                    words: entry.words.clone(),
                });
            }
        }
        match self.declared {
            Some(Timing::Unsynced) => {
                for line in &mut self.builder.lines {
                    line.start = None;
                    line.end = None;
                }
            }
            Some(Timing::Line) => {
                for line in &mut self.builder.lines {
                    line.words.clear();
                }
            }
            _ => {}
        }
        self.builder.language = self.lang.or(self.lang_fallback);
        if !self.songwriters.is_empty() {
            self.builder
                .metadata
                .insert("songwriters".to_owned(), self.songwriters.join(", "));
        }
        Ok(self.builder.finish())
    }
}

/// The first id of an IDREFS attribute such as `ttm:agent="v1 v2"`.
fn first_id(value: Option<&str>) -> Option<String> {
    value.and_then(|v| v.split_whitespace().next()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(v: &str) -> Option<u32> {
        parse_time(v, Rates::default())
    }

    #[test]
    fn clock_times() {
        assert_eq!(t("00:01:02.345"), Some(62_345));
        assert_eq!(t("1:02:03.5"), Some(3_723_500));
        assert_eq!(t("01:02.345"), Some(62_345));
        assert_eq!(t("1:02.3"), Some(62_300));
        assert_eq!(t("75:00.000"), Some(4_500_000));
        assert_eq!(t("00:00:01:15"), Some(1_500)); // 15 frames at 30 fps
        assert_eq!(t("12.345"), Some(12_345));
        assert_eq!(t("12"), Some(12_000));
        assert_eq!(t("1.0006"), Some(1_001)); // rounds to the nearest millisecond
    }

    #[test]
    fn offset_times() {
        assert_eq!(t("12.3s"), Some(12_300));
        assert_eq!(t("450ms"), Some(450));
        assert_eq!(t("1.5m"), Some(90_000));
        assert_eq!(t("1h"), Some(3_600_000));
        assert_eq!(t("45f"), Some(1_500));
        assert_eq!(t("3t"), Some(3_000));
        let rates = Rates {
            frame: 25.0,
            tick: 10_000_000.0,
        };
        assert_eq!(parse_time("25f", rates), Some(1_000));
        assert_eq!(parse_time("15000000t", rates), Some(1_500));
    }

    #[test]
    fn invalid_times() {
        for v in [
            "",
            "abc",
            "1.2.3",
            "-1s",
            "+1s",
            "1e3",
            "12.",
            ".5",
            "1:2:3:4:5",
            "1:x",
            "10q",
            "inf",
            "5000000h",
        ] {
            assert_eq!(t(v), None, "{v:?}");
        }
    }

    #[test]
    fn rejects_non_ttml_and_broken_xml() {
        let err = parse_ttml("<html><body/></html>").unwrap_err();
        assert!(matches!(err, LyricsError::NotTtml { line: 1, .. }), "{err}");
        let err = parse_ttml("<tt>\n<body>\n<p>open</body></tt>").unwrap_err();
        assert!(matches!(err, LyricsError::Xml { line: 3, .. }), "{err}");
        let err = parse_ttml("<tt><body><div><p>cut").unwrap_err();
        assert!(matches!(err, LyricsError::Xml { .. }), "{err}");
        let err = parse_ttml("<tt>\n<body><p begin=\"soon\">x</p></body></tt>").unwrap_err();
        assert_eq!(
            err,
            LyricsError::InvalidTime {
                line: 2,
                attribute: "begin".into(),
                value: "soon".into()
            }
        );
        assert!(matches!(parse_ttml("").unwrap_err(), LyricsError::NotTtml { .. }));
    }

    #[test]
    fn entities_are_never_expanded_from_a_dtd() {
        let doc = parse_ttml(concat!(
            "<?xml version=\"1.0\"?>",
            "<!DOCTYPE tt [<!ENTITY boom \"BOOM\"><!ENTITY ext SYSTEM \"file:///etc/passwd\">]>",
            "<tt><body><p begin=\"1\" end=\"2\">Salt &amp; &#x43;&#97;ramel &boom; &ext;</p></body></tt>",
        ))
        .unwrap();
        assert_eq!(doc.lines[0].text, "Salt & Caramel &boom; &ext;");
    }

    #[test]
    fn declared_timing_wins() {
        let body = r#"<body><p begin="1" end="2"><span begin="1" end="1.5">a</span> <span begin="1.5" end="2">b</span></p></body>"#;
        let doc = |timing: &str| {
            parse_ttml(&format!(
                r#"<tt xmlns:itunes="urn:x" itunes:timing="{timing}">{body}</tt>"#
            ))
            .unwrap()
        };
        let none = doc("None");
        assert_eq!(none.timing, Timing::Unsynced);
        assert_eq!((none.lines[0].start_ms, none.lines[0].end_ms), (None, None));
        assert!(none.lines[0].words.is_empty());
        let line = doc("Line");
        assert_eq!(line.timing, Timing::Line);
        assert!(line.lines[0].words.is_empty());
        assert_eq!(line.lines[0].text, "a b");
        assert_eq!(doc("Word").timing, Timing::Word);
    }

    #[test]
    fn untimed_p_takes_times_from_its_spans() {
        let doc = parse_ttml(
            r#"<tt><body><p><span begin="1.0" end="1.5">a</span> <span begin="1.5" end="2.25">b</span></p></body></tt>"#,
        )
        .unwrap();
        assert_eq!(
            (doc.lines[0].start_ms, doc.lines[0].end_ms),
            (Some(1000), Some(2250))
        );
        assert_eq!(doc.timing, Timing::Word);
    }
}
