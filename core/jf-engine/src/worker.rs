//! Decoder threads for streamed items (architecture §4: one decoder per live stream).
//!
//! A local file decodes on the engine thread: its reads never wait long. An HTTP item opens and
//! decodes on its own thread instead, into a small bounded queue, because any read may wait on
//! the network. The engine thread takes decoded blocks only when one is ready, so it never
//! waits on a socket; an empty queue while the item has not ended means the stream is starved.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, TryRecvError};

use crate::decode::{Input, Source};
use crate::types::{EngineError, TrackInfo};

/// Decoded blocks a decoder thread may run ahead (a few hundred milliseconds of audio).
const QUEUE_BLOCKS: usize = 16;

/// Where an opened track starts.
#[derive(Debug, Clone, Copy)]
pub(crate) enum StartAt {
    Micros(i64),
    /// A frame at the source rate.
    Frame(i64),
}

/// Opens `input` and seeks to `start`; returns the source and the frame it starts at.
pub(crate) fn open_source(input: &Input, start: StartAt) -> Result<(Source, i64), EngineError> {
    let mut source = Source::open(input)?;
    let mut frame = match start {
        StartAt::Micros(us) => source.frame_at(us),
        StartAt::Frame(frame) => frame,
    };
    if frame > 0
        && let Err(err) = source.seek_frame(frame)
    {
        tracing::warn!("{}: {err}; playing from the start", input.describe());
        frame = 0;
    }
    Ok((source, frame))
}

/// What a decoder thread sends.
enum Block {
    Audio {
        samples: Vec<f32>,
        /// The track frame of the first frame since the start, once.
        first_frame: Option<i64>,
        /// Track frame after this block.
        next_frame: i64,
    },
    End,
    Failed(EngineError),
}

/// Facts about an opened source, known before its first block.
#[derive(Debug, Clone)]
struct Opened {
    info: TrackInfo,
    rate: u32,
    total_frames: Option<i64>,
    start_frame: i64,
}

/// A source decoding on its own thread.
pub(crate) struct ThreadedSource {
    blocks: Receiver<Block>,
    /// The thread has queued its last block.
    done: Arc<AtomicBool>,
    /// A block received while waiting, not handed out yet.
    peeked: Option<Block>,
    info: TrackInfo,
    rate: u32,
    total_frames: Option<i64>,
    next_frame: i64,
    first_frame: Option<i64>,
    ended: bool,
}

impl fmt::Debug for ThreadedSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ThreadedSource")
            .field("info", &self.info)
            .field("next_frame", &self.next_frame)
            .field("ended", &self.ended)
            .finish_non_exhaustive()
    }
}

/// An HTTP item opening on its decoder thread.
pub(crate) struct PendingOpen {
    opened: Receiver<Result<Opened, EngineError>>,
    blocks: Receiver<Block>,
    done: Arc<AtomicBool>,
}

impl PendingOpen {
    /// The opened source and its start frame, or the failure, once the thread has opened it.
    pub(crate) fn try_finish(&self) -> Option<Result<(ThreadedSource, i64), EngineError>> {
        match self.opened.try_recv() {
            Ok(result) => Some(self.finish(result)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(EngineError::Decode("the decoder thread stopped".into())))
            }
        }
    }

    /// Like [`try_finish`](Self::try_finish), waiting up to `timeout`.
    pub(crate) fn wait_finish(
        &self,
        timeout: Duration,
    ) -> Option<Result<(ThreadedSource, i64), EngineError>> {
        match self.opened.recv_timeout(timeout) {
            Ok(result) => Some(self.finish(result)),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                Some(Err(EngineError::Decode("the decoder thread stopped".into())))
            }
        }
    }

    fn finish(&self, result: Result<Opened, EngineError>) -> Result<(ThreadedSource, i64), EngineError> {
        let opened = result?;
        Ok((
            ThreadedSource {
                blocks: self.blocks.clone(),
                done: Arc::clone(&self.done),
                peeked: None,
                info: opened.info,
                rate: opened.rate,
                total_frames: opened.total_frames,
                next_frame: opened.start_frame,
                first_frame: None,
                ended: false,
            },
            opened.start_frame,
        ))
    }
}

