//! Plugin settings forms (ADR-0004; schema profile 1.2,
//! docs/plugins/authoring.md): a row per setting from `logic::schema`,
//! under its `x-jf-group` heading, in the app's language when the plugin
//! has words for it (`localized_settings_schema`). Each field applies as it
//! is committed (docs/ui.md, "Allowed differences").

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Command, PluginSummary};
use serde_json::Value;

use crate::logic::schema::{self, Field, FieldKind};
use crate::ui::i18n::tr;
use crate::ui::state::{Change, app};

/// How long a slider waits after it stops before its value is sent.
const SLIDER_SETTLE: Duration = Duration::from_millis(150);

/// The language tag for `localized_settings_schema` from the locale the
/// app shows (`LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`); `None` in
/// English and the languages the app has no words for.
pub fn schema_language() -> Option<&'static str> {
    ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.trim().is_empty())
        .and_then(|v| schema::schema_language(&v))
}

/// The plugin's settings schema in the app's language (English where the
/// plugin has no words), else as written.
pub async fn schema_text(plugin: &PluginSummary) -> String {
    let fallback = plugin.settings_schema_json.clone();
    let Some(language) = schema_language() else {
        return fallback;
    };
    let id = plugin.id.clone();
    app()
        .query(move |core| core.localized_settings_schema(&id, language))
        .await
        .flatten()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(fallback)
}

/// Sends `key` = `value` for the plugin, on top of its settings as the core
/// has them now.
fn send(plugin: &str, key: &str, value: Value) {
    let a = app();
    let json = a
        .sources
        .borrow()
        .plugins
        .iter()
        .find(|p| p.id == plugin)
        .map(|p| p.settings_json.clone())
        .unwrap_or_default();
    a.send(Command::SetPluginSettings {
        id: plugin.to_owned(),
        settings_json: schema::with_value(&json, key, value),
    });
}

/// The plugin's settings JSON as the core has it now.
fn settings_now(plugin: &str) -> String {
    app()
        .sources
        .borrow()
        .plugins
        .iter()
        .find(|p| p.id == plugin)
        .map(|p| p.settings_json.clone())
        .unwrap_or_default()
}

/// A heading row between groups of settings inside an expander row.
pub fn heading_row(text: &str) -> gtk::Widget {
    let label = gtk::Label::builder()
        .label(text)
        .xalign(0.0)
        .accessible_role(gtk::AccessibleRole::Heading)
        .build();
    label.add_css_class("heading");
    label.add_css_class("settings-group-heading");
    let row = gtk::ListBoxRow::builder()
        .child(&label)
        .activatable(false)
        .selectable(false)
        .build();
    row.upcast()
}

/// The row for one setting.
pub fn field_row(plugin: &str, settings_json: &str, field: &Field) -> gtk::Widget {
    let current = schema::current(settings_json, field);
    let (id, key) = (plugin.to_owned(), field.key.clone());
    let title = field.title.clone();
    let widget: gtk::Widget = match &field.kind {
        FieldKind::Text { secret: true, .. } => {
            let row = adw::PasswordEntryRow::builder()
                .title(&title)
                .show_apply_button(true)
                .build();
            row.connect_apply(move |r| send(&id, &key, Value::String(r.text().to_string())));
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
            row.connect_apply(move |r| send(&id, &key, Value::String(r.text().to_string())));
            row.upcast()
        }
        FieldKind::Color => color_row(&id, &key, &title, current.as_ref()),
        FieldKind::Toggle => {
            let row = adw::SwitchRow::builder()
                .title(&title)
                .active(current.as_ref().and_then(Value::as_bool).unwrap_or(false))
                .build();
            row.connect_active_notify(move |r| send(&id, &key, Value::Bool(r.is_active())));
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
            row.connect_value_notify(move |r| send(&id, &key, number(r.value(), integer)));
            row.upcast()
        }
        FieldKind::Slider { min, max, step } => {
            slider_row(&id, &key, &title, (*min, *max, *step), current.as_ref())
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
                    send(&id, &key, v.clone());
                }
            });
            row.upcast()
        }
        FieldKind::Checklist { options } => checklist_row(&id, field, options, current.as_ref()),
    };
    if let Some(d) = &field.description {
        widget.set_tooltip_text(Some(d));
    }
    widget
}

