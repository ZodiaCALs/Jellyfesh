//! About: the app icon and the formal spaced-capitals wordmark (ink on
//! light, white on dark; design/brand), the version from the build, the
//! website, the third-party licences, the Licence Agreement and the Privacy
//! Policy where the build knows them (docs/ui.md, "Settings";
//! `logic::about`), Check for Updates in direct downloads (the core's
//! `Update` slice), and the design credits.

use std::path::PathBuf;

use adw::prelude::*;

use jf_core::Command;

use crate::logic::about;
use crate::ui::APP_ID;
use crate::ui::i18n::{tr, trf};
use crate::ui::state::{Change, app};
use crate::ui::widgets::glyph::Glyph;

/// What the third-party licences open: the website's page, or the file
/// installed next to the app.
enum Licences {
    Page(String),
    File(PathBuf),
}

/// The website and the pages it publishes, when the build knows it.
pub fn links() -> Option<about::Links> {
    let site = about::website(option_env!("JF_WEBSITE"), option_env!("GITHUB_REPOSITORY"));
    about::links(site.as_deref())
}

/// The website and the licences this build knows.
fn known() -> (Option<about::Links>, Option<Licences>) {
    let links = links();
    let licences = match &links {
        Some(l) => Some(Licences::Page(l.licences.clone())),
        None => std::env::current_exe()
            .ok()
            .and_then(|exe| about::notices_file(&exe))
            .filter(|f| f.is_file())
            .map(Licences::File),
    };
    (links, licences)
}

/// Opens a file in the app the system has for it.
fn open_file(path: &std::path::Path) {
    let launcher = gtk::FileLauncher::new(Some(&gtk::gio::File::for_path(path)));
    let parent = crate::ui::state::app().window.borrow().upgrade();
    launcher.launch(parent.as_ref(), gtk::gio::Cancellable::NONE, |result| {
        if let Err(e) = result
            && !e.matches(gtk::DialogError::Dismissed)
        {
            crate::ui::state::app().toast(e.message());
        }
    });
}

/// A row that opens something, with the link glyph at its end.
fn link_row(title: &str, subtitle: Option<&str>, open: impl Fn() + 'static) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).activatable(true).build();
    if let Some(s) = subtitle {
        row.set_subtitle(gtk::glib::markup_escape_text(s).as_str());
    }
    let glyph = Glyph::new("link", 16);
    glyph.add_css_class("dim-label");
    row.add_suffix(&glyph);
    row.connect_activated(move |_| open());
    row
}

