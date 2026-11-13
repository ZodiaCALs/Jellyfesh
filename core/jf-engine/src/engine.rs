//! The engine handle and the engine thread behind it.
//!
//! One thread owns the decoders, the processing chain and the sink. Commands reach it over a
//! channel and are handled between chunks of at most [`CHUNK_FRAMES`] frames, so a command
//! takes effect within about 25 ms even while the thread is busy rendering.
//!
//! Positions are kept on one timeline: the "stream frame", counted at the sink rate. The
//! processing chain is time-aligned, so when a track's decoder opens, the stream frame where
//! its first sample will be written is known exactly. Each opened track gets a segment
//! starting there; the audible stream frame is the number of frames written minus the sink's
//! latency, and the newest segment that started at or before it is the track the listener
//! hears. Track events fire when that audible frame crosses a segment's start or end, so a
//! gapless join or a crossfade is announced when it is heard, not when it is decoded.
//!
//! # Opening items
//!
//! Local files open on the engine thread. HTTP items open on a short-lived thread (probing can
//! need several round trips), and their stream downloads on its own thread; the engine thread
//! only decodes when enough is buffered, and otherwise waits in short steps, handling commands.
//! When the output runs dry meanwhile it reports [`EngineEvent::Buffering`] and anchors carry
//! rate 0 until the audio flows again.
//!
//! # Crossfades
//!
//! The item after the current one is opened ahead of its transition ("prepared"): for a
//! crossfade at the point where the overlap could start at the earliest, for an HTTP item
//! about [`HTTP_PREPARE_LEAD_SECS`] earlier. The mixer holds the outgoing track there; once the
//! incoming one is open the overlap is fixed (the configured length, at most half of either
//! track) and the incoming track starts in the pipeline's second lane. At most two decoders
//! are live: during an overlap nothing else is prepared. If the incoming item is not ready in
//! time the transition falls back to a gapless join.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use parking_lot::{Condvar, Mutex};

use crate::anchor::{PlaybackAnchor, monotonic_now_ns};
use crate::decode::{Input, Source};
use crate::dsp::{EqSettings, replaygain_scale};
use crate::http::{HttpStream, StreamOptions};
use crate::pipeline::{Fade, LaneId, Pipeline};
#[cfg(feature = "device")]
use crate::sink::DeviceSink;
use crate::sink::{NullSink, Sink, WavSink};
use crate::types::{
    CrossfadeSettings, EngineConfig, EngineError, EngineEvent, MediaSource, OutputKind, PlayItem,
    ReplayGainMode, TrackInfo,
};
use crate::worker::{self, PendingOpen, Read, StartAt, ThreadedSource};

/// Largest block written to the sink at once (about 21–23 ms at 44.1–48 kHz).
const CHUNK_FRAMES: usize = 1024;
/// How long one drain step waits for the sink to play out.
const DRAIN_POLL: Duration = Duration::from_millis(10);
/// Re-anchoring is checked at most this often during playback.
const REANCHOR_INTERVAL: Duration = Duration::from_secs(1);
/// For this long after the output starts, drift is checked every [`SETTLE_INTERVAL`]: start-up
/// is when an output's delay is least known.
const SETTLE_TIME: Duration = Duration::from_secs(2);
const SETTLE_INTERVAL: Duration = Duration::from_millis(100);
/// A re-anchor is published when the audio clock drifts this far from the last anchor.
const DRIFT_THRESHOLD_US: i64 = 15_000;
/// Fade length that keeps manual transitions (skip, seek, stop, output change) from clicking.
const DECLICK_MS: u64 = 10;
/// Overlaps shorter than this join gaplessly instead.
const MIN_OVERLAP_MS: u64 = 50;
/// An HTTP item is opened this long before its transition.
pub(crate) const HTTP_PREPARE_LEAD_SECS: u64 = 10;
/// How long one wait for stream data or an opening lasts before commands are checked again.
const STREAM_WAIT: Duration = Duration::from_millis(20);
/// An opening that takes longer than this, with the output dry, is reported as buffering.
const OPEN_GRACE: Duration = Duration::from_millis(50);
/// Pauses between attempts to reopen the default output after it failed.
const REOPEN_DELAYS_MS: [u64; 4] = [0, 250, 750, 1_500];
/// HTTP streams kept around (current, prepared, recently played) so seeks and repeats reuse
/// what was downloaded.
const STREAM_POOL: usize = 4;

enum Command {
    SetQueue {
        items: Vec<PlayItem>,
        start_index: usize,
    },
    SetUpcoming(Vec<PlayItem>),
    SetUpcomingAfter {
        after: u64,
        items: Vec<PlayItem>,
    },
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    Seek {
        position_ms: u64,
    },
    SetVolume(f32),
    SetEq(EqSettings),
    SetReplayGain {
        mode: ReplayGainMode,
        preamp_db: f32,
    },
    SetCrossfade(CrossfadeSettings),
    SetOutput(OutputKind),
    DevicesChanged,
    Shutdown,
}

#[derive(Debug, Default)]
struct Shared {
    state: Mutex<SharedState>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct SharedState {
    /// Sequence number of the last command the engine thread has handled.
    processed: u64,
    playing: bool,
    exited: bool,
    anchor: PlaybackAnchor,
}

/// The audio engine. Commands are queued to the engine thread and return immediately;
/// results arrive as [`EngineEvent`]s.
pub struct Engine {
    commands: Sender<(u64, Command)>,
    events: Receiver<EngineEvent>,
    shared: Arc<Shared>,
    /// Sequence number of the last command sent; the lock keeps numbering and sending in order.
    sent: Mutex<u64>,
    thread: Option<JoinHandle<()>>,
}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.shared.state.lock();
        f.debug_struct("Engine")
            .field("playing", &state.playing)
            .field("anchor", &state.anchor)
            .finish_non_exhaustive()
    }
}

impl Engine {
    /// Starts the engine thread with the output described by `config`. Fails, without
    /// leaving a thread behind, when the output cannot be opened (for example when there is
    /// no audio device).
    pub fn new(config: EngineConfig) -> Result<Self, EngineError> {
        let settings = config.clone();
        Self::spawn(settings, true, move || {
            make_sink(&config.output, config.buffer_ms)
        })
    }

    /// Starts the engine thread with a caller-provided sink. HTTP items stream without a cache
    /// directory or resolver; [`set_output`](Self::set_output) can replace the sink later.
    /// After an [`EngineEvent::OutputFailed`] there is no sink to reopen until `set_output`.
    pub fn with_sink(sink: Box<dyn Sink>) -> Result<Self, EngineError> {
        let mut sink = Some(sink);
        Self::with_sink_factory(move || {
            sink.take()
                .ok_or_else(|| EngineError::Device("the output failed and has no replacement".into()))
        })
    }

    /// Starts the engine thread with sinks from `factory`: one now, and a fresh one each time
    /// playback starts again after the output failed ([`EngineEvent::OutputFailed`]). HTTP items
    /// stream without a cache directory or resolver.
    pub fn with_sink_factory<F>(factory: F) -> Result<Self, EngineError>
    where
        F: FnMut() -> Result<Box<dyn Sink>, EngineError> + Send + 'static,
    {
        Self::spawn(EngineConfig::default(), false, factory)
    }

