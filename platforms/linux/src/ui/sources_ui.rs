//! Settings › Sources (docs/ui.md, "Settings"): Add a Source (the core's
//! presets, also on the Sources page), the installed plugins with their
//! badges, enabling, sign-in flows (QR code, browser, fields), settings forms
//! from each plugin's JSON Schema, installing from a `.jfpkg` file and
//! uninstalling, and Community Plugins with the disclosure (ADR-0012).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{AuthField, AuthMethod, AuthStep, Command, PluginBadge, PluginSummary, SourcePreset};
use serde_json::Value;

use crate::logic::schema::{self, FieldKind};
use crate::logic::signin;
use crate::ui::i18n::{tr, trf};
use crate::ui::pages::sources::{account_text, badge_text};
use crate::ui::state::{Change, app, spawn};
use crate::ui::widgets::qr::Qr;

pub fn page() -> adw::PreferencesPage {
    let p = adw::PreferencesPage::builder()
        .name("sources")
        .title(tr("Sources"))
        .icon_name("network-server-symbolic")
        .build();
    // The services Jellyfesh offers, each while its official plugin is installed.
    let add = adw::PreferencesGroup::builder()
        .title(tr("Add a Source"))
        .visible(false)
        .build();
    p.add(&add);
    let preset_rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));

    let installed = adw::PreferencesGroup::builder().title(tr("Installed")).build();
    let install = gtk::Button::with_label(&tr("Install from File…"));
    install.add_css_class("flat");
    install.connect_clicked(|_| install_from_file());
    installed.set_header_suffix(Some(&install));
    p.add(&installed);
    let rows: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));

    let community = adw::PreferencesGroup::builder()
        .title(tr("Community Plugins"))
        .description(tr(
            "Official sources are signed by Jellyfesh. Community plugins are made by others and are off unless you allow them.",
        ))
        .build();
    let allow = adw::SwitchRow::builder()
        .title(tr("Allow Community Plugins"))
        .build();
    community.add(&allow);
    p.add(&community);

    let guard = Rc::new(Cell::new(false));
    let g = guard.clone();
    allow.connect_active_notify(move |row| {
        if g.get() {
            return;
        }
        if row.is_active() {
            // Turning it on needs the disclosure first.
            g.set(true);
            row.set_active(false);
            g.set(false);
            spawn(async {
                if disclosure(None).await {
                    app().update_settings(|s| s.allow_community_plugins = true);
                }
            });
        } else {
            app().update_settings(|s| s.allow_community_plugins = false);
        }
    });

    let rebuild = {
        let installed = installed.downgrade();
        let add = add.downgrade();
        let allow = allow.downgrade();
        let guard = guard.clone();
        move || {
            let (Some(installed), Some(add), Some(allow)) =
                (installed.upgrade(), add.upgrade(), allow.upgrade())
            else {
                return;
            };
            let a = app();
            guard.set(true);
            allow.set_active(a.settings.borrow().allow_community_plugins);
            guard.set(false);
            for w in rows.borrow_mut().drain(..) {
                installed.remove(&w);
            }
            let sources = a.sources.borrow().clone();
            for w in preset_rows.borrow_mut().drain(..) {
                add.remove(&w);
            }
            add.set_visible(!sources.presets.is_empty());
            for preset in &sources.presets {
                let row = preset_row(preset, &sources.plugins);
                add.add(&row);
                preset_rows.borrow_mut().push(row.upcast());
            }
            if sources.plugins.is_empty() {
                let row = adw::ActionRow::builder()
                    .title(tr("No Sources Installed"))
                    .subtitle(tr("Install a source plugin from a .jfpkg file."))
                    .build();
                installed.add(&row);
                rows.borrow_mut().push(row.upcast());
            }
            for plugin in &sources.plugins {
                let row = plugin_row(plugin, sources.allow_community);
                installed.add(&row);
                rows.borrow_mut().push(row.upcast());
            }
        }
    };
    rebuild();
    app().listen_for(&p, move |c| {
        if matches!(c, Change::Sources | Change::Settings) {
            rebuild();
        }
    });
    p
}

