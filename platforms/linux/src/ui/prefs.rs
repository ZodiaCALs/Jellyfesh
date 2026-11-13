//! Preferences: Appearance, Playback, Library and Sources.
//!
//! Every control edits a copy of the core's `Settings` slice and sends it
//! back (`SetSettings`), or sends the matching command; controls update from
//! the slice when it changes, never from their own state. Everything applies
//! at once.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use jf_core::{Command, EqSettings, ReplayGainMode, ScanStatus, Settings};

use crate::logic::appearance::{self as look, ACCENT_HUES};
use crate::logic::eq::{self, Preset};
use crate::ui::i18n::{tr, trc, trf, trn};
use crate::ui::sources_ui;
use crate::ui::state::{Change, app, spawn};
use crate::ui::widgets::glyph;
use crate::ui::widgets::preview::{ColorDot, HueSlider, Picture, Preview};
use crate::ui::window;

/// Shows Preferences, optionally on a page (`appearance`, `playback`,
/// `library`, `sources`; `lyrics` opens Appearance, which holds them now).
pub fn present(parent: &adw::ApplicationWindow, page: Option<&str>) {
    let dialog = adw::PreferencesDialog::new();
    dialog.set_title(&tr("Preferences"));
    dialog.set_search_enabled(true);
    dialog.add(&appearance_page());
    dialog.add(&playback_page());
    dialog.add(&library_page());
    dialog.add(&sources_ui::page());
    if let Some(name) = page {
        dialog.set_visible_page_name(if name == "lyrics" { "appearance" } else { name });
    }
    // The device list is current whenever Preferences opens.
    app().send(Command::RefreshDevices);
    dialog.present(Some(parent));
}

fn page(name: &str, title: &str, icon: &str) -> adw::PreferencesPage {
    adw::PreferencesPage::builder()
        .name(name)
        .title(title)
        .icon_name(icon)
        .build()
}

fn group(title: &str, description: Option<&str>) -> adw::PreferencesGroup {
    let g = adw::PreferencesGroup::builder().title(title).build();
    if let Some(d) = description {
        g.set_description(Some(d));
    }
    g
}

/// A combo row over fixed choices.
fn combo(title: &str, subtitle: Option<&str>, choices: &[String]) -> adw::ComboRow {
    let names: Vec<&str> = choices.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder()
        .title(title)
        .model(&gtk::StringList::new(&names))
        .build();
    if let Some(s) = subtitle {
        row.set_subtitle(s);
    }
    row
}

/// Runs `apply` when the settings slice changes while `widget` exists, and
/// once now; `guard` is set meanwhile so change handlers stay quiet.
fn follow_settings(
    widget: &impl IsA<gtk::Widget>,
    guard: Rc<Cell<bool>>,
    apply: impl Fn(&Settings) + 'static,
) {
    let run = move || {
        let s = app().settings.borrow().clone();
        guard.set(true);
        apply(&s);
        guard.set(false);
    };
    run();
    app().listen_for(widget, move |c| {
        if matches!(c, Change::Settings) {
            run();
        }
    });
}

/// Settings changes from a slider are sent at most this often while it
/// moves (the core stores each one).
const SLIDER_THROTTLE: Duration = Duration::from_millis(100);

/// Sends the latest value given to [`Throttle::push`] at most every
/// [`SLIDER_THROTTLE`], and the last one when the slider stops.
struct Throttle<T: Copy + 'static> {
    pending: Rc<Cell<Option<T>>>,
    timer: Rc<RefCell<Option<glib::SourceId>>>,
    send: Rc<dyn Fn(T)>,
}

impl<T: Copy + 'static> Throttle<T> {
    fn new(send: impl Fn(T) + 'static) -> Throttle<T> {
        Throttle {
            pending: Rc::new(Cell::new(None)),
            timer: Rc::new(RefCell::new(None)),
            send: Rc::new(send),
        }
    }

    fn push(&self, value: T) {
        self.pending.set(Some(value));
        if self.timer.borrow().is_some() {
            return;
        }
        if let Some(v) = self.pending.take() {
            (self.send)(v);
        }
        let (pending, timer, send) = (self.pending.clone(), self.timer.clone(), self.send.clone());
        let id = glib::timeout_add_local(SLIDER_THROTTLE, move || match pending.take() {
            Some(v) => {
                send(v);
                glib::ControlFlow::Continue
            }
            None => {
                timer.borrow_mut().take();
                glib::ControlFlow::Break
            }
        });
        *self.timer.borrow_mut() = Some(id);
    }

    /// Whether a value is waiting or was sent within the last interval.
    fn busy(&self) -> bool {
        self.timer.borrow().is_some()
    }
}

