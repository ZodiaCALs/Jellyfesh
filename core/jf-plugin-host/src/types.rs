//! Data types of the host API.
//!
//! They are the plain mirrors of the WIT records in `jf_plugin_model::types`
//! (wasmtime's generated types stay inside this crate, converted at the
//! runtime boundary by `convert.rs`), plus the reply channel of asynchronous
//! calls.

pub use jf_plugin_model::types::*;

/// The answer to an asynchronous call: exactly one message arrives, the
/// result or the reason there is none. Dropping the receiver is fine; the
/// call still runs to completion on the plugin's worker.
pub type Reply<T> = crossbeam_channel::Receiver<Result<T, crate::CallError>>;
