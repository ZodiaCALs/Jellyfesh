//! Section and detail pages.

pub mod detail;
pub mod favourites;
pub mod home;
pub mod library;
pub mod playlists;
pub mod search;
pub mod sources;

use std::cell::Cell;

use adw::prelude::*;
use gtk::{glib, graphene};

use crate::logic::layout;
use crate::logic::motion::{Transition, plan};
use crate::ui::style;
use crate::ui::widgets::motion::MotionBin;
use crate::ui::widgets::titledscroll::TitledScroll;
use crate::ui::window::Section;

/// The page for a sidebar section.
pub fn section_page(section: Section) -> adw::NavigationPage {
    match section {
        Section::Home => home::page(),
        Section::Albums => library::albums(),
        Section::Artists => library::artists(),
        Section::Songs => library::songs(),
        Section::Playlists => playlists::page(),
        Section::Favourites => favourites::page(),
        Section::Search => search::page(),
        Section::Sources => sources::page(),
    }
}

thread_local! {
    /// When a page last began to show (µs, monotonic), to tell a page shown
    /// at once from one a swipe has been revealing.
    static LAST_SHOWING: Cell<i64> = const { Cell::new(0) };
}

/// A page with a flat header bar over the Misty field. The page's content
/// sits in a motion bin, so it can fade in and rise as it appears.
pub fn frame(
    title: &str,
    tag: &str,
    content: &impl IsA<gtk::Widget>,
) -> (adw::NavigationPage, adw::HeaderBar) {
    let header = adw::HeaderBar::new();
    header.add_css_class("flat");
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(content));
    view.set_top_bar_style(adw::ToolbarStyle::Flat);
    let bin = MotionBin::new(&view);
    let page = adw::NavigationPage::builder()
        .title(title)
        .tag(tag)
        .child(&bin)
        .build();
    page.add_css_class("jf-page");
    page.connect_showing(|_| LAST_SHOWING.with(|t| t.set(glib::monotonic_time())));
    (page, header)
}

/// Whether the page on screen began to show just now (not revealed by a
/// swipe that is ending).
pub fn shown_at_once() -> bool {
    let since = glib::monotonic_time() - LAST_SHOWING.with(Cell::get);
    since < 50_000
}

/// Runs `transition` on a page made by [`frame`].
pub fn animate(page: &adw::NavigationPage, transition: Transition) {
    if let Some(bin) = page.child().and_downcast::<MotionBin>() {
        bin.enter(plan(transition, style::allowed()), |_| {});
    }
}

/// A large page title (design-controls.md, "Large titles"): `text-h2`, or
/// `text-h1` on wide windows (CSS); it scrolls with the content.
pub fn large_title(text: &str) -> gtk::Label {
    let l = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .wrap(true)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    l.add_css_class("large-title");
    l
}

/// The header bar's own small title, which fades in as the large one
/// scrolls under it.
pub fn small_title(header: &adw::HeaderBar, text: &str) -> adw::WindowTitle {
    let title = adw::WindowTitle::new(text, "");
    title.set_opacity(0.0);
    header.set_title_widget(Some(&title));
    title
}

/// Fades `small` in as `large` (inside `scroll`'s content) scrolls away.
pub fn follow_title(scroll: &gtk::ScrolledWindow, large: &impl IsA<gtk::Widget>, small: &adw::WindowTitle) {
    let (large, small) = (
        large.clone().upcast::<gtk::Widget>().downgrade(),
        small.downgrade(),
    );
    let follow = move |adj: &gtk::Adjustment| {
        let (Some(large), Some(small)) = (large.upgrade(), small.upgrade()) else {
            return;
        };
        let height = f64::from(large.height());
        // The large title's top in the scrolled content: where it is on
        // screen now, plus how far the content has scrolled.
        let Some(content) = large.ancestor(gtk::Viewport::static_type()) else {
            return;
        };
        let top = large
            .compute_point(&content, &graphene::Point::new(0.0, 0.0))
            .map_or(0.0, |p| f64::from(p.y()))
            + adj.value();
        let hidden = layout::title_hidden(adj.value(), top.max(0.0), height);
        if (small.opacity() - hidden).abs() > 0.01 {
            small.set_opacity(hidden);
        }
    };
    scroll.vadjustment().connect_value_changed(follow);
}

