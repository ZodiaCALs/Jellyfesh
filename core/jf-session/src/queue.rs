//! The play queue: canonical order, playback order (shuffle), repeat modes
//! and a bounded play history.
//!
//! The queue keeps two sequences of entry ids:
//!
//! * the **canonical** order, the order the listener built (an album's track
//!   order, the order songs were added);
//! * the **playback** order, which is the canonical order when shuffle is off
//!   and a stable permutation of it when shuffle is on.
//!
//! `position` indexes the playback order. Because a shuffled order is a fixed
//! permutation rather than a fresh random pick each time, Previous returns to
//! the track that actually played before ("shuffle with history"), and the
//! Up Next list a UI shows is exactly what will play.
//!
//! Randomness is derived from a per-queue seed and a counter, so a queue can
//! be cloned, persisted and restored, and tests are deterministic
//! ([`Queue::with_seed`]).

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{RngExt, SeedableRng};
use serde::{Deserialize, Serialize};

/// What a queue entry plays.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackRef {
    /// A track in the local library catalogue, by its library id.
    Library(i64),
    /// A file played directly, outside the library.
    File(PathBuf),
    /// A track from an online source (a plugin).
    Source(SourceTrack),
}

/// A track from an online source, with the display data the source gave
/// when it was listed, so queues, history and playlists show it without a
/// network round trip (and while offline).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceTrack {
    /// The plugin's id, e.g. `org.jellyfesh.subsonic`.
    pub plugin: String,
    /// The plugin's own id for the track.
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    /// Artwork URL as the source gave it.
    pub artwork_url: Option<String>,
    pub isrc: Option<String>,
}

/// One entry in the queue.
///
/// `uid` is unique within the queue for its whole life, so the same track can
/// be queued twice and UIs can key rows (and animate moves) by it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueItem {
    pub uid: u64,
    pub track: TrackRef,
}

/// Repeat mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    /// Stop after the last entry.
    #[default]
    Off,
    /// Start again from the first entry (reshuffled when shuffle is on).
    All,
    /// Replay the current entry when it ends. Next and Previous still move,
    /// and wrap around at the ends like [`RepeatMode::All`].
    One,
}

/// Why the queue moves forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// The current entry finished playing.
    Auto,
    /// The listener pressed Next.
    Manual,
}

/// A track that started playing, for the history list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub track: TrackRef,
    /// Wall-clock time in milliseconds since the Unix epoch, supplied by the
    /// caller (the queue never reads a clock).
    pub played_at_ms: i64,
}

/// Result of [`Queue::remove`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveOutcome {
    /// The removed entry was the current one. The entry that followed it is
    /// now current, or nothing is when it was the last one.
    pub was_current: bool,
}

/// Errors from queue operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QueueError {
    #[error("no queue entry with uid {0}")]
    UnknownItem(u64),
    #[error("index {index} is out of range for a queue of {len} entries")]
    OutOfRange { index: usize, len: usize },
    #[error("invalid queue state: {0}")]
    InvalidState(String),
}

/// Default number of entries kept in the play history.
pub const DEFAULT_HISTORY_LIMIT: usize = 500;

/// The play queue. See the [module documentation](self).
#[derive(Debug, Clone)]
pub struct Queue {
    tracks: HashMap<u64, TrackRef>,
    canonical: Vec<u64>,
    order: Vec<u64>,
    position: Option<usize>,
    shuffle: bool,
    repeat: RepeatMode,
    history: VecDeque<HistoryEntry>,
    history_limit: usize,
    next_uid: u64,
    seed: u64,
    shuffles: u64,
    revision: u64,
}

/// A serialisable copy of a queue, used for persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueState {
    /// Entries in canonical order.
    pub items: Vec<QueueItem>,
    /// Entry uids in playback order.
    pub order: Vec<u64>,
    /// Index into `order` of the current entry.
    pub position: Option<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// Oldest first.
    pub history: Vec<HistoryEntry>,
    pub next_uid: u64,
    pub seed: u64,
    pub shuffles: u64,
}

