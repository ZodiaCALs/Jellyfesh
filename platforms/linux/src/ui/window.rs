//! The main window, laid out as Jellyfesh Web (ADR-0023): from 761 px wide
//! the floating sidebar panel (the wordmark, the library's places, a
//! separator, Search and Sources, then Open Files… and Settings at its
//! foot) beside the pages; up to 760 px the pages over the whole width with
//! the phone tabs floating at the bottom. The section and detail pages are
//! in a navigation view; Settings is one of its pages. The player floats
//! over every page (the capsule under 900 px, the player bar from 900 px)
//! and hides while Now Playing covers everything. The pages fade out just
//! above the floating player and tabs (Luminous).
//!
//! Motion (design-controls.md, "Motion"): pages fade in and rise 8 px, going
//! back fades, Now Playing slides up and fades in ("Deeper") and closes on
//! Exit ("Back"); with reduced motion each is a short fade, with the
//! system's animations off nothing animates.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, glib};

use crate::logic::layout::{self, Chrome};
use crate::logic::motion::{Transition, plan};
use crate::logic::nav::{self, Tab};
use crate::ui::i18n::{tr, trc};
use crate::ui::models::ItemData;
use crate::ui::nowplaying::{NowPlaying, Tab as NowPlayingTab};
use crate::ui::state::{App, Change, app, try_app};
use crate::ui::widgets::fade::FadeBin;
use crate::ui::widgets::glyph::Glyph;
use crate::ui::widgets::motion::MotionBin;
use crate::ui::{actions, miniplayer, pages, playerbar};

pub use crate::logic::nav::Section;

/// A section's name, as the sidebar and the phone tabs show it.
pub fn title(section: Section) -> String {
    match section {
        Section::Home => trc("sidebar", "Home"),
        Section::Albums => trc("sidebar", "Albums"),
        Section::Artists => trc("sidebar", "Artists"),
        Section::Songs => trc("sidebar", "Songs"),
        Section::Playlists => trc("sidebar", "Playlists"),
        Section::Favourites => trc("sidebar", "Favourites"),
        Section::Downloads => trc("sidebar", "Downloads"),
        Section::Search => trc("sidebar", "Search"),
        Section::Sources => trc("sidebar", "Sources"),
        Section::Settings => trc("sidebar", "Settings"),
    }
}

fn tab_title(tab: Tab) -> String {
    match tab {
        Tab::Home => title(Section::Home),
        Tab::Library => trc("sidebar", "Library"),
        Tab::Search => title(Section::Search),
        Tab::Sources => title(Section::Sources),
        Tab::Settings => title(Section::Settings),
    }
}

/// The look's places (ADR-0020), from the core: all eight while it is not
/// customized.
fn look_places() -> Vec<jf_core::Place> {
    try_app().map_or_else(
        || crate::logic::look::sidebar(&jf_core::LookSnapshot::defaults(0)),
        |a| crate::logic::look::sidebar(&a.look.borrow()),
    )
}

/// A row of the sidebar panel: its glyph and name.
fn nav_row(tag: &str, glyph: &str, label: &str) -> gtk::ListBoxRow {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    b.append(&Glyph::new(glyph, 20));
    let text = gtk::Label::new(Some(label));
    text.set_xalign(0.0);
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    b.append(&text);
    let row = gtk::ListBoxRow::builder().child(&b).build();
    row.add_css_class("nav-item");
    row.set_widget_name(tag);
    row.update_property(&[gtk::accessible::Property::Label(label)]);
    row
}

/// The sidebar's places, in the look's order (the header function puts a
/// separator before Search and Sources).
fn fill_places(list: &gtk::ListBox) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
    for (section, _) in nav::sidebar(&look_places()) {
        list.append(&nav_row(section.tag(), section.glyph(), &title(section)));
    }
    list.invalidate_headers();
}

