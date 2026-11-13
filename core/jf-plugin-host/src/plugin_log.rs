//! The `log` import: plugin diagnostics into `tracing` (target `jf_plugin`).
//!
//! Per plugin: bursts of [`BURST`] lines, then [`RATE`] lines a second;
//! dropped lines are counted and reported once logging resumes. Messages are
//! cut at [`MAX_MESSAGE`] bytes, control characters are escaped (no terminal
//! escape sequences or forged log lines), and the password fields of a login
//! being submitted are masked.

use std::time::Instant;

use crate::bindings::jellyfesh::plugin::log::Level;

/// Lines a plugin may log at once.
pub const BURST: f64 = 100.0;
/// Lines per second after a burst.
pub const RATE: f64 = 20.0;
/// Longest message kept, in bytes.
pub const MAX_MESSAGE: usize = 2048;

/// A plugin's log limiter.
#[derive(Debug)]
pub(crate) struct LogLimiter {
    tokens: f64,
    last: Option<Instant>,
    dropped: u64,
}

impl Default for LogLimiter {
    fn default() -> Self {
        LogLimiter {
            tokens: BURST,
            last: None,
            dropped: 0,
        }
    }
}

impl LogLimiter {
    /// Whether a line may be written now; `Some(n)` also reports `n` lines
    /// dropped since the last written one.
    pub(crate) fn admit(&mut self, now: Instant) -> Option<u64> {
        if let Some(last) = self.last {
            let elapsed = now.saturating_duration_since(last).as_secs_f64();
            self.tokens = (self.tokens + elapsed * RATE).min(BURST);
        }
        self.last = Some(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Some(std::mem::take(&mut self.dropped))
        } else {
            self.dropped += 1;
            None
        }
    }
}

/// Makes a plugin message safe to log: masks `secrets`, escapes control
/// characters, and cuts it at [`MAX_MESSAGE`] bytes.
pub(crate) fn sanitize<'a>(message: &str, secrets: impl IntoIterator<Item = &'a str>) -> String {
    // Only the first 2 KiB can show; mask within a generous prefix so a huge
    // message costs little, and so no secret straddles the visible part.
    let mut end = message.len().min(8 * MAX_MESSAGE);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    let mut text = message[..end].to_string();
    for secret in secrets {
        if !secret.is_empty() {
            text = text.replace(secret, "***");
        }
    }
    let mut out = String::with_capacity(text.len().min(MAX_MESSAGE));
    for c in text.chars() {
        let piece = if c.is_control() {
            c.escape_default().to_string()
        } else {
            c.to_string()
        };
        if out.len() + piece.len() > MAX_MESSAGE {
            out.push('…');
            break;
        }
        out.push_str(&piece);
    }
    out
}

/// Writes one plugin line to `tracing`.
pub(crate) fn emit(plugin: &str, level: Level, message: &str) {
    match level {
        Level::Trace => tracing::trace!(target: "jf_plugin", plugin, "{message}"),
        Level::Debug => tracing::debug!(target: "jf_plugin", plugin, "{message}"),
        Level::Info => tracing::info!(target: "jf_plugin", plugin, "{message}"),
        Level::Warn => tracing::warn!(target: "jf_plugin", plugin, "{message}"),
        Level::Error => tracing::error!(target: "jf_plugin", plugin, "{message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn limits_bursts_and_reports_drops() {
        let mut limiter = LogLimiter::default();
        let t0 = Instant::now();
        let admitted = (0..150).filter(|_| limiter.admit(t0).is_some()).count();
        assert_eq!(admitted, 100);
        // Half a second later, ten more lines are allowed; the first says 50 were dropped.
        let t1 = t0 + Duration::from_millis(500);
        assert_eq!(limiter.admit(t1), Some(50));
        let more = (0..20).filter(|_| limiter.admit(t1).is_some()).count();
        assert_eq!(more, 9);
    }

    #[test]
    fn sanitizes_messages() {
        assert_eq!(
            sanitize("line\nforged: INFO admin", []),
            "line\\nforged: INFO admin"
        );
        assert_eq!(sanitize("\u{1b}[31mred", []), "\\u{1b}[31mred");
        assert_eq!(
            sanitize("user=bob pass=hunter22", ["hunter22"]),
            "user=bob pass=***"
        );
        let long = "é".repeat(MAX_MESSAGE);
        let cut = sanitize(&long, []);
        assert!(cut.len() <= MAX_MESSAGE + '…'.len_utf8());
        assert!(cut.ends_with('…'));
    }
}
