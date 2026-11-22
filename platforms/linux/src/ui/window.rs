//! The main window: an adaptive split view with the library sections in the
//! sidebar, their pages and detail pages in a navigation view, the floating
//! mini-player (under 900 px) or player bar (from 900 px), and Now Playing
//! over everything. The pages fade out just above the floating player
//! (Luminous).
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
use crate::ui::i18n::{tr, trc};
use crate::ui::models::ItemData;
use crate::ui::nowplaying::{NowPlaying, Tab};
use crate::ui::state::{App, Change, app, try_app};
use crate::ui::style;
use crate::ui::widgets::fade::FadeBin;
use crate::ui::widgets::glyph::Glyph;
use crate::ui::widgets::motion::MotionBin;
use crate::ui::{actions, miniplayer, pages, playerbar, prefs};

/// The sidebar's sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Home,
    Albums,
    Artists,
    Songs,
    Playlists,
    Favourites,
    Search,
    Sources,
}

impl Section {
    pub const ALL: [Section; 8] = [
        Section::Home,
        Section::Albums,
        Section::Artists,
        Section::Songs,
        Section::Playlists,
        Section::Favourites,
        Section::Search,
        Section::Sources,
    ];

    pub fn title(self) -> String {
        match self {
            Section::Home => trc("sidebar", "Home"),
            Section::Albums => trc("sidebar", "Albums"),
            Section::Artists => trc("sidebar", "Artists"),
            Section::Songs => trc("sidebar", "Songs"),
            Section::Playlists => trc("sidebar", "Playlists"),
            Section::Favourites => trc("sidebar", "Favourites"),
            Section::Search => trc("sidebar", "Search"),
            Section::Sources => trc("sidebar", "Sources"),
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Section::Home => "home",
            Section::Albums => "album",
            Section::Artists => "artist",
            Section::Songs => "song",
            Section::Playlists => "playlist",
            Section::Favourites => "heart",
            Section::Search => "search",
            Section::Sources => "sources",
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Section::Home => "home",
            Section::Albums => "albums",
            Section::Artists => "artists",
            Section::Songs => "songs",
            Section::Playlists => "playlists",
            Section::Favourites => "favourites",
            Section::Search => "search",
            Section::Sources => "sources",
        }
    }

    pub fn from_tag(tag: &str) -> Option<Section> {
        Section::ALL.into_iter().find(|s| s.tag() == tag)
    }
}

pub struct Shell {
    pub window: adw::ApplicationWindow,
    pub split: adw::NavigationSplitView,
    pub content: adw::NavigationView,
    pub sidebar: gtk::ListBox,
    /// The pages, fading out above the floating player.
    fade: FadeBin,
    pub now_playing: NowPlaying,
    /// Now Playing over the library, in its motion bin.
    now_playing_bin: MotionBin,
    now_playing_open: Cell<bool>,
    mini: gtk::Widget,
    bar: playerbar::PlayerBar,
    wide: adw::Breakpoint,
    wide_compact: adw::Breakpoint,
    chrome: Cell<Chrome>,
    pages: RefCell<HashMap<Section, adw::NavigationPage>>,
    current: RefCell<Section>,
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

    // Sidebar.
    let sidebar = gtk::ListBox::new();
    sidebar.add_css_class("navigation-sidebar");
    sidebar.update_property(&[gtk::accessible::Property::Label(&tr("Library sections"))]);
    for section in Section::ALL {
        let row = gtk::ListBoxRow::new();
        let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        b.append(&Glyph::new(section.glyph(), 18));
        let label = gtk::Label::new(Some(&section.title()));
        label.set_xalign(0.0);
        b.append(&label);
        row.set_child(Some(&b));
        row.set_widget_name(section.tag());
        row.update_property(&[gtk::accessible::Property::Label(&section.title())]);
        sidebar.append(&row);
    }
    // The library's six, a separator, then Search and Sources (docs/ui.md,
    // "Navigation").
    sidebar.set_header_func(|row, _| {
        if row.widget_name() != Section::Search.tag() {
            row.set_header(None::<&gtk::Widget>);
        } else if row.header().is_none() {
            row.set_header(Some(&gtk::Separator::new(gtk::Orientation::Horizontal)));
        }
    });
    let sidebar_scroll = gtk::ScrolledWindow::builder()
        .child(&sidebar)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.set_title_widget(Some(&adw::WindowTitle::new("Jellyfesh", "")));
    sidebar_header.pack_end(&primary_menu());
    let sidebar_view = adw::ToolbarView::new();
    sidebar_view.add_top_bar(&sidebar_header);
    sidebar_view.set_content(Some(&sidebar_scroll));
    let sidebar_page = adw::NavigationPage::builder()
        .title("Jellyfesh")
        .tag("sidebar")
        .child(&sidebar_view)
        .build();

