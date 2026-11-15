//! Settings › Sources (docs/ui.md, "Settings"): Add a Source (the core's
//! presets, also on the Sources page), the installed plugins with their
//! badges, enabling, sign-in flows (QR code, browser, fields), settings forms
//! from each plugin's JSON Schema, installing from a `.jfpkg` file and
//! uninstalling, and Community Plugins with the disclosure (ADR-0012).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use jf_core::{AuthMethod, AuthStep, Command, PluginBadge, PluginSummary, SourcePreset};
use serde_json::Value;

use crate::logic::schema::{self, FieldKind};
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

/// "Connect to …": the server's address (for self-hosted servers) and one button per way to sign
/// in. A button closes it and starts the sign-in, whose steps the sign-in dialog shows.
fn connect_dialog(from: &adw::ActionRow, preset: &SourcePreset) {
    let dialog = adw::Dialog::builder()
        .title(trf("Connect to {name}", &[("name", &preset.name)]))
        .content_width(420)
        .build();
    let body = gtk::Box::new(gtk::Orientation::Vertical, 18);
    body.set_margin_top(12);
    body.set_margin_bottom(24);
    body.set_margin_start(24);
    body.set_margin_end(24);
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&body));
    dialog.set_child(Some(&view));

    let address = adw::EntryRow::builder().title(tr("Server Address")).build();
    if preset.needs_address {
        let group = adw::PreferencesGroup::new();
        if let Some(example) = &preset.address_example {
            group.set_description(Some(&trf("For example {address}", &[("address", example)])));
        }
        address.set_text(preset.address.as_deref().unwrap_or_default());
        address.set_input_purpose(gtk::InputPurpose::Url);
        group.add(&address);
        body.append(&group);
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
        let ready = move |entry: &adw::EntryRow| {
            let filled = !entry.text().trim().is_empty();
            for b in &all {
                b.set_sensitive(filled);
            }
        };
        ready(&address);
        address.connect_changed(ready);
    }
    dialog.present(Some(from));
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

/// A sign-in flow: the core drives the steps, the dialog renders each. Sends
/// `start` (`BeginLogin` or `ConnectSource`) for `plugin` and shows the steps
/// that follow.
fn sign_in(from: &impl IsA<gtk::Widget>, plugin: &str, name: &str, start: Command) {
    let a = app();
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
    let flow: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
    let done = Rc::new(Cell::new(false));
    let plugin_id = plugin.to_owned();
    let display = name.to_owned();
    let waiting = gtk::Spinner::builder()
        .spinning(true)
        .width_request(32)
        .height_request(32)
        .build();
    body.append(&waiting);
    let (b, f, d, pid) = (body.downgrade(), flow.clone(), done.clone(), plugin_id.clone());
    let dialog_weak = dialog.downgrade();
    a.listen_for(&dialog, move |c| {
        let Change::Auth(update) = c else { return };
        if update.plugin != pid {
            return;
        }
        if f.get().is_some_and(|x| x != update.flow) {
            return;
        }
        f.set(Some(update.flow));
        let Some(body) = b.upgrade() else { return };
        while let Some(child) = body.first_child() {
            body.remove(&child);
        }
        render_step(&body, &update.step, &pid, update.flow, &display, &d, &dialog_weak);
    });
    let (f, d, pid) = (flow.clone(), done.clone(), plugin_id.clone());
    dialog.connect_closed(move |_| {
        if let Some(flow) = f.get()
            && !d.get()
        {
            app().send(Command::CancelLogin {
                plugin: pid.clone(),
                flow,
            });
        }
    });
    a.send(start);
    dialog.present(Some(from));
}

fn render_step(
    body: &gtk::Box,
    step: &AuthStep,
    plugin: &str,
    flow: u64,
    name: &str,
    done: &Rc<Cell<bool>>,
    dialog: &glib::WeakRef<adw::Dialog>,
) {
    let text = |t: &str| {
        let l = gtk::Label::new(Some(t));
        l.set_wrap(true);
        l.set_justify(gtk::Justification::Center);
        l
    };
    match step {
        AuthStep::ShowQr { data, .. } => {
            let qr = Qr::new(data);
            qr.update_property(&[gtk::accessible::Property::Label(&tr("Sign-in QR code"))]);
            body.append(&qr);
            body.append(&text(&trf(
                "Scan the code with the {name} app on your phone.",
                &[("name", name)],
            )));
        }
        AuthStep::ShowCode { code, hint, .. } => {
            let code_label = gtk::Label::new(Some(code));
            code_label.add_css_class("title-1");
            code_label.set_selectable(true);
            body.append(&code_label);
            body.append(&text(hint));
        }
        AuthStep::OpenUrl { url } => {
            let launch = {
                let url = url.clone();
                let dialog = dialog.clone();
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
            body.append(&text(&tr(
                "Continue in your browser. Come back here when you are done.",
            )));
            let again = gtk::Button::with_label(&tr("Open Browser Again"));
            again.add_css_class("pill");
            again.set_halign(gtk::Align::Center);
            again.connect_clicked(move |_| launch());
            body.append(&again);
        }
        AuthStep::AskFields { fields } => {
            let list = gtk::ListBox::new();
            list.add_css_class("boxed-list");
            list.set_selection_mode(gtk::SelectionMode::None);
            let mut entries: Vec<(String, gtk::Editable)> = Vec::new();
            for field in fields {
                let row: gtk::Widget = if field.secret {
                    let r = adw::PasswordEntryRow::builder().title(&field.label).build();
                    entries.push((field.name.clone(), r.clone().upcast()));
                    r.upcast()
                } else {
                    let r = adw::EntryRow::builder().title(&field.label).build();
                    entries.push((field.name.clone(), r.clone().upcast()));
                    r.upcast()
                };
                list.append(&row);
            }
            body.append(&list);
            let submit = gtk::Button::with_label(&tr("Sign In"));
            submit.add_css_class("pill");
            submit.add_css_class("suggested-action");
            submit.set_halign(gtk::Align::Center);
            let plugin = plugin.to_owned();
            submit.connect_clicked(move |_| {
                let fields = entries
                    .iter()
                    .map(|(name, e)| (name.clone(), e.text().to_string()))
                    .collect();
                app().send(Command::SubmitLogin {
                    plugin: plugin.clone(),
                    flow,
                    fields,
                });
            });
            body.append(&submit);
        }
        AuthStep::Waiting { message } => {
            body.append(
                &gtk::Spinner::builder()
                    .spinning(true)
                    .width_request(32)
                    .height_request(32)
                    .build(),
            );
            body.append(&text(message));
        }
        AuthStep::Done { account } => {
            done.set(true);
            body.append(&text(&trf("Signed in as {account}.", &[("account", account)])));
            let close = gtk::Button::with_label(&tr("Done"));
            close.add_css_class("pill");
            close.add_css_class("suggested-action");
            close.set_halign(gtk::Align::Center);
            let dialog = dialog.clone();
            close.connect_clicked(move |_| {
                if let Some(d) = dialog.upgrade() {
                    d.close();
                }
            });
            body.append(&close);
        }
        AuthStep::Failed { message } => {
            done.set(true);
            let l = text(message);
            l.add_css_class("error-text");
            body.append(&l);
        }
    }
}