enum Step {
    Stay,
    To(usize),
    Wrap(Option<Vec<u64>>),
    End,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    /// An empty queue with a random shuffle seed.
    pub fn new() -> Self {
        Self::with_seed(rand::random())
    }

    /// An empty queue whose shuffles are reproducible from `seed`.
    pub fn with_seed(seed: u64) -> Self {
        Queue {
            tracks: HashMap::new(),
            canonical: Vec::new(),
            order: Vec::new(),
            position: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            history: VecDeque::new(),
            history_limit: DEFAULT_HISTORY_LIMIT,
            next_uid: 1,
            seed,
            shuffles: 0,
            revision: 0,
        }
    }

    // ---- reading -------------------------------------------------------

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Index of the current entry in playback order, if one is current.
    pub fn position(&self) -> Option<usize> {
        self.position
    }

    pub fn is_shuffled(&self) -> bool {
        self.shuffle
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    /// Increases on every change; UIs re-read the queue when it moves.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn current_uid(&self) -> Option<u64> {
        self.position.map(|p| self.order[p])
    }

    pub fn current(&self) -> Option<QueueItem> {
        self.position.and_then(|p| self.item_at(p))
    }

    /// The track of the entry with this uid.
    pub fn get(&self, uid: u64) -> Option<&TrackRef> {
        self.tracks.get(&uid)
    }

    /// The entry at `index` in playback order.
    pub fn item_at(&self, index: usize) -> Option<QueueItem> {
        let uid = *self.order.get(index)?;
        self.item(uid)
    }

    /// Index of the entry with this uid in playback order.
    pub fn index_of(&self, uid: u64) -> Option<usize> {
        self.order.iter().position(|u| *u == uid)
    }