    // Content: section pages and detail pages, with the floating player
    // above (the capsule or the bar, by the window's width). The app runs
    // page transitions itself (fade and rise); swipes keep libadwaita's.
    let content = adw::NavigationView::new();
    content.add_css_class("content-pages");
    content.set_animate_transitions(false);
    let fade = FadeBin::new(&content);
    let content_area = gtk::Overlay::new();
    content_area.set_child(Some(&fade));
    let mini = miniplayer::build();
    content_area.add_overlay(&mini);
    let bar = playerbar::build();
    content_area.add_overlay(&bar.root);
    let content_page = adw::NavigationPage::builder()
        .title(tr("Library"))
        .tag("content")
        .child(&content_area)
        .build();

    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&content_page)
        .min_sidebar_width(200.0)
        .max_sidebar_width(260.0)
        .build();

    // Now Playing slides in over the library ("Deeper" in, "Back" out); once
    // it covers the library, the library is hidden, so nothing under it is
    // drawn or scheduled.
    let root = gtk::Overlay::new();
    root.set_child(Some(&split));
    let now_playing = NowPlaying::new();
    let now_playing_bin = MotionBin::new(&now_playing.widget());
    now_playing_bin.set_visible(false);
    root.add_overlay(&now_playing_bin);
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&root));
    window.set_content(Some(&toasts));

    let narrow =
        adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 640sp").expect("condition"));
    narrow.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(narrow);
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
        split: split.clone(),
        content: content.clone(),
        sidebar: sidebar.clone(),
        fade,
        now_playing,
        now_playing_bin,
        now_playing_open: Cell::new(false),
        mini,
        bar,
        wide,
        wide_compact,
        chrome: Cell::new(Chrome::Mini),
        pages: RefCell::new(HashMap::new()),
        current: RefCell::new(Section::Home),
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

    sidebar.connect_row_activated(|_, row| {
        if let Some(section) = Section::from_tag(&row.widget_name()) {
            show_section(section);
        }
    });
    actions::install(&window);
    install_window_actions(&window);
    crate::ui::keys::attach(&window);
    track_visibility(&window);
    show_section(Section::Home);
    window
}