/// A segmented control: linked toggle buttons, one active.
fn segmented(labels: &[String], label: &str) -> (gtk::Box, Vec<gtk::ToggleButton>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.add_css_class("segmented");
    row.add_css_class("pref-segmented");
    row.set_valign(gtk::Align::Center);
    row.update_property(&[gtk::accessible::Property::Label(label)]);
    let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
    for l in labels {
        let b = gtk::ToggleButton::with_label(l);
        b.add_css_class("segment");
        if let Some(first) = buttons.first() {
            b.set_group(Some(first));
        }
        row.append(&b);
        buttons.push(b);
    }
    (row, buttons)
}

/// A row of picture choices (theme, Now Playing background): toggle
/// buttons each with a preview over its label.
fn picture_choices(
    choices: &[(Picture, String)],
    label: &str,
) -> (gtk::Box, Vec<gtk::ToggleButton>, Vec<Preview>) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("picture-choices");
    row.set_homogeneous(true);
    row.update_property(&[gtk::accessible::Property::Label(label)]);
    let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
    let mut previews = Vec::new();
    for (picture, text) in choices {
        let preview = Preview::new(*picture);
        let caption = gtk::Label::new(Some(text));
        caption.add_css_class("picture-caption");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.append(&preview);
        content.append(&caption);
        let b = gtk::ToggleButton::builder().child(&content).build();
        b.add_css_class("picture-choice");
        b.add_css_class("flat");
        b.update_property(&[gtk::accessible::Property::Label(text)]);
        if let Some(first) = buttons.first() {
            b.set_group(Some(first));
        }
        row.append(&b);
        buttons.push(b);
        previews.push(preview);
    }
    (row, buttons, previews)
}

/// A preferences row that holds `child`, under its title when `heading`
/// (a row alone in its group goes without: the group's title names it).
fn block_row(title: &str, heading: bool, child: &impl IsA<gtk::Widget>) -> adw::PreferencesRow {
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.add_css_class("block-row");
    if heading {
        let label = gtk::Label::builder().label(title).xalign(0.0).build();
        label.add_css_class("block-title");
        content.append(&label);
    }
    content.append(child);
    adw::PreferencesRow::builder()
        .title(title)
        .activatable(false)
        .child(&content)
        .build()
}

/// An action row with a segmented control as its suffix.
fn segmented_row(title: &str, labels: &[String]) -> (adw::ActionRow, Vec<gtk::ToggleButton>) {
    let (control, buttons) = segmented(labels, title);
    let row = adw::ActionRow::builder().title(title).build();
    row.add_suffix(&control);
    (row, buttons)
}

/// The accent swatches' names, in [`ACCENT_HUES`] order.
fn swatch_names() -> [String; 10] {
    [
        tr("Blue"),
        tr("Cyan"),
        tr("Teal"),
        tr("Green"),
        tr("Olive"),
        tr("Orange"),
        tr("Red"),
        tr("Rose"),
        tr("Magenta"),
        tr("Violet"),
    ]
}

