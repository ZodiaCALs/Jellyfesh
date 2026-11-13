//! Playlists, favourites and play history.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Subcommand;
use crossbeam_channel::Receiver;
use jf_core::{Command, Core, Event, PlaylistSummary, TrackRow};

use crate::util::{bold, dim, fmt_ms};

#[derive(Subcommand, Debug)]
pub enum PlaylistCmd {
    /// List playlists (the default)
    List,
    /// Show a playlist's entries
    Show {
        /// Playlist name or id
        playlist: String,
    },
    /// Create a playlist, optionally with files, folders or a library search
    New { name: String, targets: Vec<String> },
    /// Add files, folders or a library search to a playlist
    Add {
        /// Playlist name or id
        playlist: String,
        #[arg(required = true)]
        targets: Vec<String>,
    },
    /// Remove entries by their number in `jf playlist show`
    Remove {
        /// Playlist name or id
        playlist: String,
        #[arg(required = true)]
        numbers: Vec<usize>,
    },
    /// Move an entry to another place
    Move {
        /// Playlist name or id
        playlist: String,
        /// Entry number in `jf playlist show`
        from: usize,
        /// Its new number
        to: usize,
    },
    Rename {
        /// Playlist name or id
        playlist: String,
        name: String,
    },
    Delete {
        /// Playlist name or id
        playlist: String,
    },
    /// Import an M3U or M3U8 file as a new playlist
    Import { file: PathBuf },
    /// Write a playlist to an M3U8 file
    Export {
        /// Playlist name or id
        playlist: String,
        file: PathBuf,
    },
}

pub fn playlist(
    core: &Core,
    cmd: PlaylistCmd,
    resolve: impl Fn(&[String]) -> Result<Vec<jf_core::TrackRef>>,
) -> Result<()> {
    let events = core.subscribe();
    match cmd {
        PlaylistCmd::List => {
            let lists = core.playlists()?;
            if lists.is_empty() {
                outln!(
                    "{}",
                    dim("no playlists yet: `jf playlist new <name> [files or words]`")
                );
            }
            for p in lists {
                outln!(
                    "{:>5}  {}  {}",
                    p.id,
                    bold(&p.name),
                    dim(&format!(
                        "{} entr{} · {}",
                        p.entry_count,
                        if p.entry_count == 1 { "y" } else { "ies" },
                        fmt_ms(p.duration_ms)
                    ))
                );
            }
        }
        PlaylistCmd::Show { playlist } => {
            let p = find(core, &playlist)?;
            outln!(
                "{}  {}",
                bold(&p.name),
                dim(&format!("{} · {}", p.entry_count, fmt_ms(p.duration_ms)))
            );
            let entries = core.playlist_entries(p.id, 0, usize::MAX)?;
            for e in &entries {
                print_row(e.position as usize + 1, &e.row);
            }
        }
        PlaylistCmd::New { name, targets } => {
            let tracks = if targets.is_empty() {
                Vec::new()
            } else {
                resolve(&targets)?
            };
            let count = tracks.len();
            let id = created(core, &events, Command::CreatePlaylist { name, tracks })?;
            outln!(
                "created playlist {id} with {count} entr{}",
                if count == 1 { "y" } else { "ies" }
            );
        }
        PlaylistCmd::Add { playlist, targets } => {
            let p = find(core, &playlist)?;
            let tracks = resolve(&targets)?;
            let count = tracks.len();
            edit(core, &events, Command::AddToPlaylist { id: p.id, tracks })?;
            outln!("added {count} to {}", p.name);
        }
        PlaylistCmd::Remove { playlist, numbers } => {
            let p = find(core, &playlist)?;
            let entries = core.playlist_entries(p.id, 0, usize::MAX)?;
            let mut ids = Vec::new();
            for n in numbers {
                match n.checked_sub(1).and_then(|i| entries.get(i)) {
                    Some(e) => ids.push(e.entry_id),
                    None => bail!("{} has no entry {n}", p.name),
                }
            }
            edit(
                core,
                &events,
                Command::RemoveFromPlaylist {
                    id: p.id,
                    entries: ids,
                },
            )?;
        }
        PlaylistCmd::Move { playlist, from, to } => {
            let p = find(core, &playlist)?;
            let entries = core.playlist_entries(p.id, 0, usize::MAX)?;
            let Some(entry) = from.checked_sub(1).and_then(|i| entries.get(i)) else {
                bail!("{} has no entry {from}", p.name)
            };
            edit(
                core,
                &events,
                Command::MovePlaylistEntry {
                    id: p.id,
                    entry: entry.entry_id,
                    to: to.saturating_sub(1),
                },
            )?;
        }
        PlaylistCmd::Rename { playlist, name } => {
            let p = find(core, &playlist)?;
            edit(core, &events, Command::RenamePlaylist { id: p.id, name })?;
        }
        PlaylistCmd::Delete { playlist } => {
            let p = find(core, &playlist)?;
            edit(core, &events, Command::DeletePlaylist { id: p.id })?;
            outln!("deleted {}", p.name);
        }
        PlaylistCmd::Import { file } => {
            let id = created(core, &events, Command::ImportPlaylist { path: file })?;
            let p = core.playlist(id)?;
            outln!(
                "imported playlist {id}{}",
                p.map(|p| format!(" ({}, {} entries)", p.name, p.entry_count))
                    .unwrap_or_default()
            );
        }
        PlaylistCmd::Export { playlist, file } => {
            let p = find(core, &playlist)?;
            core.export_playlist(p.id, &file)?;
            outln!("wrote {}", file.display());
        }
    }
    Ok(())
}