    fn spawn<F>(config: EngineConfig, owns_output: bool, mut make_sink: F) -> Result<Self, EngineError>
    where
        F: FnMut() -> Result<Box<dyn Sink>, EngineError> + Send + 'static,
    {
        let (command_tx, command_rx) = crossbeam_channel::unbounded();
        let (event_tx, event_rx) = crossbeam_channel::unbounded();
        let (ready_tx, ready_rx) = crossbeam_channel::bounded(1);
        let shared = Arc::new(Shared::default());
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("jf-engine".into())
            .spawn(move || {
                let _exit = ExitGuard(Arc::clone(&thread_shared));
                // The sink is created here so a device stream lives on the engine thread.
                let sink = match make_sink() {
                    Ok(sink) => sink,
                    Err(err) => {
                        let _ = ready_tx.send(Err(err));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                let output = owns_output.then(|| config.output.clone());
                // An engine with a configured output reopens it; one built from sinks asks
                // the factory again.
                let factory: Option<SinkFactory> = (!owns_output).then(|| Box::new(make_sink) as SinkFactory);
                EngineThread::new(sink, output, factory, config, command_rx, event_tx, thread_shared).run();
            })?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                commands: command_tx,
                events: event_rx,
                shared,
                sent: Mutex::new(0),
                thread: Some(thread),
            }),
            Ok(Err(err)) => {
                let _ = thread.join();
                Err(err)
            }
            Err(_) => {
                let _ = thread.join();
                Err(EngineError::Disconnected)
            }
        }
    }

    fn send(&self, command: Command) {
        let mut sent = self.sent.lock();
        *sent += 1;
        // Fails only when the engine thread is gone; there is nobody left to tell.
        let _ = self.commands.send((*sent, command));
    }

    /// Replaces the queue and makes `start_index` the current item (clamped to the queue).
    /// While playing or paused this interrupts the current track (with a short fade, never a
    /// crossfade) and starts the new one (paused stays paused); when stopped, `play` starts it.
    pub fn set_queue(&self, items: Vec<PlayItem>, start_index: usize) {
        self.send(Command::SetQueue { items, start_index });
    }

    /// Replaces every item after the current one without interrupting the current one. If the
    /// following item was already opened for the gapless join or a crossfade and is no longer
    /// next, the prepared audio is discarded and the new first item follows instead, still
    /// seamlessly. An empty list ends the queue after the current item. The same item (same id
    /// and resource) may follow itself, as repeat-one does: it starts again from the beginning
    /// with a fresh decoder and joins sample-contiguously.
    ///
    /// The items up to and including the current one stay in the queue, so indices in events
    /// keep counting from the list given to [`set_queue`](Self::set_queue). With an empty
    /// queue this behaves like `set_queue(items, 0)` without starting playback.
    pub fn set_upcoming(&self, items: Vec<PlayItem>) {
        self.send(Command::SetUpcoming(items));
    }

    /// Like [`set_upcoming`](Self::set_upcoming), but the items follow the item with id `after`
    /// rather than the one audible when the engine gets to the call. A caller that reacts to
    /// [`EngineEvent::TrackStarted`] should use this: by the time the call arrives, playback
    /// may already have moved past that track (very short tracks, offline rendering), and the
    /// list would otherwise be appended after the wrong item. When playback has moved past
    /// every given item, the call is stale and ignored; when `after` is not in the queue, it
    /// behaves like `set_upcoming`.
    pub fn set_upcoming_after(&self, after: u64, items: Vec<PlayItem>) {
        self.send(Command::SetUpcomingAfter { after, items });
    }

    /// Starts or resumes playback. After the queue ended it continues with the item after the
    /// last one played if `set_upcoming` added one, otherwise it starts the queue over.
    pub fn play(&self) {
        self.send(Command::Play);
    }

    /// Pauses, keeping the buffered audio.
    pub fn pause(&self) {
        self.send(Command::Pause);
    }

    /// Stops (fading out over a few milliseconds) and discards buffered audio; `play`
    /// restarts the current item from the start.
    pub fn stop(&self) {
        self.send(Command::Stop);
    }

    /// Skips to the next item; past the last one the queue ends. Never crossfades.
    pub fn next(&self) {
        self.send(Command::Next);
    }

    /// Goes to the previous item (or restarts the first one). Never crossfades.
    pub fn previous(&self) {
        self.send(Command::Previous);
    }

    /// Seeks in the current item, sample-accurately. Past the end, the next item starts.
    /// When stopped, `play` starts from this position.
    pub fn seek(&self, position_ms: u64) {
        self.send(Command::Seek { position_ms });
    }

    /// Output volume, linear, clamped to 0.0–1.0. Applied after the limiter (in the device
    /// callback for device output), ramped over about 5 ms.
    pub fn set_volume(&self, linear: f32) {
        self.send(Command::SetVolume(linear));
    }

    pub fn set_eq(&self, eq: EqSettings) {
        self.send(Command::SetEq(eq));
    }

    pub fn set_replaygain(&self, mode: ReplayGainMode, preamp_db: f32) {
        self.send(Command::SetReplayGain { mode, preamp_db });
    }

    /// Crossfade settings for automatic transitions from now on (`duration_ms` is clamped to
    /// 1000–12000). An overlap that is already playing finishes as it started.
    pub fn set_crossfade(&self, settings: CrossfadeSettings) {
        self.send(Command::SetCrossfade(settings));
    }

    /// Switches the output while playing: the new output opens (at its own sample rate), the
    /// old one fades out, and playback continues from the audible position. If the new output
    /// cannot be opened, an [`EngineEvent::Error`] without an id reports it and the old one
    /// stays.
    pub fn set_output(&self, output: OutputKind) {
        self.send(Command::SetOutput(output));
    }

    /// The system's output devices changed (shells call this from the OS's device
    /// notifications; the engine never polls). When following the default device and the
    /// default is now another device, playback moves to it.
    pub fn devices_changed(&self) {
        self.send(Command::DevicesChanged);
    }

    /// The event stream. Every call returns a handle to the same channel: each event is
    /// delivered once, to whichever handle receives it first, so use one consumer.
    pub fn events(&self) -> Receiver<EngineEvent> {
        self.events.clone()
    }

    /// The most recently published anchor.
    pub fn anchor(&self) -> PlaybackAnchor {
        self.shared.state.lock().anchor
    }

    /// Waits until the engine has handled every command sent before this call and is not
    /// playing (stopped, paused, or past the end of the queue, with the output drained).
    /// Returns `false` on timeout or if the engine thread has stopped.
    pub fn wait_until_idle(&self, timeout: Duration) -> bool {
        let target = *self.sent.lock();
        let deadline = Instant::now() + timeout;
        let mut state = self.shared.state.lock();
        loop {
            if state.exited {
                return false;
            }
            if state.processed >= target && !state.playing {
                return true;
            }
            if self.shared.changed.wait_until(&mut state, deadline).timed_out() {
                return !state.exited && state.processed >= target && !state.playing;
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.send(Command::Shutdown);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("the engine thread panicked");
        }
    }
}

/// Marks the engine thread as gone, also when it unwinds.
struct ExitGuard(Arc<Shared>);

impl Drop for ExitGuard {
    fn drop(&mut self) {
        let mut state = self.0.state.lock();
        state.exited = true;
        state.playing = false;
        self.0.changed.notify_all();
    }
}

fn make_sink(output: &OutputKind, buffer_ms: u32) -> Result<Box<dyn Sink>, EngineError> {
    Ok(match output {
        OutputKind::Device { name } => device_sink(name.as_deref(), buffer_ms)?,
        OutputKind::WavFile {
            path,
            realtime: false,
        } => Box::new(WavSink::new(path, None)?),
        OutputKind::WavFile { path, realtime: true } => Box::new(WavSink::realtime(path, None, buffer_ms)?),
        OutputKind::Null { realtime: false } => Box::new(NullSink::new(None)),
        OutputKind::Null { realtime: true } => Box::new(NullSink::realtime(None, buffer_ms)),
    })
}

#[cfg(feature = "device")]
fn device_sink(name: Option<&str>, buffer_ms: u32) -> Result<Box<dyn Sink>, EngineError> {
    Ok(Box::new(DeviceSink::open(name, buffer_ms)?))
}

#[cfg(not(feature = "device"))]
fn device_sink(_name: Option<&str>, _buffer_ms: u32) -> Result<Box<dyn Sink>, EngineError> {
    Err(EngineError::DeviceSupportDisabled)
}

#[cfg(feature = "device")]
fn default_device_id() -> Option<String> {
    crate::sink::default_device_id()
}

#[cfg(not(feature = "device"))]
fn default_device_id() -> Option<String> {
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transport {
    Stopped,
    Playing,
    Paused,
    /// The queue played to its end.
    Ended,
}

/// A queue item on the output timeline.
#[derive(Debug, Clone)]
struct Segment {
    id: u64,
    /// Stream frame at which the item's first decoded frame plays.
    start: u64,
    /// Where it stops being heard, when that differs from the next segment's start: the end of
    /// a crossfade's fade-out, or an outgoing track that ended early.
    end: Option<u64>,
    index: usize,
    item: PlayItem,
    /// Track frame (at the source rate) of that first frame.
    base_frame: i64,
    source_rate: u32,
    info: TrackInfo,
    /// The fade-in it starts with (a crossfade's incoming side, or a declick).
    fade_in: Option<Fade>,
    /// `TrackStarted` has been sent.
    announced: bool,
    /// Its decoder has produced audio.
    decoded: bool,
}

/// A decoder: a file decoding on the engine thread, or a stream on its own thread.
enum DecodeSource {
    Local(Source),
    Threaded(ThreadedSource),
}

impl DecodeSource {
    fn info(&self) -> &TrackInfo {
        match self {
            DecodeSource::Local(s) => s.info(),
            DecodeSource::Threaded(s) => s.info(),
        }
    }

    fn sample_rate(&self) -> u32 {
        match self {
            DecodeSource::Local(s) => s.sample_rate(),
            DecodeSource::Threaded(s) => s.sample_rate(),
        }
    }

    fn total_frames(&self) -> Option<i64> {
        match self {
            DecodeSource::Local(s) => s.total_frames(),
            DecodeSource::Threaded(s) => s.total_frames(),
        }
    }

    fn next_frame(&self) -> i64 {
        match self {
            DecodeSource::Local(s) => s.next_frame(),
            DecodeSource::Threaded(s) => s.next_frame(),
        }
    }

    fn take_first_frame(&mut self) -> Option<i64> {
        match self {
            DecodeSource::Local(s) => s.take_first_frame(),
            DecodeSource::Threaded(s) => s.take_first_frame(),
        }
    }

    /// Appends decoded audio to `out`; a local file always has some (or ends).
    fn read(&mut self, out: &mut Vec<f32>) -> Result<Read, EngineError> {
        match self {
            DecodeSource::Local(s) => s.read(out).map(|more| if more { Read::Audio } else { Read::End }),
            DecodeSource::Threaded(s) => s.read(out),
        }
    }
}

/// A decoder feeding a pipeline lane. Its segment is found by id.
struct Decoding {
    index: usize,
    item: PlayItem,
    input: Input,
    source: DecodeSource,
    replaygain: f32,
    segment: u64,
    /// When it was put on its lane: a stream gets [`OPEN_GRACE`] to deliver its first block
    /// before a wait counts as buffering.
    since: Instant,
}

/// Why an item is being opened, which decides what happens once it is open.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Purpose {
    /// An explicit start (play, skip, `set_queue`): `TrackStarted` as soon as it is open.
    Start { declick: bool },
    /// The current item again, at a seek position: no `TrackStarted`.
    Seek { declick: bool },
    /// A gapless continuation after the previous track ended.
    Continue,
    /// The next item, opened ahead of its transition.
    Prepare,
    /// The audible item again after a re-render or an output change.
    Rebuild { announced: bool, fade_in: Option<Fade> },
}

type Opened = Result<(DecodeSource, i64), EngineError>;

/// An HTTP item being opened on its decoder thread.
struct Opening {
    index: usize,
    item: PlayItem,
    input: Input,
    purpose: Purpose,
    pending: PendingOpen,
    result: Option<Opened>,
    since: Instant,
}

/// The next item, opened ahead.
enum Prepared {
    Ready {
        index: usize,
        item: PlayItem,
        input: Input,
        source: Box<DecodeSource>,
        start_frame: i64,
    },
    /// It could not be opened (the error was reported): the join skips it.
    Failed { index: usize, item: PlayItem },
}

impl Prepared {
    fn matches(&self, queue: &[PlayItem]) -> bool {
        let (index, item) = match self {
            Prepared::Ready { index, item, .. } | Prepared::Failed { index, item } => (*index, item),
        };
        queue.get(index).is_some_and(|q| q.same_as(item))
    }
}

/// A crossfade into the next item, decided while the current one plays.
#[derive(Debug, Clone, Copy)]
struct Plan {
    next_index: usize,
    /// Stream frame where the current track ends.
    end: u64,
    /// The mixer holds the current track here until the next one is open.
    hold: u64,
}

/// HTTP streams by item, most recently used last.
#[derive(Default)]
struct StreamPool {
    entries: Vec<(PlayItem, HttpStream)>,
}