fn number(v: f64, integer: bool) -> Value {
    if integer {
        Value::from(v.round() as i64)
    } else {
        serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
    }
}

/// `#RRGGBB` in a colour well.
fn color_row(plugin: &str, key: &str, title: &str, current: Option<&Value>) -> gtk::Widget {
    let row = adw::ActionRow::builder().title(title).build();
    let dialog = gtk::ColorDialog::builder().with_alpha(false).modal(true).build();
    let button = gtk::ColorDialogButton::new(Some(dialog));
    button.set_valign(gtk::Align::Center);
    button.update_property(&[gtk::accessible::Property::Label(title)]);
    if let Some([r, g, b]) = current.and_then(Value::as_str).and_then(schema::rgb) {
        button.set_rgba(&gtk::gdk::RGBA::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            1.0,
        ));
    }
    let (id, key) = (plugin.to_owned(), key.to_owned());
    button.connect_rgba_notify(move |b| {
        let c = b.rgba();
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let hex = format!(
            "#{:02X}{:02X}{:02X}",
            byte(c.red()),
            byte(c.green()),
            byte(c.blue())
        );
        send(&id, &key, Value::String(hex));
    });
    row.add_suffix(&button);
    row.set_activatable_widget(Some(&button));
    row.upcast()
}

/// A number between a minimum and a maximum, on a slider that sends its
/// value once it settles.
fn slider_row(
    plugin: &str,
    key: &str,
    title: &str,
    (min, max, step): (f64, f64, f64),
    current: Option<&Value>,
) -> gtk::Widget {
    let row = adw::ActionRow::builder().title(title).build();
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, step);
    scale.set_width_request(200);
    scale.set_valign(gtk::Align::Center);
    scale.set_draw_value(true);
    scale.set_value_pos(gtk::PositionType::Left);
    scale.set_digits(digits(step));
    scale.update_property(&[gtk::accessible::Property::Label(title)]);
    if let Some(v) = current.and_then(Value::as_f64) {
        scale.set_value(v);
    }
    let (id, key) = (plugin.to_owned(), key.to_owned());
    let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    scale.connect_value_changed(move |s| {
        // Snap to the step, then send once the slider rests.
        let v = (((s.value() - min) / step).round() * step + min).clamp(min, max);
        if let Some(old) = pending.borrow_mut().take() {
            old.remove();
        }
        let (id, key, pending_done) = (id.clone(), key.clone(), pending.clone());
        let source = glib::timeout_add_local_once(SLIDER_SETTLE, move || {
            pending_done.borrow_mut().take();
            send(&id, &key, number(v, false));
        });
        *pending.borrow_mut() = Some(source);
    });
    row.add_suffix(&scale);
    row.upcast()
}

/// Enough decimals for `step`: 1 → 0, 0.1 → 1, 0.05 → 2.
fn digits(step: f64) -> i32 {
    let mut d = 0;
    let mut s = step.abs();
    while d < 3 && (s - s.round()).abs() > 1e-9 {
        s *= 10.0;
        d += 1;
    }
    d
}

/// An ordered checklist: each item with a check (shown or not) and Move Up
/// and Move Down; the setting lists the shown items in order.
fn checklist_row(
    plugin: &str,
    field: &Field,
    options: &[(String, String)],
    current: Option<&Value>,
) -> gtk::Widget {
    let row = adw::ExpanderRow::builder().title(&field.title).build();
    let items = Rc::new(RefCell::new(schema::checklist_rows(options, current)));
    let children: Rc<RefCell<Vec<gtk::Widget>>> = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(Checklist {
        plugin: plugin.to_owned(),
        key: field.key.clone(),
        row: row.downgrade(),
        items,
        children,
    });
    state.render();
    // The core may keep an item (Settings keeps one library place): show
    // what it has whenever it differs.
    let weak = Rc::downgrade(&state);
    let (field, options) = (field.clone(), options.to_vec());
    app().listen_for(&row, move |change| {
        let Some(state) = weak.upgrade() else { return };
        if matches!(change, Change::Sources) {
            let now = schema::current(&settings_now(&state.plugin), &field);
            let rows = schema::checklist_rows(&options, now.as_ref());
            if schema::checklist_value(&rows) != schema::checklist_value(&state.items.borrow()) {
                *state.items.borrow_mut() = rows;
                state.render();
            }
        }
    });
    row.upcast()
}

