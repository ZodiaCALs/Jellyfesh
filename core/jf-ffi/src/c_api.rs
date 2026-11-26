//! The C ABI: the `JellyfeshCore` API with UTF-8 JSON in and out, for C# (ADR-0002's fallback
//! path, used from the start) and any other C caller.
//!
//! * `jf_core_new` returns an opaque handle; `jf_core_free` shuts the core down and frees it.
//!   `jf_core_new_with_secret_store` also takes the app's secret store as a callback.
//! * Values are the mirror types in [`crate::types`] as JSON (`docs/ffi.md`, "JSON").
//! * Query functions take their arguments as one JSON object named like the UniFFI method's
//!   parameters (`{"offset": 0, "limit": 50}`; NULL or `{}` when there are none) and return
//!   `{"ok": <value>}` or `{"error": {"kind": "...", "message": "..."}}`.
//! * Every returned string belongs to the caller, who frees it with `jf_string_free`.
//! * A panic in the core never unwinds into the caller: it becomes an error result, 0 or NULL.
//!
//! The handle may be used from any thread at once, but `jf_core_free` must not race with any
//! other call on the same handle.

use std::ffi::{CStr, CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, PoisonError, RwLock};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::api::{CoreListener, JellyfeshCore};
use crate::error::CoreFfiError;
use crate::secrets::{SecretStore, SecretStoreError};
use crate::types::{AlbumSort, CoreConfig, Event, LyricTimeline, StoreEntitlement, TrackQuery, TrackRef};

/// Receives each event as JSON. `event_json` is valid only during the call.
pub type JfEventCallback = Option<unsafe extern "C" fn(user_data: *mut c_void, event_json: *const c_char)>;

/// Answers a secret store request; see [`JfSecretCallback`].
pub type JfSecretReply = Option<unsafe extern "C" fn(reply_context: *mut c_void, reply_json: *const c_char)>;

/// The app's secret store over the C ABI (ADR-0015). `request_json` is
/// `{"op": "get", "key": "..."}`, `{"op": "set", "key": "...", "value": "..."}` or
/// `{"op": "delete", "key": "..."}`; the callback answers before it returns by calling
/// `reply(reply_context, reply_json)` once with `{"ok": null}` (no value, or done),
/// `{"ok": "<value>"}` (the value of a `get`) or `{"error": "<message>"}`. Both strings are valid
/// only during the call. It is called from core threads, possibly from several at once, until
/// the core has shut down; shutting down waits for a call in progress, so the callback must never
/// wait for the thread that calls `jf_core_shutdown` or `jf_core_free`.
pub type JfSecretCallback = Option<
    unsafe extern "C" fn(
        user_data: *mut c_void,
        request_json: *const c_char,
        reply: JfSecretReply,
        reply_context: *mut c_void,
    ),
>;

// ---- lifecycle ----------------------------------------------------------------------------------

/// Starts a core from a JSON `CoreConfig`. Returns NULL on failure, and then, when `error_json`
/// is not NULL, stores `{"kind": "...", "message": "..."}` there (free it with
/// `jf_string_free`).
///
/// # Safety
///
/// `config_json` must be NULL or a NUL-terminated string; `error_json` must be NULL or point to
/// writable storage for one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_core_new(
    config_json: *const c_char,
    error_json: *mut *mut c_char,
) -> *mut JellyfeshCore {
    let started = guard(
        || {
            // SAFETY: forwarded from this function's contract.
            let config: CoreConfig = unsafe { parse(config_json) }?;
            JellyfeshCore::start(config, None, None)
        },
        || Err(panicked()),
    );
    match started {
        Ok(core) => Box::into_raw(Box::new(core)),
        Err(error) => {
            if !error_json.is_null() {
                // SAFETY: the caller guarantees `error_json` points to writable pointer storage.
                unsafe { error_json.write(into_c(error.to_json().to_string())) };
            }
            ptr::null_mut()
        }
    }
}