/// An installed source: its name, badge and sign-in; open, Enabled, the
/// sign-in buttons (or Sign Out), its settings, its version and author, and
/// Uninstall….
fn plugin_row(plugin: &PluginSummary, allow_community: bool) -> adw::ExpanderRow {
    let subtitle = crate::logic::format::dot_join([
        Some(badge_text(plugin).as_str()),
        Some(account_text(plugin).as_str()),
    ]);
    let row = adw::ExpanderRow::builder()
        .title(glib::markup_escape_text(&plugin.name).as_str())
        .subtitle(glib::markup_escape_text(&subtitle).as_str())
        .build();
    let badge = gtk::Label::new(Some(&match plugin.badge {
        PluginBadge::Official => tr("Official"),
        PluginBadge::Community => tr("Community"),
    }));
    badge.add_css_class("badge");
    badge.add_css_class(match plugin.badge {
        PluginBadge::Official => "official",
        PluginBadge::Community => "community",
    });
    badge.set_valign(gtk::Align::Center);
    row.add_suffix(&badge);

    let enabled = adw::SwitchRow::builder()
        .title(tr("Enabled"))
        .active(plugin.enabled)
        .sensitive(plugin.badge == PluginBadge::Official || allow_community)
        .build();
    let id = plugin.id.clone();
    let needs_disclosure = plugin.needs_disclosure;
    let name = plugin.name.clone();
    let quiet = Rc::new(Cell::new(false));
    let q = quiet.clone();
    enabled.connect_active_notify(move |r| {
        if q.get() {
            return;
        }
        let on = r.is_active();
        let id = id.clone();
        if on && needs_disclosure {
            q.set(true);
            r.set_active(false);
            q.set(false);
            let name = name.clone();
            spawn(async move {
                if disclosure(Some(&name)).await {
                    let a = app();
                    a.send(Command::AcknowledgeUnofficial { id: id.clone() });
                    a.send(Command::SetPluginEnabled { id, enabled: true });
                }
            });
        } else {
            app().send(Command::SetPluginEnabled { id, enabled: on });
        }
    });
    row.add_row(&enabled);

    // Account.
    let account = adw::ActionRow::builder()
        .title(tr("Account"))
        .subtitle(if account_text(plugin).is_empty() {
            tr("No sign-in needed")
        } else {
            account_text(plugin)
        })
        .build();
    if plugin.account.is_some() {
        let out = gtk::Button::with_label(&tr("Sign Out"));
        out.set_valign(gtk::Align::Center);
        let id = plugin.id.clone();
        out.connect_clicked(move |_| {
            app().send(Command::Logout { plugin: id.clone() });
        });
        account.add_suffix(&out);
    } else {
        for method in &plugin.auth_methods {
            let b = gtk::Button::with_label(&method_label(*method));
            b.set_valign(gtk::Align::Center);
            let (id, name, method) = (plugin.id.clone(), plugin.name.clone(), *method);
            b.connect_clicked(move |b| {
                let start = Command::BeginLogin {
                    plugin: id.clone(),
                    method,
                };
                sign_in(b, &id, &name, start);
            });
            account.add_suffix(&b);
        }
    }
    row.add_row(&account);

    // Settings from the schema.
    match schema::fields(&plugin.settings_schema_json) {
        Ok(fields) => {
            for field in fields {
                row.add_row(&field_row(plugin, &field));
            }
        }
        Err(e) => {
            let r = adw::ActionRow::builder()
                .title(tr("Settings"))
                .subtitle(trf(
                    "This plugin's settings could not be read: {error}",
                    &[("error", &e)],
                ))
                .build();
            row.add_row(&r);
        }
    }

    // "Version 1.2.0 · Author", then Uninstall…, which asks first.
    let version = trf("Version {version}", &[("version", &plugin.version)]);
    let about = crate::logic::format::dot_join([Some(version.as_str()), Some(plugin.author.as_str())]);
    let about_row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&about).as_str())
        .build();
    about_row.add_css_class("dim-label");
    row.add_row(&about_row);
    let remove = adw::ActionRow::builder().title(tr("Uninstall…")).build();
    let b = gtk::Button::with_label(&tr("Uninstall…"));
    b.add_css_class("destructive-action");
    b.set_valign(gtk::Align::Center);
    let (id, name) = (plugin.id.clone(), plugin.name.clone());
    b.connect_clicked(move |b| {
        let (b, id, name) = (b.clone(), id.clone(), name.clone());
        spawn(async move {
            if confirm_uninstall(&b, &name).await {
                app().send(Command::UninstallPlugin { id });
            }
        });
    });
    remove.add_suffix(&b);
    remove.set_activatable_widget(Some(&b));
    row.add_row(&remove);
    row
}

