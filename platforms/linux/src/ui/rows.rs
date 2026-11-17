//! Rows and cards for list and grid views, bound to [`Item`]s.
//!
//! A row is 52 px (token `player.row-height`): leading art, number or
//! avatar, two lines of type (`type-roles.list-title`, `list-secondary`),
//! and trailing detail and buttons. Rows are recycled by the views; binding
//! connects to the item so a placeholder fills in when its page arrives.
//!
//! After Apple Music and Media Player (design-controls.md, "Library"): a
//! track number turns into a play glyph while its row is hovered; the row
//! of the song that is playing is tinted (`accent-soft`, accent text) and
//! shows a level glyph that moves only while playing; an album card lifts
//! on hover and shows a glass play button that plays the album.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use jf_core::TrackRef;

use crate::logic::format;
use crate::logic::links::{self, MainAction};
use crate::ui::actions;
use crate::ui::i18n::{tr, trf};
use crate::ui::models::{Item, ItemData, Kind};
use crate::ui::state::{Change, app, try_app};
use crate::ui::widgets::art::Art;
use crate::ui::widgets::glyph::{self, Glyph};
use crate::ui::widgets::level::Level;

/// What a row shows before its two lines of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leading {
    Art,
    Number,
    Avatar,
}

/// Which parts a row has.
#[derive(Debug, Clone, Copy)]
pub struct RowStyle {
    pub leading: Leading,
    pub heart: bool,
    pub more: bool,
    pub remove: bool,
    pub drag: bool,
}

impl RowStyle {
    pub const TRACK: RowStyle = RowStyle {
        leading: Leading::Art,
        heart: true,
        more: true,
        remove: false,
        drag: false,
    };
    pub const ALBUM_TRACK: RowStyle = RowStyle {
        leading: Leading::Number,
        heart: true,
        more: true,
        remove: false,
        drag: false,
    };
    pub const ARTIST: RowStyle = RowStyle {
        leading: Leading::Avatar,
        heart: false,
        more: true,
        remove: false,
        drag: false,
    };
    pub const PLAYLIST: RowStyle = RowStyle {
        leading: Leading::Art,
        heart: false,
        more: true,
        remove: false,
        drag: false,
    };
    pub const QUEUE: RowStyle = RowStyle {
        leading: Leading::Art,
        heart: false,
        more: false,
        remove: true,
        drag: true,
    };
    pub const PLAYLIST_ENTRY: RowStyle = RowStyle {
        leading: Leading::Art,
        heart: true,
        more: true,
        remove: true,
        drag: true,
    };
}