/// A separator wherever Search or Sources follows a library place (docs/ui.md,
/// "Navigation").
fn separate_places(row: &gtk::ListBoxRow, before: Option<&gtk::ListBoxRow>) {
    let library = |r: &gtk::ListBoxRow| Section::from_tag(&r.widget_name()).map(Section::is_library);
    let separated = library(row) == Some(false) && before.is_some_and(|b| library(b) == Some(true));
    if !separated {
        row.set_header(None::<&gtk::Widget>);
    } else if row.header().is_none() {
        let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        sep.add_css_class("nav-gap");
        row.set_header(Some(&sep));
    }
}

/// The phone tabs: a floating Veil capsule (the web's `.tabs`).
struct PhoneTabs {
    root: gtk::Box,
    buttons: RefCell<Vec<(Tab, gtk::Button)>>,
}

impl PhoneTabs {
    fn new() -> PhoneTabs {
        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .homogeneous(true)
            .valign(gtk::Align::End)
            .accessible_role(gtk::AccessibleRole::TabList)
            .build();
        root.add_css_class("phone-tabs");
        root.add_css_class("veil");
        root.set_height_request(layout::TABS_HEIGHT);
        root.set_margin_start(layout::TABS_SIDE);
        root.set_margin_end(layout::TABS_SIDE);
        root.set_margin_bottom(layout::TABS_BOTTOM);
        root.update_property(&[gtk::accessible::Property::Label(&tr("Library sections"))]);
        root.set_visible(false);
        let tabs = PhoneTabs {
            root,
            buttons: RefCell::new(Vec::new()),
        };
        tabs.fill();
        tabs
    }

    /// The tabs, in the look's order, then Settings.
    fn fill(&self) {
        while let Some(child) = self.root.first_child() {
            self.root.remove(&child);
        }
        let mut buttons = Vec::new();
        for tab in nav::tabs(&look_places()) {
            let pill = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            pill.add_css_class("tab-pill");
            pill.set_halign(gtk::Align::Center);
            let glyph = Glyph::new(tab.glyph(), 20);
            glyph.set_hexpand(true);
            pill.append(&glyph);
            let name = tab_title(tab);
            let label = gtk::Label::new(Some(&name));
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            let column = gtk::Box::new(gtk::Orientation::Vertical, 3);
            column.set_valign(gtk::Align::Center);
            column.append(&pill);
            column.append(&label);
            let b = gtk::Button::builder()
                .child(&column)
                .accessible_role(gtk::AccessibleRole::Tab)
                .build();
            b.add_css_class("phone-tab");
            b.set_widget_name(tab.tag());
            b.update_property(&[gtk::accessible::Property::Label(&name)]);
            b.connect_clicked(move |_| show_tab(tab));
            self.root.append(&b);
            buttons.push((tab, b));
        }
        *self.buttons.borrow_mut() = buttons;
    }

    fn mark(&self, current: Tab) {
        for (tab, b) in self.buttons.borrow().iter() {
            let on = *tab == current;
            if on {
                b.add_css_class("on");
            } else {
                b.remove_css_class("on");
            }
            b.update_state(&[gtk::accessible::State::Selected(Some(on))]);
        }
    }
}

pub struct Shell {
    pub window: adw::ApplicationWindow,
    /// The sidebar panel and the pages beside it.
    body: gtk::Box,
    pub content: adw::NavigationView,
    places: gtk::ListBox,
    foot: gtk::ListBox,
    tabs: PhoneTabs,
    /// The pages, fading out above the floating player.
    fade: FadeBin,
    pub now_playing: NowPlaying,
    /// Now Playing over the library, in its motion bin.
    now_playing_bin: MotionBin,
    now_playing_open: Cell<bool>,
    mini: gtk::Widget,
    bar: playerbar::PlayerBar,
    phone_layout: adw::Breakpoint,
    wide: adw::Breakpoint,
    wide_compact: adw::Breakpoint,
    chrome: Cell<Chrome>,
    phone: Cell<bool>,
    pages: RefCell<HashMap<Section, adw::NavigationPage>>,
    current: Cell<Section>,
    /// The library place shown last, which the Library tab returns to.
    last_library: Cell<Option<Section>>,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell").finish_non_exhaustive()
    }
}