fn primary_menu() -> gtk::MenuButton {
    let menu = gtk::gio::Menu::new();
    let main = gtk::gio::Menu::new();
    main.append(Some(&tr("Add Music Folder…")), Some("win.add-folder"));
    main.append(Some(&tr("Rescan Library")), Some("win.rescan"));
    menu.append_section(None, &main);
    let app_section = gtk::gio::Menu::new();
    app_section.append(Some(&tr("Settings")), Some("app.settings"));
    app_section.append(Some(&tr("Keyboard Shortcuts")), Some("app.shortcuts"));
    app_section.append(Some(&tr("About Jellyfesh")), Some("app.about"));
    menu.append_section(None, &app_section);
    let button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .primary(true)
        .tooltip_text(tr("Main Menu"))
        .build();
    if let Some(p) = button.popover() {
        style::animate_popup(&p);
    }
    button.update_property(&[gtk::accessible::Property::Label(&tr("Main Menu"))]);
    button
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

/// Shows a section's page (created once, then kept).
pub fn show_section(section: Section) {
    let Some(s) = shell() else { return };
    let page = s
        .pages
        .borrow_mut()
        .entry(section)
        .or_insert_with(|| pages::section_page(section))
        .clone();
    *s.current.borrow_mut() = section;
    s.content.replace(&[page]);
    for i in 0.. {
        let Some(row) = s.sidebar.row_at_index(i) else {
            break;
        };
        if row.widget_name() == section.tag() {
            s.sidebar.select_row(Some(&row));
        }
    }
    s.split.set_show_content(true);
    if section == Section::Search {
        pages::search::focus();
    }
}

/// Back to the current section's own page.
pub fn pop_to_section() {
    if let Some(s) = shell() {
        let section = *s.current.borrow();
        let _ = s.content.pop_to_tag(section.tag());
    }
}

fn push(page: adw::NavigationPage) {
    let Some(s) = shell() else { return };
    close_now_playing();
    s.content.push(&page);
    s.split.set_show_content(true);
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

/// The capsule under 900 px, the bar from 900 px, while something is queued;
/// pages leave room for it under their last row.
fn update_chrome() {
    let Some(s) = shell() else { return };
    let current = s.window.current_breakpoint();
    let compact = current.as_ref() == Some(&s.wide_compact);
    let chrome = if compact || current.as_ref() == Some(&s.wide) {
        Chrome::Bar
    } else {
        Chrome::Mini
    };
    s.bar.set_compact(compact);
    s.chrome.set(chrome);
    if chrome == Chrome::Bar {
        s.window.add_css_class("wide");
    } else {
        s.window.remove_css_class("wide");
    }
    let has_current = try_app()
        .is_some_and(|a| a.now.borrow().is_some() || a.queue.borrow().current.is_some() || a.is_remote());
    s.mini.set_visible(has_current && chrome == Chrome::Mini);
    s.bar.root.set_visible(has_current && chrome == Chrome::Bar);
    // Pages end above the floating player, so it never covers a last row,
    // and fade out just above it.
    s.fade
        .set_margin_bottom(if has_current { chrome.bottom_room() } else { 0 });
    s.fade
        .set_fades(0.0, if has_current { layout::PLAYER_FADE } else { 0.0 });
}

/// Opens Now Playing on `tab` (or where it was): it slides up and fades in
/// over the library, which is hidden once covered.
pub fn open_now_playing(tab: Option<Tab>) {
    let Some(s) = shell() else { return };
    s.now_playing.prepare(tab);
    if s.now_playing_open.replace(true) {
        return;
    }
    s.window.add_css_class("np-open");
    s.now_playing_bin.set_visible(true);
    let split = s.split.downgrade();
    s.now_playing_bin
        .enter(plan(Transition::NowPlayingOpen, style::allowed()), move |_| {
            if let Some(split) = split.upgrade() {
                split.set_visible(false);
            }
        });
    s.now_playing.focus_default();
    if let Some(a) = try_app() {
        a.scheduler.refresh();
    }
}

/// Closes Now Playing: the library shows again under it as it slides down
/// and fades out (Exit).
pub fn close_now_playing() {
    let Some(s) = shell() else { return };
    if !s.now_playing_open.replace(false) {
        return;
    }
    s.window.remove_css_class("np-open");
    s.split.set_visible(true);
    s.now_playing_bin
        .leave(plan(Transition::NowPlayingClose, style::allowed()), |bin| {
            bin.set_visible(false);
        });
}

pub fn now_playing_open() -> bool {
    shell().is_some_and(|s| s.now_playing_open.get())
}

/// Back from a detail page to the page under it; false on a section's own
/// page.
pub fn go_back() -> bool {
    shell().is_some_and(|s| s.content.pop())
}

/// Opens Settings, optionally on a page (`appearance`, `playback`,
/// `library`, `sources`).
pub fn show_settings(page: Option<&str>) {
    if let Some(s) = shell() {
        prefs::present(&s.window, page);
    }
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
        // The look applies live: its stylesheets, glass in the theme and the
        // artwork's corners.
        if let Change::Look = change {
            crate::ui::style::apply_look();
            crate::ui::style::apply_theme(false);
            crate::ui::widgets::art::redraw_all();
        }
    });
}