/// "Uninstall Name?"; true when the person chose Uninstall.
async fn confirm_uninstall(from: &gtk::Button, name: &str) -> bool {
    let dialog = adw::AlertDialog::new(
        Some(&trf("Uninstall {name}?", &[("name", name)])),
        Some(&tr("Its settings and sign-in are removed.")),
    );
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("uninstall", &tr("Uninstall"));
    dialog.set_response_appearance("uninstall", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.choose_future(Some(from)).await == "uninstall"
}

/// The label of the button that signs in with `method`.
fn method_label(method: AuthMethod) -> String {
    match method {
        AuthMethod::Qr => tr("Sign In with QR Code"),
        AuthMethod::Oauth => tr("Sign In in Browser"),
        AuthMethod::CookieImport => tr("Paste Cookies"),
        AuthMethod::PasswordExchange => tr("Sign In"),
        AuthMethod::ApiKey => tr("Enter API Key"),
    }
}

/// What a service in Add a Source is, for the line under its name.
fn preset_description(preset: &SourcePreset) -> String {
    if let Some(account) = &preset.account {
        return trf("Signed in as {account}", &[("account", account)]);
    }
    match preset.id.as_str() {
        "jellyfin" => tr("Your Jellyfin server"),
        "navidrome" => tr("Your Navidrome server"),
        "subsonic" => tr("Airsonic, Gonic and other Subsonic servers"),
        "qqmusic" => tr("Your QQ Music account"),
        "spotify" => tr("Your Spotify devices"),
        _ => String::new(),
    }
}

/// A service in Add a Source (Settings › Sources and the Sources page);
/// activating it asks for the server's address and how to sign in. Its
/// picture is the icon of the plugin that connects to it, from `plugins`.
pub fn preset_row(preset: &SourcePreset, plugins: &[PluginSummary]) -> adw::ActionRow {
    let icon = plugins
        .iter()
        .find(|p| p.id == preset.plugin)
        .and_then(|p| p.icon.as_deref());
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&preset.name).as_str())
        .subtitle(glib::markup_escape_text(&preset_description(preset)).as_str())
        .activatable(true)
        .build();
    let picture = match icon {
        Some(path) => gtk::Image::from_file(path),
        None => gtk::Image::from_icon_name("network-server-symbolic"),
    };
    picture.set_pixel_size(32);
    row.add_prefix(&picture);
    row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    let preset = preset.clone();
    row.connect_activated(move |row| connect_dialog(row, &preset));
    row
}