/// Like `jf_core_new`, with the app's secret store for the sign-ins of online sources: the core
/// calls `secrets(secrets_user_data, ...)` (see [`JfSecretCallback`]) from its threads until
/// `jf_core_shutdown` or `jf_core_free` returns, never afterwards. A NULL callback is the same as
/// `jf_core_new`.
///
/// # Safety
///
/// As for `jf_core_new`; and `secrets` must be safe to call from any thread with
/// `secrets_user_data` until the core has shut down.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_core_new_with_secret_store(
    config_json: *const c_char,
    secrets: JfSecretCallback,
    secrets_user_data: *mut c_void,
    error_json: *mut *mut c_char,
) -> *mut JellyfeshCore {
    let started = guard(
        || {
            // SAFETY: forwarded from this function's contract.
            let config: CoreConfig = unsafe { parse(config_json) }?;
            let Some(callback) = secrets else {
                return JellyfeshCore::start(config, None, None);
            };
            let store = Arc::new(CSecretStore {
                callback,
                user_data: secrets_user_data,
                open: RwLock::new(true),
            });
            let closing = store.clone();
            JellyfeshCore::start(
                config,
                Some(store as Arc<dyn SecretStore>),
                Some(Box::new(move || closing.close())),
            )
        },
        || Err(panicked()),
    );
    match started {
        Ok(core) => Box::into_raw(Box::new(core)),
        Err(error) => {
            if !error_json.is_null() {
                // SAFETY: the caller guarantees `error_json` points to writable pointer storage.
                unsafe { error_json.write(into_c(error.to_json().to_string())) };
            }
            ptr::null_mut()
        }
    }
}

/// A C callback as the app's [`SecretStore`]. Calls stop once the core has shut down.
struct CSecretStore {
    callback: unsafe extern "C" fn(*mut c_void, *const c_char, JfSecretReply, *mut c_void),
    user_data: *mut c_void,
    /// Held for reading during a call; `close` waits for calls in progress.
    open: RwLock<bool>,
}

// SAFETY: `jf_core_new_with_secret_store`'s contract makes the callback and its `user_data`
// usable from any thread until the core has shut down, and `close` (run then) stops every call.
unsafe impl Send for CSecretStore {}
// SAFETY: as for `Send`; `open` synchronises the only mutable state.
unsafe impl Sync for CSecretStore {}

impl CSecretStore {
    fn close(&self) {
        *self.open.write().unwrap_or_else(PoisonError::into_inner) = false;
    }

    /// Sends one request and reads the answer: `Ok(None)` for `{"ok": null}`.
    fn call(&self, request: serde_json::Value) -> Result<Option<String>, SecretStoreError> {
        let unavailable = |reason: String| SecretStoreError::Unavailable { reason };
        let open = self.open.read().unwrap_or_else(PoisonError::into_inner);
        if !*open {
            return Err(unavailable("the core has shut down".into()));
        }
        let request = CString::new(request.to_string()).map_err(|e| unavailable(e.to_string()))?;
        let mut answer: Option<String> = None;
        // SAFETY: the callback may be called with `user_data` on any thread while the store is
        // open (checked above, and `close` waits for this read guard). `reply_to` only writes to
        // `answer`, which outlives the call, and the contract limits `reply` to the call.
        unsafe {
            (self.callback)(
                self.user_data,
                request.as_ptr(),
                Some(reply_to),
                (&raw mut answer).cast::<c_void>(),
            );
        }
        drop(open);
        let answer = answer.ok_or_else(|| unavailable("the secret store did not answer".into()))?;
        let value: serde_json::Value =
            serde_json::from_str(&answer).map_err(|e| unavailable(format!("unreadable answer: {e}")))?;
        if let Some(error) = value.get("error") {
            return Err(unavailable(
                error.as_str().map_or_else(|| error.to_string(), str::to_owned),
            ));
        }
        match value.get("ok") {
            Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(text)) => Ok(Some(text.clone())),
            _ => Err(unavailable(format!("unreadable answer: {answer}"))),
        }
    }
}

