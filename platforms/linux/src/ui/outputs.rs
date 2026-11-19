//! The device button of Now Playing's dock and the player bar (docs/ui.md,
//! "Links and remote playback").
//!
//! While a remote session runs it opens **Devices**: every device the
//! service lists, with its kind's glyph, the playing one checked; opening it
//! asks the service for its devices again (`Remote { Refresh }`), and
//! choosing one moves playback there (`Remote { Transfer }`). Otherwise it
//! lists the output devices, System Default first, and chooses one
//! (`SetOutputDevice`); opening it sends `RefreshDevices`, as the core never
//! polls. The lists come from the core's slices, never from the button.

use std::rc::Rc;

use adw::prelude::*;
use jf_core::{Command, RemoteAction};

use crate::logic::remote::{self, DeviceRow};
use crate::ui::i18n::{tr, trf};
use crate::ui::state::{Change, app};
use crate::ui::style;
use crate::ui::widgets::glyph::Glyph;

/// What a row of the popover does when chosen.
#[derive(Debug, Clone)]
enum Choice {
    /// An output device by name (`None`: the system default).
    Output(Option<String>),
    /// A remote device by id.
    Remote(String),
}

/// The button and its popover.
pub struct DeviceButton {
    pub button: gtk::MenuButton,
    glyph: Glyph,
    label: Option<gtk::Label>,
    title: gtk::Label,
    list: gtk::ListBox,
    popover: gtk::Popover,
}

impl std::fmt::Debug for DeviceButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceButton").finish_non_exhaustive()
    }
}

impl DeviceButton {
    /// A device button; `with_label` adds the device's name beside its
    /// glyph.
    pub fn new(with_label: bool) -> Rc<DeviceButton> {
        let glyph = Glyph::new("device-speaker", 18);
        let face = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        face.append(&glyph);
        let label = with_label.then(|| {
            let l = gtk::Label::new(None);
            l.set_ellipsize(gtk::pango::EllipsizeMode::End);
            l.set_max_width_chars(18);
            face.append(&l);
            l
        });
        let button = gtk::MenuButton::builder().child(&face).build();
        button.add_css_class("device-button");
        let popover = gtk::Popover::new();
        popover.add_css_class("menu");
        popover.add_css_class("device-popover");
        style::animate_popup(&popover);
        let title = gtk::Label::builder()
            .xalign(0.0)
            .accessible_role(gtk::AccessibleRole::Heading)
            .build();
        title.add_css_class("device-popover-title");
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.add_css_class("device-list");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
        content.append(&title);
        content.append(&list);
        popover.set_child(Some(&content));
        button.set_popover(Some(&popover));
        // The lists are asked for whenever the popover opens.
        popover.connect_show(|_| {
            let a = app();
            if a.is_remote() {
                a.send_quiet(Command::Remote {
                    action: RemoteAction::Refresh,
                });
            } else {
                a.send(Command::RefreshDevices);
            }
        });
        let this = Rc::new(DeviceButton {
            button,
            glyph,
            label,
            title,
            list,
            popover,
        });
        this.refresh();
        let weak = Rc::downgrade(&this);
        app().listen_for(&this.button, move |change| {
            if matches!(change, Change::Remote | Change::Devices)
                && let Some(b) = weak.upgrade()
            {
                b.refresh();
            }
        });
        this
    }

    /// Shows the current device on the button and refills the list.
    pub fn refresh(&self) {
        let a = app();
        let rows: Vec<(Choice, String, Option<&'static str>, bool)> = match a.remote_view() {
            Some(view) => {
                self.title.set_label(&tr("Devices"));
                self.glyph
                    .set_name_glyph(view.device_kind.map_or("devices", remote::device_glyph));
                self.button.add_css_class("remote");
                let name = view.device.clone().unwrap_or_else(|| view.service.clone());
                self.face(&trf("Playing on {device}", &[("device", &name)]), &name);
                remote::device_rows(&a.remote.borrow())
                    .into_iter()
                    .map(
                        |DeviceRow {
                             id,
                             name,
                             glyph,
                             checked,
                         }| (Choice::Remote(id), name, Some(glyph), checked),
                    )
                    .collect()
            }
            None => {
                self.title.set_label(&tr("Output Device"));
                self.glyph.set_name_glyph("device-speaker");
                self.button.remove_css_class("remote");
                let devices = a.devices.borrow().clone();
                let selected = devices.selected.clone();
                let shown = selected.clone().unwrap_or_else(|| tr("System Default"));
                self.face(&trf("Output Device: {device}", &[("device", &shown)]), &shown);
                std::iter::once((
                    Choice::Output(None),
                    tr("System Default"),
                    None,
                    selected.is_none(),
                ))
                .chain(devices.devices.iter().map(|d| {
                    let checked = selected.as_deref() == Some(d.name.as_str());
                    (
                        Choice::Output(Some(d.name.clone())),
                        d.name.clone(),
                        None,
                        checked,
                    )
                }))
                .collect()
            }
        };
        while let Some(c) = self.list.first_child() {
            self.list.remove(&c);
        }
        for (choice, name, glyph, checked) in rows {
            self.list.append(&self.row(choice, &name, glyph, checked));
        }
    }

    /// The button's tooltip and accessible name, and (with a label) its
    /// text.
    fn face(&self, name: &str, device: &str) {
        if let Some(l) = &self.label {
            l.set_label(device);
        }
        self.button.set_tooltip_text(Some(name));
        self.button
            .update_property(&[gtk::accessible::Property::Label(name)]);
    }

    fn row(&self, choice: Choice, name: &str, glyph: Option<&str>, checked: bool) -> gtk::Button {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let check = Glyph::new("check", 16);
        check.set_opacity(if checked { 1.0 } else { 0.0 });
        row.append(&check);
        if let Some(g) = glyph {
            let kind = Glyph::new(g, 18);
            kind.add_css_class("device-kind");
            row.append(&kind);
        }
        let l = gtk::Label::new(Some(name));
        l.set_xalign(0.0);
        l.set_ellipsize(gtk::pango::EllipsizeMode::End);
        l.set_max_width_chars(32);
        row.append(&l);
        let button = gtk::Button::builder().child(&row).build();
        button.add_css_class("flat");
        button.update_property(&[gtk::accessible::Property::Label(name)]);
        if checked {
            button.update_state(&[gtk::accessible::State::Checked(gtk::AccessibleTristate::True)]);
        }
        let pop = self.popover.downgrade();
        button.connect_clicked(move |_| {
            let a = app();
            match &choice {
                Choice::Output(name) => {
                    a.send(Command::SetOutputDevice { name: name.clone() });
                }
                Choice::Remote(id) => {
                    a.send(Command::Remote {
                        action: RemoteAction::Transfer { device: id.clone() },
                    });
                }
            }
            if let Some(p) = pop.upgrade() {
                p.popdown();
            }
        });
        button
    }
}
