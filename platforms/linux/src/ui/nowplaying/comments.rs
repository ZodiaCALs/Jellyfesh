//! Now Playing's Comments pane (docs/ui.md "Comments", ADR-0022), for a
//! song whose source shows comments: "Comments from {Service}", Popular
//! (the hot comments) then Latest, each with the author, their picture, how
//! long ago, likes and the text; more load at the end of the list.
//! Read-only. Empty: "No Comments Yet". The comments are asked for when the
//! pane first shows for a song (`logic::comments` keeps what came).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Command, CommandId, CommentEntry, CommentsPage, ItemKind, ItemRef, NowPlaying, TrackRef};

use crate::logic::comments::{self, Ago, Comments};
use crate::logic::format;
use crate::ui::convert;
use crate::ui::i18n::{tr, trf, trn};
use crate::ui::state::app;
use crate::ui::widgets::art::Art;

/// How close to the end of the list the next page is asked for, in px.
const MORE_AT: f64 = 400.0;

struct State {
    list: gtk::Box,
    note: gtk::Label,
    spinner: gtk::Spinner,
    shown: RefCell<Comments>,
    /// The song on screen, when its source shows comments.
    song: RefCell<Option<ItemRef>>,
}

pub struct CommentsPane {
    pub root: gtk::ScrolledWindow,
    state: Rc<State>,
}

impl std::fmt::Debug for CommentsPane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommentsPane").finish_non_exhaustive()
    }
}

/// The item whose comments a source song has.
fn item_of(np: &NowPlaying) -> Option<ItemRef> {
    match &np.track {
        TrackRef::Source(s) => Some(ItemRef::Source {
            plugin: s.plugin.clone(),
            item_kind: ItemKind::Track,
            id: s.id.clone(),
        }),
        _ => None,
    }
}

/// Whether the pane is offered for the song playing.
pub fn offered(np: Option<&NowPlaying>) -> bool {
    let a = app();
    let sources = a.sources.borrow();
    comments::offered(&sources.plugins, np.map(|n| &n.track)).is_some()
}

impl CommentsPane {
    pub fn new() -> CommentsPane {
        let note = gtk::Label::builder().xalign(0.0).wrap(true).build();
        note.add_css_class("attribution");
        note.set_visible(false);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 12);
        list.add_css_class("comments");
        let spinner = gtk::Spinner::builder()
            .spinning(true)
            .halign(gtk::Align::Center)
            .visible(false)
            .build();
        spinner.update_property(&[gtk::accessible::Property::Label(&tr("Loading"))]);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.append(&note);
        body.append(&list);
        body.append(&spinner);
        let root = gtk::ScrolledWindow::builder()
            .child(&body)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .build();
        root.add_css_class("comments-panel");
        let state = Rc::new(State {
            list,
            note,
            spinner,
            shown: RefCell::new(Comments::default()),
            song: RefCell::new(None),
        });
        // Asked for while the pane shows.
        let weak = Rc::downgrade(&state);
        root.connect_map(move |_| {
            if let Some(s) = weak.upgrade() {
                s.ask();
            }
        });
        // More at the end of the list.
        let weak = Rc::downgrade(&state);
        root.vadjustment().connect_value_changed(move |adj| {
            if adj.upper() - adj.page_size() - adj.value() < MORE_AT
                && let Some(s) = weak.upgrade()
            {
                s.more();
            }
        });
        CommentsPane { root, state }
    }

    /// The song playing: a new one starts over.
    pub fn update(&self, now: Option<&Arc<NowPlaying>>) {
        let item = now.and_then(|n| item_of(n));
        let s = &self.state;
        if *s.song.borrow() == item {
            return;
        }
        *s.song.borrow_mut() = item;
        *s.shown.borrow_mut() = Comments::default();
        s.render();
        if self.root.is_mapped() {
            s.ask();
        }
    }

    /// A page of comments, when it is ours.
    pub fn page(&self, page: &CommentsPage) {
        let s = &self.state;
        if s.shown.borrow_mut().page(page) {
            s.render();
        }
    }

    /// A command failed: ours shows why.
    pub fn failed(&self, id: CommandId, message: &str) {
        let s = &self.state;
        if s.shown.borrow_mut().failed(id, message) {
            s.render();
        }
    }
}