/// Appearance (design-controls.md, "Preferences → Appearance"): theme,
/// accent colour, transparency, reduced motion, the Now Playing background,
/// lyrics size and alignment, translation and romanization, the app icon.
fn appearance_page() -> adw::PreferencesPage {
    let p = page("appearance", &tr("Appearance"), "applications-graphics-symbolic");
    let guard = Rc::new(Cell::new(false));

    // Theme: System · Light · Dark, with previews.
    let theme_group = group(&tr("Theme"), None);
    let (theme_row, themes, theme_previews) = picture_choices(
        &[
            (Picture::SystemTheme, tr("System")),
            (Picture::Theme(false), tr("Light")),
            (Picture::Theme(true), tr("Dark")),
        ],
        &tr("Theme"),
    );
    theme_group.add(&block_row(&tr("Theme"), false, &theme_row));
    p.add(&theme_group);

    // Accent colour: follow the artwork, or a swatch or custom hue.
    let accent_group = group(&tr("Accent Colour"), None);
    let follow = adw::SwitchRow::builder()
        .title(tr("Follow the Artwork"))
        .subtitle(tr(
            "The accent and the Misty field take the hue of the album cover.",
        ))
        .build();
    let swatch_box = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .min_children_per_line(5)
        .max_children_per_line(10)
        .column_spacing(6)
        .row_spacing(6)
        .build();
    swatch_box.add_css_class("swatches");
    swatch_box.update_property(&[gtk::accessible::Property::Label(&tr("Accent Colour"))]);
    let mut swatches: Vec<gtk::ToggleButton> = Vec::new();
    for (hue, name) in ACCENT_HUES.iter().zip(swatch_names()) {
        let b = gtk::ToggleButton::builder()
            .child(&ColorDot::new(*hue, 28))
            .tooltip_text(&name)
            .build();
        b.add_css_class("swatch");
        b.update_property(&[gtk::accessible::Property::Label(&name)]);
        swatch_box.append(&b);
        swatches.push(b);
    }
    let swatch_row = block_row(&tr("Colour"), true, &swatch_box);
    let hue_slider = HueSlider::new();
    hue_slider.update_property(&[gtk::accessible::Property::Label(&tr("Custom Hue"))]);
    let hue_dot = ColorDot::new(250.0, 24);
    let custom = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    custom.append(&hue_slider);
    custom.append(&hue_dot);
    let custom_row = block_row(&tr("Custom Hue"), true, &custom);
    accent_group.add(&follow);
    accent_group.add(&swatch_row);
    accent_group.add(&custom_row);
    p.add(&accent_group);

    // Glass: transparency, and reduced motion and transparency.
    let glass_group = group(&tr("Glass"), None);
    let transparency = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    transparency.set_hexpand(true);
    transparency.set_draw_value(false);
    transparency.set_round_digits(0);
    transparency.add_css_class("line-scale");
    transparency.add_mark(50.0, gtk::PositionType::Bottom, None);
    transparency.update_property(&[gtk::accessible::Property::Label(&tr("Transparency"))]);
    let solid = gtk::Label::new(Some(&trc("transparency", "Solid")));
    solid.add_css_class("scale-end");
    let clear = gtk::Label::new(Some(&trc("transparency", "Clear")));
    clear.add_css_class("scale-end");
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    line.append(&solid);
    line.append(&transparency);
    line.append(&clear);
    let glass_preview = Preview::new(Picture::Glass);
    let glass_box = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    glass_box.append(&glass_preview);
    line.set_valign(gtk::Align::Center);
    line.set_hexpand(true);
    glass_box.append(&line);
    let transparency_row = block_row(&tr("Transparency"), true, &glass_box);
    let reduce = adw::SwitchRow::builder()
        .title(tr("Reduce Motion and Transparency"))
        .subtitle(tr(
            "Solid surfaces instead of glass, and short fades instead of motion.",
        ))
        .build();
    glass_group.add(&transparency_row);
    glass_group.add(&reduce);
    p.add(&glass_group);

    // Now Playing: background, lyrics size and alignment, extra lines.
    let np_group = group(
        &tr("Now Playing"),
        Some(&tr(
            "Lyrics come from a .ttml, .lrc or .vtt file with the same name as the song, or from its tags. Word-timed lyrics light up word by word.",
        )),
    );
    let (style_box, styles, style_previews) = picture_choices(
        &[(Picture::Artwork, tr("Artwork")), (Picture::Misty, tr("Misty"))],
        &tr("Background"),
    );
    np_group.add(&block_row(&tr("Background"), true, &style_box));
    let (size_row, sizes) = segmented_row(&tr("Lyrics Size"), &[tr("Small"), tr("Medium"), tr("Large")]);
    let (align_row, aligns) = segmented_row(
        &tr("Lyrics Alignment"),
        &[trc("alignment", "Start"), trc("alignment", "Centre")],
    );
    let translation = adw::SwitchRow::builder()
        .title(tr("Show Translations"))
        .subtitle(tr(
            "A line in another language under each lyric line, when the lyrics have one.",
        ))
        .build();
    let romanization = adw::SwitchRow::builder()
        .title(tr("Show Romanization"))
        .subtitle(tr(
            "How each line reads in Latin letters, when the lyrics have it.",
        ))
        .build();
    np_group.add(&size_row);
    np_group.add(&align_row);
    np_group.add(&translation);
    np_group.add(&romanization);
    p.add(&np_group);

    let icon = group(
        &tr("App Icon"),
        Some(&tr(
            "Linux shows the Ice icon everywhere: GNOME takes one icon from the app’s metadata.",
        )),
    );
    p.add(&icon);

    // Wiring: every control sends the settings slice back to the core.
    for (i, b) in themes.iter().enumerate() {
        let g = guard.clone();
        b.connect_toggled(move |b| {
            if g.get() || !b.is_active() {
                return;
            }
            let pref = look::appearance_at(i as u32);
            app().update_settings(|s| s.appearance = pref);
        });
    }
    let g = guard.clone();
    let hs = hue_slider.downgrade();
    follow.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        let theme_hue = app().theme.borrow().as_ref().map(|t| t.hue);
        let shown = hs.upgrade().map_or_else(
            || look::shown_hue(&app().settings.borrow().hue_source, theme_hue),
            |s| s.hue(),
        );
        app().update_settings(|s| s.hue_source = look::hue_source_for(on, shown));
    });
    for (i, b) in swatches.iter().enumerate() {
        let g = guard.clone();
        b.connect_toggled(move |b| {
            if g.get() || !b.is_active() {
                return;
            }
            let hue = ACCENT_HUES[i];
            app().update_settings(|s| s.hue_source = jf_core::HueSource::Fixed { hue });
        });
    }
    let hue_send = Rc::new(Throttle::new(|hue: f32| {
        app().update_settings(|s| s.hue_source = jf_core::HueSource::Fixed { hue });
    }));
    let (hd, send) = (hue_dot.downgrade(), hue_send.clone());
    hue_slider.connect_changed(move |hue| {
        if let Some(d) = hd.upgrade() {
            d.set_hue(hue);
        }
        send.push(hue);
    });
    let transparency_send = Rc::new(Throttle::new(|t: f32| {
        app().update_settings(|s| s.transparency = t);
    }));
    let (g, gp, send) = (
        guard.clone(),
        glass_preview.downgrade(),
        transparency_send.clone(),
    );
    transparency.connect_value_changed(move |s| {
        let t = look::transparency_from_percent(s.value());
        let text = trf(
            "{percent} %",
            &[("percent", &look::transparency_percent(t).to_string())],
        );
        s.update_property(&[gtk::accessible::Property::ValueText(&text)]);
        if let Some(p) = gp.upgrade() {
            p.set_transparency(t);
        }
        if !g.get() {
            send.push(t);
        }
    });
    let g = guard.clone();
    reduce.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.reduce_effects = on);
    });
    for (i, b) in styles.iter().enumerate() {
        let g = guard.clone();
        b.connect_toggled(move |b| {
            if g.get() || !b.is_active() {
                return;
            }
            let style = look::style_at(i as u32);
            app().update_settings(|s| s.style = style);
        });
    }
    for (i, b) in sizes.iter().enumerate() {
        let g = guard.clone();
        b.connect_toggled(move |b| {
            if g.get() || !b.is_active() {
                return;
            }
            let size = look::lyrics_size_at(i as u32);
            app().update_settings(|s| s.lyrics_size = size);
        });
    }
    for (i, b) in aligns.iter().enumerate() {
        let g = guard.clone();
        b.connect_toggled(move |b| {
            if g.get() || !b.is_active() {
                return;
            }
            let align = look::lyrics_align_at(i as u32);
            app().update_settings(|s| s.lyrics_align = align);
        });
    }
    let g = guard.clone();
    translation.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.show_translation = on);
    });
    let g = guard.clone();
    romanization.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.show_romanization = on);
    });

    // The controls show the settings; the previews follow theme changes.
    let previews: Vec<Preview> = theme_previews
        .into_iter()
        .chain(style_previews)
        .chain(std::iter::once(glass_preview.clone()))
        .collect();
    let dots: Vec<gtk::Widget> = swatches.iter().filter_map(|b| b.child()).collect();
    let redraw = {
        let previews: Vec<_> = previews.iter().map(|p| p.downgrade()).collect();
        let dots: Vec<_> = dots.iter().map(|d| d.downgrade()).collect();
        let (slider, dot) = (hue_slider.downgrade(), hue_dot.downgrade());
        move || {
            for p in previews.iter().filter_map(|p| p.upgrade()) {
                p.queue_draw();
            }
            for d in dots.iter().filter_map(|d| d.upgrade()) {
                d.queue_draw();
            }
            if let (Some(s), Some(d)) = (slider.upgrade(), dot.upgrade()) {
                s.queue_draw();
                d.queue_draw();
            }
        }
    };
    let r = redraw.clone();
    app().listen_for(&p, move |c| {
        if matches!(c, Change::Theme) {
            r();
        }
    });
    let manager = adw::StyleManager::default();
    let r = redraw.clone();
    let id = RefCell::new(Some(manager.connect_dark_notify(move |_| r())));
    p.connect_destroy(move |_| {
        if let Some(id) = id.borrow_mut().take() {
            adw::StyleManager::default().disconnect(id);
        }
    });
    let weak = (
        follow.downgrade(),
        swatch_row.downgrade(),
        custom_row.downgrade(),
        hue_slider.downgrade(),
        hue_dot.downgrade(),
        transparency.downgrade(),
        glass_preview.downgrade(),
        reduce.downgrade(),
        translation.downgrade(),
        romanization.downgrade(),
    );
    follow_settings(&p, guard, move |s| {
        let (
            Some(follow),
            Some(swatch_row),
            Some(custom_row),
            Some(slider),
            Some(dot),
            Some(transparency),
            Some(glass_preview),
            Some(reduce),
            Some(translation),
            Some(romanization),
        ) = (
            weak.0.upgrade(),
            weak.1.upgrade(),
            weak.2.upgrade(),
            weak.3.upgrade(),
            weak.4.upgrade(),
            weak.5.upgrade(),
            weak.6.upgrade(),
            weak.7.upgrade(),
            weak.8.upgrade(),
            weak.9.upgrade(),
        )
        else {
            return;
        };
        if let Some(b) = themes.get(look::appearance_index(s.appearance) as usize) {
            b.set_active(true);
        }
        let following = look::follows_artwork(&s.hue_source);
        follow.set_active(following);
        swatch_row.set_visible(!following);
        custom_row.set_visible(!following);
        let theme_hue = app().theme.borrow().as_ref().map(|t| t.hue);
        let hue = look::shown_hue(&s.hue_source, theme_hue);
        if !hue_send.busy() {
            slider.set_hue(hue);
            dot.set_hue(hue);
        }
        let chosen = (!following).then(|| look::swatch_index(hue)).flatten();
        for (i, b) in swatches.iter().enumerate() {
            b.set_active(chosen == Some(i));
        }
        if !transparency_send.busy() {
            let percent = f64::from(look::transparency_percent(s.transparency));
            if (transparency.value() - percent).abs() > 0.5 {
                transparency.set_value(percent);
            }
            glass_preview.set_transparency(s.transparency);
        }
        reduce.set_active(s.reduce_effects);
        if let Some(b) = styles.get(look::style_index(s.style) as usize) {
            b.set_active(true);
        }
        if let Some(b) = sizes.get(look::lyrics_size_index(s.lyrics_size) as usize) {
            b.set_active(true);
        }
        if let Some(b) = aligns.get(look::lyrics_align_index(s.lyrics_align) as usize) {
            b.set_active(true);
        }
        translation.set_active(s.show_translation);
        romanization.set_active(s.show_romanization);
    });
    p
}

