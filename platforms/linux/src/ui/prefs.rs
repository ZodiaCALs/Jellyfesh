//! Settings › Appearance, Playback and Library, with their groups and rows
//! in the order docs/ui.md ("Settings") gives every app; Sources,
//! Extensions and About are in their own modules, and `pages::settings`
//! shows them as one page of the main window.
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
use crate::logic::{eq, format, playback, quality};
use crate::ui::actions;
use crate::ui::i18n::{tr, trc, trf, trn};
use crate::ui::state::{Change, app, spawn};
use crate::ui::widgets::glyph;
use crate::ui::widgets::preview::{ColorDot, HueSlider, Picture, Preview};
use crate::ui::window;

/// A section of Settings: its groups in a column (the page shows one
/// section at a time, `pages::settings`).
pub fn section(name: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Vertical, 0);
    b.add_css_class("settings-section");
    b.set_widget_name(name);
    b
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

/// Appearance (docs/ui.md, "Settings"): Theme, Accent Colour,
/// Transparency, Now Playing Backdrop, Lyrics, and the note on the app icon.
pub fn appearance_page() -> gtk::Box {
    let p = section("appearance");
    let guard = Rc::new(Cell::new(false));

    // Theme: System · Light · Dark, with previews.
    let theme_group = group(&tr("Theme"), Some(&tr("Light, dark, or the same as the system.")));
    let (theme_row, themes, theme_previews) = picture_choices(
        &[
            (Picture::SystemTheme, tr("System")),
            (Picture::Theme(false), tr("Light")),
            (Picture::Theme(true), tr("Dark")),
        ],
        &tr("Theme"),
    );
    theme_group.add(&block_row(&tr("Theme"), false, &theme_row));
    p.append(&theme_group);

    // Accent colour: follow the artwork, or a swatch or custom hue.
    let accent_group = group(&tr("Accent Colour"), None);
    let follow = adw::SwitchRow::builder()
        .title(tr("Follow the Artwork"))
        .subtitle(tr("The colour of buttons, selections and the Misty field."))
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
    let custom_row = block_row(&tr("Custom"), true, &custom);
    accent_group.add(&follow);
    accent_group.add(&swatch_row);
    accent_group.add(&custom_row);
    p.append(&accent_group);

    // Transparency: Solid … Clear, and reduced motion and transparency.
    let glass_group = group(
        &tr("Transparency"),
        Some(&tr("How much of the background shows through the glass.")),
    );
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
    let transparency_row = block_row(&tr("Transparency"), false, &glass_box);
    let reduce = adw::SwitchRow::builder()
        .title(tr("Reduce Motion and Transparency"))
        .subtitle(tr(
            "Solid surfaces instead of glass, and short fades instead of movement.",
        ))
        .build();
    glass_group.add(&transparency_row);
    glass_group.add(&reduce);
    p.append(&glass_group);

    // Now Playing Backdrop: Artwork or Misty, with previews.
    let backdrop_group = group(
        &tr("Now Playing Backdrop"),
        Some(&tr("The artwork, blurred, or the Misty field in its colour.")),
    );
    let (style_box, styles, style_previews) = picture_choices(
        &[(Picture::Artwork, tr("Artwork")), (Picture::Misty, tr("Misty"))],
        &tr("Now Playing Backdrop"),
    );
    backdrop_group.add(&block_row(&tr("Now Playing Backdrop"), false, &style_box));
    p.append(&backdrop_group);

    // Lyrics: size and alignment, extra lines (lyrics settings live only here).
    let lyrics_group = group(
        &tr("Lyrics"),
        Some(&tr(
            "Lyrics come from a .ttml, .lrc or .vtt file with the same name as the song, or from its tags. Word-timed lyrics light up word by word.",
        )),
    );
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
    let lyrics_bar = adw::SwitchRow::builder()
        .title(tr("Show Lyrics Bar"))
        .subtitle(tr("The line being sung, under the title in the player bar."))
        .build();
    let desktop_lyrics = adw::SwitchRow::builder()
        .title(tr("Desktop Lyrics"))
        .subtitle(tr("The current and next line in a small window of their own."))
        .build();
    lyrics_group.add(&size_row);
    lyrics_group.add(&align_row);
    lyrics_group.add(&translation);
    lyrics_group.add(&romanization);
    lyrics_group.add(&lyrics_bar);
    lyrics_group.add(&desktop_lyrics);
    p.append(&lyrics_group);

    let icon = group(
        &tr("App Icon"),
        Some(&tr(
            "Linux shows the Ice icon everywhere: GNOME takes one icon from the app’s metadata.",
        )),
    );
    p.append(&icon);

    // Customize…: Settings › Extensions with Customize open; the Pro badge
    // while Pro is inactive.
    let customize_group = adw::PreferencesGroup::new();
    let customize = adw::ActionRow::builder()
        .title(tr("Customize…"))
        .subtitle(tr("Corners, glass, type, lyrics style and layout."))
        .activatable(true)
        .build();
    let badge = gtk::Label::new(Some("Pro"));
    badge.add_css_class("badge");
    badge.add_css_class("pro");
    badge.set_valign(gtk::Align::Center);
    badge.update_property(&[gtk::accessible::Property::Label("Jellyfesh Pro")]);
    badge.set_visible(!app().pro.borrow().active);
    customize.add_suffix(&badge);
    customize.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    customize.connect_activated(|_| {
        crate::ui::extensions::reveal("appearance");
        crate::ui::pages::settings::show(Some("extensions"));
    });
    let shown_badge = badge.downgrade();
    app().listen_for(&customize, move |c| {
        if matches!(c, Change::Pro)
            && let Some(b) = shown_badge.upgrade()
        {
            b.set_visible(!app().pro.borrow().active);
        }
    });
    customize_group.add(&customize);
    p.append(&customize_group);

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
    let g = guard.clone();
    lyrics_bar.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.show_lyrics_bar = on);
    });
    let g = guard.clone();
    desktop_lyrics.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.desktop_lyrics = on);
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
        lyrics_bar.downgrade(),
        desktop_lyrics.downgrade(),
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
            Some(lyrics_bar),
            Some(desktop_lyrics),
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
            weak.10.upgrade(),
            weak.11.upgrade(),
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
        lyrics_bar.set_active(s.show_lyrics_bar);
        desktop_lyrics.set_active(s.desktop_lyrics);
    });
    p
}