/// Copies the answer of a secret store request into the caller's slot.
///
/// # Safety
///
/// `reply_context` must be the context `CSecretStore::call` passed, during that call; `reply_json`
/// must be NULL or a NUL-terminated string.
unsafe extern "C" fn reply_to(reply_context: *mut c_void, reply_json: *const c_char) {
    if reply_context.is_null() || reply_json.is_null() {
        return;
    }
    // SAFETY: per the contract, `reply_context` points to the `Option<String>` of the call in
    // progress and `reply_json` is a valid string for this call.
    unsafe {
        let slot = &mut *reply_context.cast::<Option<String>>();
        *slot = Some(CStr::from_ptr(reply_json).to_string_lossy().into_owned());
    }
}

impl SecretStore for CSecretStore {
    fn get(&self, key: String) -> Result<Option<String>, SecretStoreError> {
        self.call(serde_json::json!({ "op": "get", "key": key }))
    }

    fn set(&self, key: String, value: String) -> Result<(), SecretStoreError> {
        self.call(serde_json::json!({ "op": "set", "key": key, "value": value }))
            .map(|_| ())
    }

    fn delete(&self, key: String) -> Result<(), SecretStoreError> {
        self.call(serde_json::json!({ "op": "delete", "key": key }))
            .map(|_| ())
    }
}

/// Shuts the core down (if `jf_core_shutdown` has not) and frees the handle. NULL is ignored.
///
/// # Safety
///
/// `core` must be NULL or a handle from `jf_core_new` that has not been freed, and no other call
/// may use it during or after this one.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_core_free(core: *mut JellyfeshCore) {
    if core.is_null() {
        return;
    }
    // SAFETY: the handle came from `Box::into_raw` in `jf_core_new` and is freed only once.
    let core = unsafe { Box::from_raw(core) };
    guard(move || drop(core), || ());
}

/// Saves the session, stops playback and the listener. Later queries fail with `ShutDown`,
/// `jf_core_send` returns 0 and `jf_core_snapshot` returns the last state.
///
/// # Safety
///
/// `core` must be NULL or a live handle from `jf_core_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_core_shutdown(core: *const JellyfeshCore) {
    // SAFETY: forwarded from this function's contract.
    if let Some(core) = unsafe { core.as_ref() } {
        guard(|| core.shutdown(), || ());
    }
}

/// Queues a JSON `Command` and returns its id (failures arrive as a `CommandFailed` event with
/// that id). Returns 0 when the JSON is not a command, the handle is NULL, or the core is shut
/// down.
///
/// # Safety
///
/// `core` must be NULL or a live handle from `jf_core_new`; `command_json` must be NULL or a
/// NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_core_send(core: *const JellyfeshCore, command_json: *const c_char) -> u64 {
    guard(
        || {
            // SAFETY: forwarded from this function's contract.
            let core = unsafe { core.as_ref() }?;
            // SAFETY: as above.
            let command = unsafe { parse(command_json) }.ok()?;
            Some(core.send(command))
        },
        || None,
    )
    .unwrap_or(0)
}

/// The current `Snapshot` as JSON, or NULL for a NULL handle.
///
/// # Safety
///
/// `core` must be NULL or a live handle from `jf_core_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_core_snapshot(core: *const JellyfeshCore) -> *mut c_char {
    // SAFETY: forwarded from this function's contract.
    let Some(core) = (unsafe { core.as_ref() }) else {
        return ptr::null_mut();
    };
    guard(
        || to_json(&core.snapshot()).map_or(ptr::null_mut(), into_c),
        ptr::null_mut,
    )
}