fn playback_page() -> adw::PreferencesPage {
    let p = page("playback", &tr("Playback"), "audio-speakers-symbolic");
    let guard = Rc::new(Cell::new(false));

    // Equalizer.
    let eq_group = group(&tr("Equalizer"), None);
    let eq_on = adw::SwitchRow::builder().title(tr("Equalizer")).build();
    let presets = combo(
        &tr("Preset"),
        None,
        &[
            tr("Flat"),
            tr("Bass Boost"),
            tr("Vocal"),
            tr("Treble"),
            tr("Custom"),
        ],
    );
    let bands_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    bands_box.add_css_class("eq-bands");
    bands_box.set_homogeneous(true);
    let mut sliders = Vec::new();
    for i in 0..jf_core::EqSettings::flat().bands.len() {
        let freq = EqSettings::flat().bands[i].freq_hz;
        let scale = gtk::Scale::with_range(
            gtk::Orientation::Vertical,
            f64::from(-eq::GAIN_RANGE_DB),
            f64::from(eq::GAIN_RANGE_DB),
            0.5,
        );
        scale.set_inverted(true);
        scale.set_height_request(140);
        scale.add_mark(0.0, gtk::PositionType::Right, None);
        scale.update_property(&[gtk::accessible::Property::Label(&trf(
            "{freq} Hz",
            &[("freq", &eq::band_label(freq))],
        ))]);
        let label = gtk::Label::new(Some(&eq::band_label(freq)));
        label.add_css_class("eq-label");
        label.add_css_class("numeric");
        let column = gtk::Box::new(gtk::Orientation::Vertical, 4);
        column.append(&scale);
        column.append(&label);
        bands_box.append(&column);
        sliders.push(scale);
    }
    let bands_row = adw::PreferencesRow::builder()
        .activatable(false)
        .child(&bands_box)
        .build();
    bands_row.add_css_class("eq-row");
    eq_group.add(&eq_on);
    eq_group.add(&presets);
    eq_group.add(&bands_row);
    p.add(&eq_group);

    // ReplayGain.
    let rg_group = group(
        &tr("Volume Levelling"),
        Some(&tr("ReplayGain evens out loudness using the tags in your files.")),
    );
    let rg_mode = combo(&tr("ReplayGain"), None, &[tr("Off"), tr("Track"), tr("Album")]);
    let rg_preamp = adw::SpinRow::with_range(-15.0, 15.0, 0.5);
    rg_preamp.set_title(&tr("Preamp"));
    rg_preamp.set_subtitle(&tr("Decibels added to every song"));
    rg_preamp.set_digits(1);
    rg_group.add(&rg_mode);
    rg_group.add(&rg_preamp);
    p.add(&rg_group);

    // Crossfade.
    let xf_group = group(&tr("Crossfade"), None);
    let xf = adw::ExpanderRow::builder()
        .title(tr("Crossfade"))
        .subtitle(tr("Blend the end of a song into the next."))
        .show_enable_switch(true)
        .build();
    let xf_secs = adw::SpinRow::with_range(1.0, 12.0, 1.0);
    xf_secs.set_title(&tr("Length"));
    xf_secs.set_subtitle(&tr("Seconds"));
    let xf_album = adw::SwitchRow::builder()
        .title(tr("Album Continuity"))
        .subtitle(tr("Songs of the same album join without a fade."))
        .build();
    xf.add_row(&xf_secs);
    xf.add_row(&xf_album);
    xf_group.add(&xf);
    p.add(&xf_group);

    // Output.
    let out_group = group(&tr("Output"), Some(&tr("Where the sound goes.")));
    let out_list = gtk::ListBox::new();
    out_list.add_css_class("boxed-list");
    out_list.set_selection_mode(gtk::SelectionMode::None);
    out_group.add(&out_list);
    p.add(&out_group);

    // Wiring.
    let sliders = Rc::new(sliders);
    let g = guard.clone();
    eq_on.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.eq.enabled = on);
    });
    let g = guard.clone();
    presets.connect_selected_notify(move |r| {
        if g.get() {
            return;
        }
        let preset = Preset::from_index(r.selected());
        app().update_settings(|s| s.eq = eq::apply_preset(&s.eq, preset));
    });
    for (i, scale) in sliders.iter().enumerate() {
        let g = guard.clone();
        scale.connect_value_changed(move |sc| {
            let gain = sc.value() as f32;
            // Screen readers and the tooltip say the gain in dB.
            let text = eq::gain_label(gain);
            sc.set_tooltip_text(Some(&text));
            sc.update_property(&[gtk::accessible::Property::ValueText(&text)]);
            if g.get() {
                return;
            }
            app().update_settings(|s| s.eq = eq::set_gain(&s.eq, i, gain));
        });
    }
    let g = guard.clone();
    rg_mode.connect_selected_notify(move |r| {
        if g.get() {
            return;
        }
        let mode = match r.selected() {
            1 => ReplayGainMode::Track,
            2 => ReplayGainMode::Album,
            _ => ReplayGainMode::Off,
        };
        app().update_settings(|s| s.replaygain_mode = mode);
    });
    let g = guard.clone();
    rg_preamp.connect_value_notify(move |r| {
        if g.get() {
            return;
        }
        let db = r.value() as f32;
        app().update_settings(|s| s.preamp_db = db);
    });
    let g = guard.clone();
    xf.connect_enable_expansion_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.enables_expansion();
        app().update_settings(|s| s.crossfade.enabled = on);
    });
    let g = guard.clone();
    xf_secs.connect_value_notify(move |r| {
        if g.get() {
            return;
        }
        let ms = (r.value() * 1_000.0).round() as u32;
        app().update_settings(|s| s.crossfade.duration_ms = ms);
    });
    let g = guard.clone();
    xf_album.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.crossfade.album_continuity = on);
    });
    let weak = (
        eq_on.downgrade(),
        presets.downgrade(),
        rg_mode.downgrade(),
        rg_preamp.downgrade(),
        xf.downgrade(),
        xf_secs.downgrade(),
        xf_album.downgrade(),
    );
    let sl = sliders.clone();
    follow_settings(&p, guard, move |s| {
        let (
            Some(eq_on),
            Some(presets),
            Some(rg_mode),
            Some(rg_preamp),
            Some(xf),
            Some(xf_secs),
            Some(xf_album),
        ) = (
            weak.0.upgrade(),
            weak.1.upgrade(),
            weak.2.upgrade(),
            weak.3.upgrade(),
            weak.4.upgrade(),
            weak.5.upgrade(),
            weak.6.upgrade(),
        )
        else {
            return;
        };
        eq_on.set_active(s.eq.enabled);
        presets.set_selected(eq::detect(&s.eq).index());
        for (scale, band) in sl.iter().zip(&s.eq.bands) {
            if (scale.value() - f64::from(band.gain_db)).abs() > 0.01 {
                scale.set_value(f64::from(band.gain_db));
            }
            scale.set_sensitive(s.eq.enabled);
        }
        presets.set_sensitive(s.eq.enabled);
        rg_mode.set_selected(match s.replaygain_mode {
            ReplayGainMode::Off => 0,
            ReplayGainMode::Track => 1,
            ReplayGainMode::Album => 2,
        });
        rg_preamp.set_value(f64::from(s.preamp_db));
        rg_preamp.set_sensitive(s.replaygain_mode != ReplayGainMode::Off);
        xf.set_enable_expansion(s.crossfade.enabled);
        xf_secs.set_value(f64::from(s.crossfade.duration_ms) / 1_000.0);
        xf_album.set_active(s.crossfade.album_continuity);
    });

    let fill = {
        let list = out_list.downgrade();
        move || {
            let Some(list) = list.upgrade() else { return };
            while let Some(c) = list.first_child() {
                list.remove(&c);
            }
            let a = app();
            let devices = a.devices.borrow().clone();
            let mut first: Option<gtk::CheckButton> = None;
            let mut entries: Vec<(Option<String>, String, String)> =
                vec![(None, tr("System Default"), tr("Follows the system’s choice"))];
            entries.extend(devices.devices.iter().map(|d| {
                let rates = d
                    .sample_rates
                    .iter()
                    .map(|r| crate::logic::format::sample_rate(*r))
                    .collect::<Vec<_>>()
                    .join(", ");
                (Some(d.name.clone()), d.name.clone(), rates)
            }));
            for (name, title, subtitle) in entries {
                let check = gtk::CheckButton::new();
                if let Some(f) = &first {
                    check.set_group(Some(f));
                } else {
                    first = Some(check.clone());
                }
                check.set_active(name == devices.selected);
                let row = adw::ActionRow::builder()
                    .title(glib::markup_escape_text(&title).as_str())
                    .subtitle(glib::markup_escape_text(&subtitle).as_str())
                    .activatable_widget(&check)
                    .build();
                row.add_prefix(&check);
                check.connect_toggled(move |c| {
                    if c.is_active() && app().devices.borrow().selected != name {
                        app().send(Command::SetOutputDevice { name: name.clone() });
                    }
                });
                list.append(&row);
            }
        }
    };
    fill();
    app().listen_for(&p, move |c| {
        if matches!(c, Change::Devices) {
            fill();
        }
    });
    p.connect_map(|_| {
        app().send(Command::RefreshDevices);
    });
    p
}

