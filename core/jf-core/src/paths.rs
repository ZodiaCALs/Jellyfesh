//! Default data and cache directories for each platform.
//!
//! Apps normally pass their own directories in [`crate::Config`]; these
//! defaults serve the CLI and tests. `JF_DATA_DIR` and `JF_CACHE_DIR`
//! override them.

use std::path::PathBuf;

/// Folder name on macOS and Windows.
#[cfg(any(target_os = "macos", windows))]
const APP_DIR: &str = "Jellyfesh";
/// Folder name under XDG directories.
#[cfg(all(unix, not(target_os = "macos")))]
const APP_DIR_XDG: &str = "jellyfesh";

/// Where the library catalogue and the session database live.
///
/// | Platform | Default |
/// | --- | --- |
/// | Linux and other Unix | `$XDG_DATA_HOME/jellyfesh` or `~/.local/share/jellyfesh` |
/// | macOS | `~/Library/Application Support/Jellyfesh` |
/// | Windows | `%APPDATA%\Jellyfesh` |
pub fn default_data_dir() -> Option<PathBuf> {
    env_path("JF_DATA_DIR").or_else(platform_data_dir)
}

/// Where artwork, backdrops and other rebuildable files go.
///
/// | Platform | Default |
/// | --- | --- |
/// | Linux and other Unix | `$XDG_CACHE_HOME/jellyfesh` or `~/.cache/jellyfesh` |
/// | macOS | `~/Library/Caches/Jellyfesh` |
/// | Windows | `%LOCALAPPDATA%\Jellyfesh\cache` |
pub fn default_cache_dir() -> Option<PathBuf> {
    env_path("JF_CACHE_DIR").or_else(platform_cache_dir)
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn platform_data_dir() -> Option<PathBuf> {
    env_path("HOME").map(|h| h.join("Library/Application Support").join(APP_DIR))
}

#[cfg(target_os = "macos")]
fn platform_cache_dir() -> Option<PathBuf> {
    env_path("HOME").map(|h| h.join("Library/Caches").join(APP_DIR))
}

#[cfg(windows)]
fn platform_data_dir() -> Option<PathBuf> {
    env_path("APPDATA").map(|p| p.join(APP_DIR))
}

#[cfg(windows)]
fn platform_cache_dir() -> Option<PathBuf> {
    env_path("LOCALAPPDATA").map(|p| p.join(APP_DIR).join("cache"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_data_dir() -> Option<PathBuf> {
    xdg("XDG_DATA_HOME", ".local/share")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_cache_dir() -> Option<PathBuf> {
    xdg("XDG_CACHE_HOME", ".cache")
}

/// XDG base directory: the variable when it holds an absolute path (the
/// specification says relative values are invalid), else `~/<fallback>`.
#[cfg(all(unix, not(target_os = "macos")))]
fn xdg(var: &str, fallback: &str) -> Option<PathBuf> {
    env_path(var)
        .filter(|p| p.is_absolute())
        .or_else(|| env_path("HOME").map(|h| h.join(fallback)))
        .map(|p| p.join(APP_DIR_XDG))
}

#[cfg(not(any(unix, windows)))]
fn platform_data_dir() -> Option<PathBuf> {
    None
}

#[cfg(not(any(unix, windows)))]
fn platform_cache_dir() -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_name_the_app() {
        // The environment of a test run is not ours to change (tests run in
        // parallel), so only check the shape of whatever comes back.
        for dir in [default_data_dir(), default_cache_dir()].into_iter().flatten() {
            if std::env::var_os("JF_DATA_DIR").is_none() && std::env::var_os("JF_CACHE_DIR").is_none() {
                let s = dir.to_string_lossy().to_lowercase();
                assert!(s.contains("jellyfesh"), "{s}");
            }
        }
    }
}