thread_local! {
    static SHELL: RefCell<Option<Rc<Shell>>> = const { RefCell::new(None) };
}

pub fn shell() -> Option<Rc<Shell>> {
    SHELL.with(|s| s.borrow().clone())
}

/// The floating sidebar panel: the wordmark (which moves the window, as a
/// title bar does), the places, and Open Files… and Settings at its foot.
fn sidebar_panel() -> (gtk::Box, gtk::ListBox, gtk::ListBox) {
    let icon = gtk::Image::from_icon_name(crate::ui::APP_ID);
    icon.set_pixel_size(32);
    icon.add_css_class("brand-icon");
    let name = gtk::Label::new(Some("Jellyfesh"));
    name.add_css_class("brand-name");
    let brand = gtk::Box::new(gtk::Orientation::Horizontal, 11);
    brand.add_css_class("brand");
    brand.append(&icon);
    brand.append(&name);
    let handle = gtk::WindowHandle::new();
    handle.set_child(Some(&brand));

    let places = gtk::ListBox::new();
    places.add_css_class("navigation-sidebar");
    places.add_css_class("nav-places");
    places.set_selection_mode(gtk::SelectionMode::None);
    places.update_property(&[gtk::accessible::Property::Label(&tr("Library sections"))]);
    places.set_header_func(separate_places);
    fill_places(&places);
    places.connect_row_activated(|_, row| {
        if let Some(section) = Section::from_tag(&row.widget_name()) {
            close_now_playing();
            show_section(section);
        }
    });
    let scroll = gtk::ScrolledWindow::builder()
        .child(&places)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    scroll.add_css_class("nav-scroll");

    let foot = gtk::ListBox::new();
    foot.add_css_class("navigation-sidebar");
    foot.add_css_class("nav-foot");
    foot.set_selection_mode(gtk::SelectionMode::None);
    foot.append(&nav_row("open-files", "file", &tr("Open Files…")));
    foot.append(&nav_row(
        Section::Settings.tag(),
        Section::Settings.glyph(),
        &title(Section::Settings),
    ));
    foot.connect_row_activated(|_, row| {
        if row.widget_name() == "open-files" {
            open_files();
        } else {
            show_settings(None);
        }
    });

    let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
    panel.add_css_class("sidebar-panel");
    panel.set_size_request(SIDEBAR_WIDTH, -1);
    panel.append(&handle);
    panel.append(&scroll);
    panel.append(&foot);
    (panel, places, foot)
}

/// The sidebar panel's width: the web's 248 px column less its 12 px
/// margin.
const SIDEBAR_WIDTH: i32 = 236;

