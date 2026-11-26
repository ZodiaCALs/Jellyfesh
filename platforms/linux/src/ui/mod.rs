//! The GTK 4 and libadwaita app.

mod about;
mod actions;
mod ailyrics;
mod bridge;
mod clock;
mod convert;
mod desktoplyrics;
mod details;
mod devices;
mod dnd;
mod extensions;
mod fields;
pub mod i18n;
mod keys;
mod lyricsbar;
mod miniplayer;
mod models;
mod mpris;
mod network;
mod nowplaying;
mod outputs;
mod pages;
mod playerbar;
mod prefs;
mod pro;
mod quality;
mod rows;
mod sources_ui;
mod state;
mod style;
mod thumbs;
mod transport;
mod widgets;
mod window;

use std::path::PathBuf;

use adw::prelude::*;
use gtk::{gio, glib};
use jf_core::{Command, Config, OutputKind, TrackRef};

use crate::ui::bridge::Bridge;
use crate::ui::state::{App, app, try_app};
use crate::ui::thumbs::Thumbs;

/// The application id (provisional).
pub const APP_ID: &str = "org.jellyfesh.Jellyfesh";

pub fn run() -> glib::ExitCode {
    init_logging();
    i18n::init();
    if let Err(e) = gio::resources_register_include!("jellyfesh.gresource") {
        eprintln!("cannot load the app's resources: {e}");
        return glib::ExitCode::FAILURE;
    }
    glib::set_application_name("Jellyfesh");
    glib::set_prgname(Some(APP_ID));
    let application = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .resource_base_path("/org/jellyfesh/Jellyfesh")
        .build();
    application.connect_startup(startup);
    application.connect_activate(present);
    application.connect_open(|a, files, _| {
        present(a);
        let tracks: Vec<TrackRef> = files
            .iter()
            .filter_map(|f| f.path())
            .map(TrackRef::File)
            .collect();
        if !tracks.is_empty() {
            app().send(Command::PlayTracks { tracks, start: 0 });
        }
    });
    application.connect_shutdown(|_| {
        if let Some(bridge) = App::uninstall() {
            bridge.shutdown();
        }
    });
    application.run()
}

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_env("JELLYFESH_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn config() -> Option<Config> {
    let data = jf_core::paths::default_data_dir()?;
    let cache = jf_core::paths::default_cache_dir()?;
    let mut config = Config::new(data, cache);
    // For tests and machines without sound: a paced null output.
    if std::env::var_os("JELLYFESH_NULL_OUTPUT").is_some_and(|v| !v.is_empty() && v != "0") {
        config.output = OutputKind::Null { realtime: true };
    }
    config.bundled_plugins = bundled_plugins();
    // How this build reached the person (the release workflow's flag):
    // which stores sell Pro and whether the core checks for updates.
    config.distribution = crate::logic::build::distribution(option_env!("JF_DISTRIBUTION"));
    // AI Lyrics on this device: whisper.cpp, with its models in
    // <data>/speech-models (the core's default store).
    config.speech = jf_speech::recognizer();
    // Screenshot runs of debug builds can turn on Pro Preview, so that
    // Customize's look shows (JELLYFESH_PRO_PREVIEW=1); release builds take
    // Preview only from their build (JF_PRO_PREVIEW).
    #[cfg(debug_assertions)]
    if std::env::var_os("JELLYFESH_PRO_PREVIEW").is_some_and(|v| v == "1") {
        config.pro.preview = true;
    }
    Some(config)
}

/// The official plugins installed next to the app (`<prefix>/share/jellyfesh/plugins`,
/// `/app/...` in the Flatpak), which the core installs on first start and updates
/// with the app. `JELLYFESH_PLUGINS_DIR` points elsewhere (development).
fn bundled_plugins() -> Vec<PathBuf> {
    let dir = std::env::var_os("JELLYFESH_PLUGINS_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            let exe = std::env::current_exe().ok()?;
            Some(exe.parent()?.parent()?.join("share/jellyfesh/plugins"))
        });
    let Some(dir) = dir else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "jfpkg"))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