/// Calls `callback(user_data, event_json)` with every event from now on, on one core thread, in
/// order; a NULL callback removes the listener. When this returns, the previous callback is no
/// longer called (unless this is called from inside it), so its `user_data` can be released.
/// Never block inside the callback waiting for a thread that is calling this function,
/// `jf_core_shutdown` or `jf_core_free`.
///
/// # Safety
///
/// `core` must be NULL or a live handle from `jf_core_new`. `callback` must be safe to call from
/// any thread with `user_data` until it is replaced or the core is shut down.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_core_set_listener(
    core: *const JellyfeshCore,
    callback: JfEventCallback,
    user_data: *mut c_void,
) {
    // SAFETY: forwarded from this function's contract.
    let Some(core) = (unsafe { core.as_ref() }) else {
        return;
    };
    let listener =
        callback.map(|callback| Arc::new(CListener { callback, user_data }) as Arc<dyn CoreListener>);
    guard(|| core.set_listener(listener), || ());
}

/// A C callback as a [`CoreListener`]: each event goes out as JSON.
struct CListener {
    callback: unsafe extern "C" fn(*mut c_void, *const c_char),
    user_data: *mut c_void,
}

// SAFETY: `jf_core_set_listener`'s contract makes the callback and its `user_data` usable from any
// thread until the listener is replaced, which is all this type does with them.
unsafe impl Send for CListener {}
// SAFETY: as for `Send`; the type has no interior mutability.
unsafe impl Sync for CListener {}

impl CoreListener for CListener {
    fn on_event(&self, event: Event) {
        let Some(json) = to_json(&event).and_then(|json| CString::new(json).ok()) else {
            return;
        };
        // SAFETY: the callback may be called with `user_data` on this thread (see `Send`), and
        // `json` outlives the call.
        unsafe { (self.callback)(self.user_data, json.as_ptr()) };
    }
}

// ---- queries ------------------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct NoArgs {}

#[derive(serde::Deserialize)]
struct Page {
    offset: u32,
    limit: u32,
}

#[derive(serde::Deserialize)]
struct Limit {
    limit: u32,
}

#[derive(serde::Deserialize)]
struct Id {
    id: i64,
}

#[derive(serde::Deserialize)]
struct AlbumId {
    album_id: i64,
}

#[derive(serde::Deserialize)]
struct ArtistId {
    artist_id: i64,
}

#[derive(serde::Deserialize)]
struct Query {
    query: TrackQuery,
}

#[derive(serde::Deserialize)]
struct Text {
    text: String,
    limit: u32,
}

#[derive(serde::Deserialize)]
struct PlaylistPage {
    id: i64,
    offset: u32,
    limit: u32,
}

#[derive(serde::Deserialize)]
struct TrackArg {
    track: TrackRef,
}

#[derive(serde::Deserialize)]
struct Thumbnail {
    hash: String,
    size_px: u32,
}

#[derive(serde::Deserialize)]
struct SortedPage {
    sort: AlbumSort,
    descending: bool,
    offset: u32,
    limit: u32,
}

#[derive(serde::Deserialize)]
struct Export {
    id: i64,
    path: String,
}

#[derive(serde::Deserialize)]
struct EntitlementArg {
    entitlement: StoreEntitlement,
}

#[derive(serde::Deserialize)]
struct SchemaArgs {
    plugin: String,
    language: String,
}

#[derive(serde::Deserialize)]
struct ArtworkArgs {
    url: String,
    size_px: u32,
}

#[derive(serde::Deserialize)]
struct BarArgs {
    timeline: LyricTimeline,
    t_ms: u32,
}