impl State {
    /// Asks for the first page of the song on screen, once.
    fn ask(&self) {
        let Some(item) = self.song.borrow().clone() else {
            return;
        };
        {
            let shown = self.shown.borrow();
            if shown.item.as_ref() == Some(&item) {
                return;
            }
        }
        let request = app().send_quiet(Command::Comments {
            item: item.clone(),
            page: None,
        });
        *self.shown.borrow_mut() = Comments::start(item, request);
        self.render();
    }

    /// The next page, when there is one.
    fn more(&self) {
        let Some((item, next)) = self.shown.borrow().wants_more() else {
            return;
        };
        let request = app().send_quiet(Command::Comments {
            item,
            page: Some(next),
        });
        self.shown.borrow_mut().asked(request);
        self.spinner.set_visible(true);
    }

    fn render(&self) {
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        let shown = self.shown.borrow();
        let service = shown.attribution.clone().or_else(|| {
            let item = shown.item.as_ref()?;
            let ItemRef::Source { plugin, .. } = item else {
                return None;
            };
            Some(app().source_name(plugin))
        });
        if let Some(service) = service.filter(|_| shown.loaded) {
            self.note
                .set_label(&trf("Comments from {service}", &[("service", &service)]));
            self.note.set_visible(true);
        } else {
            self.note.set_visible(false);
        }
        if !shown.hot.is_empty() {
            self.list.append(&heading(&tr("Popular")));
            for c in &shown.hot {
                self.list.append(&comment(c));
            }
        }
        if !shown.comments.is_empty() {
            self.list.append(&heading(&tr("Latest")));
            for c in &shown.comments {
                self.list.append(&comment(c));
            }
        }
        if shown.empty() {
            let status = adw::StatusPage::builder().title(tr("No Comments Yet")).build();
            status.add_css_class("compact");
            self.list.append(&status);
        }
        if let Some(error) = &shown.error {
            let e = gtk::Label::builder().label(error).xalign(0.0).wrap(true).build();
            e.add_css_class("dim-label");
            self.list.append(&e);
        }
        self.spinner
            .set_visible(shown.item.is_some() && shown.request.is_some());
    }
}

fn heading(text: &str) -> gtk::Label {
    let h = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    h.add_css_class("panel-title");
    h
}

/// "5 minutes ago", "3 hours ago", "2 days ago"; the date after a month.
fn when(time_ms: i64) -> String {
    let now = glib::real_time() / 1000;
    match comments::ago(now, time_ms) {
        Ago::JustNow => tr("Just now"),
        Ago::Minutes(n) => trn("{n} minute ago", "{n} minutes ago", n),
        Ago::Hours(n) => trn("{n} hour ago", "{n} hours ago", n),
        Ago::Days(n) => trn("{n} day ago", "{n} days ago", n),
        Ago::Long => glib::DateTime::from_unix_local(time_ms / 1000)
            .ok()
            .and_then(|d| d.format("%x").ok())
            .map(|s| s.to_string())
            .unwrap_or_default(),
    }
}

/// One comment: the picture, the author and when, the likes, the text.
fn comment(c: &CommentEntry) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("comment");
    let avatar = Art::new(36, 18.0);
    avatar.set_round(true);
    avatar.set_valign(gtk::Align::Start);
    avatar.set_art(convert::art_of(c.avatar.as_ref()));
    row.append(&avatar);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 4);
    body.set_hexpand(true);
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let author = gtk::Label::builder()
        .label(&c.author)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    author.add_css_class("comment-author");
    top.append(&author);
    if let Some(t) = c.time_ms {
        let time = gtk::Label::new(Some(&when(t)));
        time.add_css_class("dim-label");
        time.add_css_class("caption");
        top.append(&time);
    }
    let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    top.append(&spacer);
    if let Some(likes) = c.likes.filter(|l| *l > 0) {
        let like = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        like.append(&crate::ui::widgets::glyph::Glyph::new("heart", 14));
        let count = gtk::Label::new(Some(&format::count(likes)));
        count.add_css_class("caption");
        like.append(&count);
        like.add_css_class("dim-label");
        like.update_property(&[gtk::accessible::Property::Label(&trn(
            "{n} like",
            "{n} likes",
            likes,
        ))]);
        top.append(&like);
    }
    body.append(&top);
    let text = gtk::Label::builder()
        .label(&c.text)
        .xalign(0.0)
        .wrap(true)
        .wrap_mode(gtk::pango::WrapMode::WordChar)
        .selectable(true)
        .build();
    text.add_css_class("comment-text");
    body.append(&text);
    row.append(&body);
    row
}