fn startup(application: &adw::Application) {
    style::load();
    let Some(config) = config() else {
        eprintln!("no data folder: set HOME or JF_DATA_DIR");
        application.quit();
        return;
    };
    let cache_dir: PathBuf = config.cache_dir.clone();
    let (bridge, events) = match Bridge::start(config) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cannot start the Jellyfesh core: {e}");
            application.quit();
            return;
        }
    };
    let thumbs = match Thumbs::start(bridge.core().clone(), cache_dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot start the artwork loader: {e}");
            application.quit();
            return;
        }
    };
    let snapshot = bridge.core().snapshot();
    let state = App::install(bridge, thumbs, snapshot);
    // Events from the core, applied in bursts on the main loop.
    glib::spawn_future_local(async move {
        while let Ok(first) = events.recv().await {
            let mut batch = vec![first];
            while let Ok(more) = events.try_recv() {
                batch.push(more);
            }
            match try_app() {
                Some(a) => a.apply_events(batch),
                None => break,
            }
        }
    });
    style::apply_appearance();
    style::apply_look();
    style::apply_glass();
    style::apply_theme(false);
    install_actions(application);
    // Before the window: Now Playing's More then sees Desktop Lyrics open
    // or close before it rebuilds on a settings change.
    desktoplyrics::follow();
    window::build(application, &state);
    window::listen();
    mpris::start();
    devices::watch();
    // The network, for Auto quality and downloads (ADR-0026 §1).
    network::follow();
    // Resume Playback is the core's: it plays the restored queue at its
    // saved position when it starts (docs/core-api.md, "Settings").
}

fn present(application: &adw::Application) {
    if let Some(s) = window::shell() {
        s.window.present();
    } else if let Some(w) = application.active_window() {
        w.present();
    }
}

fn simple(application: &adw::Application, name: &str, accels: &[&str], f: impl Fn() + 'static) {
    let action = gio::SimpleAction::new(name, None);
    action.connect_activate(move |_, _| f());
    application.add_action(&action);
    if !accels.is_empty() {
        application.set_accels_for_action(&format!("app.{name}"), accels);
    }
}