/// Opens `input` at `start` on a new decoder thread, which then decodes ahead into a bounded
/// queue until the track ends or the [`ThreadedSource`] is dropped.
pub(crate) fn spawn(input: Input, start: StartAt) -> PendingOpen {
    let (opened_tx, opened_rx) = crossbeam_channel::bounded(1);
    let (block_tx, block_rx) = crossbeam_channel::bounded(QUEUE_BLOCKS);
    let done = Arc::new(AtomicBool::new(false));
    let thread_done = Arc::clone(&done);
    let spawned = std::thread::Builder::new()
        .name("jf-decode".into())
        .spawn(move || {
            let (mut source, start_frame) = match open_source(&input, start) {
                Ok(opened) => opened,
                Err(err) => {
                    let _ = opened_tx.send(Err(err));
                    return;
                }
            };
            let opened = Opened {
                info: source.info().clone(),
                rate: source.sample_rate(),
                total_frames: source.total_frames(),
                start_frame,
            };
            if opened_tx.send(Ok(opened)).is_err() {
                return;
            }
            loop {
                let mut samples = Vec::new();
                let block = match source.read(&mut samples) {
                    Ok(true) => Block::Audio {
                        samples,
                        first_frame: source.take_first_frame(),
                        next_frame: source.next_frame(),
                    },
                    Ok(false) => Block::End,
                    Err(err) => Block::Failed(err),
                };
                let last = !matches!(block, Block::Audio { .. });
                // Fails when the engine dropped the source: stop decoding.
                if block_tx.send(block).is_err() || last {
                    thread_done.store(true, Ordering::Release);
                    return;
                }
            }
        });
    if let Err(err) = spawned {
        let (opened_tx, opened_rx) = crossbeam_channel::bounded(1);
        let _ = opened_tx.send(Err(EngineError::Io(err)));
        return PendingOpen {
            opened: opened_rx,
            blocks: block_rx,
            done,
        };
    }
    PendingOpen {
        opened: opened_rx,
        blocks: block_rx,
        done,
    }
}

/// What [`ThreadedSource::read`] found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Read {
    /// A block was appended.
    Audio,
    /// Nothing decoded yet: the stream is waiting for data.
    Pending,
    /// The track ended.
    End,
}

impl ThreadedSource {
    pub(crate) fn info(&self) -> &TrackInfo {
        &self.info
    }

    pub(crate) fn sample_rate(&self) -> u32 {
        self.rate
    }

    pub(crate) fn total_frames(&self) -> Option<i64> {
        self.total_frames
    }

    pub(crate) fn next_frame(&self) -> i64 {
        self.next_frame
    }

    pub(crate) fn take_first_frame(&mut self) -> Option<i64> {
        self.first_frame.take()
    }

    /// Whether a block (or the end) is waiting.
    pub(crate) fn ready(&self) -> bool {
        self.ended || self.peeked.is_some() || !self.blocks.is_empty()
    }

    /// Whether the decoder thread has filled its queue (it waits for the engine).
    pub(crate) fn full(&self) -> bool {
        self.blocks.is_full()
    }

    /// Whether the decoder thread has queued its last block (end of track or failure).
    pub(crate) fn finished(&self) -> bool {
        self.ended || self.done.load(Ordering::Acquire)
    }

    /// Waits up to `timeout` for a block.
    pub(crate) fn wait(&mut self, timeout: Duration) {
        if self.ready() {
            return;
        }
        match self.blocks.recv_timeout(timeout) {
            Ok(block) => self.peeked = Some(block),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => self.ended = true,
        }
    }

    /// Appends the next decoded block to `out` when one is ready.
    pub(crate) fn read(&mut self, out: &mut Vec<f32>) -> Result<Read, EngineError> {
        if self.ended {
            return Ok(Read::End);
        }
        let block = match self.peeked.take() {
            Some(block) => block,
            None => match self.blocks.try_recv() {
                Ok(block) => block,
                Err(TryRecvError::Empty) => return Ok(Read::Pending),
                Err(TryRecvError::Disconnected) => Block::End,
            },
        };
        match block {
            Block::Audio {
                samples,
                first_frame,
                next_frame,
            } => {
                out.extend_from_slice(&samples);
                if first_frame.is_some() {
                    self.first_frame = first_frame;
                }
                self.next_frame = next_frame;
                Ok(Read::Audio)
            }
            Block::End => {
                self.ended = true;
                Ok(Read::End)
            }
            Block::Failed(err) => {
                self.ended = true;
                Err(err)
            }
        }
    }
}
