//! Now Playing's Comments pane (docs/ui.md "Comments", ADR-0022): a song's
//! comments as they load page by page from its source, Popular (the first
//! page's hot comments) then Latest; read-only and never stored. The core
//! fetches them (`Command::Comments`); this keeps what came and says what
//! shows, and how long ago a comment was written.

use jf_core::{CommandId, CommentEntry, CommentsPage, ItemRef, PluginSummary, TrackRef};

/// The comments of one song so far.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Comments {
    pub item: Option<ItemRef>,
    /// The `Comments` command waiting for its page.
    pub request: Option<CommandId>,
    pub hot: Vec<CommentEntry>,
    pub comments: Vec<CommentEntry>,
    pub total: Option<u64>,
    pub next: Option<String>,
    pub attribution: Option<String>,
    pub error: Option<String>,
    /// At least one page came (or the request failed).
    pub loaded: bool,
}

/// The plugin of a source song whose source shows comments: Now Playing
/// offers the pane.
pub fn offered<'a>(plugins: &'a [PluginSummary], track: Option<&TrackRef>) -> Option<&'a PluginSummary> {
    let Some(TrackRef::Source(s)) = track else {
        return None;
    };
    plugins
        .iter()
        .find(|p| p.id == s.plugin && p.enabled && p.comments)
}

impl Comments {
    /// A new song: its first page, asked with `request`.
    pub fn start(item: ItemRef, request: CommandId) -> Comments {
        Comments {
            item: Some(item),
            request: Some(request),
            ..Comments::default()
        }
    }

    /// The next page to ask for, when there is one and none is on its way.
    pub fn wants_more(&self) -> Option<(ItemRef, String)> {
        if self.request.is_some() || self.error.is_some() {
            return None;
        }
        Some((self.item.clone()?, self.next.clone()?))
    }

    /// The next page was asked for with `request`.
    pub fn asked(&mut self, request: CommandId) {
        self.request = Some(request);
    }

    /// A page; true when it answered our request (and so changed what
    /// shows). The hot comments come with the first page only; a comment
    /// seen before is not shown twice.
    pub fn page(&mut self, page: &CommentsPage) -> bool {
        if self.request != Some(page.request) || self.item.as_ref() != Some(&page.item) {
            return false;
        }
        if !self.loaded {
            self.hot = page.hot.clone();
        }
        for c in &page.comments {
            if !self.comments.iter().any(|seen| seen.id == c.id) {
                self.comments.push(c.clone());
            }
        }
        self.request = None;
        self.total = page.total.or(self.total);
        self.next = page.next.clone();
        self.attribution = page.attribution.clone().or(self.attribution.take());
        self.error = page.error.clone().filter(|e| !e.trim().is_empty());
        self.loaded = true;
        true
    }

    /// The request failed; true when it was ours.
    pub fn failed(&mut self, request: CommandId, message: &str) -> bool {
        if self.request != Some(request) {
            return false;
        }
        self.request = None;
        self.error = Some(message.to_owned());
        self.loaded = true;
        true
    }

    /// "No Comments Yet": a page came with nothing in it.
    pub fn empty(&self) -> bool {
        self.loaded && self.error.is_none() && self.hot.is_empty() && self.comments.is_empty()
    }
}

/// How long ago something was written, for the UI to word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ago {
    /// Under a minute (or a clock that runs behind).
    JustNow,
    Minutes(u64),
    Hours(u64),
    Days(u64),
    /// A month or more: the date shows.
    Long,
}

pub fn ago(now_ms: i64, then_ms: i64) -> Ago {
    let seconds = (now_ms.saturating_sub(then_ms) / 1000).max(0) as u64;
    match seconds {
        0..60 => Ago::JustNow,
        60..3_600 => Ago::Minutes(seconds / 60),
        3_600..86_400 => Ago::Hours(seconds / 3_600),
        86_400..2_592_000 => Ago::Days(seconds / 86_400),
        _ => Ago::Long,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str) -> CommentEntry {
        CommentEntry {
            id: id.into(),
            author: "Ama".into(),
            avatar: None,
            text: "Lovely".into(),
            likes: Some(3),
            time_ms: Some(0),
            replies: None,
        }
    }

    fn item() -> ItemRef {
        ItemRef::Source {
            plugin: "netease".into(),
            item_kind: jf_core::ItemKind::Track,
            id: "7".into(),
        }
    }

    fn answer(request: u64, hot: &[&str], comments: &[&str], next: Option<&str>) -> CommentsPage {
        CommentsPage {
            request: CommandId(request),
            item: item(),
            hot: hot.iter().map(|id| entry(id)).collect(),
            comments: comments.iter().map(|id| entry(id)).collect(),
            total: Some(40),
            next: next.map(str::to_owned),
            attribution: Some("NetEase Cloud Music".into()),
            error: None,
        }
    }

    #[test]
    fn pages_add_up_without_repeats() {
        let mut c = Comments::start(item(), CommandId(1));
        assert!(c.wants_more().is_none(), "the first page is on its way");
        assert!(!c.page(&answer(9, &[], &["x"], None)), "not ours");
        assert!(c.page(&answer(1, &["h1"], &["a", "b"], Some("p2"))));
        assert_eq!(c.wants_more(), Some((item(), "p2".into())));
        c.asked(CommandId(2));
        assert!(c.wants_more().is_none());
        assert!(c.page(&answer(2, &["h2"], &["b", "c"], None)));
        let ids: Vec<&str> = c.comments.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
        assert_eq!(c.hot.len(), 1, "hot comments come with the first page only");
        assert_eq!(c.hot[0].id, "h1");
        assert!(c.wants_more().is_none(), "no more pages");
        assert!(!c.empty());
    }

    #[test]
    fn nothing_and_failures() {
        let mut c = Comments::start(item(), CommandId(1));
        assert!(!c.empty(), "still loading");
        assert!(c.page(&answer(1, &[], &[], None)));
        assert!(c.empty());
        let mut c = Comments::start(item(), CommandId(4));
        assert!(!c.failed(CommandId(3), "no"));
        assert!(c.failed(CommandId(4), "The service is busy."));
        assert_eq!(c.error.as_deref(), Some("The service is busy."));
        assert!(!c.empty());
        assert!(c.wants_more().is_none());
    }

    #[test]
    fn how_long_ago() {
        let minute = 60_000;
        assert_eq!(ago(10 * minute, 10 * minute + 5_000), Ago::JustNow);
        assert_eq!(ago(10 * minute, 10 * minute - 30_000), Ago::JustNow);
        assert_eq!(ago(10 * minute, 9 * minute), Ago::Minutes(1));
        assert_eq!(ago(10 * minute, 0), Ago::Minutes(10));
        assert_eq!(ago(5 * 60 * minute, 0), Ago::Hours(5));
        assert_eq!(ago(3 * 24 * 60 * minute, 0), Ago::Days(3));
        assert_eq!(ago(40 * 24 * 60 * minute, 0), Ago::Long);
    }
}