fn install_actions(application: &adw::Application) {
    use crate::logic::seek::SEEK_STEP_MS;
    simple(application, "play-pause", &["AudioPlay", "AudioPause"], || {
        app().send(Command::TogglePlayPause);
    });
    simple(application, "next", &["<Primary>Right", "AudioNext"], || {
        app().send(Command::Next);
    });
    simple(application, "previous", &["<Primary>Left", "AudioPrev"], || {
        app().send(Command::Previous);
    });
    simple(application, "stop", &["AudioStop"], || {
        app().send(Command::Stop);
    });
    let seek_by = |delta: i64| {
        let a = app();
        let Some(duration) = a.duration_us() else { return };
        let pos = crate::logic::format::us_to_ms(a.position_us());
        let ms = crate::logic::seek::step(pos, delta, crate::logic::format::us_to_ms(duration));
        a.send(Command::Seek { position_ms: ms });
    };
    simple(application, "seek-forward", &["<Shift>Right"], move || {
        seek_by(SEEK_STEP_MS as i64)
    });
    simple(application, "seek-back", &["<Shift>Left"], move || {
        seek_by(-(SEEK_STEP_MS as i64))
    });
    // While a remote session plays, the keys set its device's volume.
    let volume_by = |delta: f32| {
        let a = app();
        if a.is_remote() {
            if let Some(percent) = a.remote_view().and_then(|v| v.volume) {
                let step = (delta * 100.0).round() as i32;
                let percent = crate::logic::remote::volume_step(percent, step);
                a.send(Command::Remote {
                    action: jf_core::RemoteAction::SetVolume { percent },
                });
            }
            return;
        }
        let v = (a.playback.borrow().volume + delta).clamp(0.0, 1.0);
        a.send(Command::SetVolume { volume: v });
    };
    simple(
        application,
        "volume-up",
        &["<Primary>Up", "AudioRaiseVolume"],
        move || volume_by(0.05),
    );
    simple(
        application,
        "volume-down",
        &["<Primary>Down", "AudioLowerVolume"],
        move || volume_by(-0.05),
    );
    simple(application, "mute", &["<Primary>m", "AudioMute"], || {
        let a = app();
        let muted = a.muted.get();
        a.send(Command::SetMuted { muted: !muted });
    });
    simple(application, "shuffle", &["<Primary>s"], || {
        let a = app();
        let on = a.shown_shuffle();
        a.send(Command::SetShuffle { on: !on });
    });
    simple(application, "repeat", &["<Primary>r"], || {
        let a = app();
        let mode = crate::logic::remote::next_repeat(a.shown_repeat());
        a.send(Command::SetRepeat { mode });
    });
    simple(application, "now-playing", &["<Primary>p"], || {
        if window::now_playing_open() {
            window::close_now_playing();
        } else {
            window::open_now_playing(None);
        }
    });
    simple(application, "lyrics", &["<Primary>l"], || {
        toggle_tab(nowplaying::Tab::Lyrics);
    });
    simple(application, "queue", &["<Primary>u"], || {
        toggle_tab(nowplaying::Tab::Queue);
    });
    // Desktop Lyrics, as the setting (a check item in Now Playing's More),
    // and its lock.
    let desktop = gio::SimpleAction::new_stateful(
        "desktop-lyrics",
        None,
        &app().settings.borrow().desktop_lyrics.to_variant(),
    );
    desktop.connect_change_state(|action, value| {
        if let Some(on) = value.and_then(|v| v.get::<bool>()) {
            action.set_state(&on.to_variant());
            app().update_settings(|s| s.desktop_lyrics = on);
        }
    });
    application.add_action(&desktop);
    let follow = desktop.downgrade();
    app().listen(move |change| {
        if let (crate::ui::state::Change::Settings, Some(action)) = (change, follow.upgrade()) {
            let on = app().settings.borrow().desktop_lyrics;
            if action.state().and_then(|s| s.get::<bool>()) != Some(on) {
                action.set_state(&on.to_variant());
            }
        }
    });
    let lock = gio::SimpleAction::new_stateful("lock-desktop-lyrics", None, &false.to_variant());
    lock.connect_change_state(|action, value| {
        if let Some(on) = value.and_then(|v| v.get::<bool>()) {
            action.set_state(&on.to_variant());
            desktoplyrics::set_locked(on);
        }
    });
    application.add_action(&lock);
    simple(application, "search", &["<Primary>f"], || {
        window::close_now_playing();
        window::show_section(window::Section::Search);
    });
    // Settings is a page of the window (ADR-0023); the keyboard shortcuts
    // and About Jellyfesh are in its About section.
    simple(application, "settings", &["<Primary>comma"], || {
        window::show_settings(None)
    });
    simple(application, "shortcuts", &["<Primary>question"], || {
        window::show_settings(Some("shortcuts"))
    });
    simple(application, "about", &[], || window::show_settings(Some("about")));
    let quit_app = application.downgrade();
    simple(application, "quit", &["<Primary>q"], move || {
        if let Some(a) = quit_app.upgrade() {
            a.quit();
        }
    });
    application.set_accels_for_action("window.close", &["<Primary>w"]);

    // Scriptable entry points (D-Bus: `gapplication action org.jellyfesh.Jellyfesh
    // show "'album:3'"`), used by desktop integration and the screenshot run.
    let show = gio::SimpleAction::new("show", Some(glib::VariantTy::STRING));
    show.connect_activate(|_, p| {
        if let Some(spec) = p.and_then(|p| p.get::<String>()) {
            show_view(&spec);
        }
    });
    application.add_action(&show);
    let command = gio::SimpleAction::new("command", Some(glib::VariantTy::STRING));
    command.connect_activate(|_, p| {
        if let Some(spec) = p.and_then(|p| p.get::<String>()) {
            run_command(&spec);
        }
    });
    application.add_action(&command);
}

/// `section`, `album:<id>`, `artist:<id>`, `playlist:<id>`,
/// `now-playing[:lyrics|queue|info]`, `settings[:<section>]` (also
/// `preferences[:<section>]`, its earlier name), `search:<words>`, `about`,
/// `shortcuts` (Settings › About), `library` (closes Now Playing).
fn show_view(spec: &str) {
    let (kind, arg) = spec.split_once(':').unwrap_or((spec, ""));
    let id = arg.parse::<i64>().ok();
    match (kind, id) {
        ("album", Some(id)) => window::show_album(id, None),
        ("artist", Some(id)) => window::show_artist(id, None),
        ("playlist", Some(id)) => window::show_playlist(id, None),
        ("now-playing", _) => window::open_now_playing(match arg {
            "queue" => Some(nowplaying::Tab::Queue),
            "info" => Some(nowplaying::Tab::Info),
            "lyrics" => Some(nowplaying::Tab::Lyrics),
            _ => None,
        }),
        ("library", _) => window::close_now_playing(),
        ("settings" | "preferences", _) => window::show_settings((!arg.is_empty()).then_some(arg)),
        ("search", _) => {
            window::close_now_playing();
            window::show_section(window::Section::Search);
            pages::search::search_for(arg);
        }
        ("about" | "shortcuts", _) => window::show_settings(Some(kind)),
        _ => {
            if let Some(section) = window::Section::from_tag(kind) {
                window::close_now_playing();
                window::show_section(section);
            }
        }
    }
}

