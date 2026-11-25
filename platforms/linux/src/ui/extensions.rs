//! Settings › Extensions (docs/ui.md "Settings"; ADR-0019, ADR-0020,
//! ADR-0021): Jellyfesh Pro (where it comes from, Enter Licence Key… where
//! the build takes keys, Remove Licence… for a key, and the legal pages),
//! then every extension (plugins that make lyrics or set the look and offer
//! no music): its switch, its account, its settings in the app's language
//! under their headings, and its version. AI Lyrics adds its On-Device
//! Models and the origins it may send audio to; Customize and other
//! appearance plugins add Reset to Defaults. Each ends with "Version ·
//! Author" and Uninstall…, which asks as for sources. A Pro extension says
//! so while Pro is inactive. Install from File… is in the Extensions
//! heading where the build may install plugins.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{
    AuthMethod, Command, PluginBadge, PluginKind, PluginSummary, SpeechModelState, SpeechModelStatus,
};

use crate::logic::format;
use crate::logic::pro::{self as pro_words, Status};
use crate::ui::fields;
use crate::ui::i18n::{tr, trf};
use crate::ui::pro;
use crate::ui::sources_ui;
use crate::ui::state::{Change, app, spawn};

/// AI Lyrics first, then Customize, then the others by name.
fn order(p: &PluginSummary) -> (u8, String) {
    let rank = if p.capabilities.iter().any(|c| c == "make_lyrics") {
        0
    } else if p.capabilities.iter().any(|c| c == "appearance") {
        1
    } else {
        2
    };
    (rank, p.name.to_lowercase())
}

/// What the extension rows are built from: they are built again only when
/// this changes, so a slider being dragged or a row left open stays.
fn shape(plugins: &[PluginSummary]) -> String {
    let a = app();
    let pro_active = a.pro.borrow().active;
    let ai = a.ai_lyrics.borrow().clone();
    let models: Vec<&str> = ai.models.iter().map(|m| m.id.as_str()).collect();
    let consents: Vec<(&str, &str)> = ai
        .consents
        .iter()
        .map(|c| (c.plugin.as_str(), c.origin.as_str()))
        .collect();
    let plugins: Vec<_> = plugins
        .iter()
        .map(|p| {
            (
                &p.id,
                &p.name,
                &p.version,
                p.enabled,
                p.account.is_some(),
                &p.settings_schema_json,
                &p.description,
                p.badge == PluginBadge::Official,
            )
        })
        .collect();
    format!("{pro_active} {:?} {models:?} {consents:?} {plugins:?}", ai.plugin)
}

