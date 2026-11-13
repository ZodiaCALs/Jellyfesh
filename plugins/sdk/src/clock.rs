//! Time from the host. On `wasm32-unknown-unknown`,
//! `std::time::SystemTime::now()` and `Instant::now()` panic; use these.

use std::time::Duration;

use crate::bindings::jellyfesh::plugin::clock as wit;

/// Milliseconds since the Unix epoch (UTC).
pub fn now_utc_ms() -> u64 {
    wit::now_utc_ms()
}

/// Time since the Unix epoch, like `SystemTime::now().duration_since(UNIX_EPOCH)`.
pub fn since_epoch() -> Duration {
    Duration::from_millis(now_utc_ms())
}

/// Seconds since the Unix epoch (what many APIs call a timestamp).
pub fn unix_seconds() -> u64 {
    now_utc_ms() / 1000
}

/// A monotonic clock in milliseconds, for measuring intervals.
pub fn monotonic_ms() -> u64 {
    wit::monotonic_ms()
}
