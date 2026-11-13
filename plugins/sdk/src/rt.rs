//! Runtime glue: the panic hook, settings parsing, and the `getrandom`
//! backend.

use crate::{Plugin, PluginError, Result};

/// Runs at the start of every export: installs a panic hook (once) that logs
/// the panic message, so the host can show why the instance trapped.
pub fn enter() {
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            crate::log::write(crate::log::Level::Error, &format!("panic: {info}"));
        }));
    });
}

/// Parses the settings JSON and hands it to the plugin.
pub fn settings_changed<P: Plugin>(settings: &str) -> Result<()> {
    let value: serde_json::Value = serde_json::from_str(settings)
        .map_err(|e| PluginError::InvalidInput(format!("settings are not JSON: {e}")))?;
    P::settings_changed(&value)
}

/// The `getrandom` custom backend (getrandom 0.3 and 0.4 both call this
/// symbol), over the host's `crypto.random-bytes`. Builds must pass
/// `--cfg getrandom_backend="custom"`; `cargo xtask plugin build` does.
#[cfg(target_arch = "wasm32")]
#[allow(
    unsafe_code,
    reason = "the getrandom backend hook is an unsafe extern function by definition"
)]
mod getrandom_backend {
    /// Fills `dest[..len]` with random bytes from the host.
    ///
    /// # Safety
    ///
    /// getrandom passes a pointer valid for writes of `len` bytes.
    #[unsafe(no_mangle)]
    unsafe extern "Rust" fn __getrandom_v03_custom(
        dest: *mut u8,
        len: usize,
    ) -> Result<(), getrandom::Error> {
        let mut filled = 0;
        while filled < len {
            let chunk = (len - filled).min(64 * 1024);
            let bytes = crate::bindings::jellyfesh::plugin::crypto::random_bytes(chunk as u32)
                .map_err(|_| getrandom::Error::UNEXPECTED)?;
            if bytes.len() != chunk {
                return Err(getrandom::Error::UNEXPECTED);
            }
            // SAFETY: `filled + chunk <= len`, and `dest` is valid for `len` bytes.
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), dest.add(filled), chunk) };
            filled += chunk;
        }
        Ok(())
    }
}