/// Defines one query function per line: `name(Args) => |core, args| call`.
macro_rules! queries {
    ($($(#[$doc:meta])* $name:ident($args:ty) => |$core:ident, $a:pat_param| $call:expr;)*) => {
        $(
            $(#[$doc])*
            ///
            /// # Safety
            ///
            /// `core` must be NULL or a live handle from `jf_core_new`; `args_json` must be NULL
            /// or a NUL-terminated string.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(core: *const JellyfeshCore, args_json: *const c_char) -> *mut c_char {
                // SAFETY: forwarded from this function's contract.
                unsafe { query(core, args_json, |$core: &JellyfeshCore, $a: $args| $call) }
            }
        )*
    };
}

queries! {
    /// `{"offset", "limit"}` → `[Album]`.
    jf_core_albums(Page) => |core, a| core.albums(a.offset, a.limit);
    /// `{"sort": AlbumSort, "descending", "offset", "limit"}` → `[Album]`.
    jf_core_albums_sorted(SortedPage) => |core, a| core.albums_sorted(a.sort, a.descending, a.offset, a.limit);
    /// `{"offset", "limit"}` → `[Artist]`.
    jf_core_artists(Page) => |core, a| core.artists(a.offset, a.limit);
    /// `{"query": TrackQuery}` → `[Track]`.
    jf_core_tracks(Query) => |core, a| core.tracks(a.query);
    /// `{"id"}` → `Track` or null.
    jf_core_track(Id) => |core, a| core.track(a.id);
    /// `{"album_id"}` → `[Track]`.
    jf_core_album_tracks(AlbumId) => |core, a| core.album_tracks(a.album_id);
    /// `{"artist_id"}` → `[Album]`.
    jf_core_artist_albums(ArtistId) => |core, a| core.artist_albums(a.artist_id);
    /// `{"text", "limit"}` → `SearchResults`.
    jf_core_search_library(Text) => |core, a| core.search_library(a.text, a.limit);
    /// `{"text", "limit"}` → `[TrackRef]`.
    jf_core_tracks_for_query(Text) => |core, a| core.tracks_for_query(a.text, a.limit);
    /// `{"limit"}` → `[Track]`.
    jf_core_recently_added(Limit) => |core, a| core.recently_added(a.limit);
    /// `{"limit"}` → `[TrackRow]`.
    jf_core_recently_played(Limit) => |core, a| core.recently_played(a.limit);
    /// `{"limit"}` → `[TrackRow]`.
    jf_core_most_played(Limit) => |core, a| core.most_played(a.limit);
    /// No arguments → `[PlaylistSummary]`.
    jf_core_playlists(NoArgs) => |core, _a| core.playlists();
    /// `{"id"}` → `PlaylistSummary` or null.
    jf_core_playlist(Id) => |core, a| core.playlist(a.id);
    /// `{"id", "offset", "limit"}` → `[PlaylistEntry]`.
    jf_core_playlist_entries(PlaylistPage) => |core, a| core.playlist_entries(a.id, a.offset, a.limit);
    /// `{"id", "path"}` → null, once the M3U8 file is written.
    jf_core_export_playlist(Export) => |core, a| core.export_playlist(a.id, a.path);
    /// `{"offset", "limit"}` → `[TrackRow]`.
    jf_core_favorites(Page) => |core, a| core.favorites(a.offset, a.limit);
    /// `{"track": TrackRef}` → `bool`.
    jf_core_is_favorite(TrackArg) => |core, a| core.is_favorite(a.track);
    /// `{"hash", "size_px"}` → path or null.
    jf_core_artwork_thumbnail(Thumbnail) => |core, a| core.artwork_thumbnail(a.hash, a.size_px);
    /// `{"offset", "limit"}` → `[QueueItem]`.
    jf_core_queue_window(Page) => |core, a| core.queue_window(a.offset, a.limit);
    /// No arguments → `[HistoryEntry]`.
    jf_core_history(NoArgs) => |core, _a| core.history();
    /// No arguments → `[path]`.
    jf_core_roots(NoArgs) => |core, _a| core.roots();
    /// No arguments → `LibraryStats`.
    jf_core_stats(NoArgs) => |core, _a| core.stats();
    /// `{"entitlement": StoreEntitlement}` → null.
    jf_core_set_store_entitlement(EntitlementArg) => |core, a| core.set_store_entitlement(a.entitlement);
    /// `{"plugin", "language"}` → schema text or null.
    jf_core_localized_settings_schema(SchemaArgs) => |core, a| core.localized_settings_schema(a.plugin, a.language);
    /// `{"url", "size_px"}` → path or null (an `Artwork` event follows when null).
    jf_core_source_artwork(ArtworkArgs) => |core, a| core.source_artwork(a.url, a.size_px);
}

/// `{"timeline": LyricTimeline, "t_ms"}` → `{"ok": LyricsBar}` (or `{"error"}` for bad JSON).
///
/// # Safety
///
/// `args_json` must be NULL or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_lyrics_bar(args_json: *const c_char) -> *mut c_char {
    let result = guard(
        || {
            if args_json.is_null() {
                return Err(CoreFfiError::InvalidArgument("lyrics_bar needs arguments".into()));
            }
            // SAFETY: forwarded from the caller's contract.
            let args: BarArgs = unsafe { parse(args_json) }?;
            Ok(crate::api::lyrics_bar(args.timeline, args.t_ms))
        },
        || Err(panicked()),
    );
    into_c(envelope(result))
}

/// `{"address"}` → `{"ok": bool}`: whether the address is on the local network
/// (`address_is_local` in `docs/ffi.md`).
///
/// # Safety
///
/// `args_json` must be NULL or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_address_is_local(args_json: *const c_char) -> *mut c_char {
    #[derive(serde::Deserialize)]
    struct Args {
        address: String,
    }
    let result = guard(
        || {
            if args_json.is_null() {
                return Err(CoreFfiError::InvalidArgument(
                    "address_is_local needs an address".into(),
                ));
            }
            // SAFETY: forwarded from the caller's contract.
            let args: Args = unsafe { parse(args_json) }?;
            Ok(crate::api::address_is_local(args.address))
        },
        || Err(panicked()),
    );
    into_c(envelope(result))
}