pub fn page() -> gtk::Box {
    let p = crate::ui::prefs::section("extensions");

    let pro_group = adw::PreferencesGroup::builder().title("Jellyfesh Pro").build();
    p.append(&pro_group);
    let pro_rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let fill_pro = {
        let group = pro_group.downgrade();
        move || {
            let Some(group) = group.upgrade() else { return };
            for w in pro_rows.borrow_mut().drain(..) {
                group.remove(&w);
            }
            for w in pro_rows_now() {
                group.add(&w);
                pro_rows.borrow_mut().push(w);
            }
        }
    };
    fill_pro();

    let group = adw::PreferencesGroup::builder().title(tr("Extensions")).build();
    if app().sources.borrow().can_install_plugins {
        let install = gtk::Button::with_label(&tr("Install from File…"));
        install.add_css_class("flat");
        install.connect_clicked(|_| sources_ui::install_from_file());
        group.set_header_suffix(Some(&install));
    }
    p.append(&group);
    let rows: Rc<RefCell<Vec<adw::ExpanderRow>>> = Rc::new(RefCell::new(Vec::new()));
    let shown = Rc::downgrade(&rows);
    let open: Rc<RefCell<HashSet<String>>> = Rc::new(RefCell::new(HashSet::new()));
    let models: Rc<RefCell<Vec<ModelRow>>> = Rc::new(RefCell::new(Vec::new()));
    let built = Rc::new(RefCell::new(String::new()));
    // Set by Reset to Defaults: the next settings from the core rebuild the
    // rows, whose fields all change.
    let force = Rc::new(Cell::new(true));
    let rebuild = {
        let group = group.downgrade();
        let models = models.clone();
        let force = force.clone();
        move || {
            let Some(group) = group.upgrade() else { return };
            let mut plugins: Vec<PluginSummary> = app()
                .sources
                .borrow()
                .plugins
                .iter()
                .filter(|p| p.kind == PluginKind::Extension)
                .cloned()
                .collect();
            plugins.sort_by_key(order);
            let now = shape(&plugins);
            if !force.replace(false) && *built.borrow() == now {
                update_models(&models.borrow());
                return;
            }
            *built.borrow_mut() = now;
            for r in rows.borrow_mut().drain(..) {
                if r.is_expanded() {
                    open.borrow_mut().insert(r.widget_name().to_string());
                } else {
                    open.borrow_mut().remove(r.widget_name().as_str());
                }
                group.remove(&r);
            }
            models.borrow_mut().clear();
            if plugins.is_empty() {
                let row = adw::ExpanderRow::builder()
                    .title(tr("No Extensions Installed"))
                    .subtitle(tr("Install an extension from a .jfpkg file."))
                    .enable_expansion(false)
                    .build();
                group.add(&row);
                rows.borrow_mut().push(row);
                return;
            }
            if let Some(id) = REVEAL.with(|r| r.borrow_mut().take()) {
                open.borrow_mut().insert(id);
            }
            for plugin in &plugins {
                let (row, model_rows) = extension_row(plugin, force.clone());
                row.set_widget_name(&plugin.id);
                row.set_expanded(open.borrow().contains(&plugin.id));
                group.add(&row);
                rows.borrow_mut().push(row);
                models.borrow_mut().extend(model_rows);
            }
        }
    };
    rebuild();
    // Customize… (Appearance) opens the extension that sets the look.
    OPENER.with(|o| {
        *o.borrow_mut() = Some(Box::new(move |id: &str| {
            let Some(rows) = shown.upgrade() else { return false };
            let found = rows.borrow().iter().find(|r| r.widget_name() == id).cloned();
            if let Some(r) = &found {
                r.set_expanded(true);
                r.grab_focus();
            }
            found.is_some()
        }))
    });
    app().listen_for(&p, move |c| match c {
        Change::Sources | Change::AiLyrics => rebuild(),
        Change::Pro => {
            fill_pro();
            rebuild();
        }
        _ => {}
    });
    p
}

/// The Jellyfesh Pro group's rows for the `Pro` slice.
fn pro_rows_now() -> Vec<gtk::Widget> {
    let state = app().pro.borrow().clone();
    let mut out: Vec<gtk::Widget> = Vec::new();
    let status = pro_words::status(&state);
    let line = match &status {
        Status::Preview => tr("Pro features are free during the public beta."),
        Status::Purchased => tr("Jellyfesh Pro · Purchased"),
        Status::Licence { id_suffix } => trf("Jellyfesh Pro · Licence …{suffix}", &[("suffix", id_suffix)]),
        Status::Inactive => tr("AI Lyrics and Customize are part of Jellyfesh Pro."),
    };
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&line).as_str())
        .build();
    if matches!(status, Status::Licence { .. }) {
        let remove = gtk::Button::with_label(&tr("Remove Licence…"));
        remove.set_valign(gtk::Align::Center);
        remove.connect_clicked(|_| pro::remove_licence());
        row.add_suffix(&remove);
    }
    out.push(row.upcast());
    if !state.active && state.licence_keys {
        let enter = adw::ActionRow::builder()
            .title(tr("Enter Licence Key…"))
            .activatable(true)
            .build();
        enter.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        enter.connect_activated(|_| pro::enter_licence_key());
        out.push(enter.upcast());
    }
    if let Some(links) = pro::legal_links(true) {
        let row = gtk::ListBoxRow::builder()
            .child(&links)
            .activatable(false)
            .selectable(false)
            .build();
        out.push(row.upcast());
    }
    out
}