    /// All entries in playback order.
    pub fn iter(&self) -> impl Iterator<Item = QueueItem> + '_ {
        self.order.iter().filter_map(|u| self.item(*u))
    }

    /// All entries in canonical (unshuffled) order.
    pub fn canonical(&self) -> impl Iterator<Item = QueueItem> + '_ {
        self.canonical.iter().filter_map(|u| self.item(*u))
    }

    /// A window of the playback order, for paged lists.
    pub fn window(&self, offset: usize, limit: usize) -> Vec<QueueItem> {
        self.order
            .iter()
            .skip(offset)
            .take(limit)
            .filter_map(|u| self.item(*u))
            .collect()
    }

    /// The entries after the current one in playback order (all entries when
    /// nothing is current yet).
    pub fn up_next(&self, limit: usize) -> Vec<QueueItem> {
        let start = self.position.map_or(0, |p| p + 1);
        self.window(start, limit)
    }

    /// Play history, oldest first.
    pub fn history(&self) -> impl Iterator<Item = &HistoryEntry> {
        self.history.iter()
    }

    /// What [`Queue::next`] with the same `how` would return, without moving.
    /// Engines use it to prepare the following track for gapless playback.
    pub fn peek_next(&self, how: Advance) -> Option<QueueItem> {
        match self.plan_next(how) {
            Step::Stay => self.current(),
            Step::To(p) => self.item_at(p),
            Step::Wrap(Some(order)) => order.first().and_then(|u| self.item(*u)),
            Step::Wrap(None) => self.item_at(0),
            Step::End => None,
        }
    }

    // ---- moving --------------------------------------------------------

    /// Moves to the next entry and returns it.
    ///
    /// * Nothing current yet: the first entry in playback order.
    /// * [`Advance::Auto`] with [`RepeatMode::One`]: the same entry again.
    /// * At the last entry: wraps to the first with [`RepeatMode::All`] (and
    ///   with [`RepeatMode::One`] on a manual Next), reshuffling first when
    ///   shuffle is on so a new cycle does not repeat the previous one, and
    ///   never starts it with the entry that just played. With
    ///   [`RepeatMode::Off`] it returns `None` and stays on the last entry.
    pub fn next(&mut self, how: Advance) -> Option<QueueItem> {
        match self.plan_next(how) {
            Step::Stay => self.current(),
            Step::To(p) => {
                self.position = Some(p);
                self.touch();
                self.current()
            }
            Step::Wrap(order) => {
                if let Some(order) = order {
                    self.order = order;
                    self.shuffles += 1;
                }
                self.position = Some(0);
                self.touch();
                self.current()
            }
            Step::End => None,
        }
    }

    /// Moves to the previous entry in playback order and returns it. At the
    /// first entry it wraps to the last when repeat is on, and otherwise
    /// stays (the caller restarts the current track). Whether Previous
    /// restarts the current track instead (after a few seconds of playback)
    /// is the caller's decision, because only it knows the position.
    pub fn previous(&mut self) -> Option<QueueItem> {
        let p = self.position?;
        let len = self.order.len();
        let np = if p > 0 {
            p - 1
        } else if self.repeat != RepeatMode::Off {
            len - 1
        } else {
            0
        };
        if np != p {
            self.position = Some(np);
            self.touch();
        }
        self.current()
    }

    /// Makes the entry with this uid current. Entries between the old and
    /// the new position count as skipped.
    pub fn jump_to(&mut self, uid: u64) -> Result<QueueItem, QueueError> {
        let index = self.index_of(uid).ok_or(QueueError::UnknownItem(uid))?;
        self.position = Some(index);
        self.touch();
        self.item(uid).ok_or(QueueError::UnknownItem(uid))
    }

    // ---- editing -------------------------------------------------------

    /// Replaces the whole queue. `start` (an index into `tracks`) becomes
    /// current; with shuffle on it plays first and the rest are shuffled.
    pub fn set_items(&mut self, tracks: Vec<TrackRef>, start: Option<usize>) -> Result<Vec<u64>, QueueError> {
        if let Some(s) = start
            && s >= tracks.len()
        {
            return Err(QueueError::OutOfRange {
                index: s,
                len: tracks.len(),
            });
        }
        self.tracks.clear();
        let uids = self.alloc(tracks);
        self.canonical = uids.clone();
        let first = start.map(|s| uids[s]);
        if self.shuffle {
            self.order = self.shuffle_with_first(first);
            self.position = first.map(|_| 0);
        } else {
            self.order = uids.clone();
            self.position = start;
        }
        self.touch();
        Ok(uids)
    }

    /// Adds entries at the end ("Play Last"). With shuffle on they keep the
    /// given order and play after everything already queued.
    pub fn append(&mut self, tracks: Vec<TrackRef>) -> Vec<u64> {
        let uids = self.alloc(tracks);
        if !uids.is_empty() {
            self.canonical.extend_from_slice(&uids);
            self.order.extend_from_slice(&uids);
            self.touch();
        }
        uids
    }

    /// Adds entries right after the current one ("Play Next"), in the given
    /// order, or at the front when nothing is current.
    pub fn insert_next(&mut self, tracks: Vec<TrackRef>) -> Vec<u64> {
        let uids = self.alloc(tracks);
        if uids.is_empty() {
            return uids;
        }
        let (ci, oi) = match (self.current_uid(), self.position) {
            (Some(cur), Some(p)) => {
                let ci = self
                    .canonical
                    .iter()
                    .position(|u| *u == cur)
                    .map_or(self.canonical.len(), |i| i + 1);
                (ci, p + 1)
            }
            _ => (0, 0),
        };
        self.canonical.splice(ci..ci, uids.iter().copied());
        self.order.splice(oi..oi, uids.iter().copied());
        self.touch();
        uids
    }

    /// Removes one entry.
    pub fn remove(&mut self, uid: u64) -> Result<RemoveOutcome, QueueError> {
        let oi = self.index_of(uid).ok_or(QueueError::UnknownItem(uid))?;
        let ci = self
            .canonical
            .iter()
            .position(|u| *u == uid)
            .ok_or_else(|| QueueError::InvalidState(format!("uid {uid} missing from canonical order")))?;
        self.order.remove(oi);
        self.canonical.remove(ci);
        self.tracks.remove(&uid);
        let mut was_current = false;
        if let Some(p) = self.position {
            if oi < p {
                self.position = Some(p - 1);
            } else if oi == p {
                was_current = true;
                self.position = (p < self.order.len()).then_some(p);
            }
        }
        self.touch();
        Ok(RemoveOutcome { was_current })
    }

    /// Moves an entry to `to` in playback order. With shuffle off this also
    /// changes the canonical order; with shuffle on it changes only the
    /// shuffled order. The current entry stays current.
    pub fn move_item(&mut self, uid: u64, to: usize) -> Result<(), QueueError> {
        let from = self.index_of(uid).ok_or(QueueError::UnknownItem(uid))?;
        let len = self.order.len();
        if to >= len {
            return Err(QueueError::OutOfRange { index: to, len });
        }
        if from == to {
            return Ok(());
        }
        let cur = self.current_uid();
        let u = self.order.remove(from);
        self.order.insert(to, u);
        if !self.shuffle {
            // Canonical equals playback order while shuffle is off.
            let u = self.canonical.remove(from);
            self.canonical.insert(to, u);
        }
        self.position = cur.and_then(|c| self.index_of(c));
        self.touch();
        Ok(())
    }

    /// Removes every entry. The history is kept.
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.canonical.clear();
        self.order.clear();
        self.position = None;
        self.touch();
    }

    /// Turns shuffle on or off. Turning it on keeps the current entry current
    /// and shuffles all the others after it; turning it off returns to the
    /// canonical order at the current entry.
    pub fn set_shuffle(&mut self, on: bool) {
        if self.shuffle == on {
            return;
        }
        let cur = self.current_uid();
        self.shuffle = on;
        if on {
            self.order = self.shuffle_with_first(cur);
            self.position = cur.map(|_| 0);
        } else {
            self.order = self.canonical.clone();
            self.position = cur.and_then(|c| self.index_of(c));
        }
        self.touch();
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        if self.repeat != mode {
            self.repeat = mode;
            self.touch();
        }
    }

    /// Records that a track started playing. Keeps at most the history limit,
    /// dropping the oldest.
    pub fn record_played(&mut self, track: TrackRef, played_at_ms: i64) {
        self.history.push_back(HistoryEntry { track, played_at_ms });
        while self.history.len() > self.history_limit {
            self.history.pop_front();
        }
        self.touch();
    }

    pub fn set_history_limit(&mut self, limit: usize) {
        self.history_limit = limit;
        while self.history.len() > limit {
            self.history.pop_front();
        }
    }

    // ---- persistence ---------------------------------------------------

    pub fn to_state(&self) -> QueueState {
        QueueState {
            items: self.canonical().collect(),
            order: self.order.clone(),
            position: self.position,
            shuffle: self.shuffle,
            repeat: self.repeat,
            history: self.history.iter().cloned().collect(),
            next_uid: self.next_uid,
            seed: self.seed,
            shuffles: self.shuffles,
        }
    }

    /// Rebuilds a queue from a saved state, checking that it is consistent.
    pub fn from_state(state: QueueState) -> Result<Queue, QueueError> {
        let mut tracks = HashMap::with_capacity(state.items.len());
        let mut canonical = Vec::with_capacity(state.items.len());
        for item in state.items {
            if tracks.insert(item.uid, item.track).is_some() {
                return Err(QueueError::InvalidState(format!("duplicate uid {}", item.uid)));
            }
            canonical.push(item.uid);
        }
        if state.order.len() != canonical.len() {
            return Err(QueueError::InvalidState(
                "order and items differ in length".into(),
            ));
        }
        let mut seen = HashSet::with_capacity(state.order.len());
        for u in &state.order {
            if !tracks.contains_key(u) || !seen.insert(*u) {
                return Err(QueueError::InvalidState(format!(
                    "order is not a permutation of the items (uid {u})"
                )));
            }
        }
        if !state.shuffle && state.order != canonical {
            return Err(QueueError::InvalidState(
                "unshuffled order differs from canonical order".into(),
            ));
        }
        if let Some(p) = state.position
            && p >= state.order.len()
        {
            return Err(QueueError::InvalidState(format!("position {p} out of range")));
        }
        let max_uid = canonical.iter().copied().max().unwrap_or(0);
        Ok(Queue {
            tracks,
            canonical,
            order: state.order,
            position: state.position,
            shuffle: state.shuffle,
            repeat: state.repeat,
            history: state.history.into(),
            history_limit: DEFAULT_HISTORY_LIMIT,
            next_uid: state.next_uid.max(max_uid + 1),
            seed: state.seed,
            shuffles: state.shuffles,
            revision: 0,
        })
    }

    // ---- internals -----------------------------------------------------

    fn item(&self, uid: u64) -> Option<QueueItem> {
        self.tracks.get(&uid).map(|t| QueueItem {
            uid,
            track: t.clone(),
        })
    }

    fn touch(&mut self) {
        self.revision += 1;
    }

    fn alloc(&mut self, tracks: Vec<TrackRef>) -> Vec<u64> {
        let mut uids = Vec::with_capacity(tracks.len());
        for t in tracks {
            let uid = self.next_uid;
            self.next_uid += 1;
            self.tracks.insert(uid, t);
            uids.push(uid);
        }
        uids
    }

    fn plan_next(&self, how: Advance) -> Step {
        let len = self.order.len();
        if len == 0 {
            return Step::End;
        }
        let Some(p) = self.position else {
            return Step::To(0);
        };
        if how == Advance::Auto && self.repeat == RepeatMode::One {
            return Step::Stay;
        }
        if p + 1 < len {
            return Step::To(p + 1);
        }
        match self.repeat {
            RepeatMode::Off => Step::End,
            RepeatMode::All | RepeatMode::One => {
                if self.shuffle && len > 1 {
                    Step::Wrap(Some(self.shuffled(self.order.clone(), Some(self.order[p]))))
                } else {
                    Step::Wrap(None)
                }
            }
        }
    }

    /// Shuffles `uids` with the generator for the current counter. When
    /// `avoid_first` is given and lands first, it is swapped away so a new
    /// cycle never starts with the entry that just played.
    fn shuffled(&self, mut uids: Vec<u64>, avoid_first: Option<u64>) -> Vec<u64> {
        let mut rng = StdRng::seed_from_u64(mix(self.seed, self.shuffles));
        uids.shuffle(&mut rng);
        if let Some(avoid) = avoid_first
            && uids.len() > 1
            && uids[0] == avoid
        {
            let j = rng.random_range(1..uids.len());
            uids.swap(0, j);
        }
        uids
    }

    /// Playback order with `first` (if any) first and every other entry
    /// shuffled after it.
    fn shuffle_with_first(&mut self, first: Option<u64>) -> Vec<u64> {
        let rest: Vec<u64> = self
            .canonical
            .iter()
            .copied()
            .filter(|u| Some(*u) != first)
            .collect();
        let mut order = Vec::with_capacity(self.canonical.len());
        order.extend(first);
        order.extend(self.shuffled(rest, None));
        self.shuffles += 1;
        order
    }
}

