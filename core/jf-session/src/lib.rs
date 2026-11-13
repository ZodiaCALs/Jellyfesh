//! Playback session: queue, shuffle with history, repeat modes, persistence
//! and restore.
//!
//! * [`Queue`] is the play queue: canonical and shuffled orders, repeat
//!   modes, Play Next / Play Last, moves and removals, and a bounded play
//!   history. It is plain data with no I/O, clocks or threads.
//! * [`SessionStore`] saves and restores the queue, the playback position
//!   and small settings in SQLite (feature `sqlite`, on by default; without
//!   it the crate builds for `wasm32-unknown-unknown`, as Jellyfesh Web uses it).

pub mod queue;
#[cfg(feature = "sqlite")]
pub mod store;

pub use queue::{
    Advance, DEFAULT_HISTORY_LIMIT, HistoryEntry, Queue, QueueError, QueueItem, QueueState, RemoveOutcome,
    RepeatMode, SourceTrack, TrackRef,
};
#[cfg(feature = "sqlite")]
pub use store::{PlaybackState, SCHEMA_VERSION, SessionError, SessionStore};