/// "Connect to …": the server's address (for self-hosted servers, with the
/// core's example as its placeholder) and one button per way to sign in. A
/// button closes it and starts the sign-in, whose steps the sign-in dialog
/// shows.
fn connect_dialog(from: &adw::ActionRow, preset: &SourcePreset) {
    let dialog = adw::Dialog::builder()
        .title(trf("Connect to {name}", &[("name", &preset.name)]))
        .content_width(420)
        .build();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    body.add_css_class("sign-in");
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&body));
    dialog.set_child(Some(&view));

    let address = gtk::Entry::builder()
        .input_purpose(gtk::InputPurpose::Url)
        .input_hints(gtk::InputHints::NO_SPELLCHECK | gtk::InputHints::LOWERCASE)
        .text(preset.address.as_deref().unwrap_or_default())
        .build();
    if let Some(example) = &preset.address_example {
        address.set_placeholder_text(Some(example));
    }
    if preset.needs_address {
        body.append(&field_block(&tr("Server Address"), true, None, &address, &address));
    }
    let buttons = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let mut all = Vec::new();
    for method in &preset.auth_methods {
        let b = gtk::Button::with_label(&method_label(*method));
        b.add_css_class("pill");
        if *method == AuthMethod::PasswordExchange {
            b.add_css_class("suggested-action");
        }
        let (dialog_weak, entry, preset, method, from) = (
            dialog.downgrade(),
            address.downgrade(),
            preset.clone(),
            *method,
            from.downgrade(),
        );
        b.connect_clicked(move |_| {
            let typed = entry
                .upgrade()
                .map(|e| e.text().trim().to_owned())
                .unwrap_or_default();
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.close();
            }
            let Some(from) = from.upgrade() else { return };
            let start = Command::ConnectSource {
                preset: preset.id.clone(),
                address: preset.needs_address.then_some(typed),
                method,
            };
            sign_in(&from, &preset.plugin, &preset.name, start);
        });
        buttons.append(&b);
        all.push(b);
    }
    body.append(&buttons);
    if preset.needs_address {
        // Enter in the address field chooses the first way to sign in.
        if let Some(first) = all.first() {
            dialog.set_default_widget(Some(first));
            address.set_activates_default(true);
        }
        let ready = move |entry: &gtk::Entry| {
            let filled = !entry.text().trim().is_empty();
            for b in &all {
                b.set_sensitive(filled);
            }
        };
        ready(&address);
        address.connect_changed(ready);
        dialog.set_focus(Some(&address));
    }
    dialog.present(Some(from));
}

/// A sign-in field as the source describes it: its label (with "Optional"
/// when it may stay empty), the input (`shown`, whose focusable part is
/// `input`), and its help under the input.
fn field_block(
    label: &str,
    required: bool,
    help: Option<&str>,
    shown: &impl IsA<gtk::Widget>,
    input: &impl IsA<gtk::Widget>,
) -> gtk::Box {
    let block = gtk::Box::new(gtk::Orientation::Vertical, 6);
    block.add_css_class("sign-in-field");
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let title = gtk::Label::builder()
        .label(label)
        .xalign(0.0)
        .wrap(true)
        .hexpand(true)
        .build();
    title.add_css_class("heading");
    head.append(&title);
    if !required {
        let optional = gtk::Label::new(Some(&tr("Optional")));
        optional.add_css_class("caption");
        optional.add_css_class("dim-label");
        optional.set_valign(gtk::Align::Baseline);
        head.append(&optional);
    }
    block.append(&head);
    block.append(shown);
    let input = input.as_ref();
    input.update_relation(&[gtk::accessible::Relation::LabelledBy(&[title.upcast_ref()])]);
    if let Some(help) = help.filter(|h| !h.trim().is_empty()) {
        let note = gtk::Label::builder().label(help).xalign(0.0).wrap(true).build();
        note.add_css_class("caption");
        note.add_css_class("dim-label");
        block.append(&note);
        input.update_relation(&[gtk::accessible::Relation::DescribedBy(&[note.upcast_ref()])]);
    }
    if required {
        input.update_property(&[gtk::accessible::Property::Required(true)]);
    }
    block
}

/// Where a sign-in field's answer is typed.
#[derive(Clone)]
enum Answer {
    Line(gtk::Editable),
    Lines(gtk::TextBuffer),
}

impl Answer {
    fn text(&self) -> String {
        match self {
            Answer::Line(e) => e.text().to_string(),
            Answer::Lines(b) => b.text(&b.start_iter(), &b.end_iter(), false).to_string(),
        }
    }

    fn connect_changed(&self, f: impl Fn() + 'static) {
        match self {
            Answer::Line(e) => {
                e.connect_changed(move |_| f());
            }
            Answer::Lines(b) => {
                b.connect_changed(move |_| f());
            }
        }
    }
}