/// An on-device speech model's row, kept to follow its download.
struct ModelRow {
    id: String,
    status: gtk::Label,
    download: gtk::Button,
    cancel: gtk::Button,
    delete: gtk::Button,
}

impl ModelRow {
    fn show(&self, state: &SpeechModelStatus) {
        let (text, download, cancel, delete) = match state {
            SpeechModelStatus::NotDownloaded => (String::new(), true, false, false),
            SpeechModelStatus::Downloading { fraction } => (
                trf(
                    "Downloading… {percent}%",
                    &[("percent", &format!("{:.0}", (fraction * 100.0).clamp(0.0, 100.0)))],
                ),
                false,
                true,
                false,
            ),
            SpeechModelStatus::Ready => (tr("Ready"), false, false, true),
            SpeechModelStatus::Failed { message } => (message.clone(), true, false, false),
        };
        self.status.set_label(&text);
        self.status.set_visible(!text.is_empty());
        self.download.set_visible(download);
        self.cancel.set_visible(cancel);
        self.delete.set_visible(delete);
    }
}

fn update_models(rows: &[ModelRow]) {
    let ai = app().ai_lyrics.borrow().clone();
    for row in rows {
        if let Some(m) = ai.models.iter().find(|m| m.id == row.id) {
            row.show(&m.state);
        }
    }
}

/// "Base · 142 MB".
fn model_line(model: &SpeechModelState) -> String {
    let size = format::size(model.bytes);
    format::dot_join([Some(model.name.as_str()), Some(size.as_str())])
}

fn model_row(model: &SpeechModelState) -> (adw::ActionRow, ModelRow) {
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&model_line(model)).as_str())
        .build();
    let status = gtk::Label::new(None);
    status.add_css_class("dim-label");
    status.set_valign(gtk::Align::Center);
    let button = |label: &str| {
        let b = gtk::Button::with_label(label);
        b.set_valign(gtk::Align::Center);
        b
    };
    let (download, cancel, delete) = (
        button(&tr("Download")),
        button(&tr("Cancel")),
        button(&tr("Delete…")),
    );
    let id = model.id.clone();
    download.connect_clicked(move |_| {
        app().send(Command::DownloadSpeechModel { id: id.clone() });
    });
    let id = model.id.clone();
    cancel.connect_clicked(move |_| {
        app().send(Command::CancelSpeechModelDownload { id: id.clone() });
    });
    let (id, name) = (model.id.clone(), model.name.clone());
    delete.connect_clicked(move |_| {
        let (id, name) = (id.clone(), name.clone());
        spawn(async move {
            let a = app();
            let dialog = adw::AlertDialog::new(
                Some(&trf("Delete the {model} Model?", &[("model", &name)])),
                Some(&tr("You can download it again.")),
            );
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("delete", &tr("Delete"));
            dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            dialog.set_close_response("cancel");
            let parent = a.window.borrow().upgrade();
            if dialog.choose_future(parent.as_ref()).await == "delete" {
                a.send(Command::DeleteSpeechModel { id });
            }
        });
    });
    row.add_suffix(&status);
    row.add_suffix(&download);
    row.add_suffix(&cancel);
    row.add_suffix(&delete);
    let kept = ModelRow {
        id: model.id.clone(),
        status,
        download,
        cancel,
        delete,
    };
    kept.show(&model.state);
    (row, kept)
}

/// A heading inside an extension's row.
fn heading(text: &str) -> gtk::Widget {
    fields::heading_row(text)
}

