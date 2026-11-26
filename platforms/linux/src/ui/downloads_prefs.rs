//! Settings › Downloads (docs/ui.md, "Settings"; ADR-0025 §2): Folder,
//! Quality, File Names with a live preview, Tags, Network (Pause on Metered
//! Connections, which the network GIO reports drives, and Downloads at
//! Once), and the space used with Remove All Downloads….

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use jf_core::{Command, DownloadQuality};

use crate::logic::format;
use crate::ui::actions;
use crate::ui::i18n::{tr, trf};
use crate::ui::prefs::{combo, follow_settings, group, section};
use crate::ui::state::{Change, app, spawn};

/// The qualities in the menu's order.
const QUALITIES: [DownloadQuality; 4] = [
    DownloadQuality::Original,
    DownloadQuality::High,
    DownloadQuality::Normal,
    DownloadQuality::Low,
];

fn quality_name(q: DownloadQuality) -> String {
    match q {
        DownloadQuality::Original => tr("Original"),
        DownloadQuality::High => tr("High"),
        DownloadQuality::Normal => tr("Normal"),
        DownloadQuality::Low => tr("Low"),
    }
}

fn quality_note(q: DownloadQuality) -> String {
    match q {
        DownloadQuality::Original => tr("The file exactly as your server keeps it."),
        DownloadQuality::High => tr("About 320 kbit/s"),
        DownloadQuality::Normal => tr("About 192 kbit/s"),
        DownloadQuality::Low => tr("About 128 kbit/s"),
    }
}

