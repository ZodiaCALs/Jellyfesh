//! Settings, a page in the main window (ADR-0023; docs/ui.md, "Settings"):
//! the large title, the sections as Jellyfesh Web's pill tabs (Appearance ·
//! Playback · Library · Sources · Extensions · About), then the chosen
//! section's groups, each a Mist card with its heading inside. The column is
//! at most `layout.reading` (704 px) wide, lined up with the page's start,
//! and the player bar floats over it as over every page.

use std::cell::RefCell;

use adw::prelude::*;
use gtk::glib;

use crate::logic::nav::{SETTINGS_SECTIONS as SECTIONS, settings_section as section_for};
use crate::ui::i18n::tr;
use crate::ui::pages::{follow_title, frame, large_title, pill_tabs, scroller, small_title};
use crate::ui::widgets::maxwidth::MaxWidth;
use crate::ui::{about, extensions, prefs, sources_ui};

fn section_title(name: &str) -> String {
    match name {
        "appearance" => tr("Appearance"),
        "playback" => tr("Playback"),
        "library" => tr("Library"),
        "sources" => tr("Sources"),
        "extensions" => tr("Extensions"),
        _ => tr("About"),
    }
}

/// A section's groups, built when Settings first shows.
fn build_section(name: &str) -> gtk::Widget {
    match name {
        "appearance" => prefs::appearance_page().upcast(),
        "playback" => prefs::playback_page().upcast(),
        "library" => prefs::library_page().upcast(),
        "sources" => sources_ui::page().upcast(),
        "extensions" => extensions::page().upcast(),
        _ => about::section().upcast(),
    }
}

struct Shown {
    stack: glib::WeakRef<gtk::Stack>,
    tabs: Vec<(&'static str, glib::WeakRef<gtk::ToggleButton>)>,
    scroll: glib::WeakRef<gtk::ScrolledWindow>,
}

thread_local! {
    static SHOWN: RefCell<Option<Shown>> = const { RefCell::new(None) };
}

pub fn page() -> adw::NavigationPage {
    let title = large_title(&tr("Settings"));
    let stack = gtk::Stack::builder()
        .vhomogeneous(false)
        .hhomogeneous(false)
        .transition_type(gtk::StackTransitionType::None)
        .build();
    stack.add_css_class("settings-sections");
    for name in SECTIONS {
        stack.add_titled(&build_section(name), Some(name), &section_title(name));
    }
    let labels: Vec<(&'static str, String)> = SECTIONS.iter().map(|n| (*n, section_title(n))).collect();
    let (tabs_row, tabs) = pill_tabs(&labels, &tr("Settings"));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.add_css_class("settings-page");
    column.append(&title);
    column.append(&tabs_row);
    column.append(&stack);
    let holder = MaxWidth::new(&column, jf_tokens::layout::READING as i32);
    holder.add_css_class("page-column");
    let scroll = scroller(&holder);
    let (page, header) = frame(&tr("Settings"), "settings", &scroll);
    let small = small_title(&header, &tr("Settings"));
    follow_title(&scroll, &title, &small);

    for (name, button) in &tabs {
        let (name, stack, scroll) = (*name, stack.downgrade(), scroll.downgrade());
        button.connect_toggled(move |b| {
            if !b.is_active() {
                return;
            }
            if let Some(stack) = stack.upgrade()
                && stack.visible_child_name().as_deref() != Some(name)
            {
                stack.set_visible_child_name(name);
                if let Some(scroll) = scroll.upgrade() {
                    scroll.vadjustment().set_value(0.0);
                }
            }
        });
    }
    if let Some((_, first)) = tabs.first() {
        first.set_active(true);
    }
    SHOWN.with(|s| {
        *s.borrow_mut() = Some(Shown {
            stack: stack.downgrade(),
            tabs: tabs.iter().map(|(n, b)| (*n, b.downgrade())).collect(),
            scroll: scroll.downgrade(),
        })
    });
    page
}

/// Shows a section of Settings (`logic::nav::settings_section`); `None`
/// keeps the one shown last.
pub fn show(section: Option<&str>) {
    let Some(name) = section.and_then(section_for) else {
        return;
    };
    SHOWN.with(|s| {
        let s = s.borrow();
        let Some(shown) = s.as_ref() else { return };
        if let Some((_, b)) = shown.tabs.iter().find(|(n, _)| *n == name)
            && let Some(b) = b.upgrade()
        {
            b.set_active(true);
        }
        if let Some(stack) = shown.stack.upgrade() {
            stack.set_visible_child_name(name);
        }
        if let Some(scroll) = shown.scroll.upgrade() {
            scroll.vadjustment().set_value(0.0);
        }
    });
}