/// The input for a field (docs/core-api.md, "Signing in"): masked for
/// secrets, several lines for a pasted cookie header, otherwise one line
/// with the keyboard its kind asks for, and the plugin's placeholder. Gives
/// what to show, the part that takes the focus, and where the answer is.
fn field_input(field: &AuthField) -> (gtk::Widget, gtk::Widget, Answer) {
    let placeholder = field.placeholder.as_deref().unwrap_or_default();
    match signin::input_for(field) {
        signin::Input::Secret => {
            let e = gtk::PasswordEntry::builder()
                .show_peek_icon(true)
                .activates_default(true)
                .placeholder_text(placeholder)
                .build();
            (e.clone().upcast(), e.clone().upcast(), Answer::Line(e.upcast()))
        }
        signin::Input::Line(purpose) => {
            let e = gtk::Entry::builder()
                .activates_default(true)
                .placeholder_text(placeholder)
                .input_purpose(match purpose {
                    signin::Purpose::Text => gtk::InputPurpose::FreeForm,
                    signin::Purpose::Email => gtk::InputPurpose::Email,
                    signin::Purpose::Url => gtk::InputPurpose::Url,
                    signin::Purpose::Digits => gtk::InputPurpose::Digits,
                })
                .input_hints(gtk::InputHints::NO_SPELLCHECK)
                .build();
            (e.clone().upcast(), e.clone().upcast(), Answer::Line(e.upcast()))
        }
        signin::Input::Lines => {
            let text = gtk::TextView::builder()
                .wrap_mode(gtk::WrapMode::Char)
                .monospace(true)
                .accepts_tab(false)
                .input_hints(gtk::InputHints::NO_SPELLCHECK)
                .top_margin(8)
                .bottom_margin(8)
                .left_margin(10)
                .right_margin(10)
                .build();
            let buffer = text.buffer();
            // A text view has no placeholder: a quiet line over it while it
            // is empty.
            let hint = gtk::Label::builder()
                .label(placeholder)
                .xalign(0.0)
                .yalign(0.0)
                .wrap(true)
                .can_target(false)
                .margin_top(8)
                .margin_start(10)
                .margin_end(10)
                .build();
            hint.add_css_class("dim-label");
            hint.set_visible(!placeholder.is_empty());
            let h = hint.downgrade();
            let empty_placeholder = placeholder.is_empty();
            buffer.connect_changed(move |b| {
                if let Some(h) = h.upgrade() {
                    h.set_visible(!empty_placeholder && b.char_count() == 0);
                }
            });
            let scroll = gtk::ScrolledWindow::builder()
                .child(&text)
                .hscrollbar_policy(gtk::PolicyType::Never)
                .min_content_height(96)
                .max_content_height(200)
                .propagate_natural_height(true)
                .build();
            let overlay = gtk::Overlay::new();
            overlay.set_child(Some(&scroll));
            overlay.add_overlay(&hint);
            let frame = gtk::Frame::builder().child(&overlay).build();
            frame.add_css_class("view");
            if !placeholder.is_empty() {
                text.update_property(&[gtk::accessible::Property::Placeholder(placeholder)]);
            }
            (frame.upcast(), text.upcast(), Answer::Lines(buffer))
        }
    }
}