/// SplitMix64 finaliser: decorrelates the per-shuffle seeds.
fn mix(seed: u64, n: u64) -> u64 {
    let mut z = seed ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib(ids: impl IntoIterator<Item = i64>) -> Vec<TrackRef> {
        ids.into_iter().map(TrackRef::Library).collect()
    }

    fn ids(items: &[QueueItem]) -> Vec<i64> {
        items
            .iter()
            .map(|i| match i.track {
                TrackRef::Library(id) => id,
                TrackRef::File(_) | TrackRef::Source(_) => -1,
            })
            .collect()
    }

    fn cur(q: &Queue) -> Option<i64> {
        q.current().map(|i| ids(&[i])[0])
    }

    fn all(q: &Queue) -> Vec<i64> {
        ids(&q.iter().collect::<Vec<_>>())
    }

    #[test]
    fn empty_queue_has_nothing_to_play() {
        let mut q = Queue::with_seed(1);
        assert!(q.is_empty());
        assert_eq!(q.next(Advance::Auto), None);
        assert_eq!(q.previous(), None);
        assert_eq!(q.peek_next(Advance::Auto), None);
        assert_eq!(q.current(), None);
    }

    #[test]
    fn plays_in_order_and_ends_without_repeat() {
        let mut q = Queue::with_seed(1);
        q.set_items(lib([10, 11, 12]), None).unwrap();
        assert_eq!(q.position(), None);
        let mut played = Vec::new();
        while let Some(item) = q.next(Advance::Auto) {
            played.push(item);
        }
        assert_eq!(ids(&played), vec![10, 11, 12]);
        // Stays on the last entry after the end.
        assert_eq!(cur(&q), Some(12));
        assert_eq!(q.next(Advance::Manual), None);
    }

    #[test]
    fn start_index_becomes_current() {
        let mut q = Queue::with_seed(1);
        q.set_items(lib([1, 2, 3, 4]), Some(2)).unwrap();
        assert_eq!(cur(&q), Some(3));
        assert_eq!(ids(&q.up_next(10)), vec![4]);
        assert_eq!(
            q.set_items(lib([1]), Some(1)),
            Err(QueueError::OutOfRange { index: 1, len: 1 })
        );
    }

    #[test]
    fn repeat_one_replays_on_auto_but_moves_on_manual() {
        let mut q = Queue::with_seed(1);
        q.set_items(lib([1, 2]), Some(0)).unwrap();
        q.set_repeat(RepeatMode::One);
        assert_eq!(ids(&[q.peek_next(Advance::Auto).unwrap()]), vec![1]);
        assert_eq!(ids(&[q.next(Advance::Auto).unwrap()]), vec![1]);
        assert_eq!(ids(&[q.next(Advance::Manual).unwrap()]), vec![2]);
        // Manual next at the end wraps with repeat one.
        assert_eq!(ids(&[q.next(Advance::Manual).unwrap()]), vec![1]);
    }

    #[test]
    fn repeat_all_wraps() {
        let mut q = Queue::with_seed(1);
        q.set_items(lib([1, 2, 3]), Some(2)).unwrap();
        q.set_repeat(RepeatMode::All);
        assert_eq!(ids(&[q.peek_next(Advance::Auto).unwrap()]), vec![1]);
        assert_eq!(ids(&[q.next(Advance::Auto).unwrap()]), vec![1]);
        assert_eq!(ids(&[q.previous().unwrap()]), vec![3]);
    }

    #[test]
    fn previous_at_start_stays_without_repeat() {
        let mut q = Queue::with_seed(1);
        q.set_items(lib([1, 2, 3]), Some(1)).unwrap();
        assert_eq!(cur(&q.clone()), Some(2));
        assert_eq!(ids(&[q.previous().unwrap()]), vec![1]);
        assert_eq!(ids(&[q.previous().unwrap()]), vec![1]);
    }

    #[test]
    fn shuffle_keeps_current_first_and_previous_retraces() {
        let mut q = Queue::with_seed(42);
        q.set_items(lib(1..=20), Some(4)).unwrap();
        q.set_shuffle(true);
        assert_eq!(cur(&q), Some(5));
        assert_eq!(q.position(), Some(0));
        let mut sorted = all(&q);
        sorted.sort_unstable();
        assert_eq!(sorted, (1..=20).collect::<Vec<_>>());
        assert_ne!(all(&q)[1..], (1..=20).filter(|i| *i != 5).collect::<Vec<_>>()[..]);

        let a = q.next(Advance::Auto).unwrap();
        let b = q.next(Advance::Auto).unwrap();
        assert_eq!(q.previous().unwrap(), a);
        assert_eq!(q.next(Advance::Manual).unwrap(), b);

        // Turning shuffle off returns to album order at the current entry.
        q.set_shuffle(false);
        assert_eq!(all(&q), (1..=20).collect::<Vec<_>>());
        assert_eq!(q.current().unwrap(), b);
    }

    #[test]
    fn shuffle_is_reproducible_from_the_seed() {
        let build = |seed| {
            let mut q = Queue::with_seed(seed);
            q.set_items(lib(1..=30), None).unwrap();
            q.set_shuffle(true);
            all(&q)
        };
        assert_eq!(build(7), build(7));
        assert_ne!(build(7), build(8));
    }

    #[test]
    fn repeat_all_reshuffles_each_cycle_without_an_immediate_repeat() {
        let mut q = Queue::with_seed(3);
        q.set_items(lib(1..=8), None).unwrap();
        q.set_shuffle(true);
        q.set_repeat(RepeatMode::All);
        let mut last = None;
        let mut cycles = Vec::new();
        for _ in 0..6 {
            let mut cycle = Vec::new();
            for _ in 0..8 {
                let peeked = q.peek_next(Advance::Auto);
                let item = q.next(Advance::Auto).unwrap();
                assert_eq!(peeked.as_ref(), Some(&item), "peek must match next");
                assert_ne!(Some(item.uid), last, "no immediate repeat");
                last = Some(item.uid);
                cycle.push(item);
            }
            let mut sorted = ids(&cycle);
            sorted.sort_unstable();
            assert_eq!(
                sorted,
                (1..=8).collect::<Vec<_>>(),
                "each cycle plays every entry once"
            );
            cycles.push(ids(&cycle));
        }
        assert!(cycles.windows(2).any(|w| w[0] != w[1]), "cycles are reshuffled");
    }

    #[test]
    fn insert_next_and_append() {
        let mut q = Queue::with_seed(1);
        q.set_items(lib([1, 2, 3]), Some(0)).unwrap();
        q.insert_next(lib([10, 11]));
        q.append(lib([20]));
        assert_eq!(all(&q), vec![1, 10, 11, 2, 3, 20]);
        assert_eq!(ids(&q.canonical().collect::<Vec<_>>()), vec![1, 10, 11, 2, 3, 20]);

        // With nothing current, Play Next goes to the front.
        let mut q = Queue::with_seed(1);
        q.set_items(lib([1, 2]), None).unwrap();
        q.insert_next(lib([9]));
        assert_eq!(ids(&[q.next(Advance::Auto).unwrap()]), vec![9]);
    }

    #[test]
    fn insert_next_with_shuffle_plays_next_and_lands_after_current_when_unshuffled() {
        let mut q = Queue::with_seed(5);
        q.set_items(lib(1..=10), Some(3)).unwrap();
        q.set_shuffle(true);
        q.insert_next(lib([99]));
        assert_eq!(ids(&[q.peek_next(Advance::Auto).unwrap()]), vec![99]);
        q.set_shuffle(false);
        assert_eq!(all(&q), vec![1, 2, 3, 4, 99, 5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn remove_adjusts_position() {
        let mut q = Queue::with_seed(1);
        let uids = q.set_items(lib([1, 2, 3, 4]), Some(2)).unwrap();
        assert_eq!(q.remove(uids[0]).unwrap(), RemoveOutcome { was_current: false });
        assert_eq!(cur(&q), Some(3));
        assert_eq!(q.remove(uids[2]).unwrap(), RemoveOutcome { was_current: true });
        assert_eq!(cur(&q), Some(4));
        assert_eq!(q.remove(uids[3]).unwrap(), RemoveOutcome { was_current: true });
        assert_eq!(q.current(), None);
        assert_eq!(all(&q), vec![2]);
        assert_eq!(q.remove(12345), Err(QueueError::UnknownItem(12345)));
    }

    #[test]
    fn move_keeps_current_and_reorders() {
        let mut q = Queue::with_seed(1);
        let uids = q.set_items(lib([1, 2, 3, 4]), Some(1)).unwrap();
        q.move_item(uids[3], 0).unwrap();
        assert_eq!(all(&q), vec![4, 1, 2, 3]);
        assert_eq!(cur(&q), Some(2));
        q.move_item(uids[1], 3).unwrap();
        assert_eq!(all(&q), vec![4, 1, 3, 2]);
        assert_eq!(cur(&q), Some(2));
        assert_eq!(q.position(), Some(3));
        assert_eq!(
            q.move_item(uids[1], 4),
            Err(QueueError::OutOfRange { index: 4, len: 4 })
        );
    }

    #[test]
    fn move_with_shuffle_leaves_canonical_alone() {
        let mut q = Queue::with_seed(9);
        let uids = q.set_items(lib(1..=5), Some(0)).unwrap();
        q.set_shuffle(true);
        q.move_item(uids[4], 1).unwrap();
        assert_eq!(ids(&[q.peek_next(Advance::Auto).unwrap()]), vec![5]);
        assert_eq!(ids(&q.canonical().collect::<Vec<_>>()), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn jump_to_sets_current() {
        let mut q = Queue::with_seed(1);
        let uids = q.set_items(lib([1, 2, 3]), None).unwrap();
        assert_eq!(ids(&[q.jump_to(uids[2]).unwrap()]), vec![3]);
        assert_eq!(q.position(), Some(2));
        assert_eq!(q.jump_to(77), Err(QueueError::UnknownItem(77)));
    }

    #[test]
    fn same_track_twice_has_distinct_uids() {
        let mut q = Queue::with_seed(1);
        let uids = q.set_items(lib([1, 1]), None).unwrap();
        assert_ne!(uids[0], uids[1]);
        q.remove(uids[0]).unwrap();
        assert_eq!(all(&q), vec![1]);
    }

    #[test]
    fn history_is_bounded() {
        let mut q = Queue::with_seed(1);
        q.set_history_limit(3);
        for i in 0..5 {
            q.record_played(TrackRef::Library(i), 1_000 + i);
        }
        let h: Vec<_> = q.history().map(|e| e.played_at_ms).collect();
        assert_eq!(h, vec![1_002, 1_003, 1_004]);
    }

    #[test]
    fn revision_moves_on_change_only() {
        let mut q = Queue::with_seed(1);
        let r0 = q.revision();
        q.set_repeat(RepeatMode::Off);
        q.set_shuffle(false);
        assert_eq!(q.revision(), r0);
        q.append(lib([1]));
        assert!(q.revision() > r0);
    }

    #[test]
    fn state_round_trip() {
        let mut q = Queue::with_seed(11);
        q.set_items(lib(1..=6), Some(2)).unwrap();
        q.append(vec![TrackRef::File(PathBuf::from("/music/a.flac"))]);
        q.set_shuffle(true);
        q.set_repeat(RepeatMode::All);
        q.next(Advance::Auto);
        q.record_played(TrackRef::Library(3), 5);
        let state = q.to_state();
        let r = Queue::from_state(state.clone()).unwrap();
        assert_eq!(r.to_state(), state);
        assert_eq!(r.current(), q.current());
        // Same seed and counter: the next cycle is shuffled the same way.
        let mut q2 = q.clone();
        let mut r2 = r.clone();
        for _ in 0..10 {
            assert_eq!(q2.next(Advance::Auto), r2.next(Advance::Auto));
        }
    }

    #[test]
    fn from_state_rejects_inconsistent_input() {
        let mut q = Queue::with_seed(1);
        q.set_items(lib([1, 2, 3]), Some(0)).unwrap();
        let good = q.to_state();

        let mut bad = good.clone();
        bad.order.pop();
        assert!(Queue::from_state(bad).is_err());

        let mut bad = good.clone();
        bad.order.swap(0, 1);
        assert!(
            Queue::from_state(bad).is_err(),
            "unshuffled order must equal canonical"
        );

        let mut bad = good.clone();
        bad.position = Some(3);
        assert!(Queue::from_state(bad).is_err());

        let mut bad = good.clone();
        bad.items[1].uid = bad.items[0].uid;
        assert!(Queue::from_state(bad).is_err());

        let mut repaired = good;
        repaired.next_uid = 0;
        let mut r = Queue::from_state(repaired).unwrap();
        let new = r.append(lib([4]));
        assert!(new[0] > 3, "new uids never collide with restored ones");
    }
}