/// One extension: its name and summary, and when open its switch, account,
/// settings, its own rows and its version.
fn extension_row(plugin: &PluginSummary, rebuild: Rc<Cell<bool>>) -> (adw::ExpanderRow, Vec<ModelRow>) {
    let a = app();
    let needs_pro = plugin.requires_pro && !a.pro.borrow().active;
    let row = adw::ExpanderRow::builder()
        .title(glib::markup_escape_text(&plugin.name).as_str())
        .build();
    if let Some(d) = plugin.description.as_deref().filter(|d| !d.trim().is_empty()) {
        row.set_subtitle(glib::markup_escape_text(d).as_str());
    }
    if let Some(icon) = &plugin.icon {
        let image = gtk::Image::from_file(icon);
        image.set_pixel_size(28);
        image.add_css_class("plugin-icon");
        row.add_prefix(&image);
    }
    if needs_pro {
        let badge = gtk::Label::new(Some("Pro"));
        badge.add_css_class("badge");
        badge.add_css_class("pro");
        badge.set_valign(gtk::Align::Center);
        badge.update_property(&[gtk::accessible::Property::Label("Jellyfesh Pro")]);
        row.add_suffix(&badge);
    }

    let enabled = adw::SwitchRow::builder()
        .title(tr("Enabled"))
        .active(plugin.enabled)
        .build();
    let (id, name, needs_disclosure) = (plugin.id.clone(), plugin.name.clone(), plugin.needs_disclosure);
    enabled.connect_active_notify(move |r| {
        let on = r.is_active();
        let (id, name) = (id.clone(), name.clone());
        if on && needs_disclosure {
            spawn(async move {
                if sources_ui::disclosure(Some(&name)).await {
                    let a = app();
                    a.send(Command::AcknowledgeUnofficial { id: id.clone() });
                    a.send(Command::SetPluginEnabled { id, enabled: true });
                } else {
                    app().send(Command::SetPluginEnabled { id, enabled: false });
                }
            });
        } else {
            app().send(Command::SetPluginEnabled { id, enabled: on });
        }
    });
    row.add_row(&enabled);
    if needs_pro {
        let part = adw::ActionRow::builder()
            .title(tr("Part of Jellyfesh Pro."))
            .build();
        let more = gtk::Button::with_label(&tr("Learn More…"));
        more.set_valign(gtk::Align::Center);
        more.connect_clicked(|_| pro::present_sheet());
        part.add_suffix(&more);
        row.add_row(&part);
    }
    if !plugin.auth_methods.is_empty() {
        row.add_row(&account_row(plugin));
    }

    // The settings, in the app's language once the core has them.
    let anchor = row.downgrade();
    let shown = plugin.clone();
    let settings: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let tail: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let tail_rows = tail.clone();
    spawn(async move {
        let schema = fields::schema_text(&shown).await;
        let Some(row) = anchor.upgrade() else { return };
        // The rows after the settings move below them.
        for w in tail_rows.borrow().iter() {
            row.remove(w);
        }
        for (group, widgets) in fields::grouped_rows(&shown, &schema) {
            if let Some(g) = group {
                let h = heading(&g);
                row.add_row(&h);
                settings.borrow_mut().push(h);
            }
            for w in widgets {
                row.add_row(&w);
                settings.borrow_mut().push(w);
            }
        }
        for w in tail_rows.borrow().iter() {
            row.add_row(w);
        }
    });

    let mut model_rows = Vec::new();
    let add_tail = |w: gtk::Widget| {
        row.add_row(&w);
        tail.borrow_mut().push(w);
    };
    let ai = a.ai_lyrics.borrow().clone();
    let makes_lyrics = ai.plugin.as_deref() == Some(plugin.id.as_str())
        || plugin.capabilities.iter().any(|c| c == "make_lyrics");
    if makes_lyrics && !ai.models.is_empty() {
        add_tail(heading(&tr("On-Device Models")));
        for m in &ai.models {
            let (r, kept) = model_row(m);
            add_tail(r.upcast());
            model_rows.push(kept);
        }
    }
    if makes_lyrics {
        let consents: Vec<_> = ai
            .consents
            .iter()
            .filter(|c| c.plugin == plugin.id)
            .cloned()
            .collect();
        if !consents.is_empty() {
            add_tail(heading(&tr("Sending Audio")));
            for c in consents {
                let r = adw::ActionRow::builder()
                    .title(glib::markup_escape_text(&c.origin).as_str())
                    .build();
                let stop = gtk::Button::with_label(&tr("Stop Allowing"));
                stop.set_valign(gtk::Align::Center);
                stop.connect_clicked(move |_| {
                    app().send(Command::RevokeAudioUpload {
                        plugin: c.plugin.clone(),
                        origin: c.origin.clone(),
                    });
                });
                r.add_suffix(&stop);
                add_tail(r.upcast());
            }
        }
    }
    let version = trf("Version {version}", &[("version", &plugin.version)]);
    let about = format::dot_join([Some(version.as_str()), Some(plugin.author.as_str())]);
    let about_row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&about).as_str())
        .build();
    about_row.add_css_class("dim-label");
    if plugin.capabilities.iter().any(|c| c == "appearance") {
        let b = gtk::Button::with_label(&tr("Reset to Defaults"));
        b.set_valign(gtk::Align::Center);
        let id = plugin.id.clone();
        b.connect_clicked(move |_| {
            // The fields show the defaults once the core has them.
            rebuild.set(true);
            app().send(Command::ResetPluginSettings { plugin: id.clone() });
        });
        about_row.add_suffix(&b);
    }
    add_tail(about_row.upcast());
    let remove = adw::ActionRow::builder().title(tr("Uninstall…")).build();
    let b = gtk::Button::with_label(&tr("Uninstall…"));
    b.add_css_class("destructive-action");
    b.set_valign(gtk::Align::Center);
    let (id, name) = (plugin.id.clone(), plugin.name.clone());
    b.connect_clicked(move |b| {
        let (b, id, name) = (b.clone(), id.clone(), name.clone());
        spawn(async move {
            if sources_ui::confirm_uninstall(&b, &name).await {
                app().send(Command::UninstallPlugin { id });
            }
        });
    });
    remove.add_suffix(&b);
    remove.set_activatable_widget(Some(&b));
    add_tail(remove.upcast());
    (row, model_rows)
}