struct Checklist {
    plugin: String,
    key: String,
    row: glib::WeakRef<adw::ExpanderRow>,
    items: Rc<RefCell<Vec<(String, String, bool)>>>,
    children: Rc<RefCell<Vec<gtk::Widget>>>,
}

impl Checklist {
    fn render(self: &Rc<Self>) {
        let Some(row) = self.row.upgrade() else { return };
        for c in self.children.borrow_mut().drain(..) {
            row.remove(&c);
        }
        let items = self.items.borrow().clone();
        let shown: Vec<&str> = items
            .iter()
            .filter(|(_, _, s)| *s)
            .map(|(_, l, _)| l.as_str())
            .collect();
        row.set_subtitle(&gtk::glib::markup_escape_text(&shown.join(" · ")));
        let last = items.len().saturating_sub(1);
        for (i, (_, label, on)) in items.iter().enumerate() {
            let item = adw::ActionRow::builder().title(label.as_str()).build();
            let check = gtk::CheckButton::builder()
                .active(*on)
                .valign(gtk::Align::Center)
                .build();
            check.update_property(&[gtk::accessible::Property::Label(label)]);
            let weak = Rc::downgrade(self);
            check.connect_toggled(move |c| {
                if let Some(s) = weak.upgrade() {
                    if let Some(entry) = s.items.borrow_mut().get_mut(i) {
                        entry.2 = c.is_active();
                    }
                    s.changed();
                }
            });
            item.add_prefix(&check);
            item.set_activatable_widget(Some(&check));
            for (icon, name, to) in [
                ("go-up-symbolic", tr("Move Up"), i.checked_sub(1)),
                ("go-down-symbolic", tr("Move Down"), (i < last).then_some(i + 1)),
            ] {
                let b = gtk::Button::from_icon_name(icon);
                b.add_css_class("flat");
                b.set_valign(gtk::Align::Center);
                b.set_tooltip_text(Some(&name));
                b.update_property(&[gtk::accessible::Property::Label(&name)]);
                b.set_sensitive(to.is_some());
                let weak = Rc::downgrade(self);
                b.connect_clicked(move |_| {
                    if let (Some(s), Some(to)) = (weak.upgrade(), to) {
                        s.items.borrow_mut().swap(i, to);
                        s.changed();
                    }
                });
                item.add_suffix(&b);
            }
            row.add_row(&item);
            self.children.borrow_mut().push(item.upcast());
        }
    }

    /// Sends the shown items in order, and redraws the rows once the click
    /// that changed them has finished.
    fn changed(self: &Rc<Self>) {
        let value = schema::checklist_value(&self.items.borrow());
        send(&self.plugin, &self.key, value);
        let s = self.clone();
        glib::idle_add_local_once(move || s.render());
    }
}

/// The settings of a plugin under their headings: `(heading, rows)`, from
/// `schema_json` (the localised schema when the app shows another
/// language). A schema that cannot be read says so.
pub fn grouped_rows(plugin: &PluginSummary, schema_json: &str) -> Vec<(Option<String>, Vec<gtk::Widget>)> {
    match schema::fields(schema_json) {
        Ok(fields) => schema::grouped(fields)
            .into_iter()
            .map(|(group, fields)| {
                let rows = fields
                    .iter()
                    .map(|f| field_row(&plugin.id, &plugin.settings_json, f))
                    .collect();
                (group, rows)
            })
            .collect(),
        Err(e) => {
            let row = adw::ActionRow::builder()
                .title(tr("Settings"))
                .subtitle(crate::ui::i18n::trf(
                    "This plugin's settings could not be read: {error}",
                    &[("error", &e)],
                ))
                .build();
            vec![(None, vec![row.upcast()])]
        }
    }
}