impl StreamPool {
    fn get(&mut self, item: &PlayItem, options: StreamOptions) -> std::io::Result<HttpStream> {
        let MediaSource::Http(source) = &item.source else {
            return Err(std::io::Error::other("not an HTTP item"));
        };
        if let Some(at) = self
            .entries
            .iter()
            .position(|(i, s)| i.same_as(item) && s.error().is_none())
        {
            let entry = self.entries.remove(at);
            let stream = entry.1.clone();
            self.entries.push(entry);
            return Ok(stream);
        }
        let stream = HttpStream::open(source.clone(), options)?;
        self.entries.push((item.clone(), stream.clone()));
        if self.entries.len() > STREAM_POOL {
            self.entries.remove(0);
        }
        Ok(stream)
    }

    /// Keeps only the streams of items `keep` accepts.
    fn retain(&mut self, keep: impl Fn(&PlayItem) -> bool) {
        self.entries.retain(|(item, _)| keep(item));
    }
}

/// Makes a fresh sink for an engine built from caller-provided sinks.
type SinkFactory = Box<dyn FnMut() -> Result<Box<dyn Sink>, EngineError> + Send>;

struct EngineThread {
    config: EngineConfig,
    /// The output this thread opened and manages (`None` for a caller-provided sink).
    output: Option<OutputKind>,
    /// Where a fresh sink comes from when there is no configured output.
    factory: Option<SinkFactory>,
    /// The output failed for good ([`EngineEvent::OutputFailed`]): the next start reopens it.
    output_failed: bool,
    commands: Receiver<(u64, Command)>,
    events: Sender<EngineEvent>,
    shared: Arc<Shared>,
    sink: Box<dyn Sink>,
    sink_applies_volume: bool,
    volume: f32,
    pipeline: Pipeline,
    queue: Vec<PlayItem>,
    transport: Transport,
    /// The current item when nothing is on the timeline (stopped, ended or opening).
    current: usize,
    /// Where `play` starts the current item when stopped.
    start_position_us: i64,
    /// Position and duration reported while nothing is on the timeline.
    idle_position_us: i64,
    idle_duration_us: Option<i64>,
    /// The main lane's decoder (the incoming track during a crossfade).
    decoder: Option<Decoding>,
    /// The outgoing track during a crossfade.
    fading: Option<Decoding>,
    openings: Vec<Opening>,
    prepared: Option<Prepared>,
    plan: Option<Plan>,
    crossfade: CrossfadeSettings,
    streams: StreamPool,
    /// The end of the queue has been decided and the chain flushed.
    end_committed: bool,
    segments: VecDeque<Segment>,
    next_segment_id: u64,
    /// Processed audio waiting to be written to the sink, and how much of it is written.
    out: Vec<f32>,
    out_pos: usize,
    decoded: Vec<f32>,
    mapped: Vec<f32>,
    /// Frames written to the sink: the stream frame of the next one.
    written: u64,
    generation: u64,
    replaygain_mode: ReplayGainMode,
    replaygain_preamp_db: f32,
    eq: EqSettings,
    last_anchor: PlaybackAnchor,
    last_anchor_check: Instant,
    /// The last anchor was published with the clock stopped because the output had not
    /// started taking audio yet.
    output_waiting: bool,
    /// Drift is checked often until then (see [`SETTLE_TIME`]).
    settle_until: Instant,
    reported_underruns: u64,
    /// The item whose stream ran dry while the output played out (reported as buffering).
    buffering: Option<u64>,
    /// The sink has played out everything written (set while waiting for data).
    sink_dry: bool,
    /// Nothing has been written to the sink yet: it may still take the first track's rate.
    sink_fresh: bool,
    /// The next start fades in (a manual transition faded the previous audio out).
    declick_next: bool,
}

impl EngineThread {
    #[allow(clippy::too_many_arguments)]
    fn new(
        mut sink: Box<dyn Sink>,
        output: Option<OutputKind>,
        factory: Option<SinkFactory>,
        config: EngineConfig,
        commands: Receiver<(u64, Command)>,
        events: Sender<EngineEvent>,
        shared: Arc<Shared>,
    ) -> Self {
        sink.set_paused(true);
        let sink_applies_volume = sink.set_volume(1.0);
        let eq = EqSettings::default();
        Self {
            config,
            output,
            factory,
            output_failed: false,
            commands,
            events,
            shared,
            pipeline: Pipeline::new(sink.sample_rate(), &eq),
            sink,
            sink_applies_volume,
            volume: 1.0,
            queue: Vec::new(),
            transport: Transport::Stopped,
            current: 0,
            start_position_us: 0,
            idle_position_us: 0,
            idle_duration_us: None,
            decoder: None,
            fading: None,
            openings: Vec::new(),
            prepared: None,
            plan: None,
            crossfade: CrossfadeSettings::default(),
            streams: StreamPool::default(),
            end_committed: false,
            segments: VecDeque::new(),
            next_segment_id: 0,
            out: Vec::new(),
            out_pos: 0,
            decoded: Vec::new(),
            mapped: Vec::new(),
            written: 0,
            generation: 0,
            replaygain_mode: ReplayGainMode::Off,
            replaygain_preamp_db: 0.0,
            eq,
            last_anchor: PlaybackAnchor::default(),
            last_anchor_check: Instant::now(),
            output_waiting: false,
            settle_until: Instant::now(),
            reported_underruns: 0,
            buffering: None,
            sink_dry: false,
            declick_next: false,
            sink_fresh: true,
        }
    }