fn field_row(plugin: &PluginSummary, field: &schema::Field) -> gtk::Widget {
    let current = schema::current(&plugin.settings_json, field);
    let id = plugin.id.clone();
    let key = field.key.clone();
    let send = move |value: Value| {
        let a = app();
        let json = a
            .sources
            .borrow()
            .plugins
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.settings_json.clone())
            .unwrap_or_default();
        a.send(Command::SetPluginSettings {
            id: id.clone(),
            settings_json: schema::with_value(&json, &key, value),
        });
    };
    let title = field.title.clone();
    let widget: gtk::Widget = match &field.kind {
        FieldKind::Text { secret: true, .. } => {
            let row = adw::PasswordEntryRow::builder()
                .title(&title)
                .show_apply_button(true)
                .build();
            row.connect_apply(move |r| send(Value::String(r.text().to_string())));
            row.upcast()
        }
        FieldKind::Text { .. } => {
            let row = adw::EntryRow::builder()
                .title(&title)
                .show_apply_button(true)
                .build();
            if let Some(Value::String(s)) = &current {
                row.set_text(s);
            }
            row.connect_apply(move |r| send(Value::String(r.text().to_string())));
            row.upcast()
        }
        FieldKind::Toggle => {
            let row = adw::SwitchRow::builder()
                .title(&title)
                .active(current.as_ref().and_then(Value::as_bool).unwrap_or(false))
                .build();
            row.connect_active_notify(move |r| send(Value::Bool(r.is_active())));
            row.upcast()
        }
        FieldKind::Integer { min, max, step } | FieldKind::Number { min, max, step } => {
            let integer = matches!(field.kind, FieldKind::Integer { .. });
            let row = adw::SpinRow::with_range(*min, *max, *step);
            row.set_title(&title);
            if !integer {
                row.set_digits(2);
            }
            if let Some(v) = current.as_ref().and_then(Value::as_f64) {
                row.set_value(v);
            }
            row.connect_value_notify(move |r| {
                let v = r.value();
                send(if integer {
                    Value::from(v.round() as i64)
                } else {
                    serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
                });
            });
            row.upcast()
        }
        FieldKind::Choice { options } => {
            let labels: Vec<&str> = options.iter().map(|(_, l)| l.as_str()).collect();
            let row = adw::ComboRow::builder()
                .title(&title)
                .model(&gtk::StringList::new(&labels))
                .build();
            if let Some(v) = &current
                && let Some(i) = options.iter().position(|(o, _)| o == v)
            {
                row.set_selected(i as u32);
            }
            let values: Vec<Value> = options.iter().map(|(v, _)| v.clone()).collect();
            row.connect_selected_notify(move |r| {
                if let Some(v) = values.get(r.selected() as usize) {
                    send(v.clone());
                }
            });
            row.upcast()
        }
    };
    if let Some(d) = &field.description {
        widget.set_tooltip_text(Some(d));
    }
    widget
}

fn install_from_file() {
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(&tr("Jellyfesh Plugins")));
    filter.add_suffix("jfpkg");
    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    let dialog = gtk::FileDialog::builder()
        .title(tr("Install Plugin"))
        .filters(&filters)
        .modal(true)
        .build();
    let parent = app().window.borrow().upgrade();
    dialog.open(parent.as_ref(), gtk::gio::Cancellable::NONE, |result| {
        if let Ok(file) = result
            && let Some(path) = file.path()
        {
            app().send(Command::InstallPlugin { path });
        }
    });
}

