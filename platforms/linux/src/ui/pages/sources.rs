//! Sources: adding a source, the installed ones, and browsing a source's
//! catalogue.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Collection, Command, CommandId, ItemKind, PluginBadge, PluginSummary};

use crate::ui::convert;
use crate::ui::i18n::{tr, trf};
use crate::ui::models::ItemData;
use crate::ui::pages::detail::{fill_track_box, track_box};
use crate::ui::pages::{follow_title, frame, keep_alive, large_title, scroller, small_title, status};
use crate::ui::rows::RowStyle;
use crate::ui::sources_ui::preset_row;
use crate::ui::state::{Change, app};
use crate::ui::widgets::glyph;
use crate::ui::{actions, window};

/// The badge line for a source (ADR-0004, ADR-0012).
pub fn badge_text(p: &PluginSummary) -> String {
    let mut parts = vec![match p.badge {
        PluginBadge::Official => tr("Official"),
        PluginBadge::Community => tr("Community · unofficial"),
    }];
    if p.vendor_playback {
        parts.push(tr("Vendor playback"));
    }
    parts.join(" · ")
}

/// Who is signed in, or that nobody is.
pub fn account_text(p: &PluginSummary) -> String {
    match &p.account {
        Some(a) => trf("Signed in as {account}", &[("account", a)]),
        None if p.auth_methods.is_empty() => String::new(),
        None => tr("Not signed in"),
    }
}

/// Sources (docs/ui.md, "Sources"): what sources are, Add a Source (the
/// core's presets, with the same connect step as Settings), Your Sources
/// (each installed source with its status; choosing one browses it), and
/// Sources Settings….
pub fn page() -> adw::NavigationPage {
    let title = large_title(&tr("Sources"));
    let intro = gtk::Label::builder()
        .label(tr(
            "Sources bring music from servers and services into Jellyfesh.",
        ))
        .xalign(0.0)
        .wrap(true)
        .build();
    intro.add_css_class("page-subtitle");
    let add = adw::PreferencesGroup::builder()
        .title(tr("Add a Source"))
        .visible(false)
        .build();
    let presets = gtk::ListBox::new();
    presets.add_css_class("boxed-list");
    presets.set_selection_mode(gtk::SelectionMode::None);
    presets.update_property(&[gtk::accessible::Property::Label(&tr("Add a Source"))]);
    add.add(&presets);
    let yours = adw::PreferencesGroup::builder().title(tr("Your Sources")).build();
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.update_property(&[gtk::accessible::Property::Label(&tr("Your Sources"))]);
    yours.add(&list);
    let settings = gtk::Button::with_label(&tr("Sources Settings…"));
    settings.add_css_class("pill");
    settings.set_halign(gtk::Align::Start);
    settings.connect_clicked(|_| window::show_settings(Some("sources")));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 24);
    column.add_css_class("sources-page");
    let top = gtk::Box::new(gtk::Orientation::Vertical, 6);
    top.append(&title);
    top.append(&intro);
    column.append(&top);
    column.append(&add);
    column.append(&yours);
    column.append(&settings);
    let clamp = adw::Clamp::builder().child(&column).maximum_size(800).build();
    clamp.add_css_class("detail");
    let scroll = scroller(&clamp);
    let (page, header) = frame(&tr("Sources"), "sources", &scroll);
    let small = small_title(&header, &tr("Sources"));
    follow_title(&scroll, &title, &small);

    let rebuild = {
        let (presets, list, add) = (presets.downgrade(), list.downgrade(), add.downgrade());
        move || {
            let (Some(presets), Some(list), Some(add)) = (presets.upgrade(), list.upgrade(), add.upgrade())
            else {
                return;
            };
            let sources = app().sources.borrow().clone();
            while let Some(c) = presets.first_child() {
                presets.remove(&c);
            }
            for preset in &sources.presets {
                presets.append(&preset_row(preset, &sources.plugins));
            }
            add.set_visible(!sources.presets.is_empty());
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            if sources.plugins.is_empty() {
                let row = adw::ActionRow::builder()
                    .title(tr("No Sources Installed"))
                    .subtitle(tr("Install a source plugin from a .jfpkg file."))
                    .build();
                list.append(&row);
            }
            for p in &sources.plugins {
                list.append(&source_row(p));
            }
        }
    };
    rebuild();
    app().listen_for(&page, move |change| {
        if matches!(change, Change::Sources) {
            rebuild();
        }
    });
    page
}