/// Sign In (or Enter API Key), or what is signed in and Sign Out; a
/// key-only sign-in (AI Lyrics' online mode) says "API Key Added".
fn account_row(plugin: &PluginSummary) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(tr("Account")).build();
    if let Some(name) = &plugin.account {
        let line = if plugin.auth_methods == [AuthMethod::ApiKey] {
            tr("API Key Added")
        } else {
            trf("Signed in as {account}", &[("account", name)])
        };
        row.set_subtitle(glib::markup_escape_text(&line).as_str());
        let out = gtk::Button::with_label(&tr("Sign Out"));
        out.set_valign(gtk::Align::Center);
        let id = plugin.id.clone();
        out.connect_clicked(move |_| {
            app().send(Command::Logout { plugin: id.clone() });
        });
        row.add_suffix(&out);
    } else {
        for method in &plugin.auth_methods {
            let b = gtk::Button::with_label(&sources_ui::method_label(*method));
            b.set_valign(gtk::Align::Center);
            b.set_sensitive(plugin.enabled);
            let (id, name, method) = (plugin.id.clone(), plugin.name.clone(), *method);
            b.connect_clicked(move |b| {
                let start = Command::BeginLogin {
                    plugin: id.clone(),
                    method,
                };
                sources_ui::sign_in(b, &id, &name, start);
            });
            row.add_suffix(&b);
        }
    }
    row
}

/// Opens the extension with `capability` on this page: `appearance` for
/// Appearance › Customize…, `make_lyrics` for Open AI Lyrics Settings. Shown
/// at once when the page is up, else when it is built.
pub fn reveal(capability: &str) {
    let Some(id) = app()
        .sources
        .borrow()
        .plugins
        .iter()
        .find(|p| p.kind == PluginKind::Extension && p.capabilities.iter().any(|c| c == capability))
        .map(|p| p.id.clone())
    else {
        return;
    };
    let opened = OPENER.with(|o| o.borrow().as_ref().is_some_and(|f| f(&id)));
    if !opened {
        REVEAL.with(|r| *r.borrow_mut() = Some(id));
    }
}

type Opener = Box<dyn Fn(&str) -> bool>;

thread_local! {
    /// The extension to open when the page is built next.
    static REVEAL: RefCell<Option<String>> = const { RefCell::new(None) };
    /// Opens an extension's row on the page that is up.
    static OPENER: RefCell<Option<Opener>> = const { RefCell::new(None) };
}