fn library_page() -> adw::PreferencesPage {
    let p = page("library", &tr("Library"), "folder-music-symbolic");
    let guard = Rc::new(Cell::new(false));
    let folders = group(
        &tr("Music Folders"),
        Some(&tr("Jellyfesh reads every audio file in these folders.")),
    );
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    folders.add(&list);
    let add = gtk::Button::builder()
        .child(&glyph::Glyph::new("plus", 18))
        .build();
    add.add_css_class("flat");
    add.set_tooltip_text(Some(&tr("Add Folder")));
    add.update_property(&[gtk::accessible::Property::Label(&tr("Add Folder"))]);
    add.connect_clicked(|_| window::choose_folder());
    folders.set_header_suffix(Some(&add));
    p.add(&folders);

    let scan = group(&tr("Scanning"), None);
    let rescan_row = adw::ActionRow::builder()
        .title(tr("Rescan Library"))
        .subtitle(tr("Look for new, changed and removed files."))
        .build();
    let rescan = gtk::Button::with_label(&tr("Rescan"));
    rescan.set_valign(gtk::Align::Center);
    rescan.connect_clicked(|_| {
        app().send(Command::Rescan);
    });
    rescan_row.add_suffix(&rescan);
    let progress = gtk::ProgressBar::new();
    progress.add_css_class("scan-progress");
    progress.set_visible(false);
    let progress_row = adw::PreferencesRow::builder()
        .activatable(false)
        .child(&progress)
        .build();
    progress_row.set_visible(false);
    let watch = adw::SwitchRow::builder()
        .title(tr("Watch Folders"))
        .subtitle(tr("Update the library when files change."))
        .build();
    scan.add(&rescan_row);
    scan.add(&progress_row);
    scan.add(&watch);
    p.add(&scan);

    let startup = group(&tr("Startup"), None);
    let resume = adw::SwitchRow::builder()
        .title(tr("Resume Playback"))
        .subtitle(tr("Start playing where you left off when Jellyfesh opens."))
        .build();
    startup.add(&resume);
    p.add(&startup);

    let g = guard.clone();
    watch.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.watch_folders = on);
    });
    let g = guard.clone();
    resume.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.resume_on_launch = on);
    });
    let (w, r) = (watch.downgrade(), resume.downgrade());
    follow_settings(&p, guard, move |s| {
        if let (Some(w), Some(r)) = (w.upgrade(), r.upgrade()) {
            w.set_active(s.watch_folders);
            r.set_active(s.resume_on_launch);
        }
    });

    let refresh_roots = {
        let list = list.downgrade();
        move || {
            let list = list.clone();
            spawn(async move {
                let roots = app().query(|core| core.roots()).await;
                let Some(list) = list.upgrade() else { return };
                while let Some(c) = list.first_child() {
                    list.remove(&c);
                }
                let roots = match roots {
                    Some(Ok(r)) => r,
                    _ => Vec::new(),
                };
                if roots.is_empty() {
                    let row = adw::ActionRow::builder()
                        .title(tr("No folders yet"))
                        .subtitle(tr("Add a folder with music."))
                        .build();
                    list.append(&row);
                }
                for root in roots {
                    let name = root
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| root.display().to_string());
                    let row = adw::ActionRow::builder()
                        .title(glib::markup_escape_text(&name).as_str())
                        .subtitle(glib::markup_escape_text(&root.display().to_string()).as_str())
                        .build();
                    row.add_prefix(&glyph::Glyph::new("folder", 20));
                    let remove = glyph::button("trash", 18, &tr("Remove Folder"));
                    remove.add_css_class("flat");
                    remove.set_valign(gtk::Align::Center);
                    remove.connect_clicked(move |_| {
                        app().send(Command::RemoveRoot { path: root.clone() });
                    });
                    row.add_suffix(&remove);
                    list.append(&row);
                }
            });
        }
    };
    refresh_roots();
    let (pr, pb, rb) = (progress_row.downgrade(), progress.downgrade(), rescan.downgrade());
    app().listen_for(&p, move |c| match c {
        Change::Library => refresh_roots(),
        Change::Scan(status) => {
            let (Some(pr), Some(pb), Some(rb)) = (pr.upgrade(), pb.upgrade(), rb.upgrade()) else {
                return;
            };
            match status {
                ScanStatus::Progress(p) => {
                    pr.set_visible(true);
                    pb.set_visible(true);
                    pb.pulse();
                    pb.set_show_text(true);
                    pb.set_text(Some(&trn("{n} file checked", "{n} files checked", p.files_seen)));
                    rb.set_sensitive(false);
                }
                ScanStatus::Finished(report) => {
                    pr.set_visible(true);
                    pb.set_fraction(1.0);
                    pb.set_text(Some(&trf(
                        "Done: {added} added, {updated} updated, {removed} removed",
                        &[
                            ("added", &report.added.to_string()),
                            ("updated", &report.updated.to_string()),
                            ("removed", &report.removed.to_string()),
                        ],
                    )));
                    rb.set_sensitive(true);
                }
                ScanStatus::Failed(e) => {
                    pr.set_visible(true);
                    pb.set_text(Some(e));
                    rb.set_sensitive(true);
                }
            }
        }
        _ => {}
    });
    p
}