/// Builds and shows the main window.
pub fn build(application: &adw::Application, state: &Rc<App>) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("Jellyfesh")
        .default_width(1200)
        .default_height(780)
        .width_request(360)
        .height_request(480)
        .icon_name(crate::ui::APP_ID)
        .build();
    window.add_css_class("jf-window");

    let (sidebar, places, foot) = sidebar_panel();

    // Content: section pages and detail pages, with the floating player
    // above (the capsule or the bar, by the window's width) and, on
    // phones, the tabs. The app runs page transitions itself (fade and
    // rise); swipes keep libadwaita's.
    let content = adw::NavigationView::new();
    content.add_css_class("content-pages");
    content.set_animate_transitions(false);
    let fade = FadeBin::new(&content);
    let content_area = gtk::Overlay::new();
    content_area.set_child(Some(&fade));
    content_area.set_hexpand(true);
    let mini = miniplayer::build();
    content_area.add_overlay(&mini);
    let bar = playerbar::build();
    content_area.add_overlay(&bar.root);
    let tabs = PhoneTabs::new();
    content_area.add_overlay(&tabs.root);

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.add_css_class("jf-body");
    body.append(&sidebar);
    body.append(&content_area);

    // Now Playing slides in over everything ("Deeper" in, "Back" out); once
    // it covers the window, the rest is hidden, so nothing under it is
    // drawn or scheduled.
    let root = gtk::Overlay::new();
    root.set_child(Some(&body));
    let now_playing = NowPlaying::new();
    let now_playing_bin = MotionBin::new(&now_playing.widget());
    now_playing_bin.set_visible(false);
    root.add_overlay(&now_playing_bin);
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&root));
    window.set_content(Some(&toasts));

    // Up to 760 px: the phone layout (the setters apply during the
    // allocation, so the window never asks for the panel's width).
    let phone_layout =
        adw::Breakpoint::new(adw::BreakpointCondition::parse(&nav::phone_condition()).expect("condition"));
    phone_layout.add_setter(&sidebar, "visible", Some(&false.to_value()));
    phone_layout.add_setter(&tabs.root, "visible", Some(&true.to_value()));
    window.add_breakpoint(phone_layout.clone());
    // From 900 px: the player bar instead of the capsule, and larger titles.
    let wide =
        adw::Breakpoint::new(adw::BreakpointCondition::parse(&layout::bar_condition()).expect("condition"));
    window.add_breakpoint(wide.clone());
    // 900 to 1099 px: the bar without its volume line (added last: it wins).
    let wide_compact = adw::Breakpoint::new(
        adw::BreakpointCondition::parse(&layout::compact_bar_condition()).expect("condition"),
    );
    window.add_breakpoint(wide_compact.clone());

    *state.window.borrow_mut() = window.downgrade();
    *state.toasts.borrow_mut() = toasts.downgrade();

    let s = Rc::new(Shell {
        window: window.clone(),
        body,
        content: content.clone(),
        places,
        foot,
        tabs,
        fade,
        now_playing,
        now_playing_bin,
        now_playing_open: Cell::new(false),
        mini,
        bar,
        phone_layout,
        wide,
        wide_compact,
        chrome: Cell::new(Chrome::Mini),
        phone: Cell::new(false),
        pages: RefCell::new(HashMap::new()),
        current: Cell::new(Section::Home),
        last_library: Cell::new(None),
    });
    SHELL.with(|sh| *sh.borrow_mut() = Some(s.clone()));
    // Breakpoints change during allocation: apply the new chrome after it.
    window.connect_current_breakpoint_notify(|_| {
        glib::idle_add_local_once(update_chrome);
    });
    update_chrome();
    content.connect_pushed(|view| {
        if let Some(page) = view.visible_page() {
            pages::animate(&page, Transition::PageEnter);
        }
    });
    content.connect_replaced(|view| {
        if let Some(page) = view.visible_page() {
            pages::animate(&page, Transition::PageEnter);
        }
    });
    // Back fades in the page underneath, unless a swipe has been showing it.
    content.connect_popped(|view, _| {
        if let Some(page) = view.visible_page()
            && pages::shown_at_once()
        {
            pages::animate(&page, Transition::PageBack);
        }
    });

    actions::install(&window);
    install_window_actions(&window);
    crate::ui::keys::attach(&window);
    track_visibility(&window);
    show_section(Section::Home);
    window
}

fn install_window_actions(window: &adw::ApplicationWindow) {
    let add_folder = gtk::gio::SimpleAction::new("add-folder", None);
    add_folder.connect_activate(|_, _| choose_folder());
    window.add_action(&add_folder);
    let rescan = gtk::gio::SimpleAction::new("rescan", None);
    rescan.connect_activate(|_, _| {
        app().send(jf_core::Command::Rescan);
    });
    window.add_action(&rescan);
    let open = gtk::gio::SimpleAction::new("open-files", None);
    open.connect_activate(|_, _| open_files());
    window.add_action(&open);
    let section = gtk::gio::SimpleAction::new("section", Some(glib::VariantTy::STRING));
    section.connect_activate(|_, p| {
        if let Some(s) = p
            .and_then(|p| p.get::<String>())
            .and_then(|t| Section::from_tag(&t))
        {
            show_section(s);
        }
    });
    window.add_action(&section);
}