/// The community-plugin disclosure (ADR-0012); true when accepted.
async fn disclosure(plugin: Option<&str>) -> bool {
    let heading = match plugin {
        Some(name) => trf("Turn On “{name}”?", &[("name", name)]),
        None => tr("Allow Community Plugins?"),
    };
    let body = [
        tr("Community plugins are not made or checked by Jellyfesh."),
        format!(
            "• {}",
            tr("They may break the terms of the service they connect to.")
        ),
        format!("• {}", tr("They may stop working at any time.")),
        format!("• {}", tr("Use them for personal use only.")),
    ]
    .join("\n");
    let dialog = adw::AlertDialog::new(Some(&heading), Some(&body));
    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("allow", &tr("I Understand, Allow"));
    dialog.set_response_appearance("allow", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    let parent = app().window.borrow().upgrade();
    dialog.choose_future(parent.as_ref()).await == "allow"
}

/// A sign-in flow in its dialog: the core drives the steps, the dialog
/// draws each one as the source describes it.
struct Flow {
    plugin: String,
    /// The source's name, for the sentences.
    name: String,
    /// `BeginLogin` or `ConnectSource`.
    start: Command,
    /// The flow number from the core's first step; steps of other flows
    /// are not this dialog's.
    id: Cell<Option<u64>>,
    /// Signed in, or failed: closing the dialog cancels nothing.
    over: Cell<bool>,
    dialog: glib::WeakRef<adw::Dialog>,
    body: glib::WeakRef<gtk::Box>,
}

impl Flow {
    /// Starts: a spinner until the first step arrives.
    fn begin(&self) {
        if let Some(body) = self.body.upgrade() {
            clear(&body);
            body.append(&spinner());
        }
        app().send(self.start.clone());
    }
}

fn clear(body: &gtk::Box) {
    while let Some(child) = body.first_child() {
        body.remove(&child);
    }
}

fn spinner() -> gtk::Spinner {
    gtk::Spinner::builder()
        .spinning(true)
        .halign(gtk::Align::Center)
        .width_request(32)
        .height_request(32)
        .build()
}

/// A sign-in flow: sends `start` (`BeginLogin` or `ConnectSource`) for
/// `plugin` and shows the steps that follow (docs/core-api.md, "Signing
/// in"), as in every app.
fn sign_in(from: &impl IsA<gtk::Widget>, plugin: &str, name: &str, start: Command) {
    let dialog = adw::Dialog::builder()
        .title(trf("Sign In to {name}", &[("name", name)]))
        .content_width(420)
        .build();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
    body.add_css_class("sign-in");
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&body));
    dialog.set_child(Some(&view));
    let flow = Rc::new(Flow {
        plugin: plugin.to_owned(),
        name: name.to_owned(),
        start,
        id: Cell::new(None),
        over: Cell::new(false),
        dialog: dialog.downgrade(),
        body: body.downgrade(),
    });
    let weak = Rc::downgrade(&flow);
    app().listen_for(&dialog, move |c| {
        let (Change::Auth(update), Some(flow)) = (c, weak.upgrade()) else {
            return;
        };
        if update.plugin != flow.plugin || flow.id.get().is_some_and(|x| x != update.flow) {
            return;
        }
        flow.id.set(Some(update.flow));
        render_step(&flow, &update.step, update.flow);
    });
    let f = flow.clone();
    dialog.connect_closed(move |_| {
        if let Some(id) = f.id.get()
            && !f.over.get()
        {
            app().send(Command::CancelLogin {
                plugin: f.plugin.clone(),
                flow: id,
            });
        }
    });
    flow.begin();
    dialog.present(Some(from));
}

/// A centred sentence.
fn sentence(t: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(t));
    l.set_wrap(true);
    l.set_justify(gtk::Justification::Center);
    l
}