pub fn present(parent: &impl IsA<gtk::Widget>) {
    let dialog = adw::Dialog::builder()
        .title(tr("About Jellyfesh"))
        .content_width(420)
        .content_height(560)
        .build();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 14);
    body.add_css_class("about");
    body.set_halign(gtk::Align::Center);

    let icon = gtk::Image::from_icon_name(APP_ID);
    icon.set_pixel_size(128);
    icon.add_css_class("about-icon");
    icon.update_property(&[gtk::accessible::Property::Label(&tr("Jellyfesh icon"))]);
    let wordmark = gtk::Picture::new();
    wordmark.set_can_shrink(true);
    wordmark.set_content_fit(gtk::ContentFit::Contain);
    wordmark.set_size_request(260, 39);
    wordmark.set_halign(gtk::Align::Center);
    wordmark.add_css_class("about-wordmark");
    wordmark.set_alternative_text(Some("JELLYFESH"));
    let set_wordmark = {
        let wordmark = wordmark.downgrade();
        move || {
            if let Some(w) = wordmark.upgrade() {
                let dark = adw::StyleManager::default().is_dark();
                w.set_resource(Some(if dark {
                    "/org/jellyfesh/Jellyfesh/brand/wordmark-white.svg"
                } else {
                    "/org/jellyfesh/Jellyfesh/brand/wordmark-ink.svg"
                }));
            }
        }
    };
    set_wordmark();
    let handler = std::cell::Cell::new(Some(
        adw::StyleManager::default().connect_dark_notify(move |_| set_wordmark()),
    ));
    dialog.connect_closed(move |_| {
        if let Some(h) = handler.take() {
            adw::StyleManager::default().disconnect(h);
        }
    });

    let version = gtk::Label::new(Some(&trf(
        "Version {version}",
        &[("version", env!("CARGO_PKG_VERSION"))],
    )));
    version.add_css_class("about-version");
    let summary = gtk::Label::new(Some(&tr("An elegant, lightweight music player.")));
    summary.set_wrap(true);
    summary.set_justify(gtk::Justification::Center);
    summary.add_css_class("about-summary");

    // The website and the third-party licences, where the build knows them.
    let (links, licences) = known();
    let places = gtk::ListBox::new();
    places.add_css_class("boxed-list");
    places.set_selection_mode(gtk::SelectionMode::None);
    if let Some(l) = &links {
        let site = l.website.clone();
        places.append(&link_row(&tr("Website"), Some(&l.website), move || {
            crate::ui::actions::open_link(&site);
        }));
    }
    match licences {
        Some(Licences::Page(page)) => {
            places.append(&link_row(&tr("Third-Party Licences"), None, move || {
                crate::ui::actions::open_link(&page);
            }))
        }
        Some(Licences::File(file)) => {
            places.append(&link_row(&tr("Third-Party Licences"), None, move || {
                open_file(&file);
            }))
        }
        None => {}
    }
    if let Some(l) = &links {
        for (title, page) in [
            (tr("Licence Agreement"), l.licence_agreement.clone()),
            (tr("Privacy Policy"), l.privacy_policy.clone()),
        ] {
            places.append(&link_row(&title, None, move || {
                crate::ui::actions::open_link(&page);
            }));
        }
    }
    places.set_visible(places.first_child().is_some());

    let credits = gtk::ListBox::new();
    credits.add_css_class("boxed-list");
    credits.set_selection_mode(gtk::SelectionMode::None);
    for (title, subtitle) in [
        (tr("Design"), tr("Designed with the Misty design language.")),
        (
            tr("Design Tokens"),
            tr("Derived from the Misty Design Language 1.1 tokens, © 2026 ZodiaCALs, Apache License 2.0."),
        ),
    ] {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(gtk::glib::markup_escape_text(&subtitle).as_str())
            .build();
        row.add_css_class("property");
        credits.append(&row);
    }

    body.append(&icon);
    body.append(&wordmark);
    body.append(&version);
    body.append(&summary);
    body.append(&places);
    if app().update.borrow().supported {
        body.append(&updates(&dialog));
    }
    body.append(&credits);
    let scroll = gtk::ScrolledWindow::builder()
        .child(&body)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&scroll));
    dialog.set_child(Some(&view));
    dialog.present(Some(parent));
}

/// Check for Updates and Check for Updates Automatically (direct
/// downloads), with "Jellyfesh {version} Is Available" and View Release
/// when the core found a newer release.
fn updates(dialog: &adw::Dialog) -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    let check = adw::ActionRow::builder().title(tr("Check for Updates")).build();
    let button = gtk::Button::with_label(&tr("Check for Updates"));
    button.set_valign(gtk::Align::Center);
    button.connect_clicked(|_| {
        app().send(Command::CheckForUpdates);
    });
    let view = gtk::Button::with_label(&tr("View Release"));
    view.set_valign(gtk::Align::Center);
    view.add_css_class("suggested-action");
    view.connect_clicked(|_| {
        let url = app().update.borrow().available.as_ref().map(|u| u.url.clone());
        if let Some(url) = url {
            crate::ui::actions::open_link(&url);
        }
    });
    check.add_suffix(&view);
    check.add_suffix(&button);
    let auto = adw::SwitchRow::builder()
        .title(tr("Check for Updates Automatically"))
        .build();
    let quiet = std::rc::Rc::new(std::cell::Cell::new(false));
    let q = quiet.clone();
    auto.connect_active_notify(move |r| {
        if !q.get() {
            let on = r.is_active();
            app().update_settings(|s| s.check_for_updates = on);
        }
    });
    list.append(&check);
    list.append(&auto);
    let show = {
        let (check, button, view, auto) = (
            check.downgrade(),
            button.downgrade(),
            view.downgrade(),
            auto.downgrade(),
        );
        move || {
            let (Some(check), Some(button), Some(view), Some(auto)) =
                (check.upgrade(), button.upgrade(), view.upgrade(), auto.upgrade())
            else {
                return;
            };
            let a = app();
            let update = a.update.borrow().clone();
            let line = if update.checking {
                tr("Checking…")
            } else if let Some(found) = &update.available {
                trf("Jellyfesh {version} Is Available", &[("version", &found.version)])
            } else {
                update.error.clone().unwrap_or_default()
            };
            check.set_subtitle(&gtk::glib::markup_escape_text(&line));
            button.set_sensitive(!update.checking);
            view.set_visible(update.available.is_some());
            quiet.set(true);
            auto.set_active(a.settings.borrow().check_for_updates);
            quiet.set(false);
        }
    };
    show();
    app().listen_for(dialog, move |c| {
        if matches!(c, Change::Update | Change::Settings) {
            show();
        }
    });
    list
}
