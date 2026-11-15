//! About: the app icon and the formal spaced-capitals wordmark (ink on
//! light, white on dark; design/brand), version, credits and legal notes.

use adw::prelude::*;

use crate::ui::APP_ID;
use crate::ui::i18n::{tr, trf};

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

    let credits = gtk::ListBox::new();
    credits.add_css_class("boxed-list");
    credits.set_selection_mode(gtk::SelectionMode::None);
    for (title, subtitle) in [
        (tr("Design"), tr("Designed with the Misty design language.")),
        (
            tr("Design Tokens"),
            tr("Derived from the Misty Design Language 1.1 tokens, © 2026 ZodiaCALs, Apache License 2.0."),
        ),
        (
            tr("Licence"),
            tr("No licence has been chosen yet: all rights reserved."),
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