/// An installed source with its status; activating it browses the source.
fn source_row(p: &PluginSummary) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(gtk::glib::markup_escape_text(&p.name).as_str())
        .subtitle(gtk::glib::markup_escape_text(&crate::logic::format::dot_join([
            Some(badge_text(p).as_str()),
            Some(account_text(p).as_str()),
        ])))
        .activatable(p.enabled)
        .build();
    match &p.icon {
        Some(path) => {
            let icon = gtk::Image::from_file(path);
            icon.set_pixel_size(32);
            row.add_prefix(&icon);
        }
        None => row.add_prefix(&glyph::Glyph::new("sources", 22)),
    }
    if p.enabled {
        row.add_suffix(&glyph::Glyph::new("forward", 18));
        let (id, name) = (p.id.clone(), p.name.clone());
        row.connect_activated(move |_| window::show_source(&id, None, &name));
    } else {
        let off = gtk::Label::new(Some(&tr("Off")));
        off.add_css_class("dim-label");
        row.add_suffix(&off);
    }
    row
}

struct Browse {
    plugin: String,
    node: Option<String>,
    requests: RefCell<Vec<CommandId>>,
    next_page: Cell<Option<u32>>,
    items: Rc<RefCell<Vec<ItemData>>>,
    list: gtk::ListBox,
    more: gtk::Button,
    stack: gtk::Stack,
    error: adw::StatusPage,
}

/// One container of a source's catalogue (`node` `None` is its root).
pub fn browse_page(plugin: &str, node: Option<String>, title: &str) -> adw::NavigationPage {
    let browse_items: Rc<RefCell<Vec<ItemData>>> = Rc::new(RefCell::new(Vec::new()));
    let opened = browse_items.clone();
    let list = track_box(RowStyle::TRACK, move |i| {
        let items = opened.borrow();
        let Some(d) = items.get(i) else { return };
        match d.source.as_ref().map(|s| s.kind) {
            Some(ItemKind::Track) => {
                let refs: Vec<_> = items.iter().filter_map(|x| x.track.clone()).collect();
                let start = items[..i].iter().filter(|x| x.track.is_some()).count();
                actions::play(Collection::Tracks { tracks: refs }, start, false);
            }
            _ => {
                if let Some(s) = &d.source {
                    window::show_source(&s.source, Some(s.id.clone()), &s.title);
                }
            }
        }
    });
    let more = gtk::Button::with_label(&tr("Load More"));
    more.add_css_class("pill");
    more.set_halign(gtk::Align::Center);
    more.set_visible(false);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.append(&list);
    body.append(&more);
    let clamp = adw::Clamp::builder().child(&body).maximum_size(1000).build();
    clamp.add_css_class("detail");
    let stack = gtk::Stack::new();
    let loading = gtk::Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .width_request(32)
        .height_request(32)
        .build();
    stack.add_named(&loading, Some("loading"));
    stack.add_named(&scroller(&clamp), Some("list"));
    let error = status("dialog-warning-symbolic", &tr("This Source Did Not Answer"), "");
    stack.add_named(&error, Some("error"));
    let (page, _) = frame(
        title,
        &format!("source-{plugin}-{}", node.as_deref().unwrap_or("root")),
        &stack,
    );

    let browse = Rc::new(Browse {
        plugin: plugin.to_owned(),
        node,
        requests: RefCell::new(Vec::new()),
        next_page: Cell::new(None),
        items: browse_items,
        list,
        more: more.clone(),
        stack,
        error,
    });
    request(&browse, 0);
    more.connect_clicked(glib::clone!(
        #[weak]
        browse,
        move |_| {
            if let Some(p) = browse.next_page.get() {
                request(&browse, p);
            }
        }
    ));
    let weak = Rc::downgrade(&browse);
    app().listen_for(&page, move |change| {
        let Some(browse) = weak.upgrade() else { return };
        match change {
            Change::Browse(update) if browse.requests.borrow().contains(&update.request) => {
                if let Some(e) = &update.error {
                    browse.error.set_description(Some(e));
                    browse.stack.set_visible_child_name("error");
                    return;
                }
                browse
                    .items
                    .borrow_mut()
                    .extend(update.items.iter().map(convert::source_item));
                let items = browse.items.borrow().clone();
                fill_track_box(&browse.list, RowStyle::TRACK, &items);
                browse.next_page.set(update.next_page);
                browse.more.set_visible(update.next_page.is_some());
                browse.stack.set_visible_child_name("list");
            }
            Change::Failed(id, error) if browse.requests.borrow().contains(id) => {
                browse.error.set_description(Some(error));
                browse.stack.set_visible_child_name("error");
            }
            _ => {}
        }
    });
    keep_alive(&page, browse);
    page
}

fn request(browse: &Browse, page: u32) {
    let id = app().send_quiet(Command::Browse {
        plugin: browse.plugin.clone(),
        node: browse.node.clone(),
        page,
    });
    browse.requests.borrow_mut().push(id);
}