fn render_step(flow: &Rc<Flow>, step: &AuthStep, id: u64) {
    let Some(body) = flow.body.upgrade() else { return };
    clear(&body);
    match step {
        AuthStep::ShowQr { data, hint, .. } => {
            let qr = Qr::new(data);
            qr.update_property(&[gtk::accessible::Property::Label(&tr("Sign-in QR code"))]);
            body.append(&qr);
            // The source's own words when it gives them.
            let words = hint.clone().filter(|h| !h.trim().is_empty()).unwrap_or_else(|| {
                trf(
                    "Scan the code with the {name} app on your phone.",
                    &[("name", &flow.name)],
                )
            });
            body.append(&sentence(&words));
        }
        AuthStep::ShowCode { code, hint, .. } => {
            // Large, to type on another device (Jellyfin's Quick Connect).
            let code_label = gtk::Label::new(Some(code));
            code_label.add_css_class("title-1");
            code_label.add_css_class("sign-in-code");
            code_label.set_selectable(true);
            let spelled: String = code
                .chars()
                .filter(|c| !c.is_whitespace())
                .map(String::from)
                .collect::<Vec<_>>()
                .join(" ");
            code_label.update_property(&[gtk::accessible::Property::Label(&trf(
                "Code {code}",
                &[("code", &spelled)],
            ))]);
            body.append(&code_label);
            body.append(&sentence(hint));
            let waiting = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            waiting.set_halign(gtk::Align::Center);
            let small = gtk::Spinner::builder().spinning(true).build();
            waiting.append(&small);
            let note = gtk::Label::new(Some(&tr("Waiting for approval…")));
            note.add_css_class("dim-label");
            waiting.append(&note);
            body.append(&waiting);
        }
        AuthStep::OpenUrl { url } => {
            let launch = {
                let url = url.clone();
                let dialog = flow.dialog.clone();
                move || {
                    let launcher = gtk::UriLauncher::new(&url);
                    let parent = dialog
                        .upgrade()
                        .and_then(|d| d.root())
                        .and_downcast::<gtk::Window>();
                    launcher.launch(parent.as_ref(), gtk::gio::Cancellable::NONE, |_| {});
                }
            };
            launch();
            body.append(&sentence(&tr(
                "Continue in your browser. Come back here when you are done.",
            )));
            let again = gtk::Button::with_label(&tr("Open Browser Again"));
            again.add_css_class("pill");
            again.set_halign(gtk::Align::Center);
            again.connect_clicked(move |_| launch());
            body.append(&again);
        }
        AuthStep::AskFields { fields } => ask_fields(flow, &body, fields, id),
        AuthStep::Waiting { message } => {
            body.append(&spinner());
            body.append(&sentence(message));
        }
        AuthStep::Done { account } => {
            flow.over.set(true);
            body.append(&sentence(&trf("Signed in as {account}.", &[("account", account)])));
            let close = gtk::Button::with_label(&tr("Done"));
            close.add_css_class("pill");
            close.add_css_class("suggested-action");
            close.set_halign(gtk::Align::Center);
            let dialog = flow.dialog.clone();
            close.connect_clicked(move |_| {
                if let Some(d) = dialog.upgrade() {
                    d.close();
                }
            });
            body.append(&close);
            if let Some(d) = flow.dialog.upgrade() {
                d.set_default_widget(Some(&close));
            }
        }
        AuthStep::Failed { message } => {
            // The plugin's message alone, as in every app.
            flow.over.set(true);
            let l = sentence(message);
            l.add_css_class("error-text");
            body.append(&l);
        }
    }
}

/// The fields a source asks for, each with its label, input, placeholder
/// and help, then Sign In, which waits for the required ones.
fn ask_fields(flow: &Rc<Flow>, body: &gtk::Box, fields: &[AuthField], id: u64) {
    let form = gtk::Box::new(gtk::Orientation::Vertical, 14);
    let mut answers: Vec<(AuthField, Answer)> = Vec::new();
    let mut first: Option<gtk::Widget> = None;
    for field in fields {
        let (shown, input, answer) = field_input(field);
        form.append(&field_block(
            &field.label,
            field.required,
            field.help.as_deref(),
            &shown,
            &input,
        ));
        first.get_or_insert(input);
        answers.push((field.clone(), answer));
    }
    body.append(&form);
    let submit = gtk::Button::with_label(&tr("Sign In"));
    submit.add_css_class("pill");
    submit.add_css_class("suggested-action");
    submit.set_halign(gtk::Align::Center);
    body.append(&submit);
    let answers = Rc::new(answers);
    let check = {
        let (answers, submit) = (answers.clone(), submit.downgrade());
        move || {
            let texts: Vec<String> = answers.iter().map(|(_, a)| a.text()).collect();
            let ready = signin::ready(answers.iter().zip(&texts).map(|((f, _), t)| (f, t.as_str())));
            if let Some(b) = submit.upgrade() {
                b.set_sensitive(ready);
            }
        }
    };
    let check = Rc::new(check);
    for (_, answer) in answers.iter() {
        let c = check.clone();
        answer.connect_changed(move || c());
    }
    check();
    let plugin = flow.plugin.clone();
    submit.connect_clicked(move |b| {
        let fields = answers
            .iter()
            .map(|(f, a)| (f.name.clone(), a.text()))
            .collect();
        // Until the next step arrives.
        b.set_sensitive(false);
        b.set_label(&tr("Signing in…"));
        app().send(Command::SubmitLogin {
            plugin: plugin.clone(),
            flow: id,
            fields,
        });
    });
    if let Some(d) = flow.dialog.upgrade() {
        d.set_default_widget(Some(&submit));
        if let Some(first) = &first {
            d.set_focus(Some(first));
        }
    }
}