pub type Handler = Rc<dyn Fn(&ItemData)>;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct ListRow {
        pub art: RefCell<Option<Art>>,
        pub number: RefCell<Option<gtk::Label>>,
        /// Over the number or the artwork: the play glyph on hover, the
        /// level glyph on the playing row.
        pub level: RefCell<Option<Level>>,
        pub playing: Cell<bool>,
        pub avatar: RefCell<Option<gtk::Label>>,
        pub title: gtk::Label,
        pub subtitle: gtk::Label,
        pub detail: gtk::Label,
        /// Shown on rows that open in their service (they cannot play
        /// here, but have a link).
        pub link: RefCell<Option<Glyph>>,
        pub heart: RefCell<Option<gtk::Button>>,
        pub remove: RefCell<Option<gtk::Button>>,
        pub item: RefCell<Option<(Item, glib::SignalHandlerId)>>,
        pub on_remove: RefCell<Option<Handler>>,
    }

    impl std::fmt::Debug for ListRow {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ListRow").finish_non_exhaustive()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ListRow {
        const NAME: &'static str = "JfListRow";
        type Type = super::ListRow;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for ListRow {}
    impl WidgetImpl for ListRow {}
    impl BoxImpl for ListRow {}

    #[derive(Default)]
    pub struct Card {
        pub art: RefCell<Option<Art>>,
        pub play: RefCell<Option<gtk::Button>>,
        pub title: gtk::Label,
        pub subtitle: gtk::Label,
        pub item: RefCell<Option<(Item, glib::SignalHandlerId)>>,
    }

    impl std::fmt::Debug for Card {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Card").finish_non_exhaustive()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Card {
        const NAME: &'static str = "JfCard";
        type Type = super::Card;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for Card {}
    impl WidgetImpl for Card {}
    impl BoxImpl for Card {}
}

glib::wrapper! {
    /// A list row: art or number, title and subtitle, detail and buttons.
    pub struct ListRow(ObjectSubclass<imp::ListRow>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl ListRow {
    pub fn new(style: RowStyle) -> ListRow {
        let row: ListRow = glib::Object::builder()
            .property("orientation", gtk::Orientation::Horizontal)
            .property("spacing", 12)
            .build();
        row.add_css_class("list-row");
        let imp = row.imp();
        if style.drag {
            let handle = Glyph::new("drag", 16);
            handle.add_css_class("drag-handle");
            row.append(&handle);
        }
        let level = Level::new(14);
        level.add_css_class("row-level");
        level.set_visible(false);
        match style.leading {
            Leading::Art => {
                let art = Art::new(40, jf_tokens::artwork::RADIUS_LIST);
                art.set_size_request(40, 40);
                art.set_valign(gtk::Align::Center);
                art.set_hexpand(false);
                // The playing row's level glyph sits on a scrim over the art.
                level.add_css_class("over-art");
                let stack = gtk::Overlay::new();
                stack.set_child(Some(&art));
                stack.add_overlay(&level);
                stack.set_valign(gtk::Align::Center);
                row.append(&stack);
                *imp.art.borrow_mut() = Some(art);
            }
            Leading::Number => {
                let n = gtk::Label::builder().width_chars(3).xalign(1.0).build();
                n.add_css_class("row-number");
                n.add_css_class("numeric");
                // Hovered, the number turns into a play glyph.
                let hover = Glyph::new("play", 14);
                hover.add_css_class("row-play");
                hover.set_halign(gtk::Align::End);
                hover.set_margin_end(4);
                level.set_halign(gtk::Align::End);
                level.set_margin_end(4);
                let stack = gtk::Overlay::new();
                stack.set_child(Some(&n));
                stack.add_overlay(&hover);
                stack.add_overlay(&level);
                stack.add_css_class("row-lead");
                row.append(&stack);
                *imp.number.borrow_mut() = Some(n);
            }
            Leading::Avatar => {
                let a = gtk::Label::new(None);
                a.add_css_class("avatar");
                a.set_size_request(40, 40);
                a.set_valign(gtk::Align::Center);
                row.append(&a);
                *imp.avatar.borrow_mut() = Some(a);
            }
        }
        if style.leading != Leading::Avatar {
            *imp.level.borrow_mut() = Some(level);
        }
        register(&row);
        let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
        texts.set_valign(gtk::Align::Center);
        texts.set_hexpand(true);
        for (label, class) in [(&imp.title, "row-title"), (&imp.subtitle, "row-subtitle")] {
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_single_line_mode(true);
            label.add_css_class(class);
            texts.append(label);
        }
        row.append(&texts);
        imp.detail.add_css_class("row-detail");
        imp.detail.add_css_class("numeric");
        imp.detail.set_valign(gtk::Align::Center);
        imp.detail.set_ellipsize(gtk::pango::EllipsizeMode::End);
        imp.detail.set_max_width_chars(28);
        row.append(&imp.detail);
        let link = Glyph::new("link", 16);
        link.add_css_class("row-link");
        link.set_valign(gtk::Align::Center);
        link.set_visible(false);
        row.append(&link);
        *imp.link.borrow_mut() = Some(link);
        if style.heart {
            let heart = glyph::button("heart", 18, &tr("Add to Favourites"));
            heart.add_css_class("row-button");
            heart.set_valign(gtk::Align::Center);
            heart.connect_clicked(glib::clone!(
                #[weak]
                row,
                move |_| {
                    if let Some(data) = row.data() {
                        actions::toggle_favorite(&row, &data);
                    }
                }
            ));
            row.append(&heart);
            *imp.heart.borrow_mut() = Some(heart);
        }
        if style.more {
            let more = glyph::button("more", 18, &tr("More"));
            more.add_css_class("row-button");
            more.set_valign(gtk::Align::Center);
            more.connect_clicked(glib::clone!(
                #[weak]
                row,
                move |b| {
                    if let Some(data) = row.data() {
                        actions::popup_menu(b, &data);
                    }
                }
            ));
            row.append(&more);
        }
        if style.remove {
            let remove = glyph::button("close", 16, &tr("Remove"));
            remove.add_css_class("row-button");
            remove.set_valign(gtk::Align::Center);
            remove.connect_clicked(glib::clone!(
                #[weak]
                row,
                move |_| {
                    let handler = row.imp().on_remove.borrow().clone();
                    if let (Some(h), Some(data)) = (handler, row.data()) {
                        h(&data);
                    }
                }
            ));
            row.append(&remove);
            *imp.remove.borrow_mut() = Some(remove);
        }
        // Right click or long press: the item's menu.
        let click = gtk::GestureClick::builder().button(3).build();
        click.connect_pressed(glib::clone!(
            #[weak]
            row,
            move |g, _, x, y| {
                if let Some(data) = row.data() {
                    g.set_state(gtk::EventSequenceState::Claimed);
                    actions::popup_menu_at(&row, &data, x, y);
                }
            }
        ));
        row.add_controller(click);
        let press = gtk::GestureLongPress::new();
        press.connect_pressed(glib::clone!(
            #[weak]
            row,
            move |_, x, y| {
                if let Some(data) = row.data() {
                    actions::popup_menu_at(&row, &data, x, y);
                }
            }
        ));
        row.add_controller(press);
        row
    }

    /// What the remove button does (queue: remove the entry; playlist:
    /// remove from the playlist).
    pub fn set_on_remove(&self, handler: Handler) {
        *self.imp().on_remove.borrow_mut() = Some(handler);
    }

    pub fn data(&self) -> Option<ItemData> {
        self.imp().item.borrow().as_ref().and_then(|(i, _)| i.data())
    }

    pub fn item(&self) -> Option<Item> {
        self.imp().item.borrow().as_ref().map(|(i, _)| i.clone())
    }

    pub fn bind(&self, item: &Item) {
        self.unbind();
        let id = item.connect_changed(glib::clone!(
            #[weak(rename_to = row)]
            self,
            move |_| row.update()
        ));
        *self.imp().item.borrow_mut() = Some((item.clone(), id));
        self.update();
    }

    pub fn unbind(&self) {
        if let Some((item, id)) = self.imp().item.borrow_mut().take() {
            item.disconnect(id);
        }
    }

    fn update(&self) {
        let imp = self.imp();
        let data = self.data();
        let d = data.as_ref();
        imp.title.set_label(d.map_or("", |d| d.title.as_str()));
        imp.subtitle.set_label(d.map_or("", |d| d.subtitle.as_str()));
        imp.subtitle
            .set_visible(d.is_some_and(|d| !d.subtitle.is_empty()));
        imp.detail.set_label(d.map_or("", |d| d.detail.as_str()));
        if let Some(art) = imp.art.borrow().as_ref() {
            art.set_art(d.and_then(|d| d.art.clone()));
            art.set_round(d.is_some_and(|d| d.kind == Kind::Artist));
        }
        if let Some(n) = imp.number.borrow().as_ref() {
            n.set_label(
                &d.and_then(|d| d.number)
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
            );
        }
        if let Some(a) = imp.avatar.borrow().as_ref() {
            a.set_label(&d.map(|d| format::initials(&d.title)).unwrap_or_default());
        }
        if let Some(h) = imp.heart.borrow().as_ref() {
            let fav = d.is_some_and(|d| d.favorite);
            if let Some(g) = glyph::of(h) {
                g.set_name_glyph(if fav { "heart-filled" } else { "heart" });
            }
            let label = if fav {
                tr("Remove from Favourites")
            } else {
                tr("Add to Favourites")
            };
            h.set_tooltip_text(Some(&label));
            h.update_property(&[gtk::accessible::Property::Label(&label)]);
            h.set_visible(d.is_some_and(|d| d.track.is_some()));
            if fav {
                h.add_css_class("on");
            } else {
                h.remove_css_class("on");
            }
        }
        // Items that cannot play here but have a link open in their service
        // (docs/ui.md, "Links and remote playback"); others are unavailable.
        let opens_link = d.is_some_and(|d| {
            d.source
                .as_deref()
                .is_some_and(|s| links::main_action(s) == MainAction::OpenLink)
        });
        let playable = d.is_none_or(|d| d.playable);
        self.set_sensitive(playable || opens_link);
        if playable {
            self.remove_css_class("unplayable");
        } else {
            self.add_css_class("unplayable");
        }
        let service = opens_link
            .then(|| d.and_then(actions::link_of).map(|(_, service)| service))
            .flatten();
        if let Some(l) = imp.link.borrow().as_ref() {
            l.set_visible(service.is_some());
        }
        self.set_tooltip_text(service.map(|s| actions::open_in_label(&s)).as_deref());
        // One accessible name for the row: title and subtitle (and why it
        // cannot play).
        if let Some(d) = d {
            let reason = (!playable).then_some(d.detail.as_str());
            let name = format::dot_join([Some(d.title.as_str()), Some(d.subtitle.as_str()), reason]);
            self.update_property(&[gtk::accessible::Property::Label(&name)]);
        }
        self.update_playing();
    }

    /// Marks the row of the song that is playing (tint and level glyph).
    fn update_playing(&self) {
        let imp = self.imp();
        let now = try_app().and_then(|a| a.shown_track());
        let track = imp
            .item
            .borrow()
            .as_ref()
            .and_then(|(i, _)| i.with(|d| d.and_then(|d| d.track.clone())));
        let current = is_current(track.as_ref(), now.as_ref());
        let playing = current && try_app().is_some_and(|a| a.is_playing());
        if current {
            self.add_css_class("playing");
        } else {
            self.remove_css_class("playing");
        }
        imp.playing.set(current);
        if let Some(level) = imp.level.borrow().as_ref() {
            level.set_visible(current);
            level.set_playing(playing);
        }
    }
}

/// Whether a row's track is the one playing.
fn is_current(track: Option<&TrackRef>, now: Option<&TrackRef>) -> bool {
    matches!((track, now), (Some(a), Some(b)) if a == b)
}

thread_local! {
    /// Every list row, to mark the playing one when playback changes.
    static ROWS: RefCell<Vec<glib::WeakRef<ListRow>>> = const { RefCell::new(Vec::new()) };
    static LISTENING: Cell<bool> = const { Cell::new(false) };
}

/// Keeps `row` in the list of rows that follow playback.
fn register(row: &ListRow) {
    ROWS.with(|rows| {
        let mut rows = rows.borrow_mut();
        // Forget rows that are gone now and then.
        if rows.len() % 64 == 63 {
            rows.retain(|w| w.upgrade().is_some());
        }
        rows.push(row.downgrade());
    });
    if !LISTENING.with(|l| l.replace(true)) {
        app().listen(|change| {
            if matches!(change, Change::NowPlaying | Change::Playback | Change::Remote) {
                let rows: Vec<ListRow> =
                    ROWS.with(|r| r.borrow().iter().filter_map(|w| w.upgrade()).collect());
                for row in rows {
                    row.update_playing();
                }
            }
        });
    }
}

glib::wrapper! {
    /// A grid card: artwork, title and subtitle.
    pub struct Card(ObjectSubclass<imp::Card>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl Card {
    pub fn new(size: i32) -> Card {
        let card: Card = glib::Object::builder()
            .property("orientation", gtk::Orientation::Vertical)
            .property("spacing", 6)
            .build();
        card.add_css_class("card-tile");
        let imp = card.imp();
        let art = Art::new(size, jf_tokens::artwork::RADIUS_GRID);
        // Grid views size columns by the narrowest child: never so wide
        // that two columns miss a 360 px window.
        art.set_min_size((size * 7 / 8).min(crate::logic::layout::GRID_COVER_MIN));
        art.add_css_class("grid-art");
        // Hovering lifts the cover and shows a glass play button in its
        // bottom start corner that plays the album.
        let play = gtk::Button::builder()
            .child(&Glyph::new("play", 18))
            .halign(gtk::Align::Start)
            .valign(gtk::Align::End)
            .build();
        play.add_css_class("card-play");
        play.add_css_class("veil");
        play.connect_clicked(glib::clone!(
            #[weak]
            card,
            move |_| {
                if let Some(c) = card.data().as_ref().and_then(actions::collection_of) {
                    actions::play(c, 0, false);
                }
            }
        ));
        let cover = gtk::Overlay::new();
        cover.add_css_class("card-cover");
        cover.set_child(Some(&art));
        cover.add_overlay(&play);
        card.append(&cover);
        *imp.art.borrow_mut() = Some(art);
        *imp.play.borrow_mut() = Some(play);
        for (label, class) in [(&imp.title, "card-title"), (&imp.subtitle, "card-subtitle")] {
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            label.set_single_line_mode(true);
            label.add_css_class(class);
            card.append(label);
        }
        let click = gtk::GestureClick::builder().button(3).build();
        click.connect_pressed(glib::clone!(
            #[weak]
            card,
            move |g, _, x, y| {
                if let Some(data) = card.data() {
                    g.set_state(gtk::EventSequenceState::Claimed);
                    actions::popup_menu_at(&card, &data, x, y);
                }
            }
        ));
        card.add_controller(click);
        card
    }

    pub fn data(&self) -> Option<ItemData> {
        self.imp().item.borrow().as_ref().and_then(|(i, _)| i.data())
    }

    /// Makes a card outside a grid view open its item when clicked or when
    /// Enter or Space is pressed on it (it takes the keyboard focus). Its
    /// play button keeps its own click.
    pub fn set_open(&self, open: impl Fn(&ItemData) + 'static) {
        self.set_focusable(true);
        self.add_css_class("card-button");
        self.set_cursor_from_name(Some("pointer"));
        let open = Rc::new(open);
        let click = gtk::GestureClick::builder().button(1).build();
        let o = open.clone();
        click.connect_released(glib::clone!(
            #[weak(rename_to = card)]
            self,
            move |g, _, x, y| {
                if card.contains(x, y)
                    && let Some(d) = card.data()
                {
                    g.set_state(gtk::EventSequenceState::Claimed);
                    o(&d);
                }
            }
        ));
        self.add_controller(click);
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(glib::clone!(
            #[weak(rename_to = card)]
            self,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| match key {
                gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter | gtk::gdk::Key::space => {
                    if let Some(d) = card.data() {
                        open(&d);
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        ));
        self.add_controller(keys);
    }

    pub fn bind(&self, item: &Item) {
        self.unbind();
        let id = item.connect_changed(glib::clone!(
            #[weak(rename_to = card)]
            self,
            move |_| card.update()
        ));
        *self.imp().item.borrow_mut() = Some((item.clone(), id));
        self.update();
    }

    pub fn unbind(&self) {
        if let Some((item, id)) = self.imp().item.borrow_mut().take() {
            item.disconnect(id);
        }
    }

    fn update(&self) {
        let imp = self.imp();
        let data = self.data();
        let d = data.as_ref();
        imp.title.set_label(d.map_or("", |d| d.title.as_str()));
        imp.subtitle.set_label(d.map_or("", |d| d.subtitle.as_str()));
        if let Some(art) = imp.art.borrow().as_ref() {
            art.set_art(d.and_then(|d| d.art.clone()));
        }
        if let Some(play) = imp.play.borrow().as_ref() {
            let playable = d.is_some_and(|d| d.playable && actions::collection_of(d).is_some());
            play.set_visible(playable);
            let label = trf("Play {title}", &[("title", d.map_or("", |d| d.title.as_str()))]);
            play.set_tooltip_text(Some(&label));
            play.update_property(&[gtk::accessible::Property::Label(&label)]);
        }
        if let Some(d) = d {
            let name = format::dot_join([Some(d.title.as_str()), Some(d.subtitle.as_str())]);
            self.update_property(&[gtk::accessible::Property::Label(&name)]);
        }
    }
}

/// A factory of list rows in `style`.
pub fn row_factory(style: RowStyle, on_remove: Option<Handler>) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, obj| {
        let Some(li) = obj.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let row = ListRow::new(style);
        if let Some(h) = &on_remove {
            row.set_on_remove(h.clone());
        }
        li.set_child(Some(&row));
    });
    factory.connect_bind(|_, obj| {
        let Some(li) = obj.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        if let (Some(row), Some(item)) = (
            li.child().and_downcast::<ListRow>(),
            li.item().and_downcast::<Item>(),
        ) {
            row.bind(&item);
        }
    });
    factory.connect_unbind(|_, obj| {
        let Some(li) = obj.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        if let Some(row) = li.child().and_downcast::<ListRow>() {
            row.unbind();
        }
    });
    factory
}

/// A factory of grid cards with artwork of natural `size`.
pub fn card_factory(size: i32) -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(move |_, obj| {
        let Some(li) = obj.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        li.set_child(Some(&Card::new(size)));
    });
    factory.connect_bind(|_, obj| {
        let Some(li) = obj.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        if let (Some(card), Some(item)) = (
            li.child().and_downcast::<Card>(),
            li.item().and_downcast::<Item>(),
        ) {
            card.bind(&item);
        }
    });
    factory.connect_unbind(|_, obj| {
        let Some(li) = obj.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        if let Some(card) = li.child().and_downcast::<Card>() {
            card.unbind();
        }
    });
    factory
}