/// Lists favourites, or marks (or with `remove`, unmarks) tracks.
pub fn favorites(core: &Core, tracks: Option<Vec<jf_core::TrackRef>>, remove: bool) -> Result<()> {
    let Some(tracks) = tracks else {
        let rows = core.favorites(0, usize::MAX)?;
        if rows.is_empty() {
            outln!("{}", dim("no favourites yet: `jf fav <files or words>`"));
        }
        for (i, row) in rows.iter().enumerate() {
            print_row(i + 1, row);
        }
        return Ok(());
    };
    let events = core.subscribe();
    let count = tracks.len();
    for track in tracks {
        edit(
            core,
            &events,
            Command::SetFavorite {
                track,
                favorite: !remove,
            },
        )?;
    }
    outln!("{} {count}", if remove { "unmarked" } else { "marked" });
    Ok(())
}

/// Recently played or most played tracks.
pub fn played(core: &Core, top: bool, limit: usize) -> Result<()> {
    let rows = if top {
        core.most_played(limit)?
    } else {
        core.recently_played(limit)?
    };
    if rows.is_empty() {
        outln!("{}", dim("nothing played yet"));
    }
    for (i, row) in rows.iter().enumerate() {
        print_row(i + 1, row);
    }
    Ok(())
}

/// A playlist by id or by name (ignoring case).
pub fn find(core: &Core, text: &str) -> Result<PlaylistSummary> {
    let lists = core.playlists()?;
    if let Ok(id) = text.parse::<i64>()
        && let Some(p) = lists.iter().find(|p| p.id == id)
    {
        return Ok(p.clone());
    }
    let wanted = text.to_lowercase();
    let mut named = lists.into_iter().filter(|p| p.name.to_lowercase() == wanted);
    match (named.next(), named.next()) {
        (Some(p), None) => Ok(p),
        (Some(a), Some(b)) => bail!(
            "more than one playlist is named {text:?} (ids {}, {}, …): use the id",
            a.id,
            b.id
        ),
        (None, _) => bail!("no playlist named {text:?} (see `jf playlist`)"),
    }
}

fn print_row(number: usize, row: &TrackRow) {
    let artist = row.artist.as_deref().unwrap_or("");
    let duration = row.duration_ms.map(fmt_ms).unwrap_or_default();
    let heart = if row.favorite { "♥" } else { " " };
    let title = if row.missing {
        dim(&format!("{} (missing)", row.title))
    } else {
        bold(&row.title)
    };
    outln!("{number:>4} {heart} {title}  {artist}  {}", dim(&duration));
}

/// Sends a library edit and waits until it is applied.
fn edit(core: &Core, events: &Receiver<Event>, command: Command) -> Result<()> {
    let before = core.snapshot().library_revision;
    let id = core.send(command);
    loop {
        match events.recv_timeout(Duration::from_secs(60)) {
            Ok(Event::CommandFailed { id: failed, error }) if failed == id => bail!("{error}"),
            Ok(Event::LibraryChanged { revision }) if revision > before => return Ok(()),
            Ok(_) => {}
            Err(_) => bail!("no answer from the library"),
        }
    }
}

/// Sends a command that makes a playlist and returns the playlist's id.
fn created(core: &Core, events: &Receiver<Event>, command: Command) -> Result<i64> {
    let id = core.send(command);
    loop {
        match events.recv_timeout(Duration::from_secs(60)) {
            Ok(Event::CommandFailed { id: failed, error }) if failed == id => bail!("{error}"),
            Ok(Event::PlaylistCreated {
                request,
                id: playlist,
            }) if request == id => return Ok(playlist),
            Ok(_) => {}
            Err(_) => bail!("no answer from the library"),
        }
    }
}