/// Asks for a music folder, adds it to the library and scans it.
pub fn choose_folder() {
    let dialog = gtk::FileDialog::builder()
        .title(tr("Add Music Folder"))
        .accept_label(tr("Add"))
        .modal(true)
        .build();
    let parent = shell().map(|s| s.window.clone());
    dialog.select_folder(parent.as_ref(), gtk::gio::Cancellable::NONE, |result| {
        if let Ok(folder) = result
            && let Some(path) = folder.path()
        {
            // AddRoot scans the folder it adds.
            let a = app();
            a.send(jf_core::Command::AddRoot { path });
            a.toast(&tr("Adding the folder to your library…"));
        }
    });
}

/// Open Files… (the sidebar's foot): plays the chosen files, through the
/// file chooser portal, without adding them to the library.
pub fn open_files() {
    let audio = gtk::FileFilter::new();
    audio.set_name(Some(&tr("Audio Files")));
    audio.add_mime_type("audio/*");
    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&audio);
    let dialog = gtk::FileDialog::builder()
        .title(tr("Open Files"))
        .accept_label(tr("Play"))
        .modal(true)
        .filters(&filters)
        .default_filter(&audio)
        .build();
    let parent = shell().map(|s| s.window.clone());
    dialog.open_multiple(parent.as_ref(), gtk::gio::Cancellable::NONE, |result| {
        let Ok(files) = result else { return };
        let tracks: Vec<jf_core::TrackRef> = (0..files.n_items())
            .filter_map(|i| files.item(i).and_downcast::<gtk::gio::File>())
            .filter_map(|f| f.path())
            .map(jf_core::TrackRef::File)
            .collect();
        if !tracks.is_empty() {
            app().send(jf_core::Command::PlayTracks { tracks, start: 0 });
        }
    });
}

/// Watches whether the window can be seen, so time-driven views stop while
/// it is minimised or hidden (Still mode).
fn track_visibility(window: &adw::ApplicationWindow) {
    window.connect_realize(|w| {
        if let Some(surface) = w.surface()
            && let Ok(toplevel) = surface.downcast::<gdk::Toplevel>()
        {
            let update = |t: &gdk::Toplevel| {
                let state = t.state();
                let hidden = state.contains(gdk::ToplevelState::MINIMIZED)
                    || state.contains(gdk::ToplevelState::SUSPENDED);
                if let Some(a) = try_app() {
                    a.scheduler.set_window_live(!hidden);
                }
            };
            update(&toplevel);
            toplevel.connect_state_notify(move |t| update(t));
        }
    });
    window.connect_visible_notify(|w| {
        if let Some(a) = try_app() {
            a.scheduler.set_window_live(w.is_visible());
        }
    });
}

/// Shows which section is on screen: its sidebar row and its phone tab.
fn mark_current(s: &Shell) {
    let current = s.current.get();
    for list in [&s.places, &s.foot] {
        let mut i = 0;
        while let Some(row) = list.row_at_index(i) {
            let on = row.widget_name() == current.tag();
            if on {
                row.add_css_class("current");
            } else {
                row.remove_css_class("current");
            }
            row.update_state(&[gtk::accessible::State::Selected(Some(on))]);
            i += 1;
        }
    }
    s.tabs.mark(nav::tab_of(current));
}

/// Shows a section's page (created once, then kept).
pub fn show_section(section: Section) {
    let Some(s) = shell() else { return };
    let page = s
        .pages
        .borrow_mut()
        .entry(section)
        .or_insert_with(|| pages::section_page(section))
        .clone();
    s.current.set(section);
    if section.in_library_tab() {
        s.last_library.set(Some(section));
    }
    s.content.replace(&[page]);
    mark_current(&s);
    if section == Section::Search {
        pages::search::focus();
    }
}