/// The name of one of the core's equalizer presets (`jf_core::eq_presets()`).
fn preset_name(id: &str) -> String {
    match id {
        "flat" => tr("Flat"),
        "bass_boost" => tr("Bass Boost"),
        "vocal" => tr("Vocal"),
        "treble" => tr("Treble"),
        other => other.to_owned(),
    }
}

/// Shows a spin row's whole seconds with their unit ("5 s"). `with_range`
/// makes the row numeric, which would refuse the unit, so the row takes any
/// text; GTK's own input reads the number a value starts with.
fn show_seconds(row: &adw::SpinRow) {
    row.set_numeric(false);
    row.set_width_chars(5);
    row.connect_output(|row| {
        let seconds = row.value().round() as i64;
        row.set_text(&trf("{n} s", &[("n", &seconds.to_string())]));
        true
    });
}

/// Playback (docs/ui.md, "Settings"): Output, Crossfade, Volume Levelling,
/// Equalizer, Streaming and Startup.
pub fn playback_page() -> gtk::Box {
    let p = section("playback");
    let guard = Rc::new(Cell::new(false));

    // Output: the device, System Default first.
    let out_group = group(&tr("Output"), None);
    let device = adw::ComboRow::builder()
        .title(tr("Output Device"))
        .subtitle(tr("Where the sound goes."))
        .build();
    out_group.add(&device);
    p.append(&out_group);

    // Crossfade.
    let xf_group = group(&tr("Crossfade"), None);
    let xf = adw::ExpanderRow::builder()
        .title(tr("Crossfade"))
        .subtitle(tr("Blend the end of a song into the next."))
        .show_enable_switch(true)
        .build();
    let xf_secs = adw::SpinRow::with_range(1.0, 12.0, 1.0);
    xf_secs.set_title(&tr("Length"));
    show_seconds(&xf_secs);
    let xf_album = adw::SwitchRow::builder()
        .title(tr("Album Continuity"))
        .subtitle(tr("Songs of the same album join without a fade."))
        .build();
    xf.add_row(&xf_secs);
    xf.add_row(&xf_album);
    xf_group.add(&xf);
    p.append(&xf_group);

    // Volume Levelling: ReplayGain and its preamp.
    let rg_group = group(
        &tr("Volume Levelling"),
        Some(&tr("Evens out loudness using the ReplayGain tags in your files.")),
    );
    let rg_mode = combo(&tr("ReplayGain"), None, &[tr("Off"), tr("Track"), tr("Album")]);
    let rg_preamp = adw::SpinRow::with_range(-15.0, 15.0, 0.5);
    rg_preamp.set_title(&tr("Preamp"));
    rg_preamp.set_subtitle(&tr("Decibels added to every song"));
    rg_preamp.set_digits(1);
    rg_group.add(&rg_mode);
    rg_group.add(&rg_preamp);
    p.append(&rg_group);

    // Equalizer: on or off, the core's presets (Custom once a band moves),
    // the preamp, the bands and Reset to Flat.
    let eq_group = group(&tr("Equalizer"), None);
    let eq_on = adw::SwitchRow::builder().title(tr("Equalizer")).build();
    let presets = Rc::new(eq::presets());
    let preset_names: Vec<String> = presets.iter().map(|p| preset_name(&p.id)).collect();
    let preset_row = combo(&tr("Preset"), None, &preset_names);
    let range = f64::from(eq::GAIN_RANGE_DB);
    let eq_preamp = adw::SpinRow::with_range(-range, range, 0.5);
    eq_preamp.set_title(&tr("Preamp"));
    eq_preamp.set_subtitle(&tr("Decibels added before the bands"));
    eq_preamp.set_digits(1);
    let bands_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    bands_box.add_css_class("eq-bands");
    bands_box.set_homogeneous(true);
    let mut sliders = Vec::new();
    for band in &EqSettings::flat().bands {
        let freq = band.freq_hz;
        let scale = gtk::Scale::with_range(gtk::Orientation::Vertical, -range, range, 0.5);
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
    let reset = gtk::Button::with_label(&tr("Reset to Flat"));
    reset.set_halign(gtk::Align::End);
    reset.add_css_class("eq-reset");
    reset.connect_clicked(|_| {
        app().update_settings(|s| s.eq = eq::reset_to_flat(&s.eq));
    });
    let bands_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    bands_column.append(&bands_box);
    bands_column.append(&reset);
    let bands_row = adw::PreferencesRow::builder()
        .activatable(false)
        .child(&bands_column)
        .build();
    bands_row.add_css_class("eq-row");
    eq_group.add(&eq_on);
    eq_group.add(&preset_row);
    eq_group.add(&eq_preamp);
    eq_group.add(&bands_row);
    p.append(&eq_group);

    // Streaming (ADR-0026 §1): the level asked of sources, the same setting
    // as the player bar's quality menu.
    let streaming = group(&tr("Streaming"), None);
    let level_names: Vec<String> = quality::LEVELS
        .iter()
        .map(|l| crate::ui::quality::level_name(*l))
        .collect();
    let streaming_quality = combo(
        &tr("Streaming Quality"),
        Some(&tr("Auto plays lossless, or High on a metered connection.")),
        &level_names,
    );
    streaming.add(&streaming_quality);
    p.append(&streaming);
    let g = guard.clone();
    streaming_quality.connect_selected_notify(move |row| {
        if g.get() {
            return;
        }
        let level = quality::level_at(row.selected());
        app().update_settings(|s| s.streaming_quality = level);
    });
    let row = streaming_quality.downgrade();
    follow_settings(&streaming_quality, guard.clone(), move |s| {
        if let Some(row) = row.upgrade() {
            row.set_selected(quality::level_index(s.streaming_quality));
        }
    });

    // Startup.
    let startup = group(&tr("Startup"), None);
    let resume = adw::SwitchRow::builder()
        .title(tr("Resume Playback"))
        .subtitle(tr("Start playing where you left off when Jellyfesh opens."))
        .build();
    startup.add(&resume);
    p.append(&startup);

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
    let (g, chosen) = (guard.clone(), presets.clone());
    preset_row.connect_selected_notify(move |r| {
        if g.get() {
            return;
        }
        // Custom (after the presets) stands for the bands as they are.
        let Some(preset) = chosen.get(r.selected() as usize) else {
            return;
        };
        let bands = preset.settings.clone();
        app().update_settings(|s| s.eq = eq::apply_preset(&s.eq, &bands));
    });
    let g = guard.clone();
    eq_preamp.connect_value_notify(move |r| {
        if g.get() {
            return;
        }
        let db = r.value() as f32;
        app().update_settings(|s| s.eq = eq::set_preamp(&s.eq, db));
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
    let g = guard.clone();
    resume.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.resume_on_launch = on);
    });
    let weak = (
        eq_on.downgrade(),
        preset_row.downgrade(),
        eq_preamp.downgrade(),
        reset.downgrade(),
        rg_mode.downgrade(),
        rg_preamp.downgrade(),
        xf.downgrade(),
        xf_secs.downgrade(),
        xf_album.downgrade(),
        resume.downgrade(),
    );
    let sl = sliders.clone();
    follow_settings(&p, guard.clone(), move |s| {
        let (
            Some(eq_on),
            Some(preset_row),
            Some(eq_preamp),
            Some(reset),
            Some(rg_mode),
            Some(rg_preamp),
            Some(xf),
            Some(xf_secs),
            Some(xf_album),
            Some(resume),
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
        eq_on.set_active(s.eq.enabled);
        // Custom is listed only while the bands match no preset.
        let named = u32::try_from(presets.len()).unwrap_or(u32::MAX);
        let model = preset_row.model().and_downcast::<gtk::StringList>();
        match eq::detect(&presets, &s.eq) {
            Some(i) => {
                if let Some(m) = &model
                    && m.n_items() > named
                {
                    m.remove(named);
                }
                preset_row.set_selected(u32::try_from(i).unwrap_or(0));
            }
            None => {
                if let Some(m) = &model
                    && m.n_items() == named
                {
                    m.append(&tr("Custom"));
                }
                preset_row.set_selected(named);
            }
        }
        preset_row.set_sensitive(s.eq.enabled);
        eq_preamp.set_value(f64::from(s.eq.preamp_db));
        eq_preamp.set_sensitive(s.eq.enabled);
        for (scale, band) in sl.iter().zip(&s.eq.bands) {
            if (scale.value() - f64::from(band.gain_db)).abs() > 0.01 {
                scale.set_value(f64::from(band.gain_db));
            }
            scale.set_sensitive(s.eq.enabled);
        }
        reset.set_sensitive(s.eq.enabled && !eq::is_flat(&s.eq));
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
        resume.set_active(s.resume_on_launch);
    });

    // The Output Device choices follow the core's device list.
    let choices: Rc<RefCell<Vec<Option<String>>>> = Rc::new(RefCell::new(Vec::new()));
    let fill = {
        let device = device.downgrade();
        let choices = choices.clone();
        let guard = guard.clone();
        move || {
            let Some(device) = device.upgrade() else { return };
            let devices = app().devices.borrow().clone();
            let names: Vec<String> = devices.devices.iter().map(|d| d.name.clone()).collect();
            let (list, at) = playback::output_choices(&names, devices.selected.as_deref());
            guard.set(true);
            if *choices.borrow() != list {
                let labels: Vec<String> = list
                    .iter()
                    .map(|c| c.clone().unwrap_or_else(|| tr("System Default")))
                    .collect();
                let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
                device.set_model(Some(&gtk::StringList::new(&labels)));
                *choices.borrow_mut() = list;
            }
            device.set_selected(at);
            guard.set(false);
        }
    };
    fill();
    let g = guard.clone();
    device.connect_selected_notify(move |row| {
        if g.get() {
            return;
        }
        let Some(name) = choices.borrow().get(row.selected() as usize).cloned() else {
            return;
        };
        if app().devices.borrow().selected != name {
            app().send(Command::SetOutputDevice { name });
        }
    });
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

/// Library (docs/ui.md, "Settings"): Music Folders (each with Show in Files
/// and Remove Folder…, then Add Folder…), Watch Folders, and Rescan Library
/// with the scan's progress and Last Scan.
pub fn library_page() -> gtk::Box {
    let p = section("library");
    let guard = Rc::new(Cell::new(false));
    let folders = group(
        &tr("Music Folders"),
        Some(&tr("Jellyfesh reads every audio file in these folders.")),
    );
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    folders.add(&list);
    p.append(&folders);

    let watch_group = adw::PreferencesGroup::new();
    let watch = adw::SwitchRow::builder()
        .title(tr("Watch Folders"))
        .subtitle(tr("Update the library when files change."))
        .build();
    watch_group.add(&watch);
    p.append(&watch_group);

    // The sentence beside the Rescan Library button, as in every app.
    let scan = adw::PreferencesGroup::new();
    let rescan_row = adw::ActionRow::builder()
        .title(tr("Look for new, changed and removed files."))
        .build();
    let rescan = gtk::Button::with_label(&tr("Rescan Library"));
    rescan.set_valign(gtk::Align::Center);
    rescan.connect_clicked(|_| {
        app().send(Command::Rescan);
    });
    rescan_row.add_suffix(&rescan);
    rescan_row.set_activatable_widget(Some(&rescan));
    let progress = gtk::ProgressBar::new();
    progress.add_css_class("scan-progress");
    progress.set_show_text(true);
    let progress_row = adw::PreferencesRow::builder()
        .activatable(false)
        .child(&progress)
        .build();
    let last = adw::ActionRow::builder()
        .title(tr("Last Scan"))
        .subtitle_selectable(true)
        .build();
    last.add_css_class("property");
    scan.add(&rescan_row);
    scan.add(&progress_row);
    scan.add(&last);
    p.append(&scan);

    let g = guard.clone();
    watch.connect_active_notify(move |r| {
        if g.get() {
            return;
        }
        let on = r.is_active();
        app().update_settings(|s| s.watch_folders = on);
    });
    let w = watch.downgrade();
    follow_settings(&p, guard, move |s| {
        if let Some(w) = w.upgrade() {
            w.set_active(s.watch_folders);
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
                    let row = adw::ActionRow::builder().title(tr("No Folders Yet")).build();
                    row.add_css_class("dim-label");
                    list.append(&row);
                }
                for root in roots {
                    list.append(&folder_row(root));
                }
                let add = adw::ActionRow::builder()
                    .title(tr("Add Folder…"))
                    .activatable(true)
                    .build();
                add.add_prefix(&glyph::Glyph::new("plus", 20));
                add.connect_activated(|_| window::choose_folder());
                list.append(&add);
            });
        }
    };
    refresh_roots();

    // The scan: progress while it runs, then how it ended.
    let show_scan = {
        let (pr, pb, rb, last) = (
            progress_row.downgrade(),
            progress.downgrade(),
            rescan.downgrade(),
            last.downgrade(),
        );
        move |progress: Option<&jf_core::ScanProgress>| {
            let (Some(pr), Some(pb), Some(rb), Some(last)) =
                (pr.upgrade(), pb.upgrade(), rb.upgrade(), last.upgrade())
            else {
                return;
            };
            let a = app();
            let scanning = a.scanning.get();
            pr.set_visible(scanning);
            rb.set_sensitive(!scanning);
            if scanning {
                pb.pulse();
                let checked = progress
                    .map(|p| trn("{n} file checked", "{n} files checked", p.files_seen))
                    .unwrap_or_default();
                pb.set_text(Some(&checked));
            }
            let ended = a.last_scan.borrow().clone();
            last.set_visible(ended.is_some());
            last.remove_css_class("error");
            match ended {
                Some(ScanStatus::Finished(report)) => last.set_subtitle(&trf(
                    "{added} added, {updated} updated, {removed} removed",
                    &[
                        ("added", &format::count(report.added)),
                        ("updated", &format::count(report.updated)),
                        ("removed", &format::count(report.removed)),
                    ],
                )),
                Some(ScanStatus::Failed(e)) => {
                    last.set_subtitle(&glib::markup_escape_text(&e));
                    last.add_css_class("error");
                }
                _ => {}
            }
        }
    };
    show_scan(None);
    app().listen_for(&p, move |c| match c {
        Change::Library => refresh_roots(),
        Change::Scan(ScanStatus::Progress(progress)) => show_scan(Some(progress)),
        Change::Scan(_) => show_scan(None),
        _ => {}
    });
    p
}

