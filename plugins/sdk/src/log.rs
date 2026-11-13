//! Logging into the host's log (rate-limited there; never log secrets).

pub use crate::bindings::jellyfesh::plugin::log::Level;

/// Writes one line.
pub fn write(level: Level, message: &str) {
    crate::bindings::jellyfesh::plugin::log::log(level, message);
}

/// Logs at error level: `error!("failed: {why}")`.
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::log::write($crate::log::Level::Error, &::std::format!($($arg)*)) };
}

/// Logs at warning level.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::log::write($crate::log::Level::Warn, &::std::format!($($arg)*)) };
}

/// Logs at info level.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { $crate::log::write($crate::log::Level::Info, &::std::format!($($arg)*)) };
}

/// Logs at debug level.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => { $crate::log::write($crate::log::Level::Debug, &::std::format!($($arg)*)) };
}

/// Logs at trace level.
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => { $crate::log::write($crate::log::Level::Trace, &::std::format!($($arg)*)) };
}