/// A phone tab: its section, or for Library the library place shown last.
fn show_tab(tab: Tab) {
    let Some(s) = shell() else { return };
    let section = match tab {
        Tab::Home => Section::Home,
        Tab::Search => Section::Search,
        Tab::Sources => Section::Sources,
        Tab::Settings => Section::Settings,
        Tab::Library if s.current.get().in_library_tab() => s.current.get(),
        Tab::Library => nav::library_tab_target(s.last_library.get()),
    };
    show_section(section);
}

/// Back to the current section's own page.
pub fn pop_to_section() {
    if let Some(s) = shell() {
        let section = s.current.get();
        let _ = s.content.pop_to_tag(section.tag());
    }
}

fn push(page: adw::NavigationPage) {
    let Some(s) = shell() else { return };
    close_now_playing();
    s.content.push(&page);
}

pub fn show_album(id: i64, known: Option<ItemData>) {
    push(pages::detail::album_page(id, known));
}

pub fn show_artist(id: i64, known: Option<ItemData>) {
    push(pages::detail::artist_page(id, known));
}

/// Opens the library artist with this name, as songs and albums carry it
/// (the core gives them no artist id): the core's search finds the artist,
/// the one with exactly that name when there is one.
pub fn show_artist_named(name: &str) {
    let name = name.to_owned();
    crate::ui::state::spawn(async move {
        let wanted = name.clone();
        let found = app().query(move |core| core.search(&wanted, 5)).await;
        if let Some(Ok(results)) = found
            && let Some(a) = results
                .artists
                .iter()
                .find(|a| a.name == name)
                .or(results.artists.first())
        {
            show_artist(a.id, Some(crate::ui::convert::artist(a)));
        }
    });
}

pub fn show_playlist(id: i64, known: Option<ItemData>) {
    push(pages::playlists::playlist_page(id, known));
}

/// A source's Discover page (its shelves; Browse is in its header).
pub fn show_discover(plugin: &str) {
    push(pages::discover::page(plugin));
}

/// Browses a source's catalogue: its root (`item` `None`, titled `title`),
/// or the page of an album, artist, playlist or folder it listed.
pub fn show_source(plugin: &str, item: Option<jf_core::SourceItem>, title: &str) {
    push(pages::sources::browse_page(plugin, item, title));
}

/// Whether the window shows the phone layout (up to 760 px wide).
pub fn is_phone() -> bool {
    shell().is_some_and(|s| s.phone.get())
}

/// The capsule under 900 px, the bar from 900 px, while something is queued;
/// the sidebar panel from 761 px, the phone tabs up to 760; pages leave room
/// for the player and the tabs under their last row.
fn update_chrome() {
    let Some(s) = shell() else { return };
    let current = s.window.current_breakpoint();
    let phone = current.as_ref() == Some(&s.phone_layout);
    let compact = current.as_ref() == Some(&s.wide_compact);
    let wide = compact || current.as_ref() == Some(&s.wide);
    // The look's player: Bar (v0.0.6), Compact or Floating (ADR-0020).
    let style = try_app().map_or(jf_core::MiniPlayerStyle::Bar, |a| {
        crate::logic::look::mini_player(&a.look.borrow())
    });
    let (chrome, floating) = layout::chrome_for(style, wide);
    if let Some(clamp) = s.mini.downcast_ref::<adw::Clamp>() {
        clamp.set_maximum_size(if floating { layout::FLOATING_WIDTH } else { 560 });
        clamp.set_tightening_threshold(if floating { layout::FLOATING_WIDTH } else { 400 });
        clamp.set_halign(if floating {
            gtk::Align::End
        } else {
            gtk::Align::Fill
        });
    }
    if floating {
        s.mini.add_css_class("floating");
    } else {
        s.mini.remove_css_class("floating");
    }
    s.bar.set_compact(compact);
    s.chrome.set(chrome);
    if chrome == Chrome::Bar {
        s.window.add_css_class("wide");
    } else {
        s.window.remove_css_class("wide");
    }
    if phone {
        s.window.add_css_class("phone");
    } else {
        s.window.remove_css_class("phone");
    }
    // The player floats over the content's gutters, over the tabs on phones.
    let side = layout::gutter(phone);
    for w in [&s.mini, &s.bar.root] {
        w.set_margin_start(side);
        w.set_margin_end(side);
        w.set_margin_bottom(layout::dock_bottom(phone));
    }
    let has_current = try_app()
        .is_some_and(|a| a.now.borrow().is_some() || a.queue.borrow().current.is_some() || a.is_remote());
    s.mini.set_visible(has_current && chrome == Chrome::Mini);
    s.bar.root.set_visible(has_current && chrome == Chrome::Bar);
    // Pages end above the floating player and tabs, so they never cover a
    // last row, and fade out just above them.
    let room = layout::bottom_room(has_current.then_some(chrome), phone);
    s.fade.set_margin_bottom(room);
    s.fade
        .set_fades(0.0, if room > 0 { layout::PLAYER_FADE } else { 0.0 });
    if s.phone.replace(phone) != phone
        && let Some(a) = try_app()
    {
        a.announce(&Change::Layout);
    }
}

