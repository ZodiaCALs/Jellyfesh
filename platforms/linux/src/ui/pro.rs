//! Jellyfesh Pro (ADR-0021; docs/ui.md "AI Lyrics, Customize and Pro"):
//! the Pro sheet, which any Pro action opens while Pro is inactive, Enter
//! Licence Key… and Remove Licence…. Linux sells nothing through a store:
//! Pro comes from a licence key, or a Pro Preview build. The core checks
//! keys and says whether Pro is active (the `Pro` slice).

use adw::prelude::*;
use jf_core::{Command, ProSource};

use crate::logic::pro::{self, KeyProblem};
use crate::ui::i18n::tr;
use crate::ui::state::{Change, app, spawn};

/// A refused key's message.
pub fn key_problem_text(problem: KeyProblem) -> String {
    match problem {
        KeyProblem::Malformed => tr("That doesn’t look like a Jellyfesh licence key."),
        KeyProblem::Invalid => tr("This licence key isn’t valid."),
        KeyProblem::OtherProduct => tr("This key is for another product."),
        KeyProblem::Withdrawn => tr("This licence key has been withdrawn."),
    }
}

/// Licence Agreement and Privacy Policy (and Terms of Sale) as links, when
/// the build knows the website that publishes them.
pub fn legal_links(terms_of_sale: bool) -> Option<gtk::Box> {
    let links = crate::ui::about::links()?;
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    row.set_halign(gtk::Align::Center);
    row.add_css_class("legal-links");
    let mut pages = vec![
        (tr("Licence Agreement"), links.licence_agreement),
        (tr("Privacy Policy"), links.privacy_policy),
    ];
    if terms_of_sale {
        pages.push((tr("Terms of Sale"), links.terms_of_sale));
    }
    for (label, url) in pages {
        let b = gtk::LinkButton::with_label(&url, &label);
        b.connect_activate_link(|b| {
            crate::ui::actions::open_link(&b.uri());
            gtk::glib::Propagation::Stop
        });
        row.append(&b);
    }
    Some(row)
}

/// The Pro sheet: what Pro adds, Enter Licence Key… where the build takes
/// keys, Not Now, and the legal pages. It closes itself once Pro is active.
pub fn present_sheet() {
    let a = app();
    let dialog = adw::Dialog::builder()
        .title(tr("Jellyfesh Pro"))
        .content_width(420)
        .build();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.add_css_class("pro-sheet");
    let title = gtk::Label::builder()
        .label(tr("Jellyfesh Pro"))
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    title.add_css_class("title-1");
    body.append(&title);
    let pitch = gtk::Label::builder()
        .label(tr(
            "AI Lyrics makes synced lyrics for any song. Customize makes Jellyfesh yours: corners, glass, type, lyrics and layout.",
        ))
        .wrap(true)
        .justify(gtk::Justification::Center)
        .build();
    pitch.add_css_class("pro-pitch");
    body.append(&pitch);
    let buttons = gtk::Box::new(gtk::Orientation::Vertical, 8);
    buttons.set_halign(gtk::Align::Center);
    if a.pro.borrow().licence_keys {
        let enter = gtk::Button::with_label(&tr("Enter Licence Key…"));
        enter.add_css_class("pill");
        enter.add_css_class("suggested-action");
        let weak = dialog.downgrade();
        enter.connect_clicked(move |_| {
            if let Some(d) = weak.upgrade() {
                d.close();
            }
            enter_licence_key();
        });
        buttons.append(&enter);
    }
    let not_now = gtk::Button::with_label(&tr("Not Now"));
    not_now.add_css_class("pill");
    let weak = dialog.downgrade();
    not_now.connect_clicked(move |_| {
        if let Some(d) = weak.upgrade() {
            d.close();
        }
    });
    buttons.append(&not_now);
    body.append(&buttons);
    if let Some(links) = legal_links(true) {
        body.append(&links);
    }
    let view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    header.set_show_title(false);
    view.add_top_bar(&header);
    view.set_content(Some(&body));
    dialog.set_child(Some(&view));
    let weak = dialog.downgrade();
    a.listen_for(&dialog, move |change| {
        if matches!(change, Change::Pro)
            && app().pro.borrow().active
            && let Some(d) = weak.upgrade()
        {
            d.close();
        }
    });
    let parent = a.window.borrow().upgrade();
    dialog.present(parent.as_ref());
}

/// Asks for a licence key ("JFP1-…") and activates it; a refused key asks
/// again, saying why.
pub fn enter_licence_key() {
    spawn(async {
        let mut key = String::new();
        let mut problem: Option<String> = None;
        loop {
            let dialog = adw::AlertDialog::new(Some(&tr("Enter Licence Key")), problem.as_deref());
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("activate", &tr("Activate"));
            dialog.set_response_appearance("activate", adw::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("activate"));
            dialog.set_close_response("cancel");
            let entry = gtk::Entry::builder()
                .text(&key)
                .placeholder_text("JFP1-…")
                .activates_default(true)
                .build();
            entry.update_property(&[gtk::accessible::Property::Label(&tr("Licence Key"))]);
            dialog.set_response_enabled("activate", pro::can_activate(&key));
            let weak = dialog.downgrade();
            entry.connect_changed(move |e| {
                if let Some(d) = weak.upgrade() {
                    d.set_response_enabled("activate", pro::can_activate(&e.text()));
                }
            });
            dialog.set_extra_child(Some(&entry));
            let parent = app().window.borrow().upgrade();
            if dialog.choose_future(parent.as_ref()).await != "activate" {
                return;
            }
            key = entry.text().trim().to_owned();
            match activate(&key).await {
                Ok(()) => {
                    app().toast(&tr("Jellyfesh Pro is active."));
                    return;
                }
                Err(message) => problem = Some(message),
            }
        }
    });
}

/// Sends the key; `Ok` once Pro comes from it, else the problem in words.
async fn activate(key: &str) -> Result<(), String> {
    let a = app();
    let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
    let request = a.send_quiet(Command::ActivateLicence { key: key.to_owned() });
    let id = a.listen(move |change| match change {
        Change::Pro => {
            let pro = app().pro.borrow().clone();
            if pro.licence_error.is_none() && matches!(pro.source, ProSource::Licence { .. }) {
                let _ = tx.try_send(Ok(()));
            }
        }
        Change::Failed(id, error) if *id == request => {
            // The core's reason, worded by the app when it is the key's.
            let refused = app().pro.borrow().licence_error;
            let text = refused.map_or_else(|| (*error).to_owned(), |e| key_problem_text(pro::key_problem(e)));
            let _ = tx.try_send(Err(text));
        }
        _ => {}
    });
    let answer = rx
        .recv()
        .await
        .unwrap_or_else(|_| Err(tr("This licence key isn’t valid.")));
    a.unlisten(id);
    answer
}

/// Remove Licence… asks first (docs/ui.md, "Steps that ask first").
pub fn remove_licence() {
    spawn(async {
        let a = app();
        let dialog = adw::AlertDialog::new(
            Some(&tr("Remove the Licence Key?")),
            Some(&tr(
                "Pro features stop until you enter a key or buy Jellyfesh Pro.",
            )),
        );
        dialog.add_response("cancel", &tr("Cancel"));
        dialog.add_response("remove", &tr("Remove Licence"));
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_close_response("cancel");
        let parent = a.window.borrow().upgrade();
        if dialog.choose_future(parent.as_ref()).await == "remove" {
            a.send(Command::RemoveLicence);
        }
    });
}