/// `play-album:<id>`, `play-playlist:<id>`, `seek:<ms>`, `play`, `pause`,
/// `appearance:system|light|dark`, `style:artwork|misty`,
/// `hue:artwork|<degrees>`, `transparency:<0 to 1>`, `reduce-effects:on|off`,
/// `lyrics-size:small|medium|large`, `lyrics-align:start|center`.
fn run_command(spec: &str) {
    let (kind, arg) = spec.split_once(':').unwrap_or((spec, ""));
    let a = app();
    match kind {
        "play-album" => {
            if let Ok(id) = arg.parse() {
                a.send(Command::PlayCollection {
                    collection: jf_core::Collection::Album { id },
                    start: 0,
                    shuffle: false,
                });
            }
        }
        "play-playlist" => {
            if let Ok(id) = arg.parse() {
                a.send(Command::PlayCollection {
                    collection: jf_core::Collection::Playlist { id },
                    start: 0,
                    shuffle: false,
                });
            }
        }
        "seek" => {
            if let Ok(position_ms) = arg.parse() {
                a.send(Command::Seek { position_ms });
            }
        }
        "play" => {
            a.send(Command::Play);
        }
        "pause" => {
            a.send(Command::Pause);
        }
        "appearance" => a.update_settings(|s| {
            s.appearance = match arg {
                "light" => jf_core::AppearancePref::Light,
                "dark" => jf_core::AppearancePref::Dark,
                _ => jf_core::AppearancePref::System,
            }
        }),
        "style" => a.update_settings(|s| {
            s.style = if arg == "misty" {
                jf_core::BackdropStyle::Misty
            } else {
                jf_core::BackdropStyle::Artwork
            }
        }),
        "hue" => a.update_settings(|s| {
            s.hue_source = match arg.parse::<f64>() {
                Ok(h) => jf_core::HueSource::Fixed {
                    hue: crate::logic::appearance::wrap_hue(h),
                },
                Err(_) => jf_core::HueSource::Artwork,
            }
        }),
        "transparency" => {
            if let Ok(t) = arg.parse::<f32>() {
                a.update_settings(|s| s.transparency = t);
            }
        }
        "reduce-effects" => a.update_settings(|s| s.reduce_effects = arg == "on"),
        "lyrics-size" => a.update_settings(|s| {
            s.lyrics_size = match arg {
                "small" => jf_core::LyricsSize::Small,
                "large" => jf_core::LyricsSize::Large,
                _ => jf_core::LyricsSize::Medium,
            }
        }),
        "lyrics-align" => a.update_settings(|s| {
            s.lyrics_align = if arg == "center" {
                jf_core::LyricsAlign::Center
            } else {
                jf_core::LyricsAlign::Start
            }
        }),
        _ if run_debug_command(kind, arg) => {}
        _ => tracing::warn!("unknown command {spec}"),
    }
}

/// Screenshot runs of debug builds install and set up plugins through the
/// core as Settings would (`install-plugin:<path>`, `allow-community:on`,
/// `acknowledge:<id>`, `enable-plugin:<id>`, `plugin-settings:<id>=<json>`);
/// release builds have none of these.
#[cfg(debug_assertions)]
fn run_debug_command(kind: &str, arg: &str) -> bool {
    let a = app();
    match kind {
        "install-plugin" => {
            a.send(Command::InstallPlugin { path: arg.into() });
        }
        "allow-community" => a.update_settings(|s| s.allow_community_plugins = arg == "on"),
        "acknowledge" => {
            a.send(Command::AcknowledgeUnofficial { id: arg.to_owned() });
        }
        "enable-plugin" => {
            a.send(Command::SetPluginEnabled {
                id: arg.to_owned(),
                enabled: true,
            });
        }
        "plugin-settings" => {
            let Some((id, json)) = arg.split_once('=') else {
                return false;
            };
            a.send(Command::SetPluginSettings {
                id: id.to_owned(),
                settings_json: json.to_owned(),
            });
        }
        _ => return false,
    }
    true
}

#[cfg(not(debug_assertions))]
fn run_debug_command(_kind: &str, _arg: &str) -> bool {
    false
}

/// Opens Now Playing on `tab`, or closes it when it already shows `tab`.
fn toggle_tab(tab: nowplaying::Tab) {
    let open = window::now_playing_open();
    let current = window::shell().and_then(|s| s.now_playing.current_tab());
    if open && current == Some(tab) {
        window::close_now_playing();
    } else {
        window::open_now_playing(Some(tab));
    }
}