/// A scrolled list or grid under a large title that scrolls away with it,
/// and the small title for the page's header bar ([`show_small_title`]),
/// which fades in as it goes. `max_width` clamps the title like the list
/// under it.
pub fn titled_scroller(
    title: &str,
    list: &(impl IsA<gtk::Scrollable> + IsA<gtk::Widget>),
    max_width: Option<i32>,
) -> (gtk::ScrolledWindow, adw::WindowTitle) {
    titled_scroller_with(title, None, list, max_width)
}

/// [`titled_scroller`] with `below` (a page's Play and Shuffle) under the
/// large title, scrolling away with it.
pub fn titled_scroller_with(
    title: &str,
    below: Option<&gtk::Widget>,
    list: &(impl IsA<gtk::Scrollable> + IsA<gtk::Widget>),
    max_width: Option<i32>,
) -> (gtk::ScrolledWindow, adw::WindowTitle) {
    let label = large_title(title);
    let block: gtk::Widget = match below {
        Some(below) => {
            let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
            b.add_css_class("title-block");
            b.append(&label);
            b.append(below);
            b.upcast()
        }
        None => label.clone().upcast(),
    };
    let holder: gtk::Widget = match max_width {
        Some(w) => {
            let clamp = adw::Clamp::builder()
                .child(&block)
                .maximum_size(w)
                .tightening_threshold(700)
                .build();
            clamp.add_css_class("title-clamp");
            clamp.upcast()
        }
        None => {
            block.add_css_class("title-wide");
            block
        }
    };
    let titled = TitledScroll::new(&holder, list);
    let s = scroller(&titled);
    let small = adw::WindowTitle::new(title, "");
    small.set_opacity(0.0);
    let weak = (titled.downgrade(), small.downgrade());
    s.vadjustment().connect_value_changed(move |adj| {
        let (Some(titled), Some(small)) = (weak.0.upgrade(), weak.1.upgrade()) else {
            return;
        };
        let hidden = layout::title_hidden(adj.value(), 0.0, titled.title_height());
        if (small.opacity() - hidden).abs() > 0.01 {
            small.set_opacity(hidden);
        }
    });
    (s, small)
}

/// Puts a small title made by [`titled_scroller`] in a header bar.
pub fn show_small_title(header: &adw::HeaderBar, small: &adw::WindowTitle) {
    header.set_title_widget(Some(small));
}

/// A scrolled area for a page's content, leaving room for the mini-player.
pub fn scroller(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let s = gtk::ScrolledWindow::builder()
        .child(child)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    s.add_css_class("page-scroll");
    s
}

/// An empty or error state.
pub fn status(icon: &str, title: &str, description: &str) -> adw::StatusPage {
    let s = adw::StatusPage::builder()
        .icon_name(icon)
        .title(title)
        .description(description)
        .vexpand(true)
        .build();
    s.add_css_class("compact");
    s
}

/// A section heading inside a page.
pub fn heading(text: &str) -> gtk::Label {
    let l = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    l.add_css_class("section-heading");
    l
}

/// Play (accent) and Shuffle (glass) side by side, as on album pages.
pub fn play_and_shuffle() -> (gtk::Box, gtk::Button, gtk::Button) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("page-actions");
    let play = pill(&crate::ui::i18n::tr("Play"), "play", true);
    let shuffle = pill(&crate::ui::i18n::tr("Shuffle"), "shuffle", false);
    shuffle.add_css_class("glass-pill");
    row.append(&play);
    row.append(&shuffle);
    (row, play, shuffle)
}

/// A pill button (Play, Shuffle).
pub fn pill(label: &str, glyph: &str, suggested: bool) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    content.append(&crate::ui::widgets::glyph::Glyph::new(glyph, 18));
    content.append(&gtk::Label::new(Some(label)));
    let b = gtk::Button::builder().child(&content).build();
    b.add_css_class("pill");
    b.add_css_class("action-pill");
    if suggested {
        b.add_css_class("suggested-action");
    }
    b.update_property(&[gtk::accessible::Property::Label(label)]);
    b
}

/// Keeps `value` alive as long as `widget` exists.
pub fn keep_alive<T: 'static>(widget: &impl IsA<gtk::Widget>, value: T) {
    let cell = std::cell::RefCell::new(Some(value));
    widget.as_ref().connect_destroy(move |_| {
        cell.borrow_mut().take();
    });
}