    fn run(mut self) {
        loop {
            loop {
                match self.commands.try_recv() {
                    Ok((seq, command)) => {
                        if !self.receive(seq, command) {
                            return;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            if self.transport == Transport::Playing {
                self.work();
            } else {
                match self.commands.recv() {
                    Ok((seq, command)) => {
                        if !self.receive(seq, command) {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        }
    }

    /// Handles one command and publishes that it was handled. Returns `false` on shutdown.
    fn receive(&mut self, seq: u64, command: Command) -> bool {
        let keep_running = self.handle(command);
        let mut state = self.shared.state.lock();
        state.processed = state.processed.max(seq);
        state.playing = self.transport == Transport::Playing;
        self.shared.changed.notify_all();
        keep_running
    }

    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::SetQueue { items, start_index } => self.set_queue(items, start_index),
            Command::SetUpcoming(items) => self.set_upcoming(items),
            Command::SetUpcomingAfter { after, items } => self.set_upcoming_after(after, items),
            Command::Play => self.play(),
            Command::Pause => {
                if self.transport == Transport::Playing {
                    self.set_transport(Transport::Paused);
                    self.publish_anchor();
                }
            }
            Command::Stop => self.stop(),
            Command::Next => self.skip(true),
            Command::Previous => self.skip(false),
            Command::Seek { position_ms } => self.seek(position_ms),
            Command::SetVolume(volume) => {
                if volume.is_finite() {
                    self.volume = volume.clamp(0.0, 1.0);
                    if self.sink_applies_volume {
                        self.sink.set_volume(self.volume);
                    } else {
                        self.pipeline.set_volume(self.volume);
                    }
                }
            }
            Command::SetEq(eq) => {
                self.pipeline.set_eq(&eq);
                self.eq = eq;
                self.update_limiter();
            }
            Command::SetReplayGain { mode, preamp_db } => {
                self.replaygain_mode = mode;
                self.replaygain_preamp_db = preamp_db;
                for decoding in [self.decoder.as_mut(), self.fading.as_mut()]
                    .into_iter()
                    .flatten()
                {
                    decoding.replaygain =
                        replaygain_scale(decoding.item.replaygain.as_ref(), mode, preamp_db);
                }
                self.update_limiter();
            }
            Command::SetCrossfade(settings) => {
                let duration_ms = settings.duration_ms.clamp(
                    *CrossfadeSettings::DURATION_RANGE_MS.start(),
                    *CrossfadeSettings::DURATION_RANGE_MS.end(),
                );
                self.crossfade = CrossfadeSettings {
                    duration_ms,
                    ..settings
                };
                self.replan();
            }
            Command::SetOutput(output) => self.set_output(output),
            Command::DevicesChanged => self.devices_changed(),
            Command::Shutdown => return false,
        }
        true
    }

    // ----- commands -------------------------------------------------------------------------

    fn set_queue(&mut self, items: Vec<PlayItem>, start_index: usize) {
        let transport = self.transport;
        self.cut();
        self.streams.retain(|item| items.iter().any(|i| i.same_as(item)));
        self.queue = items;
        self.start_position_us = 0;
        self.set_idle_position(0, None);
        if self.queue.is_empty() {
            self.finish_fade();
            self.current = 0;
            self.generation += 1;
            self.set_transport(Transport::Stopped);
            self.publish_anchor();
            return;
        }
        self.current = start_index.min(self.queue.len() - 1);
        match transport {
            Transport::Playing | Transport::Paused => {
                let declick = self.take_declick();
                if !self.start_at(self.current, 0, declick) {
                    self.end_now();
                }
            }
            Transport::Stopped | Transport::Ended => {
                self.declick_next = false;
                self.generation += 1;
                self.set_transport(Transport::Stopped);
                self.publish_anchor();
            }
        }
    }

    fn set_upcoming(&mut self, items: Vec<PlayItem>) {
        if self.queue.is_empty() {
            self.queue = items;
            self.current = 0;
            self.start_position_us = 0;
            return;
        }
        let current = self.current_index().min(self.queue.len() - 1);
        self.splice_upcoming(current, items);
    }

    /// `set_upcoming` relative to the item `after` (its latest place in the queue).
    fn set_upcoming_after(&mut self, after: u64, items: Vec<PlayItem>) {
        let Some(at) = self.queue.iter().rposition(|item| item.id == after) else {
            self.set_upcoming(items);
            return;
        };
        let audible = self.current_index().min(self.queue.len() - 1);
        if at < audible {
            // Playback already moved on from `after`. Still right if what plays now is what
            // the caller lists next; otherwise the caller hears of the newer track and
            // sends a newer list.
            let offset = audible - at;
            let consistent = items
                .get(offset - 1)
                .is_some_and(|item| item.same_as(&self.queue[audible]));
            if !consistent {
                return;
            }
        }
        self.splice_upcoming(at, items);
    }

    /// Replaces what follows queue index `current` with `items`.
    fn splice_upcoming(&mut self, current: usize, items: Vec<PlayItem>) {
        // Streams stay open for the audible item and the one after it.
        let audible = self.current_index().clamp(current, self.queue.len() - 1);
        let mut queue = self.queue[..=current].to_vec();
        queue.extend(items);
        if !matches!(self.transport, Transport::Playing | Transport::Paused) {
            self.queue = queue;
            return;
        }
        // Audio of later items that is already on its way to the output must still be right,
        // and so must an already decided end of the queue.
        let mixed = self.pipeline.mix_pos();
        let committed_ok = self
            .segments
            .iter()
            .filter(|s| s.index > current && s.start < mixed)
            .all(|s| queue.get(s.index).is_some_and(|item| item.same_as(&s.item)));
        let last_open = self.segments.back().map_or(current, |s| s.index);
        let end_ok = !self.end_committed || queue.len() <= last_open + 1;
        self.queue = queue;
        let keep: Vec<PlayItem> =
            self.queue[current.min(self.queue.len())..self.queue.len().min(audible + 2)].to_vec();
        self.streams.retain(|item| keep.iter().any(|k| k.same_as(item)));
        if !(committed_ok && end_ok) {
            self.rerender();
            return;
        }
        self.drop_stale_next();
        self.replan();
    }

    /// Drops what was opened for a next item that no longer follows.
    fn drop_stale_next(&mut self) {
        if self.prepared.as_ref().is_some_and(|p| !p.matches(&self.queue)) {
            self.prepared = None;
        }
        let mixed = self.pipeline.mix_pos();
        // An incoming crossfade lane nothing of which was mixed yet is cancelled.
        if let Some(decoding) = &self.decoder
            && self.fading.is_some()
            && !self
                .queue
                .get(decoding.index)
                .is_some_and(|i| i.same_as(&decoding.item))
            && self.pipeline.cancel_incoming()
        {
            let stale = decoding.segment;
            self.segments.retain(|s| s.id != stale);
            self.decoder = self.fading.take();
            if let Some(outgoing) = self.decoder.as_ref().map(|d| d.segment) {
                for segment in self.segments.iter_mut().filter(|s| s.id == outgoing) {
                    segment.end = None;
                }
            }
            self.update_limiter();
            return;
        }
        // A decoder opened for the join that has not produced audible audio is replaced.
        let stale = self.decoder.as_ref().filter(|d| {
            d.index > self.current_index()
                && self
                    .segments
                    .back()
                    .is_some_and(|s| s.id == d.segment && s.start >= mixed)
                && !self.queue.get(d.index).is_some_and(|item| item.same_as(&d.item))
        });
        if let Some(index) = stale.map(|d| d.index) {
            self.decoder = None;
            self.segments.pop_back();
            self.continue_from(index);
            return;
        }
        // An item opening for a continuation that no longer follows: open the new one instead.
        let stale_open = self.openings.iter().position(|o| {
            matches!(o.purpose, Purpose::Continue | Purpose::Prepare)
                && !self.queue.get(o.index).is_some_and(|item| item.same_as(&o.item))
        });
        if let Some(at) = stale_open {
            let opening = self.openings.remove(at);
            if opening.purpose == Purpose::Continue {
                self.continue_from(opening.index);
            }
        }
    }

    fn play(&mut self) {
        match self.transport {
            Transport::Playing => {}
            Transport::Paused => {
                self.set_transport(Transport::Playing);
                self.publish_anchor();
            }
            Transport::Stopped | Transport::Ended => {
                if self.queue.is_empty() {
                    return;
                }
                if self.output_failed && !self.reopen_output() {
                    return;
                }
                let (index, position_us) = if self.transport == Transport::Ended {
                    let next = self.current + 1;
                    (if next < self.queue.len() { next } else { 0 }, 0)
                } else {
                    (self.current.min(self.queue.len() - 1), self.start_position_us)
                };
                self.reset_playback();
                self.set_transport(Transport::Playing);
                if !self.start_at(index, position_us, false) {
                    self.end_now();
                }
            }
        }
    }

    fn stop(&mut self) {
        self.current = self.current_index();
        self.cut();
        self.finish_fade();
        self.start_position_us = 0;
        self.set_idle_position(0, None);
        self.generation += 1;
        self.set_transport(Transport::Stopped);
        self.publish_anchor();
    }

    fn skip(&mut self, forward: bool) {
        if self.queue.is_empty() {
            return;
        }
        let current = self.current_index();
        let target = if forward {
            current + 1
        } else {
            current.saturating_sub(1)
        };
        if target >= self.queue.len() {
            self.cut();
            self.current = self.queue.len() - 1;
            self.end_now();
            return;
        }
        match self.transport {
            Transport::Playing | Transport::Paused => {
                self.cut();
                let declick = self.take_declick();
                if !self.start_at(target, 0, declick) {
                    self.end_now();
                }
            }
            Transport::Stopped | Transport::Ended => {
                self.current = target;
                self.start_position_us = 0;
                self.set_idle_position(0, None);
                self.generation += 1;
                self.set_transport(Transport::Stopped);
                self.publish_anchor();
            }
        }
    }

    fn seek(&mut self, position_ms: u64) {
        let position_us = i64::try_from(position_ms.saturating_mul(1000)).unwrap_or(i64::MAX);
        match self.transport {
            Transport::Playing | Transport::Paused => {
                let index = self.current_index();
                self.cut();
                let declick = self.take_declick();
                self.generation += 1;
                self.current = index;
                self.set_idle_position(position_us, None);
                let Some(item) = self.queue.get(index).cloned() else {
                    self.end_now();
                    return;
                };
                if !self.open_item(
                    index,
                    item,
                    StartAt::Micros(position_us),
                    Purpose::Seek { declick },
                ) {
                    if !self.start_at(index + 1, 0, declick) {
                        self.end_now();
                    }
                } else if self.segments.is_empty() {
                    // Opening on its own thread: show the target position, not moving yet.
                    self.publish_anchor();
                }
            }
            Transport::Stopped | Transport::Ended => {
                self.start_position_us = position_us;
                self.set_idle_position(position_us, None);
                self.generation += 1;
                self.set_transport(Transport::Stopped);
                self.publish_anchor();
            }
        }
    }

    fn set_output(&mut self, output: OutputKind) {
        match make_sink(&output, self.config.buffer_ms) {
            Ok(sink) => {
                tracing::info!("switching output to {output:?}");
                self.config.output = output.clone();
                self.output = Some(output);
                let old_works = !self.output_failed;
                self.output_failed = false;
                self.replace_sink(sink, old_works);
            }
            Err(err) => self.emit_error(None, &err),
        }
    }

    /// Whether the output is the system default device, which the engine follows.
    fn follows_default(&self) -> bool {
        matches!(self.output, Some(OutputKind::Device { name: None }))
    }

    fn devices_changed(&mut self) {
        if !self.follows_default() {
            return;
        }
        let Some(current) = self.sink.device_id() else {
            return;
        };
        match default_device_id() {
            Some(default) if default != current => match device_sink(None, self.config.buffer_ms) {
                Ok(sink) => {
                    tracing::info!("the default output changed to {default}; following it");
                    self.replace_sink(sink, true);
                }
                Err(err) => self.emit_error(None, &err),
            },
            _ => {}
        }
    }

    /// The output failed while playing and could not be reopened: stop where the listener
    /// stopped hearing, keep that position, and reopen the output on the next start.
    fn fail_output(&mut self, err: &EngineError) {
        tracing::warn!("audio output failed: {err}");
        let index = self.current_index();
        let anchor = self.current_anchor();
        self.reset_playback();
        self.declick_next = false;
        self.output_failed = true;
        self.current = index;
        self.start_position_us = anchor.position_us.max(0);
        self.set_idle_position(self.start_position_us, anchor.duration_us);
        self.generation += 1;
        self.set_transport(Transport::Stopped);
        self.emit(EngineEvent::OutputFailed {
            message: err.to_string(),
        });
        self.publish_anchor();
    }

    /// Opens a fresh sink after the output failed: the configured output, or the factory's
    /// next sink. Reports [`EngineEvent::OutputFailed`] again when that fails too.
    fn reopen_output(&mut self) -> bool {
        let opened = match (&self.output, self.factory.as_mut()) {
            (Some(output), _) => make_sink(output, self.config.buffer_ms),
            (None, Some(factory)) => factory(),
            (None, None) => Err(EngineError::Device("no output to reopen".into())),
        };
        match opened {
            Ok(sink) => {
                tracing::info!("reopened the output after a failure");
                self.output_failed = false;
                self.replace_sink(sink, false);
                true
            }
            Err(err) => {
                self.emit(EngineEvent::OutputFailed {
                    message: err.to_string(),
                });
                false
            }
        }
    }

    /// The default output failed: try to open the default again a few times.
    fn reopen_default_output(&mut self) -> bool {
        for delay in REOPEN_DELAYS_MS {
            std::thread::sleep(Duration::from_millis(delay));
            match device_sink(None, self.config.buffer_ms) {
                Ok(sink) => {
                    tracing::info!("reopened the default output after a failure");
                    self.replace_sink(sink, false);
                    return true;
                }
                Err(err) => tracing::debug!("reopening the default output: {err}"),
            }
        }
        false
    }

    /// Moves playback to `sink`: the old one fades out (when it still works and audio plays)
    /// and is closed; the chain is rebuilt at the new rate and playback continues from the
    /// audible position.
    fn replace_sink(&mut self, sink: Box<dyn Sink>, old_works: bool) {
        let playing = self.transport == Transport::Playing;
        let declick = declick_frames(self.sink.sample_rate());
        let discarded = if old_works && playing {
            let discarded = self.sink.flush_faded(declick);
            let _ = self.sink.drain(Duration::from_millis(DECLICK_MS * 3));
            discarded
        } else {
            self.sink.flush()
        };
        let resume = self.written.saturating_sub(discarded);
        let old_rate = self.sink.sample_rate();
        self.sink = sink;
        self.sink.set_paused(true);
        self.sink_applies_volume = self.sink.set_volume(self.volume);
        self.pipeline.set_volume(if self.sink_applies_volume {
            1.0
        } else {
            self.volume
        });
        self.reported_underruns = 0;
        self.output_waiting = false;
        self.sink_dry = false;
        self.sink_fresh = true;
        self.pipeline.set_out_rate(self.sink.sample_rate(), self.written);
        self.sink.set_paused(self.transport != Transport::Playing);
        if matches!(self.transport, Transport::Playing | Transport::Paused) {
            self.rebuild_from(resume, old_rate, playing);
        }
        self.publish_anchor();
    }

    // ----- playback -------------------------------------------------------------------------

    /// One step of playback: write a chunk, finish an opening, decode a packet, wait for a
    /// stream, or wait for the output to drain.
    fn work(&mut self) {
        if self.out_pos < self.out.len() {
            self.write_chunk();
            return;
        }
        self.out.clear();
        self.out_pos = 0;
        self.poll_openings();
        if !self.out.is_empty() {
            return;
        }
        if self.end_committed {
            self.drain_step();
            return;
        }
        if let Some(id) = self.main_opening() {
            // Nothing can play on the main lane until its item is open.
            if self.fading.is_some() {
                self.decode_or_wait(LaneId::Fading);
            } else {
                self.wait_for_stream(id);
            }
            return;
        }
        match self.next_lane() {
            Some(lane) => self.decode_or_wait(lane),
            None => self.commit_end(),
        }
    }

    /// The lane to decode next, `None` when no decoder is left.
    fn next_lane(&mut self) -> Option<LaneId> {
        if let Some(fading) = &self.fading {
            let cut = self
                .segments
                .iter()
                .find(|s| s.id == fading.segment)
                .and_then(|s| s.end);
            let produced = self.pipeline.produced_until(LaneId::Fading);
            if cut.is_some_and(|cut| produced >= cut) {
                // The outgoing track reached the end of its fade-out.
                self.fading = None;
                if let Err(err) = self.pipeline.finish_lane(LaneId::Fading, &mut self.out) {
                    tracing::warn!("finishing the outgoing track: {err}");
                }
                self.update_limiter();
            } else if self.decoder.is_none() || produced <= self.pipeline.produced_until(LaneId::Main) {
                return Some(LaneId::Fading);
            }
        }
        if let Some(plan) = self.plan
            && self.decoder.is_some()
            && self.pipeline.produced_until(LaneId::Main) >= plan.hold
            && self.fading.is_none()
        {
            // A crossfade that just ended may have kept the next item from being prepared.
            self.maybe_prepare();
            self.resolve_plan();
        }
        self.decoder.as_ref().map(|_| LaneId::Main)
    }

    fn decode_or_wait(&mut self, lane: LaneId) {
        let decoding = match lane {
            LaneId::Main => self.decoder.as_ref(),
            LaneId::Fading => self.fading.as_ref(),
        };
        let Some(decoding) = decoding else {
            return;
        };
        if let DecodeSource::Threaded(source) = &decoding.source {
            let ready = if self.buffering.is_some() {
                // Resume with a cushion, so a stream slower than playback does not stutter:
                // a full decoder queue and enough downloaded behind it (or the end in sight).
                let cushion = match &decoding.input {
                    Input::Http(stream) => stream.ready_to_resume(),
                    Input::File(_) => true,
                };
                source.finished() || (source.full() && cushion)
            } else {
                source.ready()
            };
            if !ready {
                let id = decoding.item.id;
                self.wait_for_stream(id);
                return;
            }
        }
        self.end_buffering();
        self.decode_step(lane);
    }

    fn write_chunk(&mut self) {
        let end = (self.out_pos + CHUNK_FRAMES * 2).min(self.out.len());
        let frames = ((end - self.out_pos) / 2) as u64;
        let channels = usize::from(self.sink.channels());
        let result = if channels == 2 {
            self.sink.write(&self.out[self.out_pos..end])
        } else {
            self.mapped.clear();
            map_channels(&self.out[self.out_pos..end], channels, &mut self.mapped);
            self.sink.write(&self.mapped)
        };
        self.out_pos = end;
        match result {
            Ok(()) => {
                self.written += frames;
                self.sink_fresh = false;
                self.sink_dry = false;
                self.update_audible();
                if self.output_waiting && self.sink.output_started() {
                    // The output took its first samples: start the clock from here.
                    self.publish_anchor();
                    self.settle_until = Instant::now() + SETTLE_TIME;
                } else {
                    self.maybe_reanchor();
                }
            }
            Err(err) => {
                if self.follows_default() && self.reopen_default_output() {
                    return;
                }
                self.fail_output(&err);
            }
        }
    }

    fn decode_step(&mut self, lane: LaneId) {
        let decoding = match lane {
            LaneId::Main => self.decoder.as_mut(),
            LaneId::Fading => self.fading.as_mut(),
        };
        let Some(decoding) = decoding else {
            return;
        };
        self.decoded.clear();
        let id = decoding.item.id;
        let segment_id = decoding.segment;
        match decoding.source.read(&mut self.decoded) {
            Ok(Read::Pending) => self.wait_for_stream(id),
            Ok(Read::Audio) => {
                let first_frame = decoding.source.take_first_frame();
                let gain = decoding.replaygain;
                if gain != 1.0 {
                    self.decoded.iter_mut().for_each(|s| *s *= gain);
                }
                if let Some(segment) = self.segments.iter_mut().find(|s| s.id == segment_id)
                    && !segment.decoded
                {
                    segment.decoded = true;
                    if let Some(frame) = first_frame {
                        // Where decoding really started (a coarse seek may land elsewhere).
                        segment.base_frame = frame;
                    }
                }
                let was_crossfading = self.pipeline.crossfading();
                if let Err(err) = self.pipeline.push(lane, &self.decoded, &mut self.out) {
                    self.emit_error(Some(id), &err);
                    self.end_of_track(lane);
                    return;
                }
                if was_crossfading && !self.pipeline.crossfading() {
                    self.update_limiter();
                }
                if lane == LaneId::Main {
                    self.maybe_prepare();
                }
            }
            Ok(Read::End) => self.end_of_track(lane),
            Err(err) => {
                self.emit_error(Some(id), &err);
                self.end_of_track(lane);
            }
        }
    }

    /// A decoder ran out.
    fn end_of_track(&mut self, lane: LaneId) {
        match lane {
            LaneId::Main => {
                // Continue gaplessly with the next item that opens (a planned crossfade that
                // did not start is moot now).
                self.plan = None;
                self.pipeline.set_hold(None, &mut self.out);
                if let Some(finished) = self.decoder.take() {
                    // The pipeline continues the lane, so the next segment starts right where
                    // this one ends.
                    self.continue_from(finished.index + 1);
                }
            }
            LaneId::Fading => {
                if let Some(finished) = self.fading.take() {
                    if let Err(err) = self.pipeline.finish_lane(LaneId::Fading, &mut self.out) {
                        tracing::warn!("finishing the outgoing track: {err}");
                    }
                    let end = self.pipeline.produced_until(LaneId::Fading);
                    let incoming = self.decoder.as_ref().map(|d| d.segment);
                    let incoming_start = incoming
                        .and_then(|id| self.segments.iter().find(|s| s.id == id))
                        .map(|s| s.start);
                    if let (Some(id), Some(start)) = (incoming, incoming_start)
                        && end < start
                    {
                        // It ended before the overlap: join the incoming track right there.
                        self.pipeline.join_incoming_at(end);
                        for segment in self.segments.iter_mut().filter(|s| s.id == id) {
                            segment.start = end;
                            segment.fade_in = None;
                        }
                    }
                    let cut = self
                        .segments
                        .iter()
                        .find(|s| s.id == finished.segment)
                        .and_then(|s| s.end);
                    self.set_segment_end(finished.segment, cut.map_or(end, |cut| cut.min(end)));
                    self.update_limiter();
                }
            }
        }
    }

    fn set_segment_end(&mut self, id: u64, end: u64) {
        for segment in self.segments.iter_mut().filter(|s| s.id == id) {
            segment.end = Some(end.max(segment.start));
        }
    }

    /// Opens the first item from `index` on that can be opened, joining it to what is already
    /// in the chain; with none left, the end of the queue follows once the chain has played.
    fn continue_from(&mut self, mut index: usize) {
        while let Some(item) = self.queue.get(index).cloned() {
            match self.take_prepared(index, &item) {
                Some(Prepared::Ready {
                    index,
                    item,
                    input,
                    source,
                    start_frame,
                }) => {
                    if self.install(index, item, input, *source, start_frame, Purpose::Continue) {
                        return;
                    }
                }
                Some(Prepared::Failed { .. }) => {}
                None => {
                    if let Some(opening) = self
                        .openings
                        .iter_mut()
                        .find(|o| o.index == index && o.purpose == Purpose::Prepare && o.item.same_as(&item))
                    {
                        opening.purpose = Purpose::Continue;
                        return;
                    }
                    if self.open_item(index, item, StartAt::Frame(0), Purpose::Continue) {
                        return;
                    }
                }
            }
            index += 1;
        }
    }

    fn take_prepared(&mut self, index: usize, item: &PlayItem) -> Option<Prepared> {
        let matches = match &self.prepared {
            Some(
                Prepared::Ready {
                    index: i, item: p, ..
                }
                | Prepared::Failed { index: i, item: p },
            ) => *i == index && p.same_as(item),
            None => false,
        };
        if matches { self.prepared.take() } else { None }
    }

    /// Starts `index` (or the first item after it that opens) from `position_us`, announcing
    /// it as soon as it is open. Returns `false` when nothing from `index` on can be played.
    fn start_at(&mut self, mut index: usize, mut position_us: i64, declick: bool) -> bool {
        while let Some(item) = self.queue.get(index).cloned() {
            self.current = index;
            if self.open_item(
                index,
                item,
                StartAt::Micros(position_us),
                Purpose::Start { declick },
            ) {
                if self.segments.is_empty() {
                    // Opening on its own thread: show the new item, not moving yet.
                    self.generation += 1;
                    self.set_idle_position(position_us, None);
                    self.publish_anchor();
                }
                return true;
            }
            index += 1;
            position_us = 0;
        }
        false
    }

    /// What to decode `item` from: its file, or its HTTP stream (started now or reused).
    fn input_for(&mut self, item: &PlayItem) -> Result<Input, EngineError> {
        match &item.source {
            MediaSource::File(path) => Ok(Input::File(path.clone())),
            MediaSource::Http(source) => {
                let options = StreamOptions {
                    id: item.id,
                    cache_dir: self.config.cache_dir.clone(),
                    resolver: self.config.resolver.clone(),
                };
                self.streams
                    .get(item, options)
                    .map(Input::Http)
                    .map_err(|err| EngineError::Stream {
                        url: source.redacted_url().to_string(),
                        message: err.to_string(),
                    })
            }
        }
    }

    /// Opens `item` for `purpose`. Files open here and now; HTTP items open on their own
    /// thread and finish in [`Self::poll_openings`]. Returns `false` when the item failed (the
    /// failure is reported and handled), `true` when it is open or opening.
    fn open_item(&mut self, index: usize, item: PlayItem, start: StartAt, purpose: Purpose) -> bool {
        let input = match self.input_for(&item) {
            Ok(input) => input,
            Err(err) => {
                self.emit_error(Some(item.id), &err);
                return self.open_failed(index, item, purpose);
            }
        };
        if let Input::Http(_) = input {
            let pending = worker::spawn(input.clone(), start);
            self.openings.push(Opening {
                index,
                item,
                input,
                purpose,
                pending,
                result: None,
                since: Instant::now(),
            });
            return true;
        }
        let opened = worker::open_source(&input, start).map(|(s, frame)| (DecodeSource::Local(s), frame));
        self.finish_open(index, item, input, purpose, opened)
    }

    /// Finishes openings whose result arrived.
    fn poll_openings(&mut self) {
        let mut at = 0;
        while at < self.openings.len() {
            let opening = &mut self.openings[at];
            if opening.result.is_none() {
                match opening.pending.try_finish() {
                    Some(result) => {
                        opening.result = Some(result.map(|(s, frame)| (DecodeSource::Threaded(s), frame)))
                    }
                    None => {
                        at += 1;
                        continue;
                    }
                }
            }
            let opening = self.openings.remove(at);
            let result = opening.result.expect("result just set");
            let (index, purpose) = (opening.index, opening.purpose);
            if !self.finish_open(index, opening.item, opening.input, purpose, result) {
                self.continue_after_failure(index, purpose);
            }
        }
    }

    /// The item an opening for the main lane waits for.
    fn main_opening(&self) -> Option<u64> {
        self.openings
            .iter()
            .find(|o| o.purpose != Purpose::Prepare)
            .map(|o| o.item.id)
    }

    fn finish_open(
        &mut self,
        index: usize,
        item: PlayItem,
        input: Input,
        purpose: Purpose,
        opened: Opened,
    ) -> bool {
        match opened {
            Ok((source, start_frame)) => {
                if purpose == Purpose::Prepare {
                    self.prepared = Some(Prepared::Ready {
                        index,
                        item,
                        input,
                        source: Box::new(source),
                        start_frame,
                    });
                    return true;
                }
                if self.install(index, item.clone(), input, source, start_frame, purpose) {
                    true
                } else {
                    self.open_failed(index, item, purpose)
                }
            }
            Err(err) => {
                self.emit_error(Some(item.id), &err);
                self.open_failed(index, item, purpose)
            }
        }
    }

    /// An item could not be opened (already reported). A prepared item is remembered as failed so
    /// the join skips it; everything else is up to the caller. Returns `false`.
    fn open_failed(&mut self, index: usize, item: PlayItem, purpose: Purpose) -> bool {
        if purpose == Purpose::Prepare {
            self.prepared = Some(Prepared::Failed { index, item });
        }
        false
    }

    /// An item that opened on its own thread failed: carry on with the following one.
    fn continue_after_failure(&mut self, index: usize, purpose: Purpose) {
        match purpose {
            Purpose::Start { declick } | Purpose::Seek { declick } => {
                if !self.start_at(index + 1, 0, declick) {
                    self.end_now();
                }
            }
            Purpose::Continue | Purpose::Rebuild { .. } => self.continue_from(index + 1),
            Purpose::Prepare => {}
        }
    }

    /// Puts an opened source on the main lane, right after whatever is in the chain.
    fn install(
        &mut self,
        index: usize,
        item: PlayItem,
        input: Input,
        source: DecodeSource,
        start_frame: i64,
        purpose: Purpose,
    ) -> bool {
        let rate = source.sample_rate();
        let explicit = matches!(purpose, Purpose::Start { .. } | Purpose::Seek { .. });
        if explicit && self.sink_fresh && self.out.is_empty() {
            // A sink that has not started yet may take the first track's rate.
            self.sink.prefer_sample_rate(rate);
            let sink_rate = self.sink.sample_rate();
            if sink_rate != self.pipeline.out_rate() {
                self.pipeline.set_out_rate(sink_rate, self.written);
            }
        }
        if let Err(err) = self.pipeline.prepare_input(LaneId::Main, rate, &mut self.out) {
            self.emit_error(Some(item.id), &err);
            return false;
        }
        if let (Input::Http(stream), Some(duration_us), Some(total)) =
            (&input, source.info().duration_us, input_len(&input))
            && duration_us > 0
        {
            stream.set_byte_rate(total.saturating_mul(1_000_000) / duration_us.unsigned_abs());
        }
        let replaygain = replaygain_scale(
            item.replaygain.as_ref(),
            self.replaygain_mode,
            self.replaygain_preamp_db,
        );
        let start = self.pipeline.next_input_index(LaneId::Main);
        let fade_in = match purpose {
            Purpose::Start { declick: true } | Purpose::Seek { declick: true } => Some(Fade {
                start: to_i64(start),
                len: declick_frames(self.pipeline.out_rate()),
            }),
            Purpose::Rebuild { fade_in, .. } => fade_in,
            _ => None,
        };
        if let Some(fade) = fade_in {
            self.pipeline.fade_in_main(fade);
        }
        let id = self.next_segment_id;
        self.next_segment_id += 1;
        let announced = match purpose {
            Purpose::Start { .. } | Purpose::Seek { .. } => true,
            Purpose::Rebuild { announced, .. } => announced,
            Purpose::Continue | Purpose::Prepare => false,
        };
        let info = source.info().clone();
        self.segments.push_back(Segment {
            id,
            start,
            end: None,
            index,
            item: item.clone(),
            base_frame: start_frame.max(0),
            source_rate: rate,
            info: info.clone(),
            fade_in,
            announced,
            decoded: false,
        });
        self.decoder = Some(Decoding {
            index,
            item: item.clone(),
            input,
            source,
            replaygain,
            segment: id,
            since: Instant::now(),
        });
        self.update_limiter();
        match purpose {
            Purpose::Start { .. } => {
                self.current = index;
                self.generation += 1;
                self.emit(EngineEvent::TrackStarted {
                    index,
                    id: item.id,
                    info,
                });
                self.publish_anchor();
            }
            Purpose::Seek { .. } => {
                self.current = index;
                self.publish_anchor();
            }
            Purpose::Rebuild { .. } => {
                self.current = index;
            }
            Purpose::Continue | Purpose::Prepare => {}
        }
        self.prefetch_next(index);
        self.replan();
        true
    }

    /// Starts downloading the next item when it is an HTTP one, so its join does not wait.
    fn prefetch_next(&mut self, index: usize) {
        if let Some(next) = self.queue.get(index + 1).cloned()
            && matches!(next.source, MediaSource::Http(_))
        {
            let _ = self.input_for(&next);
        }
    }

    /// Opens the next item ahead of its transition when it is time.
    fn maybe_prepare(&mut self) {
        if self.fading.is_some()
            || self.prepared.is_some()
            || self.openings.iter().any(|o| o.purpose == Purpose::Prepare)
        {
            return;
        }
        let Some(decoding) = &self.decoder else {
            return;
        };
        let next_index = decoding.index + 1;
        let Some(next) = self.queue.get(next_index).cloned() else {
            return;
        };
        let http = matches!(next.source, MediaSource::Http(_));
        let lead = if http {
            HTTP_PREPARE_LEAD_SECS * u64::from(self.pipeline.out_rate())
        } else {
            0
        };
        let at = match self.plan {
            Some(plan) => plan.hold.saturating_sub(lead),
            // Files open at the join itself; HTTP items ahead of it.
            None if http => match self.track_end(decoding) {
                Some(end) => end.saturating_sub(lead),
                None => return,
            },
            None => return,
        };
        if self.pipeline.produced_until(LaneId::Main) >= at {
            self.open_item(next_index, next, StartAt::Frame(0), Purpose::Prepare);
        }
    }

    /// Stream frame where `decoding`'s track ends, when its length is known.
    fn track_end(&self, decoding: &Decoding) -> Option<u64> {
        let total = decoding.source.total_frames()?;
        let left = u64::try_from(total - decoding.source.next_frame()).ok()?;
        Some(
            self.pipeline
                .index_after(LaneId::Main, decoding.source.sample_rate(), left),
        )
    }

    /// Decides whether the transition after the current track crossfades, and holds the mixer
    /// where the overlap could start at the earliest.
    fn replan(&mut self) {
        self.plan = None;
        let hold = self.compute_plan();
        self.plan = hold;
        self.pipeline.set_hold(hold.map(|p| p.hold), &mut self.out);
    }

    /// The plan for the main decoder's track. During a crossfade that is the incoming track: its
    /// own transition holds at or after the running overlap's end (each overlap takes at most
    /// half of a track), so the two never overlap.
    fn compute_plan(&self) -> Option<Plan> {
        if !self.crossfade.enabled {
            return None;
        }
        let decoding = self.decoder.as_ref()?;
        let next_index = decoding.index + 1;
        let next = self.queue.get(next_index)?;
        if self.crossfade.album_continuity && decoding.item.same_album(next) {
            return None;
        }
        let out_rate = u64::from(self.pipeline.out_rate());
        let total = u64::try_from(decoding.source.total_frames()?).ok()?;
        let end = self.track_end(decoding)?;
        let length = total * out_rate / u64::from(decoding.source.sample_rate().max(1));
        let overlap = (u64::from(self.crossfade.duration_ms) * out_rate / 1000).min(length / 2);
        let hold = end.saturating_sub(overlap).max(self.pipeline.mix_pos());
        (end.saturating_sub(hold) >= MIN_OVERLAP_MS * out_rate / 1000).then_some(Plan {
            next_index,
            end,
            hold,
        })
    }

    /// The current track reached the hold point: start the crossfade if the next item is open,
    /// else let the tracks join gaplessly.
    fn resolve_plan(&mut self) {
        let Some(plan) = self.plan.take() else {
            return;
        };
        let next = self.queue.get(plan.next_index).cloned();
        let prepared = match next {
            Some(item) => self.take_prepared(plan.next_index, &item),
            None => None,
        };
        let Some(Prepared::Ready {
            index,
            item,
            input,
            source,
            start_frame,
        }) = prepared
        else {
            // Not ready (still opening, failed or never opened): no crossfade this time.
            self.prepared = prepared;
            self.pipeline.set_hold(None, &mut self.out);
            return;
        };
        let out_rate = u64::from(self.pipeline.out_rate());
        let mut overlap = plan.end - plan.hold;
        if let Some(total) = source.total_frames().and_then(|t| u64::try_from(t).ok()) {
            let length = total * out_rate / u64::from(source.sample_rate().max(1));
            overlap = overlap.min(length / 2);
        }
        if overlap < MIN_OVERLAP_MS * out_rate / 1000 {
            self.prepared = Some(Prepared::Ready {
                index,
                item,
                input,
                source,
                start_frame,
            });
            self.pipeline.set_hold(None, &mut self.out);
            return;
        }
        let start = plan.end - overlap;
        self.pipeline.begin_crossfade(start, overlap);
        let outgoing = self.decoder.take();
        if let Some(outgoing) = &outgoing {
            self.set_segment_end(outgoing.segment, plan.end);
        }
        self.fading = outgoing;
        let fade = Fade {
            start: to_i64(start),
            len: overlap,
        };
        if self.install(index, item, input, *source, start_frame, Purpose::Continue) {
            if let Some(segment) = self.segments.back_mut() {
                segment.fade_in = Some(fade);
            }
        } else {
            // The incoming item could not be put on the lane: let the outgoing one end alone.
            self.pipeline.cancel_incoming();
            self.decoder = self.fading.take();
            if let Some(outgoing) = &self.decoder {
                let id = outgoing.segment;
                for segment in self.segments.iter_mut().filter(|s| s.id == id) {
                    segment.end = None;
                }
            }
        }
        self.update_limiter();
    }

    /// The queue end is decided: flush the chain's tails behind the last track.
    fn commit_end(&mut self) {
        if let Err(err) = self.pipeline.finish(&mut self.out) {
            tracing::warn!("flushing the processing chain: {err}");
        }
        self.end_committed = true;
    }

    fn drain_step(&mut self) {
        self.update_audible();
        match self.sink.drain(DRAIN_POLL) {
            Ok(true) => self.queue_finished(),
            Ok(false) => {}
            Err(err) => {
                self.emit_error(None, &err);
                self.queue_finished();
            }
        }
    }

    /// Everything has been heard: close out the remaining tracks and the queue.
    fn queue_finished(&mut self) {
        let anchor = self.current_anchor();
        let duration = self.segments.back().and_then(|s| s.info.duration_us);
        self.emit_track_events(u64::MAX);
        self.decoder = None;
        self.fading = None;
        self.end_committed = false;
        self.set_idle_position(duration.unwrap_or(anchor.position_us), duration);
        self.emit(EngineEvent::QueueEnded);
        self.set_transport(Transport::Ended);
        self.publish_anchor();
    }

    /// Nothing more can be played: the queue ends now.
    fn end_now(&mut self) {
        self.finish_fade();
        self.reset_playback();
        self.set_idle_position(0, None);
        self.emit(EngineEvent::QueueEnded);
        self.generation += 1;
        self.set_transport(Transport::Ended);
        self.publish_anchor();
    }

    /// Re-renders from what is audible now, after the upcoming items changed under audio that
    /// was already on its way to the output: drop the queued audio, reopen the audible item at
    /// the first frame not yet played and continue from there with the new queue.
    fn rerender(&mut self) {
        let discarded = self.sink.flush();
        let resume = self.written.saturating_sub(discarded);
        let rate = self.sink.sample_rate();
        self.rebuild_from(resume, rate, false);
    }

    /// The segment audible at stream frame `at`: the newest one that started by then (the
    /// incoming track during a crossfade), else the first.
    fn audible_segment_at(&self, at: u64) -> Option<&Segment> {
        self.segments
            .iter()
            .rev()
            .find(|s| s.start <= at)
            .or_else(|| self.segments.front())
    }

    /// Reopens the item audible at stream frame `resume` (counted at `old_rate`) at its first
    /// unplayed frame, on a fresh timeline. A running crossfade keeps its incoming track and
    /// the remaining part of its fade-in; the outgoing track is dropped.
    fn rebuild_from(&mut self, resume: u64, old_rate: u32, declick: bool) {
        let Some(segment) = self.audible_segment_at(resume).cloned() else {
            // Nothing on the timeline: an item may be opening; start it again.
            let opening = self
                .openings
                .iter()
                .find(|o| o.purpose != Purpose::Prepare)
                .map(|o| (o.index, o.item.clone()));
            self.clear_timeline();
            if let Some((index, item)) = opening {
                let purpose = Purpose::Rebuild {
                    announced: true,
                    fade_in: None,
                };
                if !self.open_item(index, item, StartAt::Micros(self.idle_position_us), purpose) {
                    self.continue_from(index + 1);
                }
            }
            return;
        };
        let mut into = resume.saturating_sub(segment.start);
        let next_start = self
            .segments
            .iter()
            .find(|s| s.start > segment.start)
            .map(|s| s.start);
        if let Some(end) = segment.end.or(next_start) {
            into = into.min(end.saturating_sub(segment.start));
        }
        // Stream frames are at the old output rate; convert to the source's frames.
        let (source_rate, sink_rate) = (u128::from(segment.source_rate), u128::from(old_rate.max(1)));
        let into_source = (u128::from(into) * source_rate + sink_rate / 2) / sink_rate;
        let frame = segment.base_frame + i64::try_from(into_source).unwrap_or(i64::MAX);
        self.clear_timeline();
        let new_rate = self.pipeline.out_rate();
        let start = to_i64(self.pipeline.next_input_index(LaneId::Main));
        let fade_in = match segment.fade_in {
            Some(fade) if i128::from(fade.end()) > i128::from(resume) => {
                let scale = |frames: i128| frames * i128::from(new_rate) / i128::from(old_rate.max(1));
                let done = i128::from(resume) - i128::from(fade.start);
                let len = scale(i128::from(fade.len)).max(1);
                Some(Fade {
                    start: start - i64::try_from(scale(done)).unwrap_or(0),
                    len: u64::try_from(len).unwrap_or(1),
                })
            }
            _ if declick => Some(Fade {
                start,
                len: declick_frames(new_rate),
            }),
            _ => None,
        };
        self.current = segment.index;
        let purpose = Purpose::Rebuild {
            announced: segment.announced,
            fade_in,
        };
        if !self.open_item(
            segment.index,
            segment.item.clone(),
            StartAt::Frame(frame),
            purpose,
        ) {
            self.continue_from(segment.index + 1);
        }
    }

    /// A manual transition: discards everything queued for output and in the chain. While
    /// playing, what was about to be heard fades out over a few milliseconds instead of being
    /// cut mid-wave, and the next start fades in.
    fn cut(&mut self) {
        let audible = self.transport == Transport::Playing && !self.segments.is_empty();
        if audible {
            let fade = declick_frames(self.sink.sample_rate());
            if self.sink.latency_us() == 0 {
                // An unbuffered (offline) sink: fade what the engine has not written yet,
                // decoding a little more of the old track when too little is left.
                self.refill_main(fade as usize);
                self.write_faded_tail(fade);
                self.sink.flush();
            } else {
                self.sink.flush_faded(fade);
            }
        } else {
            self.sink.flush();
        }
        self.clear_timeline();
        self.declick_next = audible;
    }

    /// Decodes the main lane until at least `frames` are pending for output (or it runs out),
    /// without the side effects of a normal decode step.
    fn refill_main(&mut self, frames: usize) {
        self.pipeline.set_hold(None, &mut self.out);
        while (self.out.len() - self.out_pos) / 2 < frames {
            let Some(decoding) = self.decoder.as_mut() else {
                break;
            };
            self.decoded.clear();
            if !matches!(decoding.source.read(&mut self.decoded), Ok(Read::Audio)) {
                break;
            }
            let gain = decoding.replaygain;
            if gain != 1.0 {
                self.decoded.iter_mut().for_each(|s| *s *= gain);
            }
            if self
                .pipeline
                .push(LaneId::Main, &self.decoded, &mut self.out)
                .is_err()
            {
                break;
            }
        }
    }

    /// Writes up to `frames` of the pending output with a fade to silence.
    fn write_faded_tail(&mut self, frames: u64) {
        let pending = (self.out.len() - self.out_pos) / 2;
        let count = (frames as usize).min(pending);
        if count == 0 {
            return;
        }
        let tail = &mut self.out[self.out_pos..self.out_pos + count * 2];
        for (i, frame) in tail.chunks_exact_mut(2).enumerate() {
            let gain = 1.0 - (i + 1) as f32 / count as f32;
            frame[0] *= gain;
            frame[1] *= gain;
        }
        let end = self.out_pos + count * 2;
        if self.sink.write(&self.out[self.out_pos..end]).is_ok() {
            self.written += count as u64;
        }
        self.out_pos = end;
    }

    fn take_declick(&mut self) -> bool {
        std::mem::take(&mut self.declick_next)
    }

    /// After a cut that faded out, lets the fade play before the output pauses or stops.
    fn finish_fade(&mut self) {
        if self.take_declick() {
            let _ = self.sink.drain(Duration::from_millis(DECLICK_MS * 3));
        }
    }

    /// Discards everything queued for output and in the chain, without a fade.
    fn reset_playback(&mut self) {
        self.sink.flush();
        self.clear_timeline();
    }

    fn clear_timeline(&mut self) {
        self.decoder = None;
        self.fading = None;
        self.openings.clear();
        self.prepared = None;
        self.plan = None;
        self.segments.clear();
        self.end_committed = false;
        self.out.clear();
        self.out_pos = 0;
        self.pipeline.reset(self.written);
        self.end_buffering();
    }

    fn update_limiter(&mut self) {
        let gains = [self.decoder.as_ref(), self.fading.as_ref()]
            .into_iter()
            .flatten()
            .map(|d| d.replaygain)
            .fold(1.0f32, f32::max);
        // Two tracks overlapping can add up past full scale even at unity gain.
        let needed = self.limiter_needed(gains) || self.pipeline.crossfading();
        self.pipeline.set_limiter_engaged(needed, &mut self.out);
    }

    /// The limiter is engaged only when gain could take the signal past full scale.
    fn limiter_needed(&self, replaygain: f32) -> bool {
        replaygain > 1.0 + 1e-6 || self.eq.max_boost_db() > 0.0
    }

    /// Changes the transport. Waiters (`wait_until_idle`) see it with the next
    /// anchor or when the current command is done, so they never wake between a
    /// stop and the events and anchor that report it.
    fn set_transport(&mut self, transport: Transport) {
        if transport != self.transport {
            self.sink
                .set_paused(transport != Transport::Playing || self.buffering.is_some());
            self.transport = transport;
        }
    }

    fn set_idle_position(&mut self, position_us: i64, duration_us: Option<i64>) {
        self.idle_position_us = position_us;
        self.idle_duration_us = duration_us;
    }

    /// The item the listener hears (or would hear when playback starts).
    fn current_index(&self) -> usize {
        let audible = self.audible_frame();
        self.audible_segment_at(audible).map_or(self.current, |s| s.index)
    }

    // ----- buffering ------------------------------------------------------------------------

    /// The audio of item `id` is not there yet (its stream is starved or it is opening): let
    /// the output play out what it has, then report buffering, and wait a little for data.
    fn wait_for_stream(&mut self, id: u64) {
        if !self.sink_dry {
            match self.sink.drain(DRAIN_POLL) {
                Ok(true) | Err(_) => self.sink_dry = true,
                Ok(false) => return,
            }
        }
        let since = self
            .openings
            .iter()
            .find(|o| o.item.id == id)
            .map(|o| o.since)
            .or_else(|| {
                [self.decoder.as_ref(), self.fading.as_ref()]
                    .into_iter()
                    .flatten()
                    .find(|d| d.item.id == id)
                    .map(|d| d.since)
            });
        if self.buffering.is_none() && since.is_none_or(|since| since.elapsed() >= OPEN_GRACE) {
            tracing::debug!("item {id}: waiting for data");
            self.buffering = Some(id);
            self.sink.set_paused(true);
            self.emit(EngineEvent::Buffering { id, buffering: true });
            self.publish_anchor();
        }
        // Wait for the opening's result or the decoder's next block, briefly.
        if let Some(opening) = self
            .openings
            .iter_mut()
            .find(|o| o.item.id == id && o.result.is_none())
        {
            if let Some(result) = opening.pending.wait_finish(STREAM_WAIT) {
                opening.result = Some(result.map(|(s, frame)| (DecodeSource::Threaded(s), frame)));
            }
            return;
        }
        let decoding = [self.decoder.as_mut(), self.fading.as_mut()]
            .into_iter()
            .flatten()
            .find(|d| d.item.id == id);
        match decoding.map(|d| &mut d.source) {
            Some(DecodeSource::Threaded(source)) if !source.full() => source.wait(STREAM_WAIT),
            _ => std::thread::sleep(STREAM_WAIT),
        }
    }

    /// Data flows again: report the end of buffering and restart the output.
    fn end_buffering(&mut self) {
        if let Some(id) = self.buffering.take() {
            tracing::debug!("item {id}: buffered enough, playing on");
            self.emit(EngineEvent::Buffering { id, buffering: false });
            if self.transport == Transport::Playing {
                self.sink.set_paused(false);
            }
            self.publish_anchor();
        }
    }

    // ----- timeline, events and anchors -----------------------------------------------------

    /// [`Self::audible_frame`] without the floor at zero.
    fn audible_frame_signed(&self) -> i64 {
        let rate = u128::from(self.sink.sample_rate());
        let latency =
            i64::try_from(u128::from(self.sink.latency_us()) * rate / 1_000_000).unwrap_or(i64::MAX);
        i64::try_from(self.written)
            .unwrap_or(i64::MAX)
            .saturating_sub(latency)
    }

    fn audible_frame(&self) -> u64 {
        let rate = u128::from(self.sink.sample_rate());
        let latency = (u128::from(self.sink.latency_us()) * rate / 1_000_000) as u64;
        self.written.saturating_sub(latency)
    }

    /// Fires track events for boundaries the audible frame has crossed.
    fn update_audible(&mut self) {
        let audible = self.audible_frame();
        self.emit_track_events(audible);
    }

    /// Emits, in timeline order, the track starts and ends at or before stream frame `until`.
    /// At a gapless join the end comes first; during a crossfade the incoming track starts
    /// before the outgoing one ends.
    fn emit_track_events(&mut self, until: u64) {
        loop {
            let front_end = self.segments.front().and_then(|front| {
                front
                    .end
                    .or_else(|| self.segments.iter().skip(1).map(|s| s.start).min())
                    .or((until == u64::MAX).then_some(u64::MAX))
            });
            let next_start = self
                .segments
                .iter()
                .position(|s| !s.announced)
                .map(|i| (i, self.segments[i].start));
            let end_due = front_end.filter(|&end| end <= until);
            let start_due = next_start.filter(|&(_, start)| start <= until);
            match (end_due, start_due) {
                (Some(end), Some((i, start))) if end <= start && i > 0 => self.end_front(),
                (Some(_), None) => self.end_front(),
                (_, Some((i, _))) => self.announce(i, until != u64::MAX),
                (None, None) => break,
            }
        }
    }

    fn end_front(&mut self) {
        let Some(old) = self.segments.pop_front() else {
            return;
        };
        if !old.announced {
            // Shorter than one write: it played in between two checks.
            self.emit(EngineEvent::TrackStarted {
                index: old.index,
                id: old.item.id,
                info: old.info,
            });
        }
        if self.segments.is_empty() {
            self.current = old.index;
        }
        self.emit(EngineEvent::TrackEnded {
            index: old.index,
            id: old.item.id,
        });
    }

    fn announce(&mut self, at: usize, anchor: bool) {
        let segment = &mut self.segments[at];
        segment.announced = true;
        let event = EngineEvent::TrackStarted {
            index: segment.index,
            id: segment.item.id,
            info: segment.info.clone(),
        };
        self.current = segment.index;
        self.generation += 1;
        self.emit(event);
        if anchor {
            self.publish_anchor();
        }
    }

    fn current_anchor(&self) -> PlaybackAnchor {
        let audible = self.audible_frame_signed();
        let segment = self
            .segments
            .iter()
            .rev()
            .find(|s| i128::from(s.start) <= i128::from(audible))
            .or_else(|| self.segments.front());
        let (position_us, duration_us) = match segment {
            Some(segment) => {
                // Negative right after a start: the output's delay has not passed yet, so the
                // track's first sample is still on its way ("starts in N ms").
                let into = audible - i64::try_from(segment.start).unwrap_or(i64::MAX);
                let base = frames_to_us_signed(segment.base_frame, segment.source_rate);
                let position = base + frames_to_us_signed(into, self.sink.sample_rate());
                (position, segment.info.duration_us)
            }
            None => (self.idle_position_us, self.idle_duration_us),
        };
        // The clock runs once the output is taking audio, not from the first write, and stops
        // while a stream buffers.
        let running = self.transport == Transport::Playing
            && self.buffering.is_none()
            && segment.is_some()
            && self.sink.output_started();
        PlaybackAnchor {
            track_generation: self.generation,
            position_us,
            anchor_mono_ns: monotonic_now_ns(),
            rate: if running { 1.0 } else { 0.0 },
            duration_us,
        }
    }

    fn publish_anchor(&mut self) {
        let anchor = self.current_anchor();
        self.output_waiting = self.transport == Transport::Playing && anchor.rate == 0.0;
        self.last_anchor = anchor;
        self.last_anchor_check = Instant::now();
        // Store the anchor before announcing it: whoever receives the event and then asks
        // `Engine::anchor()` (the core does) must never get an older one. `playing` changes
        // only after the event, so `wait_until_idle` wakes with the event already sent.
        self.shared.state.lock().anchor = anchor;
        self.emit(EngineEvent::Anchor(anchor));
        let mut state = self.shared.state.lock();
        state.playing = self.transport == Transport::Playing;
        self.shared.changed.notify_all();
    }

    /// At most once a second: re-anchor if the audio clock drifted from the last anchor.
    fn maybe_reanchor(&mut self) {
        let interval = if Instant::now() < self.settle_until {
            SETTLE_INTERVAL
        } else {
            REANCHOR_INTERVAL
        };
        if self.last_anchor_check.elapsed() < interval {
            return;
        }
        self.last_anchor_check = Instant::now();
        let underruns = self.sink.underruns();
        if underruns > self.reported_underruns {
            tracing::warn!(
                "audio output ran dry {} time(s)",
                underruns - self.reported_underruns
            );
            self.reported_underruns = underruns;
        }
        let now = self.current_anchor();
        let predicted = self.last_anchor.position_at(now.anchor_mono_ns);
        if now.track_generation != self.last_anchor.track_generation
            || now.rate != self.last_anchor.rate
            || (now.position_us - predicted).abs() > DRIFT_THRESHOLD_US
        {
            tracing::debug!(
                "re-anchoring: drift {} ms, output latency {} ms",
                (now.position_us - predicted) / 1_000,
                self.sink.latency_us() / 1_000
            );
            self.publish_anchor();
        }
    }

    fn emit(&self, event: EngineEvent) {
        // Fails only when the handle (and every receiver) is gone.
        let _ = self.events.send(event);
    }

    fn emit_error(&self, id: Option<u64>, err: &EngineError) {
        tracing::warn!("engine error (item {id:?}): {err}");
        self.emit(EngineEvent::Error {
            id,
            message: err.to_string(),
        });
    }
}

fn input_len(input: &Input) -> Option<u64> {
    match input {
        Input::Http(stream) => stream.total(),
        Input::File(_) => None,
    }
}

fn declick_frames(rate: u32) -> u64 {
    u64::from(rate) * DECLICK_MS / 1000
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Stereo to `channels` channels: mono sums, wider outputs get silence beyond the first two.
fn map_channels(stereo: &[f32], channels: usize, out: &mut Vec<f32>) {
    for frame in stereo.chunks_exact(2) {
        if channels == 1 {
            out.push((frame[0] + frame[1]) * 0.5);
        } else {
            out.extend_from_slice(frame);
            out.extend(std::iter::repeat_n(0.0, channels.saturating_sub(2)));
        }
    }
}

fn frames_to_us_signed(frames: i64, rate: u32) -> i64 {
    let us = i128::from(frames) * 1_000_000 / i128::from(rate.max(1));
    i64::try_from(us).unwrap_or(if us < 0 { i64::MIN } else { i64::MAX })
}

#[cfg(test)]
mod tests {
    #[test]
    fn engine_can_be_shared_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<super::Engine>();
    }

    #[test]
    fn stereo_maps_to_other_channel_counts() {
        let mut out = Vec::new();
        super::map_channels(&[0.5, 0.25], 1, &mut out);
        assert_eq!(out, [0.375]);
        out.clear();
        super::map_channels(&[0.5, 0.25], 4, &mut out);
        assert_eq!(out, [0.5, 0.25, 0.0, 0.0]);
    }
}