/// A music folder: Show in Files and Remove Folder…, which asks first.
fn folder_row(root: std::path::PathBuf) -> adw::ActionRow {
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    let row = adw::ActionRow::builder()
        .title(glib::markup_escape_text(&name).as_str())
        .subtitle(glib::markup_escape_text(&root.display().to_string()).as_str())
        .build();
    row.add_prefix(&glyph::Glyph::new("folder", 20));
    let show = gtk::Button::from_icon_name("folder-open-symbolic");
    show.add_css_class("flat");
    show.set_valign(gtk::Align::Center);
    show.set_tooltip_text(Some(&tr("Show in Files")));
    show.update_property(&[gtk::accessible::Property::Label(&tr("Show in Files"))]);
    let path = root.clone();
    show.connect_clicked(move |_| actions::show_in_files(&path));
    row.add_suffix(&show);
    let remove = glyph::button("trash", 18, &tr("Remove Folder…"));
    remove.set_valign(gtk::Align::Center);
    remove.connect_clicked(move |b| {
        let (b, root, name) = (b.clone(), root.clone(), name.clone());
        spawn(async move {
            let dialog = adw::AlertDialog::new(
                Some(&trf("Remove “{name}” from the library?", &[("name", &name)])),
                Some(&tr(
                    "Its songs leave the library and its playlists. The files stay where they are.",
                )),
            );
            dialog.add_response("cancel", &tr("Cancel"));
            dialog.add_response("remove", &tr("Remove Folder"));
            dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
            dialog.set_default_response(Some("cancel"));
            dialog.set_close_response("cancel");
            if dialog.choose_future(Some(&b)).await == "remove" {
                app().send(Command::RemoveRoot { path: root });
            }
        });
    });
    row.add_suffix(&remove);
    row
}
