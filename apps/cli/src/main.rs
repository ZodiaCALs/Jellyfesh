//! `jf`: the Jellyfesh command-line player.
//!
//! A thin front end over `jf-core`, like the native shells will be: it
//! parses arguments, sends commands and prints what the core reports.

#[macro_use]
mod util;
mod lists;
mod play;
mod sources;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use crossbeam_channel::RecvTimeoutError;
use jf_core::{
    Appearance, Command, Config, Core, EqSettings, Event, OutputKind, RepeatMode, ReplayGainMode, ScanStatus,
    TrackQuery, TrackRef,
};

use crate::util::{bold, dim, fmt_ms, fmt_stamp, hex, parse_time, swatch};

#[derive(Parser, Debug)]
#[command(
    name = "jf",
    version,
    about = "Jellyfesh: an elegant, lightweight music player (command-line edition)"
)]
struct Cli {
    /// Folder for the library and session databases [default: the platform
    /// data folder, or JF_DATA_DIR]
    #[arg(long, global = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,
    /// Folder for artwork and backdrops [default: the platform cache folder,
    /// or JF_CACHE_DIR]
    #[arg(long, global = true, value_name = "DIR")]
    cache_dir: Option<PathBuf>,
    /// More log output (-v, -vv)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Add folders to the library and scan them; with none, scan every
    /// library folder
    Scan {
        /// Folders to add
        paths: Vec<PathBuf>,
    },
    /// List or remove library folders
    Roots {
        /// Remove this folder and its tracks from the library
        #[arg(long, value_name = "DIR")]
        remove: Option<PathBuf>,
    },
    /// List tracks, albums or artists in the library
    Ls(LsArgs),
    /// Search the library and every online source that is on
    Search {
        /// Words to search for
        #[arg(required = true)]
        query: Vec<String>,
        /// Search only this source (plugin id or name)
        #[arg(long, conflicts_with = "library")]
        source: Option<String>,
        /// Search only the library
        #[arg(long)]
        library: bool,
        /// Most results shown of each kind from each source
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Install, list, set up and sign in to online sources (plugins)
    Plugin {
        #[command(subcommand)]
        cmd: Option<sources::PluginCmd>,
    },
    /// Play files, folders or a library search; with no arguments, resume
    /// the saved queue
    Play(PlayArgs),
    /// Show the saved queue
    Queue {
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// Show lyrics from a lyrics file, or find the lyrics of an audio file
    Lyrics(LyricsArgs),
    /// Theme colours and Now Playing backdrop from an image
    Theme(ThemeArgs),
    /// List audio output devices
    Devices,
    /// List, show, create and edit playlists
    Playlist {
        #[command(subcommand)]
        cmd: Option<lists::PlaylistCmd>,
    },
    /// List favourites, or mark files, folders or a library search as favourites
    Fav {
        targets: Vec<String>,
        /// Unmark instead
        #[arg(long, requires = "targets")]
        remove: bool,
    },
    /// Recently played tracks
    Played {
        /// Most played instead
        #[arg(long)]
        top: bool,
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
}

#[derive(Args, Debug)]
struct LsArgs {
    /// List albums
    #[arg(long, conflicts_with = "artists")]
    albums: bool,
    /// List artists
    #[arg(long)]
    artists: bool,
    #[arg(long, default_value_t = 50)]
    limit: usize,
    #[arg(long, default_value_t = 0)]
    offset: usize,
    /// Print JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct PlayArgs {
    /// Files, folders, or words to search the library (or --source) for
    targets: Vec<String>,
    /// Search this online source (plugin id or name) for the words: an
    /// album, playlist or artist named exactly plays whole, otherwise the
    /// matching tracks
    #[arg(long, value_name = "PLUGIN", requires = "targets", conflicts_with_all = ["playlist", "favorites"])]
    source: Option<String>,
    /// Play a playlist (name or id)
    #[arg(long, conflicts_with_all = ["targets", "favorites"])]
    playlist: Option<String>,
    /// Play the favourites
    #[arg(long, conflicts_with = "targets")]
    favorites: bool,
    /// Write the audio to a WAV file instead of the audio device
    #[arg(long, value_name = "FILE.wav")]
    out: Option<PathBuf>,
    /// With --out, render in real time instead of as fast as possible
    #[arg(long, requires = "out")]
    realtime: bool,
    /// Output device name (see `jf devices`)
    #[arg(long, conflicts_with = "out")]
    device: Option<String>,
    #[arg(long)]
    shuffle: bool,
    #[arg(long, value_enum)]
    repeat: Option<Repeat>,
    /// Equalizer preset
    #[arg(long, value_enum)]
    eq: Option<EqPreset>,
    /// ReplayGain mode
    #[arg(long, value_enum)]
    rg: Option<Rg>,
    /// ReplayGain preamp in dB (-15 to 15)
    #[arg(long, allow_hyphen_values = true, requires = "rg")]
    preamp: Option<f32>,
    /// Volume from 0 to 100
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100))]
    volume: Option<u8>,
    /// Start at this position (83, 1:23 or 1:23.5)
    #[arg(long, value_parser = parse_time)]
    seek: Option<u64>,
    /// Print each lyric line as it is sung
    #[arg(long)]
    lyrics: bool,
    /// Only print errors
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Args, Debug)]
struct LyricsArgs {
    /// A lyrics file (.lrc, .ttml, .vtt, .txt) or an audio file
    file: PathBuf,
    /// Show the line (and word) active at this time
    #[arg(long, value_parser = parse_time, conflicts_with = "json")]
    at: Option<u64>,
    /// Print lines as they come: plays an audio file, or runs a clock for a
    /// lyrics file
    #[arg(long, conflicts_with_all = ["json", "at"])]
    follow: bool,
    /// Print the parsed document as JSON
    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct ThemeArgs {
    /// A JPEG or PNG image (album artwork)
    image: PathBuf,
    /// Show and bake the dark appearance instead of the light one
    #[arg(long)]
    dark: bool,
    /// Bake the Now Playing backdrop to this PNG file
    #[arg(long, value_name = "FILE.png")]
    out: Option<PathBuf>,
    /// Print JSON
    #[arg(long)]
    json: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Repeat {
    Off,
    All,
    One,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum EqPreset {
    Flat,
    BassBoost,
    Vocal,
    Treble,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Rg {
    Off,
    Track,
    Album,
}

fn main() {
    let cli = Cli::parse();
    let filter = match cli.verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("JF_LOG").unwrap_or_else(|_| filter.into()),
        )
        .with_writer(std::io::stderr)
        .init();
    if let Err(e) = run(cli) {
        eprintln!("jf: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        Cmd::Lyrics(args) if !is_audio(&args.file) || !args.follow => return lyrics(args),
        Cmd::Theme(args) => return theme(args),
        Cmd::Devices => return devices(),
        _ => {}
    }
    let mut config = config(&cli)?;
    // Commands that never reach a plugin leave the plugin host off.
    config.sources.enabled = matches!(
        cli.command,
        Cmd::Search { .. } | Cmd::Plugin { .. } | Cmd::Play(_)
    );
    match cli.command {
        Cmd::Scan { paths } => scan(&Core::start(config)?, &paths),
        Cmd::Roots { remove } => roots(&Core::start(config)?, remove),
        Cmd::Ls(args) => ls(&Core::start(config)?, &args),
        Cmd::Search {
            query,
            source,
            library,
            limit,
            json,
        } => {
            let core = Core::start(config)?;
            sources::search(&core, &query.join(" "), source.as_deref(), library, limit, json)?;
            core.shutdown();
            Ok(())
        }
        Cmd::Plugin { cmd } => sources::plugin(
            Core::start(config)?,
            cmd.unwrap_or(sources::PluginCmd::List { json: false }),
        ),
        Cmd::Play(args) => play(config, &args),
        Cmd::Queue { limit } => queue(&Core::start(config)?, limit),
        Cmd::Lyrics(args) => {
            // Following an audio file's lyrics plays it.
            let play_args = PlayArgs::follow_lyrics(&args.file);
            play(config, &play_args)
        }
        Cmd::Playlist { cmd } => {
            let core = Core::start(config)?;
            lists::playlist(&core, cmd.unwrap_or(lists::PlaylistCmd::List), |t| {
                resolve_targets(&core, t)
            })
        }
        Cmd::Fav { targets, remove } => {
            let core = Core::start(config)?;
            let tracks = if targets.is_empty() {
                None
            } else {
                Some(resolve_targets(&core, &targets)?)
            };
            lists::favorites(&core, tracks, remove)
        }
        Cmd::Played { top, limit } => lists::played(&Core::start(config)?, top, limit),
        Cmd::Theme(_) | Cmd::Devices => Ok(()),
    }
}

fn config(cli: &Cli) -> Result<Config> {
    let data_dir = cli
        .data_dir
        .clone()
        .or_else(jf_core::paths::default_data_dir)
        .ok_or_else(|| anyhow!("no data folder: pass --data-dir or set JF_DATA_DIR"))?;
    let cache_dir = cli
        .cache_dir
        .clone()
        .or_else(jf_core::paths::default_cache_dir)
        .unwrap_or_else(|| data_dir.join("cache"));
    let mut config = Config::new(data_dir, cache_dir);
    // Each run is short; scans are asked for explicitly.
    config.watch_folders = false;
    Ok(config)
}

// ---- library --------------------------------------------------------------

fn scan(core: &Core, paths: &[PathBuf]) -> Result<()> {
    let events = core.subscribe();
    let mut pending = Vec::new();
    for path in paths {
        if !path.is_dir() {
            bail!("{} is not a folder", path.display());
        }
        // Adding a folder scans it.
        pending.push(core.send(Command::AddRoot { path: path.clone() }));
    }
    if pending.is_empty() {
        if core.roots()?.is_empty() {
            bail!("the library has no folders yet: run `jf scan <folder>`");
        }
        pending.push(core.send(Command::Rescan));
    }
    let mut left = pending.len();
    let mut total = jf_core::ScanReport::default();
    let progress = std::io::stderr().is_terminal();
    loop {
        match events.recv_timeout(Duration::from_secs(600)) {
            Ok(Event::Scan(ScanStatus::Progress(p))) => {
                if progress {
                    eprint!(
                        "\r\x1b[2K  scanning: {} files, {} changed",
                        p.files_seen, p.files_changed
                    );
                }
            }
            Ok(Event::Scan(ScanStatus::Finished(r))) => {
                total.added += r.added;
                total.updated += r.updated;
                total.removed += r.removed;
                total.unchanged += r.unchanged;
                total.errors.extend(r.errors);
                left -= 1;
                if left > 0 {
                    continue;
                }
                let r = total;
                if progress {
                    eprint!("\r\x1b[2K");
                }
                outln!(
                    "added {}, updated {}, removed {}, unchanged {}, errors {}",
                    r.added,
                    r.updated,
                    r.removed,
                    r.unchanged,
                    r.errors.len()
                );
                for (path, error) in r.errors.iter().take(20) {
                    eprintln!("  ! {}: {error}", path.display());
                }
                let s = core.stats()?;
                outln!(
                    "{}",
                    dim(&format!(
                        "library: {} folder{}, {} tracks, {} albums, {} artists, {} total",
                        s.roots,
                        if s.roots == 1 { "" } else { "s" },
                        s.tracks,
                        s.albums,
                        s.artists,
                        fmt_ms(s.total_duration_ms)
                    ))
                );
                return Ok(());
            }
            Ok(Event::Scan(ScanStatus::Failed(e))) => bail!("scan failed: {e}"),
            Ok(Event::CommandFailed { id, error }) if pending.contains(&id) => bail!("{error}"),
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => bail!("the scan made no progress for 10 minutes"),
            Err(RecvTimeoutError::Disconnected) => bail!("the core stopped"),
        }
    }
}

fn roots(core: &Core, remove: Option<PathBuf>) -> Result<()> {
    if let Some(path) = remove {
        let events = core.subscribe();
        let id = core.send(Command::RemoveRoot { path });
        loop {
            match events.recv_timeout(Duration::from_secs(60)) {
                Ok(Event::LibraryChanged { .. }) => break,
                Ok(Event::CommandFailed { id: failed, error }) if failed == id => bail!("{error}"),
                Ok(_) => {}
                Err(_) => bail!("no answer from the library"),
            }
        }
    }
    for root in core.roots()? {
        outln!("{}", root.display());
    }
    Ok(())
}

fn ls(core: &Core, args: &LsArgs) -> Result<()> {
    if args.albums {
        let albums = core.albums(args.offset, args.limit)?;
        if args.json {
            return print_json(&albums);
        }
        for a in albums {
            let artist = a.album_artist.as_deref().unwrap_or("Unknown artist");
            outln!(
                "{}  {}  {}",
                bold(&a.title),
                dim(artist),
                dim(&format!("{} tracks", a.track_count))
            );
        }
    } else if args.artists {
        let artists = core.artists(args.offset, args.limit)?;
        if args.json {
            return print_json(&artists);
        }
        for a in artists {
            outln!("{}  {}", bold(&a.name), dim(&format!("{} tracks", a.track_count)));
        }
    } else {
        let query = TrackQuery {
            offset: args.offset,
            limit: args.limit,
            ..TrackQuery::default()
        };
        let tracks = core.tracks(&query)?;
        if args.json {
            return print_json(&tracks);
        }
        for t in tracks {
            print_track(&t);
        }
    }
    Ok(())
}

fn print_track(t: &jf_core::Track) {
    let artist = t.artist.as_deref().unwrap_or("Unknown artist");
    let album = t.album.as_deref().unwrap_or("");
    let duration = t.duration_ms.map(fmt_ms).unwrap_or_default();
    outln!(
        "{:>6}  {}  {}  {}  {}",
        t.id,
        bold(&t.title),
        artist,
        dim(album),
        dim(&duration)
    );
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    outln!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

// ---- playback -------------------------------------------------------------

impl PlayArgs {
    fn follow_lyrics(file: &Path) -> Self {
        PlayArgs {
            targets: vec![file.display().to_string()],
            source: None,
            playlist: None,
            favorites: false,
            out: None,
            realtime: false,
            device: None,
            shuffle: false,
            repeat: None,
            eq: None,
            rg: None,
            preamp: None,
            volume: None,
            seek: None,
            lyrics: true,
            quiet: false,
        }
    }
}

fn play(mut config: Config, args: &PlayArgs) -> Result<()> {
    config.output = match (&args.out, &args.device) {
        (Some(path), _) => OutputKind::WavFile {
            path: path.clone(),
            realtime: args.realtime,
        },
        (None, name) => OutputKind::Device { name: name.clone() },
    };
    let resume = args.targets.is_empty() && args.playlist.is_none() && !args.favorites;
    // A render to a file is a one-off; it leaves the saved session alone.
    // Playing a source still needs the saved settings (whether community
    // plugins are allowed); the restored queue is replaced right away.
    if args.out.is_some() {
        config.persist_session = false;
        config.restore_session = resume || args.source.is_some();
    }
    let core = Core::start(config)?;
    let events = core.subscribe();

    if let Some(v) = args.volume {
        core.send(Command::SetVolume {
            volume: f32::from(v) / 100.0,
        });
    }
    if let Some(preset) = args.eq {
        let eq = match preset {
            EqPreset::Flat => EqSettings::flat(),
            EqPreset::BassBoost => EqSettings::bass_boost(),
            EqPreset::Vocal => EqSettings::vocal(),
            EqPreset::Treble => EqSettings::treble(),
        };
        core.send(Command::SetEq { eq });
    }
    if let Some(rg) = args.rg {
        let mode = match rg {
            Rg::Off => ReplayGainMode::Off,
            Rg::Track => ReplayGainMode::Track,
            Rg::Album => ReplayGainMode::Album,
        };
        core.send(Command::SetReplayGain {
            mode,
            preamp_db: args.preamp.unwrap_or(0.0),
        });
    }
    if args.shuffle {
        core.send(Command::SetShuffle { on: true });
    }
    if let Some(r) = args.repeat {
        let mode = match r {
            Repeat::Off => RepeatMode::Off,
            Repeat::All => RepeatMode::All,
            Repeat::One => RepeatMode::One,
        };
        core.send(Command::SetRepeat { mode });
    }

    let collection = match &args.playlist {
        Some(name) => Some(jf_core::Collection::Playlist {
            id: lists::find(&core, name)?.id,
        }),
        None if args.favorites => Some(jf_core::Collection::Favorites),
        None => None,
    };
    let start = match (collection, &args.source) {
        (Some(collection), _) => core.send(Command::PlayCollection {
            collection,
            start: 0,
            shuffle: args.shuffle,
        }),
        (None, Some(plugin)) => match sources::source_play(&core, plugin, &args.targets.join(" "))? {
            sources::SourcePlay::Collection(collection) => core.send(Command::PlayCollection {
                collection,
                start: 0,
                shuffle: args.shuffle,
            }),
            sources::SourcePlay::Tracks(tracks) => {
                if !args.quiet && tracks.len() > 1 {
                    outln!("{}", dim(&format!("{} tracks", tracks.len())));
                }
                core.send(Command::PlayTracks { tracks, start: 0 })
            }
        },
        (None, None) => {
            let tracks = resolve_targets(&core, &args.targets)?;
            if !args.quiet && tracks.len() > 1 {
                outln!("{}", dim(&format!("{} tracks", tracks.len())));
            }
            play::start(&core, tracks, resume)?
        }
    };
    if let Some(ms) = args.seek {
        core.send(Command::Seek { position_ms: ms });
    }
    if let Err(e) = play::run(
        &events,
        start,
        &play::View {
            lyrics: args.lyrics,
            quiet: args.quiet,
        },
    ) {
        if args.out.is_none() && e.to_string().starts_with("audio") {
            bail!("{e}\n  no usable audio output: see `jf devices`, or render to a file with --out file.wav");
        }
        return Err(e);
    }
    core.shutdown();
    if let Some(out) = &args.out
        && !args.quiet
    {
        outln!("{}", dim(&format!("wrote {}", out.display())));
    }
    Ok(())
}

/// Files and folders play directly; other words search the library.
fn resolve_targets(core: &Core, targets: &[String]) -> Result<Vec<TrackRef>> {
    let (paths, words): (Vec<&String>, Vec<&String>) = targets.iter().partition(|t| play::is_path(t));
    let mut tracks = jf_core::targets::expand_paths(&paths.iter().map(PathBuf::from).collect::<Vec<_>>());
    if !words.is_empty() {
        let query = words.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
        let found = core.tracks_for_query(&query, 500)?;
        if found.is_empty() {
            bail!("nothing in the library matches {query:?} (and no such file)");
        }
        tracks.extend(found);
    }
    Ok(tracks)
}

fn queue(core: &Core, limit: usize) -> Result<()> {
    let snap = core.snapshot().queue;
    if snap.len == 0 {
        outln!("{}", dim("the queue is empty"));
        return Ok(());
    }
    outln!(
        "{}",
        dim(&format!(
            "{} entries · shuffle {} · repeat {:?}",
            snap.len,
            if snap.shuffle { "on" } else { "off" },
            snap.repeat
        ))
    );
    let start = snap.position.unwrap_or(0).saturating_sub(2);
    for (i, item) in core.queue_window(start, limit).into_iter().enumerate() {
        let index = start + i;
        let label = match &item.track {
            TrackRef::Library(id) => core
                .track(*id)?
                .map(|t| format!("{} — {}", t.title, t.artist.unwrap_or_default()))
                .unwrap_or_else(|| format!("(missing library track {id})")),
            TrackRef::File(p) => p.display().to_string(),
            TrackRef::Source(t) => format!(
                "{} — {}  {}",
                t.title,
                t.artist.clone().unwrap_or_default(),
                dim(&format!("({})", t.plugin))
            ),
        };
        let marker = if Some(index) == snap.position { "▶" } else { " " };
        outln!("{marker} {:>4}  {label}", index + 1);
    }
    Ok(())
}

// ---- lyrics, theme, devices -------------------------------------------

fn is_audio(path: &Path) -> bool {
    jf_core::lyrics::format_for_path(path).is_none() && path.is_file()
}

fn lyrics(args: &LyricsArgs) -> Result<()> {
    let (doc, source) = if jf_core::lyrics::format_for_path(&args.file).is_some() {
        (
            jf_core::lyrics::load_lyrics_file(&args.file)?,
            args.file.display().to_string(),
        )
    } else {
        let (doc, source) = jf_core::lyrics::find_lyrics_for_file(&args.file)
            .ok_or_else(|| anyhow!("no lyrics found for {}", args.file.display()))?;
        let label = match source {
            jf_core::LyricsSource::Sidecar(path) => path.display().to_string(),
            jf_core::LyricsSource::Embedded => "embedded in the file".to_string(),
            jf_core::LyricsSource::Plugin(plugin) => format!("from {plugin}"),
            jf_core::LyricsSource::Made { plugin, engine } => format!("made by {plugin} ({engine})"),
        };
        (doc, label)
    };
    if args.json {
        return print_json(&doc);
    }
    if let Some(at) = args.at {
        let timeline = doc.timeline();
        let cursor = timeline.cursor(u32::try_from(at).unwrap_or(u32::MAX));
        let Some(line) = cursor.line.and_then(|i| doc.lines.get(i)) else {
            outln!("{}", dim("(no line at that time)"));
            return Ok(());
        };
        let text = if cursor.word.is_some_and(|w| w < line.words.len()) {
            let mut out = String::new();
            for (i, w) in line.words.iter().enumerate() {
                if Some(i) == cursor.word {
                    out.push_str(&bold(&w.text));
                } else {
                    out.push_str(&w.text);
                }
                if !w.joins_next {
                    out.push(' ');
                }
            }
            out.trim_end().to_string()
        } else {
            line.text.clone()
        };
        outln!(
            "{}  {text}",
            dim(&line.start_ms.map(fmt_stamp).unwrap_or_default())
        );
        return Ok(());
    }
    if args.follow {
        return follow_lyrics_file(&doc);
    }
    outln!(
        "{}",
        dim(&format!(
            "{} · {:?} timing · {} lines",
            source,
            doc.timing,
            doc.lines.len()
        ))
    );
    for line in &doc.lines {
        let stamp = line.start_ms.map(fmt_stamp).unwrap_or_else(|| "        ".into());
        let agent = line
            .agent
            .as_deref()
            .map(|a| format!("{a}: "))
            .unwrap_or_default();
        outln!("{}  {}{}", dim(&stamp), dim(&agent), line.text);
        for track in &doc.tracks {
            if let Some(aux) = track.lines.get(&line.id) {
                outln!("{}  {}", " ".repeat(stamp.chars().count()), dim(&aux.text));
            }
        }
    }
    Ok(())
}

/// Prints each line of a lyrics file at its time, against the clock.
fn follow_lyrics_file(doc: &jf_core::LyricDoc) -> Result<()> {
    let start = std::time::Instant::now();
    for line in &doc.lines {
        let Some(at) = line.start_ms else {
            outln!("{}", line.text);
            continue;
        };
        let due = Duration::from_millis(u64::from(at));
        if let Some(wait) = due.checked_sub(start.elapsed()) {
            std::thread::sleep(wait);
        }
        outln!("{}  {}", dim(&fmt_stamp(at)), bold(&line.text));
    }
    Ok(())
}

fn theme(args: &ThemeArgs) -> Result<()> {
    let bytes = std::fs::read(&args.image).with_context(|| format!("reading {}", args.image.display()))?;
    let t = jf_core::theme::theme_for_artwork(Some(&bytes));
    let appearance = if args.dark {
        Appearance::Dark
    } else {
        Appearance::Light
    };
    if let Some(out) = &args.out {
        let png = jf_core::theme::backdrop_png(&bytes, appearance)?;
        std::fs::write(out, png).with_context(|| format!("writing {}", out.display()))?;
    }
    let theme = if args.dark { &t.dark } else { &t.light };
    if args.json {
        let colors: std::collections::BTreeMap<&str, String> =
            theme.colors.iter().map(|(k, v)| (k.as_str(), hex(*v))).collect();
        return print_json(&serde_json::json!({
            "hue": t.pick.hue,
            "chroma": t.pick.chroma,
            "from_artwork": t.pick.from_artwork,
            "appearance": if args.dark { "dark" } else { "light" },
            "colors": colors,
        }));
    }
    outln!(
        "hue {:.1}° · chroma {:.3} · {}",
        t.pick.hue,
        t.pick.chroma,
        if t.pick.from_artwork {
            "from the artwork"
        } else {
            "fallback hue (artwork too grey)"
        }
    );
    for (name, rgba) in &theme.colors {
        outln!("{} {:<11} {name}", swatch(*rgba), hex(*rgba));
    }
    if let Some(out) = &args.out {
        outln!("{}", dim(&format!("backdrop written to {}", out.display())));
    }
    Ok(())
}

fn devices() -> Result<()> {
    let devices = jf_core::output_devices()?;
    if devices.is_empty() {
        outln!("{}", dim("no audio output devices"));
    }
    for d in devices {
        let default = if d.is_default { " (default)" } else { "" };
        outln!(
            "{}{}  {}",
            d.name,
            default,
            dim(&format!("{:?} Hz", d.sample_rates))
        );
    }
    Ok(())
}
