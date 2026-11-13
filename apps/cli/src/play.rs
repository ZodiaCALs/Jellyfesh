//! `jf play`: drive the core and draw what it reports.
//!
//! Like any Jellyfesh UI, this only sends commands and renders events. In a
//! terminal the bottom of the screen is a live view: the lyric line being
//! sung, with a sweep that lights each word as it is sung in the artwork's
//! accent colour, its translation, and a progress bar. Lines already sung
//! scroll up above it.
//!
//! Timing follows the core's rules for every shell: positions come from a
//! [`DisplayClock`] fed by the playback anchors (never polled), and the view
//! is redrawn only when something visible changes: the next lyric line, word
//! or swept character, the next step of the bar, the next second. Between
//! those moments the process sleeps, and while paused no timer runs at all.

use std::io::{IsTerminal, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};
use crossbeam_channel::{Receiver, RecvTimeoutError};
use jf_core::timing::{next_second_us, next_step_us};
use jf_core::{
    Command, CommandId, Core, Cursor, DisplayClock, Event, LyricsSnapshot, NowPlaying, Status,
    monotonic_now_ns,
};

use crate::util::{bold, dim, display_width, fmt_ms};

/// Progress bar length in cells; each cell has two steps.
const BAR_CELLS: usize = 28;
/// Never wake sooner than this, even when a change is due "now".
const MIN_WAIT: Duration = Duration::from_millis(2);
/// Wake this much after a change is due, so the draw lands past it.
const WAKE_LATE: Duration = Duration::from_millis(1);

/// What to show while playing.
pub struct View {
    pub lyrics: bool,
    pub quiet: bool,
}