/// `{"template"}` → `{"ok": "Album Artist/Album/01 Title.flac"}` (`download_preview`).
///
/// # Safety
///
/// `args_json` must be NULL or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_download_preview(args_json: *const c_char) -> *mut c_char {
    #[derive(serde::Deserialize)]
    struct Args {
        template: String,
    }
    let result = guard(
        || {
            if args_json.is_null() {
                return Err(CoreFfiError::InvalidArgument(
                    "download_preview needs a template".into(),
                ));
            }
            // SAFETY: forwarded from the caller's contract.
            let args: Args = unsafe { parse(args_json) }?;
            Ok(crate::api::download_preview(args.template))
        },
        || Err(panicked()),
    );
    into_c(envelope(result))
}

/// Parses the arguments, runs one query and wraps the result in `{"ok"}` or `{"error"}`.
///
/// # Safety
///
/// As for the query functions.
unsafe fn query<A: DeserializeOwned, T: Serialize>(
    core: *const JellyfeshCore,
    args_json: *const c_char,
    run: impl FnOnce(&JellyfeshCore, A) -> Result<T, CoreFfiError>,
) -> *mut c_char {
    let result = guard(
        || {
            // SAFETY: forwarded from the caller's contract.
            let core = unsafe { core.as_ref() }
                .ok_or_else(|| CoreFfiError::InvalidArgument("the core handle is NULL".into()))?;
            let args = if args_json.is_null() {
                serde_json::from_str("{}").map_err(invalid)?
            } else {
                // SAFETY: forwarded from the caller's contract.
                unsafe { parse(args_json) }?
            };
            run(core, args)
        },
        || Err(panicked()),
    );
    into_c(envelope(result))
}

// ---- free functions -----------------------------------------------------------------------------

/// `{"ok": [DeviceInfo]}` or `{"error"}`.
#[unsafe(no_mangle)]
pub extern "C" fn jf_output_devices() -> *mut c_char {
    into_c(envelope(guard(crate::api::output_devices, || Err(panicked()))))
}

/// `{"ok": ["mp3", "flac", …]}` (`audio_extensions`).
#[unsafe(no_mangle)]
pub extern "C" fn jf_audio_extensions() -> *mut c_char {
    into_c(envelope(guard(
        || Ok(crate::api::audio_extensions()),
        || Err(panicked()),
    )))
}