/// Opens Now Playing on `tab` (or where it was): it slides up and fades in
/// over the window, whose pages are hidden once covered.
pub fn open_now_playing(tab: Option<NowPlayingTab>) {
    let Some(s) = shell() else { return };
    s.now_playing.prepare(tab);
    if s.now_playing_open.replace(true) {
        return;
    }
    s.window.add_css_class("np-open");
    s.now_playing_bin.set_visible(true);
    let body = s.body.downgrade();
    s.now_playing_bin.enter(
        plan(Transition::NowPlayingOpen, crate::ui::style::allowed()),
        move |_| {
            if let Some(body) = body.upgrade() {
                body.set_visible(false);
            }
        },
    );
    s.now_playing.focus_default();
    if let Some(a) = try_app() {
        a.scheduler.refresh();
    }
}

/// Closes Now Playing: the pages show again under it as it slides down
/// and fades out (Exit).
pub fn close_now_playing() {
    let Some(s) = shell() else { return };
    if !s.now_playing_open.replace(false) {
        return;
    }
    s.window.remove_css_class("np-open");
    s.body.set_visible(true);
    s.now_playing_bin.leave(
        plan(Transition::NowPlayingClose, crate::ui::style::allowed()),
        |bin| {
            bin.set_visible(false);
        },
    );
}

pub fn now_playing_open() -> bool {
    shell().is_some_and(|s| s.now_playing_open.get())
}

/// Back from a detail page to the page under it; false on a section's own
/// page.
pub fn go_back() -> bool {
    shell().is_some_and(|s| s.content.pop())
}

/// Opens Settings, optionally on a section (`appearance`, `playback`,
/// `library`, `sources`, `extensions`, `about`; `lyrics` is in Appearance,
/// `shortcuts` in About).
pub fn show_settings(section: Option<&str>) {
    close_now_playing();
    show_section(Section::Settings);
    pages::settings::show(section);
}

/// Reacts to app-wide changes the window owns.
pub fn listen() {
    app().listen(|change| {
        if matches!(change, Change::NowPlaying | Change::Queue | Change::Remote) {
            update_chrome();
        }
        if let Change::Settings = change {
            crate::ui::style::apply_appearance();
            crate::ui::style::apply_glass();
            crate::ui::style::apply_theme(false);
        }
        if let Change::Theme = change {
            crate::ui::style::apply_theme(true);
        }
        // The look applies live: its stylesheets, glass in the theme, the
        // artwork's corners, the sidebar's places and tabs and the player
        // at the bottom.
        if let Change::Look = change {
            crate::ui::style::apply_look();
            crate::ui::style::apply_theme(false);
            crate::ui::widgets::art::redraw_all();
            if let Some(s) = shell() {
                fill_places(&s.places);
                s.tabs.fill();
                mark_current(&s);
            }
            update_chrome();
        }
    });
}
