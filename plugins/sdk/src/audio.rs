//! The song's audio inside [`crate::Plugin::make_lyrics`] (API 1.2,
//! permission `audio`).
//!
//! Only during `make_lyrics`, for the [`crate::TrackHandle`] in its
//! [`crate::Song`]. Reading audio needs the person's consent for at least one
//! of the plugin's origins, and once a call has read audio, the rest of it
//! may send HTTP requests only to the allowed origins and may not write to
//! [`crate::kv`] (ADR-0019).

use crate::Result;
use crate::bindings::jellyfesh::plugin::audio as wit;

pub use wit::{AudioError, ClipInfo};

/// The most audio one [`wav`] call returns: 8 minutes.
pub const MAX_WAV_MS: u64 = 8 * 60 * 1000;

/// The song's length in milliseconds.
pub fn duration_ms(track: crate::TrackHandle) -> core::result::Result<u64, AudioError> {
    wit::info(track).map(|info| info.duration_ms)
}

/// 16 kHz mono 16-bit PCM WAV of `[start_ms, start_ms + length_ms)`, at most
/// [`MAX_WAV_MS`] per call.
pub fn wav(
    track: crate::TrackHandle,
    start_ms: u64,
    length_ms: u64,
) -> core::result::Result<Vec<u8>, AudioError> {
    wit::wav(track, start_ms, length_ms)
}

/// The whole song as WAV parts of at most `part_ms` (capped at
/// [`MAX_WAV_MS`]), with each part's start, for providers with an upload
/// limit.
pub fn parts(track: crate::TrackHandle, part_ms: u64) -> Result<Vec<(u64, Vec<u8>)>> {
    let part_ms = part_ms.clamp(1000, MAX_WAV_MS);
    let duration = duration_ms(track)?;
    let mut out = Vec::new();
    let mut start = 0;
    while start < duration {
        out.push((start, wav(track, start, part_ms)?));
        start += part_ms;
    }
    Ok(out)
}