/// Runs until the queue ends, playback stops, or the starting command fails.
pub fn run(events: &Receiver<Event>, start: CommandId, view: &View) -> Result<()> {
    let mut screen = Screen::new(view);
    let mut state = State::default();
    loop {
        let wait = screen.next_wake(&mut state, monotonic_now_ns());
        let first = match wait {
            Some(wait) => match events.recv_timeout(wait.max(MIN_WAIT)) {
                Ok(event) => Some(event),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            },
            None => match events.recv() {
                Ok(event) => Some(event),
                Err(_) => return Ok(()),
            },
        };
        // Apply everything already queued before drawing once.
        for event in first.into_iter().chain(events.try_iter()) {
            if state.apply(event, &mut screen, start)? == Flow::Done {
                screen.finish(state.status);
                return Ok(());
            }
        }
        screen.draw(&mut state, monotonic_now_ns());
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Flow {
    Continue,
    Done,
}

/// What the view knows, all of it from events.
struct State {
    clock: DisplayClock,
    status: Status,
    started: bool,
    /// An online source is fetching the current track.
    buffering: bool,
    now: Option<Arc<NowPlaying>>,
    lyrics: Option<Arc<LyricsSnapshot>>,
    theme: Option<Arc<jf_core::ThemeSnapshot>>,
}

impl Default for State {
    fn default() -> Self {
        State {
            clock: DisplayClock::new(),
            status: Status::Stopped,
            started: false,
            buffering: false,
            now: None,
            lyrics: None,
            theme: None,
        }
    }
}

impl State {
    fn apply(&mut self, event: Event, screen: &mut Screen, start: CommandId) -> Result<Flow> {
        match event {
            Event::NowPlaying(Some(np)) => {
                if self.now.as_ref().map(|n| n.uid) != Some(np.uid) {
                    screen.track_changed(&np, self.lyrics.as_deref());
                    self.lyrics = None;
                }
                self.now = Some(np);
            }
            Event::NowPlaying(None) => self.now = None,
            Event::Lyrics(l) => {
                if Some(l.uid) == self.now.as_ref().map(|n| n.uid) {
                    if l.doc.is_none() {
                        screen.note("(no lyrics found)");
                    }
                    self.lyrics = Some(l);
                }
            }
            Event::Theme(t) => self.theme = Some(t),
            Event::Buffering { buffering, .. } => self.buffering = buffering,
            Event::Playback(p) => {
                self.clock.update(p.anchor, monotonic_now_ns());
                self.status = p.status;
                self.started |= p.status == Status::Playing;
                if self.started && matches!(p.status, Status::Ended | Status::Stopped) {
                    return Ok(Flow::Done);
                }
            }
            Event::Error { message } => screen.error(&message),
            Event::CommandFailed { id, error } => {
                if id == start {
                    screen.clear_live();
                    bail!("{error}");
                }
                screen.error(&error);
            }
            _ => {}
        }
        Ok(Flow::Continue)
    }

    fn duration_us(&self) -> Option<i64> {
        let from_anchor = self.clock.anchor().and_then(|a| a.duration_us);
        from_anchor.or_else(|| {
            self.now
                .as_ref()?
                .duration_ms
                .map(|d| i64::try_from(d).unwrap_or(0) * 1_000)
        })
    }

    /// The lyrics of the current track and the cursor at `pos_us`.
    fn cursor(&self, pos_us: i64) -> Option<(&LyricsSnapshot, Cursor)> {
        let lyrics = self.lyrics.as_deref()?;
        let timeline = lyrics.timeline.as_ref()?;
        Some((lyrics, timeline.cursor(to_ms(pos_us))))
    }
}

fn to_ms(us: i64) -> u32 {
    u32::try_from(us.max(0) / 1_000).unwrap_or(u32::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Colour {
    None,
    Basic,
    True,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Sung,
    Unsung,
}

/// The terminal side: a live region at the bottom, permanent lines above it.
struct Screen {
    /// Draw the live region (a terminal, not quiet).
    live: bool,
    /// Print lyric lines as they start (output is not a terminal).
    plain: bool,
    lyrics: bool,
    quiet: bool,
    colour: Colour,
    light_background: bool,
    width: usize,
    /// Rows of the live region on screen now.
    live_rows: usize,
    /// The live frame last written, to skip identical redraws.
    frame: String,
    /// The lyric line shown live (or last printed), as (track uid, line).
    line: Option<(u64, usize)>,
}

impl Screen {
    fn new(view: &View) -> Self {
        let tty = std::io::stdout().is_terminal();
        let no_colour = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        let colour = match (tty, no_colour, std::env::var("COLORTERM").as_deref()) {
            (false, _, _) | (_, true, _) => Colour::None,
            (true, false, Ok("truecolor" | "24bit")) => Colour::True,
            _ => Colour::Basic,
        };
        // COLORFGBG is "fg;bg"; a background of 7 or 15 is a light terminal.
        let light_background = std::env::var("COLORFGBG")
            .ok()
            .and_then(|v| v.rsplit(';').next().and_then(|bg| bg.parse::<u8>().ok()))
            .is_some_and(|bg| bg == 7 || bg == 15);
        let width = std::env::var("COLUMNS")
            .ok()
            .and_then(|c| c.parse::<usize>().ok())
            .filter(|&w| w >= 40)
            .unwrap_or(80);
        Screen {
            live: tty && !view.quiet,
            plain: !tty && !view.quiet,
            lyrics: view.lyrics,
            quiet: view.quiet,
            colour,
            light_background,
            width,
            live_rows: 0,
            frame: String::new(),
            line: None,
        }
    }

    /// When the next visible change is due, or `None` to wait for events.
    fn next_wake(&self, state: &mut State, now_ns: u64) -> Option<Duration> {
        if state.status != Status::Playing || !(self.live || self.plain) {
            return None;
        }
        let pos = state.clock.position_us(now_ns);
        let mut target: Option<i64> = None;
        let mut consider = |t: i64| target = Some(target.map_or(t, |x| x.min(t)));
        if self.live {
            consider(next_second_us(pos));
            if let Some(step) = state
                .duration_us()
                .and_then(|d| next_step_us(pos, d / (BAR_CELLS as i64 * 2)))
            {
                consider(step);
            }
        }
        if self.lyrics
            && let Some((lyrics, cursor)) = state.cursor(pos)
            && let Some(timeline) = &lyrics.timeline
        {
            if let Some(next) = timeline.next_change(to_ms(pos)) {
                consider(i64::from(next) * 1_000);
            }
            // The sweep lights the current word one character at a time.
            if self.live
                && self.colour != Colour::None
                && let (Some(doc), Some(line), Some(word)) = (&lyrics.doc, cursor.line, cursor.word)
                && let Some(w) = doc.lines.get(line).and_then(|l| l.words.get(word))
            {
                let n = w.text.chars().count();
                if n > 1 && cursor.word_progress < 1.0 && w.end_ms > w.start_ms {
                    let lit = lit_chars(n, cursor.word_progress);
                    let at = f64::from(w.start_ms)
                        + (lit + 1) as f64 / n as f64 * f64::from(w.end_ms - w.start_ms);
                    consider((at * 1_000.0).ceil() as i64);
                }
            }
        }
        state.clock.wait_until(target?, now_ns).map(|d| d + WAKE_LATE)
    }

    fn draw(&mut self, state: &mut State, now_ns: u64) {
        if !(self.live || self.plain) || state.now.is_none() {
            return;
        }
        let pos = state.clock.position_us(now_ns);
        let uid = state.now.as_ref().map_or(0, |n| n.uid);

        // Lyric line changes: the old live line scrolls up as sung, the new
        // one becomes live (or, without a terminal, is printed now).
        let current = if self.lyrics { state.cursor(pos) } else { None };
        let key = current.and_then(|(_, c)| c.line).map(|line| (uid, line));
        if key != self.line {
            if let (Some((old_uid, old)), true) = (self.line, self.live)
                && old_uid == uid
                && let Some(doc) = state.lyrics.as_ref().and_then(|l| l.doc.as_ref())
                && let Some(line) = doc.lines.get(old)
            {
                self.commit_line(doc, line);
            }
            if self.plain
                && let Some((lyrics, cursor)) = current
                && let (Some(doc), Some(i)) = (&lyrics.doc, cursor.line)
                && let Some(line) = doc.lines.get(i)
            {
                if !line.text.is_empty() {
                    outln!("  {}", line.text);
                }
                for aux in aux_rows(doc, line) {
                    outln!("  {aux}");
                }
            }
            self.line = key;
        }
        if !self.live {
            return;
        }

        let mut rows = Vec::new();
        if let Some((lyrics, cursor)) = current
            && let (Some(doc), Some(i)) = (&lyrics.doc, cursor.line)
            && let Some(line) = doc.lines.get(i)
        {
            rows.push(format!("  {}", self.sweep(line, cursor, state.theme.as_deref())));
            rows.extend(
                aux_rows(doc, line)
                    .into_iter()
                    .map(|t| format!("  {}", dim(&self.fit(&t, 2)))),
            );
        }
        rows.push(self.progress_row(state, pos));
        let frame = rows.join("\n");
        if frame != self.frame {
            self.write_live(&rows);
            self.frame = frame;
        }
    }

    /// The lyric line with every sung character lit.
    fn sweep(&self, line: &jf_core::Line, cursor: Cursor, theme: Option<&jf_core::ThemeSnapshot>) -> String {
        let mut cells: Vec<(char, Tone)> = Vec::new();
        match cursor.word {
            Some(current) if !line.words.is_empty() => {
                for (i, word) in line.words.iter().enumerate() {
                    let n = word.text.chars().count();
                    let lit = match i.cmp(&current) {
                        std::cmp::Ordering::Less => n,
                        std::cmp::Ordering::Equal => lit_chars(n, cursor.word_progress),
                        std::cmp::Ordering::Greater => 0,
                    };
                    cells.extend(
                        word.text
                            .chars()
                            .enumerate()
                            .map(|(k, c)| (c, if k < lit { Tone::Sung } else { Tone::Unsung })),
                    );
                    if !word.joins_next && i + 1 < line.words.len() {
                        cells.push((' ', if i < current { Tone::Sung } else { Tone::Unsung }));
                    }
                }
            }
            // Line-timed lyrics light the whole line; a word-timed line
            // before its first word is all unsung.
            _ => {
                let tone = if line.words.is_empty() {
                    Tone::Sung
                } else {
                    Tone::Unsung
                };
                cells.extend(line.text.chars().map(|c| (c, tone)));
            }
        }
        let accent = theme.and_then(|t| {
            let theme = if self.light_background { &t.light } else { &t.dark };
            theme.color("accent.accent-text")
        });
        self.paint_cells(&self.fit_cells(cells, 2), accent)
    }

    fn progress_row(&self, state: &State, pos_us: i64) -> String {
        let icon = if state.status == Status::Paused {
            "‖"
        } else {
            "▶"
        };
        if state.buffering && state.status == Status::Playing {
            return format!("  ⋯ {}", dim("buffering"));
        }
        let pos_ms = u64::try_from(pos_us.max(0) / 1_000).unwrap_or(0);
        let Some(duration) = state.duration_us().filter(|&d| d > 0) else {
            return format!("  {icon} {}", fmt_ms(pos_ms));
        };
        let halves =
            ((pos_us as f64 / duration as f64).clamp(0.0, 1.0) * (BAR_CELLS * 2) as f64).floor() as usize;
        let (full, half) = (halves / 2, halves % 2 == 1);
        let mut done = "━".repeat(full);
        if half {
            done.push('╸');
        }
        let rest = "─".repeat(BAR_CELLS - full - usize::from(half));
        let accent = state.theme.as_deref().and_then(|t| {
            let theme = if self.light_background { &t.light } else { &t.dark };
            theme.color("accent.accent")
        });
        let duration_ms = u64::try_from(duration / 1_000).unwrap_or(0);
        format!(
            "  {icon} {}{}  {} / {}",
            self.paint_accent(&done, accent),
            dim(&rest),
            fmt_ms(pos_ms),
            fmt_ms(duration_ms)
        )
    }

    // ---- writing --------------------------------------------------------

    /// A new track: the previous track's live lyric line scrolls up as sung
    /// (a last line without an end stays live until the track ends), then the
    /// new track's header is printed.
    fn track_changed(&mut self, np: &NowPlaying, old_lyrics: Option<&LyricsSnapshot>) {
        if self.quiet {
            return;
        }
        if self.live
            && let Some((_, old)) = self.line
            && let Some(doc) = old_lyrics.and_then(|l| l.doc.as_ref())
            && let Some(line) = doc.lines.get(old)
        {
            self.commit_line(doc, line);
        }
        let mut header = format!("▶ {}", bold(&np.title));
        if let Some(artist) = &np.artist {
            header.push_str(&format!(" — {artist}"));
        }
        if let Some(album) = &np.album {
            header.push_str(&dim(&format!(" · {album}")));
        }
        if let Some(f) = &np.format {
            header.push_str(&dim(&format!(
                "  [{} {} Hz {}ch{}]",
                f.codec,
                f.sample_rate,
                f.channels,
                if f.gapless { ", gapless" } else { "" }
            )));
        }
        self.line = None;
        if self.live {
            self.commit(&header);
        } else {
            outln!("{header}");
        }
    }

    fn note(&mut self, text: &str) {
        if !self.lyrics || self.quiet {
            return;
        }
        let text = format!("  {}", dim(text));
        if self.live {
            self.commit(&text);
        } else {
            outln!("{text}");
        }
    }

    fn error(&mut self, message: &str) {
        self.clear_live();
        eprintln!("! {message}");
    }

    fn finish(&mut self, status: Status) {
        self.clear_live();
        if !self.quiet {
            outln!(
                "{}",
                dim(if status == Status::Ended {
                    "■ end of queue"
                } else {
                    "■ stopped"
                })
            );
        }
    }

    /// Scrolls a sung lyric line (and its translations) up above the live region.
    fn commit_line(&mut self, doc: &jf_core::LyricDoc, line: &jf_core::Line) {
        let mut rows = vec![format!(
            "  {}",
            self.paint(&self.fit(&line.text, 2), Tone::Unsung)
        )];
        rows.extend(
            aux_rows(doc, line)
                .into_iter()
                .map(|t| format!("  {}", dim(&self.fit(&t, 2)))),
        );
        self.commit(&rows.join("\n"));
    }

    /// Prints `text` as permanent output above the live region.
    fn commit(&mut self, text: &str) {
        self.clear_live();
        outln!("{text}");
    }

    /// Removes the live region; the next draw puts it back.
    fn clear_live(&mut self) {
        if self.live && self.live_rows > 0 {
            print!("{}\x1b[J", self.to_live_start());
            let _ = std::io::stdout().flush();
        }
        self.live_rows = 0;
        self.frame.clear();
    }

    fn write_live(&mut self, rows: &[String]) {
        let mut out = self.to_live_start();
        out.push_str("\x1b[J");
        out.push_str(&rows.join("\n"));
        self.live_rows = rows.len();
        print!("{out}");
        let _ = std::io::stdout().flush();
    }

    /// Moves the cursor to the first column of the live region's first row.
    fn to_live_start(&self) -> String {
        match self.live_rows {
            0 | 1 => "\r".to_string(),
            n => format!("\r\x1b[{}A", n - 1),
        }
    }

    // ---- styling ----------------------------------------------------------

    /// Cuts `text` to the terminal width less `indent` columns.
    fn fit(&self, text: &str, indent: usize) -> String {
        self.fit_cells(text.chars().map(|c| (c, Tone::Unsung)).collect(), indent)
            .into_iter()
            .map(|(c, _)| c)
            .collect()
    }

    fn fit_cells(&self, cells: Vec<(char, Tone)>, indent: usize) -> Vec<(char, Tone)> {
        let room = self.width.saturating_sub(indent + 1);
        let total: usize = cells.iter().map(|(c, _)| display_width(*c)).sum();
        if total <= room {
            return cells;
        }
        let mut used = 0;
        let mut out = Vec::new();
        for (c, tone) in cells {
            let w = display_width(c);
            if used + w + 1 > room {
                break;
            }
            used += w;
            out.push((c, tone));
        }
        out.push(('…', Tone::Unsung));
        out
    }

    fn paint(&self, text: &str, tone: Tone) -> String {
        match tone {
            Tone::Sung => bold(text),
            Tone::Unsung => dim(text),
        }
    }

    fn paint_accent(&self, text: &str, accent: Option<[u8; 4]>) -> String {
        match (self.colour, accent) {
            (Colour::None, _) => text.to_string(),
            (Colour::True, Some([r, g, b, _])) => format!("\x1b[38;2;{r};{g};{b}m{text}\x1b[0m"),
            _ => format!("\x1b[1m{text}\x1b[0m"),
        }
    }

    /// Runs of sung and unsung characters, with as few escape codes as possible.
    fn paint_cells(&self, cells: &[(char, Tone)], accent: Option<[u8; 4]>) -> String {
        if self.colour == Colour::None {
            return cells.iter().map(|(c, _)| c).collect();
        }
        let sung = match (self.colour, accent) {
            (Colour::True, Some([r, g, b, _])) => format!("\x1b[0;1;38;2;{r};{g};{b}m"),
            _ => "\x1b[0;1m".to_string(),
        };
        let mut out = String::new();
        let mut current: Option<Tone> = None;
        for &(c, tone) in cells {
            if current != Some(tone) {
                out.push_str(if tone == Tone::Sung { &sung } else { "\x1b[0;2m" });
                current = Some(tone);
            }
            out.push(c);
        }
        out.push_str("\x1b[0m");
        out
    }
}

/// Characters of an `n`-character word lit at `progress`.
fn lit_chars(n: usize, progress: f32) -> usize {
    ((progress.clamp(0.0, 1.0) * n as f32).floor() as usize).min(n)
}

/// Translation and romanization lines for `line`.
fn aux_rows(doc: &jf_core::LyricDoc, line: &jf_core::Line) -> Vec<String> {
    doc.tracks
        .iter()
        .filter_map(|t| t.lines.get(&line.id).map(|a| a.text.clone()))
        .collect()
}

/// Sends the commands that set up a play run and returns the id of the
/// starting command.
pub fn start(core: &Core, tracks: Vec<jf_core::TrackRef>, resume: bool) -> Result<CommandId> {
    if tracks.is_empty() {
        if !resume {
            bail!("nothing playable matched");
        }
        return Ok(core.send(Command::Play));
    }
    Ok(core.send(Command::PlayTracks { tracks, start: 0 }))
}

/// True when the argument names an existing file or folder rather than
/// search words.
pub fn is_path(arg: &str) -> bool {
    Path::new(arg).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lights_whole_characters() {
        assert_eq!(lit_chars(4, 0.0), 0);
        assert_eq!(lit_chars(4, 0.49), 1);
        assert_eq!(lit_chars(4, 0.5), 2);
        assert_eq!(lit_chars(4, 1.0), 4);
        assert_eq!(lit_chars(4, 7.0), 4);
    }
}