pub fn page() -> gtk::Box {
    let p = section("downloads");
    let guard = Rc::new(Cell::new(false));

    // Folder: where downloads go, with Change… and Show in Files.
    let folder_group = group(&tr("Folder"), None);
    let folder = adw::ActionRow::builder().title(tr("Downloads Folder")).build();
    folder.add_css_class("property");
    let show = gtk::Button::builder()
        .label(tr("Show in Files"))
        .valign(gtk::Align::Center)
        .build();
    show.add_css_class("flat");
    show.connect_clicked(|_| {
        let path = app().downloads.borrow().folder.clone();
        actions::show_in_files(&path);
    });
    let change = gtk::Button::builder()
        .label(tr("Change…"))
        .valign(gtk::Align::Center)
        .build();
    change.connect_clicked(|_| {
        let dialog = gtk::FileDialog::builder()
            .title(tr("Choose a Downloads Folder"))
            .accept_label(tr("Choose"))
            .modal(true)
            .build();
        let parent = app().window.borrow().upgrade();
        dialog.select_folder(parent.as_ref(), gtk::gio::Cancellable::NONE, |result| {
            if let Ok(chosen) = result
                && let Some(path) = chosen.path()
            {
                app().update_settings(|s| s.downloads.folder = Some(path));
            }
        });
    });
    folder.add_suffix(&show);
    folder.add_suffix(&change);
    folder_group.add(&folder);
    p.append(&folder_group);

    // Quality.
    let quality_group = group(&tr("Quality"), None);
    let names: Vec<String> = QUALITIES.iter().map(|q| quality_name(*q)).collect();
    let quality = combo(&tr("Quality"), None, &names);
    quality_group.add(&quality);
    p.append(&quality_group);

    // File Names: the template, its fields and one song's path.
    let names_group = group(
        &tr("File Names"),
        Some(&tr(
            "{artist}, {album_artist}, {album}, {title}, {track}, {disc}, {year} and {source}, with / between folders.",
        )),
    );
    let template = adw::EntryRow::builder()
        .title(tr("Template"))
        .show_apply_button(true)
        .build();
    let preview = adw::ActionRow::builder().title(tr("Example")).build();
    preview.add_css_class("property");
    names_group.add(&template);
    names_group.add(&preview);
    p.append(&names_group);

    // Tags.
    let tags = group(&tr("Tags"), None);
    let add_missing = adw::SwitchRow::builder()
        .title(tr("Add Missing Cover and Lyrics"))
        .subtitle(tr("Original files are kept as they are unless this is on."))
        .build();
    let lyrics = adw::SwitchRow::builder().title(tr("Lyrics")).build();
    let lrc = adw::SwitchRow::builder().title(tr("Save .lrc Files")).build();
    tags.add(&add_missing);
    tags.add(&lyrics);
    tags.add(&lrc);
    p.append(&tags);

    // Network.
    let network = group(&tr("Network"), None);
    let metered = adw::SwitchRow::builder()
        .title(tr("Pause on Metered Connections"))
        .subtitle(tr(
            "Downloads wait while you're on mobile data, a hotspot or another metered connection.",
        ))
        .build();
    let at_once = adw::SpinRow::with_range(1.0, 4.0, 1.0);
    at_once.set_title(&tr("Downloads at Once"));
    network.add(&metered);
    network.add(&at_once);
    p.append(&network);

    // Space Used and Remove All Downloads….
    let space_group = adw::PreferencesGroup::new();
    let space = adw::ActionRow::builder().title(tr("Space Used")).build();
    space.add_css_class("property");
    let remove_all = gtk::Button::builder()
        .label(tr("Remove All Downloads…"))
        .valign(gtk::Align::Center)
        .build();
    remove_all.add_css_class("destructive-action");
    remove_all.connect_clicked(|_| {
        spawn(async move {
            let a = app();
            let dialog = adw::AlertDialog::new(
                Some(&tr("Remove All Downloads?")),
                Some(&tr("Every downloaded file is deleted from this device.")),
            );
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("remove", &tr("Remove All"));
            dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
            dialog.set_close_response("cancel");
            let parent = a.window.borrow().upgrade();
            if dialog.choose_future(parent.as_ref()).await == "remove" {
                a.send(Command::RemoveAllDownloads);
            }
        });
    });
    space.add_suffix(&remove_all);
    space_group.add(&space);
    p.append(&space_group);

    // Wiring: every control sends the settings back to the core.
    let g = guard.clone();
    quality.connect_selected_notify(move |row| {
        let q = QUALITIES
            .get(row.selected() as usize)
            .copied()
            .unwrap_or(DownloadQuality::Original);
        row.set_subtitle(&quality_note(q));
        if !g.get() {
            app().update_settings(|s| s.downloads.quality = q);
        }
    });
    let shown_preview = preview.downgrade();
    template.connect_changed(move |row| {
        if let Some(p) = shown_preview.upgrade() {
            p.set_subtitle(&jf_core::download_preview(&row.text()));
        }
    });
    template.connect_apply(|row| {
        let text = row.text().to_string();
        app().update_settings(|s| s.downloads.name_template = text);
    });
    let switch = |row: &adw::SwitchRow, set: fn(&mut jf_core::DownloadSettings, bool)| {
        let g = guard.clone();
        row.connect_active_notify(move |r| {
            if !g.get() {
                let on = r.is_active();
                app().update_settings(|s| set(&mut s.downloads, on));
            }
        });
    };
    switch(&add_missing, |d, on| d.add_missing_cover_and_lyrics = on);
    switch(&lyrics, |d, on| d.lyrics_in_tags = on);
    switch(&lrc, |d, on| d.save_lrc_files = on);
    switch(&metered, |d, on| d.wifi_only = on);
    let g = guard.clone();
    at_once.connect_value_notify(move |row| {
        if !g.get() {
            let n = row.value().round().clamp(1.0, 4.0) as u32;
            app().update_settings(|s| s.downloads.at_once = n);
        }
    });

    let rows = (
        quality.downgrade(),
        template.downgrade(),
        add_missing.downgrade(),
        lyrics.downgrade(),
        lrc.downgrade(),
        metered.downgrade(),
        at_once.downgrade(),
    );
    follow_settings(&p, guard, move |s| {
        let d = &s.downloads;
        if let Some(q) = rows.0.upgrade() {
            let i = QUALITIES.iter().position(|x| *x == d.quality).unwrap_or(0);
            q.set_selected(i as u32);
            q.set_subtitle(&quality_note(d.quality));
        }
        if let Some(t) = rows.1.upgrade()
            && t.text() != d.name_template
        {
            t.set_text(&d.name_template);
        }
        let switches = [
            (rows.2.upgrade(), d.add_missing_cover_and_lyrics),
            (rows.3.upgrade(), d.lyrics_in_tags),
            (rows.4.upgrade(), d.save_lrc_files),
            (rows.5.upgrade(), d.wifi_only),
        ];
        for (row, on) in switches {
            if let Some(row) = row {
                row.set_active(on);
            }
        }
        if let Some(n) = rows.6.upgrade() {
            n.set_value(f64::from(d.at_once));
        }
    });

    // The folder in use and the space used come with the Downloads slice.
    let shown = (folder.downgrade(), space.downgrade());
    let follow_downloads = move || {
        let d = app().downloads.borrow().clone();
        if let Some(f) = shown.0.upgrade() {
            f.set_subtitle(&gtk::glib::markup_escape_text(&d.folder.display().to_string()));
        }
        if let Some(s) = shown.1.upgrade() {
            s.set_subtitle(&trf("{size} used", &[("size", &format::size(d.bytes_used))]));
        }
    };
    follow_downloads();
    app().listen_for(&p, move |change| {
        if matches!(change, Change::Downloads) {
            follow_downloads();
        }
    });
    p
}