/// `{"ok": [EqPreset]}`.
#[unsafe(no_mangle)]
pub extern "C" fn jf_eq_presets() -> *mut c_char {
    into_c(envelope(guard(
        || Ok(crate::api::eq_presets()),
        || Err(panicked()),
    )))
}

/// See `monotonic_now_ns` in `docs/ffi.md`.
#[unsafe(no_mangle)]
pub extern "C" fn jf_monotonic_now_ns() -> u64 {
    crate::api::monotonic_now_ns()
}

/// The default data folder as a plain string, or NULL when the platform has none.
#[unsafe(no_mangle)]
pub extern "C" fn jf_default_data_dir() -> *mut c_char {
    guard(crate::api::default_data_dir, || None).map_or(ptr::null_mut(), into_c)
}

/// The default cache folder as a plain string, or NULL when the platform has none.
#[unsafe(no_mangle)]
pub extern "C" fn jf_default_cache_dir() -> *mut c_char {
    guard(crate::api::default_cache_dir, || None).map_or(ptr::null_mut(), into_c)
}

/// The library version as a plain string.
#[unsafe(no_mangle)]
pub extern "C" fn jf_library_version() -> *mut c_char {
    into_c(crate::api::library_version())
}

/// Frees a string returned by any `jf_` function. NULL is ignored.
///
/// # Safety
///
/// `s` must be NULL or a string returned by a `jf_` function that has not been freed yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn jf_string_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: every string handed out comes from `CString::into_raw` in `into_c`, and the
        // caller frees each once.
        drop(unsafe { CString::from_raw(s) });
    }
}

// ---- helpers ------------------------------------------------------------------------------------

/// Runs `f`, turning a panic into `fallback()` so it never unwinds into C.
fn guard<T>(f: impl FnOnce() -> T, fallback: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| fallback())
}

fn panicked() -> CoreFfiError {
    CoreFfiError::Core("internal error: the core panicked (see the log)".into())
}

fn invalid(error: serde_json::Error) -> CoreFfiError {
    CoreFfiError::InvalidArgument(format!("invalid JSON: {error}"))
}

/// Reads a NUL-terminated UTF-8 JSON string into `T`.
///
/// # Safety
///
/// `json` must be NULL or a NUL-terminated string that stays valid during the call.
unsafe fn parse<T: DeserializeOwned>(json: *const c_char) -> Result<T, CoreFfiError> {
    if json.is_null() {
        return Err(CoreFfiError::InvalidArgument("the JSON argument is NULL".into()));
    }
    // SAFETY: the caller guarantees a valid NUL-terminated string.
    let bytes = unsafe { CStr::from_ptr(json) }.to_bytes();
    serde_json::from_slice(bytes).map_err(invalid)
}

fn to_json<T: Serialize>(value: &T) -> Option<String> {
    match serde_json::to_string(value) {
        Ok(json) => Some(json),
        Err(error) => {
            tracing::error!("could not serialise a value for the C ABI: {error}");
            None
        }
    }
}

/// `{"ok": <value>}` or `{"error": {...}}`. The value is written straight from its type, never
/// through `serde_json::Value` (which sorts keys): every enum's `type` tag must stay the first
/// key, as .NET 8's System.Text.Json requires.
fn envelope<T: Serialize>(result: Result<T, CoreFfiError>) -> String {
    match result.and_then(|v| serde_json::to_string(&v).map_err(|e| CoreFfiError::Core(e.to_string()))) {
        Ok(json) => format!("{{\"ok\":{json}}}"),
        Err(error) => serde_json::json!({ "error": error.to_json() }).to_string(),
    }
}

/// Hands a string to the caller. Strings with an interior NUL (never JSON, which escapes it)
/// lose everything from the NUL on.
fn into_c(s: String) -> *mut c_char {
    let s = match CString::new(s) {
        Ok(s) => s,
        Err(error) => {
            let end = error.nul_position();
            let mut bytes = error.into_vec();
            bytes.truncate(end);
            CString::new(bytes).unwrap_or_default()
        }
    };
    s.into_raw()
}
