//! The C ABI as C# uses it: JSON in and out, a listener callback with user data, freeing.

// These tests are a C caller: every call goes through raw pointers.
#![allow(unsafe_code)]

mod common;

use std::ffi::{CStr, CString, c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use jf_ffi::c_api::*;
use serde_json::{Value, json};

/// Takes ownership of a string the library returned.
fn take(s: *mut c_char) -> String {
    assert!(!s.is_null());
    // SAFETY: `s` is a live string returned by a `jf_` function, freed right after.
    let out = unsafe { CStr::from_ptr(s) }.to_str().unwrap().to_owned();
    // SAFETY: as above; freed once.
    unsafe { jf_string_free(s) };
    out
}

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

type Query = unsafe extern "C" fn(*const jf_ffi::JellyfeshCore, *const c_char) -> *mut c_char;

fn query(f: Query, core: *const jf_ffi::JellyfeshCore, args: Value) -> Value {
    let args = c(&args.to_string());
    // SAFETY: `core` is live for the test and `args` outlives the call.
    serde_json::from_str(&take(unsafe { f(core, args.as_ptr()) })).unwrap()
}

fn ok(result: Value) -> Value {
    assert!(result.get("error").is_none(), "{result}");
    result["ok"].clone()
}

fn send(core: *const jf_ffi::JellyfeshCore, command: Value) -> u64 {
    let command = c(&command.to_string());
    // SAFETY: as in `query`.
    unsafe { jf_core_send(core, command.as_ptr()) }
}

/// Collects event JSON from the callback.
#[derive(Default)]
struct Inbox {
    events: Mutex<Vec<Value>>,
    arrived: Condvar,
}

impl Inbox {
    /// Waits until an event after `from` satisfies `done`; returns the index after it.
    fn wait(&self, from: usize, timeout: Duration, done: impl Fn(&Value) -> bool) -> usize {
        let end = Instant::now() + timeout;
        let mut events = self.events.lock().unwrap();
        loop {
            if let Some(i) = events.iter().skip(from).position(&done) {
                return from + i + 1;
            }
            let left = end
                .checked_duration_since(Instant::now())
                .unwrap_or_else(|| panic!("timed out; events: {:#?}", &events[from..]));
            events = self.arrived.wait_timeout(events, left).unwrap().0;
        }
    }

    /// Waits until no event has arrived for `quiet`.
    fn settle(&self, quiet: Duration) -> Vec<Value> {
        let mut events = self.events.lock().unwrap();
        loop {
            let count = events.len();
            events = self.arrived.wait_timeout(events, quiet).unwrap().0;
            if events.len() == count {
                return events.clone();
            }
        }
    }
}

unsafe extern "C" fn on_event(user_data: *mut c_void, event_json: *const c_char) {
    // SAFETY: `user_data` is the test's `Inbox`, which outlives the listener.
    let inbox = unsafe { &*user_data.cast::<Inbox>() };
    // SAFETY: the library passes a NUL-terminated string valid during the call.
    let json = unsafe { CStr::from_ptr(event_json) }.to_str().unwrap();
    inbox
        .events
        .lock()
        .unwrap()
        .push(serde_json::from_str(json).unwrap());
    inbox.arrived.notify_all();
}

fn start(config: Value) -> *mut jf_ffi::JellyfeshCore {
    let config = c(&config.to_string());
    let mut error = ptr::null_mut();
    // SAFETY: valid string and error slot.
    let core = unsafe { jf_core_new(config.as_ptr(), &mut error) };
    assert!(!core.is_null(), "{}", take(error));
    core
}

/// Calls `probe` every `every` until it returns true, or panics after `timeout`.
fn poll(timeout: Duration, every: Duration, what: &str, mut probe: impl FnMut() -> bool) {
    let end = Instant::now() + timeout;
    while !probe() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(every);
    }
}

fn titles(rows: &Value, title: impl Fn(&Value) -> &Value) -> Vec<String> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|row| title(row).as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn the_shared_scenario_over_json() {
    let scenario = common::scenario();
    let expect = &scenario["expect"];
    let timeout = Duration::from_millis(scenario["timeout_ms"].as_u64().unwrap());
    let quiet = Duration::from_millis(scenario["quiet_ms"].as_u64().unwrap());
    let every = Duration::from_millis(scenario["poll_ms"].as_u64().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let music = common::write_tones(dir.path(), &scenario);

    // 1. Start with the scenario's config and temporary folders.
    let mut config = scenario["config"].clone();
    config["data_dir"] = json!(dir.path().join("data"));
    config["cache_dir"] = json!(dir.path().join("cache"));
    let core = start(config);
    let inbox = Box::new(Inbox::default());
    let user_data = (&*inbox as *const Inbox).cast_mut().cast::<c_void>();
    // SAFETY: `inbox` outlives the listener: it is removed before the inbox is dropped.
    unsafe { jf_core_set_listener(core, Some(on_event), user_data) };

    // 2. AddRoot scans the folder.
    assert!(send(core, json!({"type": "AddRoot", "path": music})) > 0);
    let mut seen = inbox.wait(0, timeout, |e| {
        e["type"] == "Scan" && e["status"]["type"] == "Finished"
    });
    let scan = inbox.events.lock().unwrap()[seen - 1].clone();
    assert_eq!(scan["status"]["report"]["added"], expect["scan_added"]);

    // 3. Query and search the library.
    let tracks = ok(query(
        jf_core_tracks,
        core,
        json!({"query": {"offset": 0, "limit": 100, "sort": "Path", "descending": false,
                         "album_id": null, "artist_id": null}}),
    ));
    let found = ok(query(
        jf_core_search_library,
        core,
        json!({"text": scenario["search"]["text"], "limit": scenario["search"]["limit"]}),
    ));
    assert_eq!(
        titles(&found["tracks"], |t| &t["title"]),
        common::strings(&scenario, "/search/titles")
    );
    assert_eq!(ok(query(jf_core_roots, core, json!({}))), json!([music]));
    let refs: Vec<Value> = tracks
        .as_array()
        .unwrap()
        .iter()
        .map(|t| json!({"type": "Library", "id": t["id"]}))
        .collect();

    // 4. A playlist of every track.
    let name = &scenario["playlist"]["name"];
    let request = send(
        core,
        json!({"type": "CreatePlaylist", "name": name, "tracks": refs}),
    );
    seen = inbox.wait(seen, timeout, |e| {
        e["type"] == "PlaylistCreated" && e["request"] == request
    });
    let playlist = inbox.events.lock().unwrap()[seen - 1]["id"].clone();
    let summary = ok(query(jf_core_playlist, core, json!({"id": playlist})));
    assert_eq!((&summary["name"], &summary["entry_count"]), (name, &json!(2)));
    let entries = ok(query(
        jf_core_playlist_entries,
        core,
        json!({"id": playlist, "offset": 0, "limit": 50}),
    ));
    assert_eq!(
        titles(&entries, |e| &e["row"]["title"]),
        common::strings(&scenario, "/expect/playlist_titles")
    );
    let lists = ok(query(jf_core_playlists, core, json!({})));
    assert!(lists.as_array().unwrap().iter().any(|p| p["id"] == playlist));
    // Enum tags come first in every object the C ABI writes (System.Text.Json needs that).
    let args = c(&json!({"id": playlist, "offset": 0, "limit": 50}).to_string());
    // SAFETY: `core` is live and `args` outlives the call.
    let raw = take(unsafe { jf_core_playlist_entries(core, args.as_ptr()) });
    assert!(raw.contains(r#""track":{"type":"Library","id":"#), "{raw}");

    // 5. A favourite.
    let favorite = &scenario["favorite"];
    let index = tracks
        .as_array()
        .unwrap()
        .iter()
        .position(|t| &t["title"] == favorite)
        .unwrap();
    let track = refs[index].clone();
    send(
        core,
        json!({"type": "SetFavorite", "track": track, "favorite": true}),
    );
    poll(timeout, every, "the favourite", || {
        ok(query(jf_core_is_favorite, core, json!({"track": track}))) == json!(true)
    });
    let favorites = ok(query(jf_core_favorites, core, json!({"offset": 0, "limit": 50})));
    assert_eq!(
        titles(&favorites, |r| &r["title"]),
        common::strings(&scenario, "/expect/favorite_titles")
    );

    // 6. Export.
    let export = dir
        .path()
        .join(scenario["playlist"]["export_file"].as_str().unwrap());
    let exported = ok(query(
        jf_core_export_playlist,
        core,
        json!({"id": playlist, "path": export}),
    ));
    assert_eq!(exported, Value::Null);
    let text = std::fs::read_to_string(&export).unwrap();
    for needle in common::strings(&scenario, "/playlist/export_contains") {
        assert!(text.contains(&needle), "{needle:?} missing from {text}");
    }

    // 7. Play the playlist.
    let collection = json!({"type": "Playlist", "id": playlist});
    assert!(
        send(
            core,
            json!({"type": "PlayCollection", "collection": collection, "start": 0, "shuffle": false})
        ) > 0
    );
    let before_play = seen;
    inbox.wait(before_play, timeout, |e| {
        e["type"] == "Playback" && e["snapshot"]["status"] == expect["final_status"]
    });
    let events = inbox.settle(quiet);
    let events = &events[before_play..];
    let mut announced: Vec<(&Value, &str)> = events
        .iter()
        .filter(|e| e["type"] == "NowPlaying" && !e["snapshot"].is_null())
        .map(|e| (&e["snapshot"]["uid"], e["snapshot"]["title"].as_str().unwrap()))
        .collect();
    // Consecutive announcements of the same entry count once (see the scenario's "about").
    announced.dedup_by_key(|(uid, _)| *uid);
    let played: Vec<&str> = announced.into_iter().map(|(_, title)| title).collect();
    assert_eq!(played, common::strings(&scenario, "/expect/now_playing_titles"));
    let last = |kind: &str| events.iter().rev().find(|e| e["type"] == kind).unwrap()["snapshot"].clone();
    // SAFETY: live handle.
    let snapshot: Value = serde_json::from_str(&take(unsafe { jf_core_snapshot(core) })).unwrap();
    assert_eq!(snapshot["playback"], last("Playback"));
    assert_eq!(snapshot["queue"], last("Queue"));
    assert_eq!(snapshot["now_playing"], last("NowPlaying"));
    assert_eq!(snapshot["queue"]["len"], expect["queue_len"]);
    let window = ok(query(
        jf_core_queue_window,
        core,
        json!({"offset": 0, "limit": 10}),
    ));
    assert_eq!(window.as_array().unwrap().len(), 2);

    // 8. Play statistics.
    let played = common::strings(&scenario, "/expect/played_titles");
    poll(timeout, every, "the plays", || {
        let mut recent = titles(
            &ok(query(jf_core_recently_played, core, json!({"limit": 50}))),
            |r| &r["title"],
        );
        recent.sort();
        recent.dedup();
        recent == played
    });

    // 9. Sorted albums and thumbnails.
    let albums = ok(query(
        jf_core_albums_sorted,
        core,
        json!({"sort": "Title", "descending": false, "offset": 0, "limit": 50}),
    ));
    assert_eq!(albums, json!([]));
    let hash = &scenario["unknown_artwork_hash"];
    let thumbnail = ok(query(
        jf_core_artwork_thumbnail,
        core,
        json!({"hash": hash, "size_px": 128}),
    ));
    assert_eq!(thumbnail, Value::Null);

    // Errors come back as {"error": {kind, message}}; bad commands are not sent.
    let bad = query(jf_core_albums, core, json!({"offset": "zero"}));
    assert_eq!(bad["error"]["kind"], "InvalidArgument", "{bad}");
    assert_eq!(send(core, json!({"type": "NoSuchCommand"})), 0);
    let failed = send(core, json!({"type": "PlayTracks", "tracks": [], "start": 0}));
    inbox.wait(0, timeout, |e| e["type"] == "CommandFailed" && e["id"] == failed);

    // 10. After shutdown: queries fail, the last snapshot stays, no more events.
    // SAFETY: live handle.
    unsafe { jf_core_shutdown(core) };
    let count = inbox.events.lock().unwrap().len();
    let after = query(jf_core_stats, core, json!({}));
    assert_eq!(after["error"]["kind"], "ShutDown", "{after}");
    assert_eq!(send(core, json!({"type": "Play"})), 0);
    // SAFETY: live handle.
    let last_state: Value = serde_json::from_str(&take(unsafe { jf_core_snapshot(core) })).unwrap();
    assert_eq!(last_state["playback"]["status"], "Ended");
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(inbox.events.lock().unwrap().len(), count);
    // SAFETY: the handle is not used again.
    unsafe { jf_core_free(core) };
}

#[test]
fn replacing_the_listener_stops_the_old_callback() {
    let dir = tempfile::tempdir().unwrap();
    let core = start(json!({
        "data_dir": dir.path().join("data"),
        "cache_dir": dir.path().join("cache"),
        "output": {"type": "Null", "realtime": false},
        "buffer_ms": 200,
        "restore_session": false,
        "persist_session": false,
        "watch_folders": false,
    }));
    let first = Box::new(Inbox::default());
    let second = Box::new(Inbox::default());
    let data = |inbox: &Inbox| (inbox as *const Inbox).cast_mut().cast::<c_void>();
    // SAFETY: both inboxes outlive the core.
    unsafe { jf_core_set_listener(core, Some(on_event), data(&first)) };
    send(core, json!({"type": "SetVolume", "volume": 0.5}));
    first.wait(0, Duration::from_secs(10), |e| e["type"] == "Playback");
    // SAFETY: as above.
    unsafe { jf_core_set_listener(core, Some(on_event), data(&second)) };
    let before = first.events.lock().unwrap().len();
    send(core, json!({"type": "SetVolume", "volume": 0.25}));
    second.wait(0, Duration::from_secs(10), |e| e["type"] == "Playback");
    assert_eq!(first.events.lock().unwrap().len(), before);
    // SAFETY: as above.
    unsafe { jf_core_set_listener(core, None, ptr::null_mut()) };
    // SAFETY: not used again; freeing shuts the core down.
    unsafe { jf_core_free(core) };
}

#[test]
fn bad_input_and_null_handles() {
    let mut error = ptr::null_mut();
    let config = c(r#"{"data_dir": "/tmp"}"#);
    // SAFETY: valid string and error slot.
    let core = unsafe { jf_core_new(config.as_ptr(), &mut error) };
    assert!(core.is_null());
    let error: Value = serde_json::from_str(&take(error)).unwrap();
    assert_eq!(error["kind"], "InvalidArgument", "{error}");
    // SAFETY: NULL inputs are documented as accepted.
    unsafe {
        assert!(jf_core_new(ptr::null(), ptr::null_mut()).is_null());
        assert_eq!(jf_core_send(ptr::null(), ptr::null()), 0);
        assert!(jf_core_snapshot(ptr::null()).is_null());
        jf_core_set_listener(ptr::null(), None, ptr::null_mut());
        jf_core_shutdown(ptr::null());
        jf_core_free(ptr::null_mut());
        jf_string_free(ptr::null_mut());
    }
    let null = query(jf_core_albums, ptr::null(), json!({"offset": 0, "limit": 1}));
    assert_eq!(null["error"]["kind"], "InvalidArgument");

    assert_eq!(take(jf_library_version()), env!("CARGO_PKG_VERSION"));
    assert!(jf_monotonic_now_ns() > 0);
    let devices: Value = serde_json::from_str(&take(jf_output_devices())).unwrap();
    assert!(devices.get("ok").is_some() || devices.get("error").is_some());
    if std::env::var_os("HOME").is_some() && std::env::var_os("JF_DATA_DIR").is_none() {
        assert!(take(jf_default_data_dir()).to_lowercase().contains("jellyfesh"));
        assert!(!take(jf_default_cache_dir()).is_empty());
    }
}

/// The app's secret store behind the C callback: records each request and answers from `reply`.
struct Keychain {
    requests: Mutex<Vec<Value>>,
    locked: AtomicBool,
}

unsafe extern "C" fn on_secret(
    user_data: *mut c_void,
    request_json: *const c_char,
    reply: JfSecretReply,
    reply_context: *mut c_void,
) {
    // SAFETY: `user_data` is the test's `Keychain`, which outlives the core.
    let keychain = unsafe { &*user_data.cast::<Keychain>() };
    // SAFETY: the library passes a NUL-terminated string valid during the call.
    let request = unsafe { CStr::from_ptr(request_json) }.to_str().unwrap();
    keychain
        .requests
        .lock()
        .unwrap()
        .push(serde_json::from_str(request).unwrap());
    let answer = if keychain.locked.load(Ordering::SeqCst) {
        c(r#"{"error": "the keychain is locked"}"#)
    } else {
        c(r#"{"ok": null}"#)
    };
    // SAFETY: answered during the call with the context the library passed.
    unsafe { reply.unwrap()(reply_context, answer.as_ptr()) };
}

#[test]
fn the_apps_secret_store_over_callbacks() {
    for (locked, storage) in [(false, "Keystore"), (true, "SessionOnly")] {
        let dir = tempfile::tempdir().unwrap();
        let keychain = Box::new(Keychain {
            requests: Mutex::new(Vec::new()),
            locked: AtomicBool::new(locked),
        });
        let config = c(&json!({
            "data_dir": dir.path().join("data"),
            "cache_dir": dir.path().join("cache"),
            "output": {"type": "Null", "realtime": false},
            "buffer_ms": 200,
            "restore_session": false,
            "persist_session": false,
            "watch_folders": false,
        })
        .to_string());
        let user_data = (&*keychain as *const Keychain).cast_mut().cast::<c_void>();
        let mut error = ptr::null_mut();
        // SAFETY: valid strings and error slot; the keychain outlives the core.
        let core =
            unsafe { jf_core_new_with_secret_store(config.as_ptr(), Some(on_secret), user_data, &mut error) };
        assert!(!core.is_null(), "{}", take(error));
        // The store is checked once at start by reading an entry no plugin can name.
        poll(
            Duration::from_secs(10),
            Duration::from_millis(10),
            "the check",
            || !keychain.requests.lock().unwrap().is_empty(),
        );
        assert_eq!(
            keychain.requests.lock().unwrap()[0],
            json!({"op": "get", "key": "jellyfesh/~probe"})
        );
        poll(
            Duration::from_secs(10),
            Duration::from_millis(10),
            "the slice",
            || {
                // SAFETY: live handle.
                let snapshot: Value = serde_json::from_str(&take(unsafe { jf_core_snapshot(core) })).unwrap();
                snapshot["sources"]["secret_storage"] == storage
            },
        );
        // SAFETY: not used again; once freed, the callback is never called again.
        unsafe { jf_core_free(core) };
        let count = keychain.requests.lock().unwrap().len();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(keychain.requests.lock().unwrap().len(), count);
    }
}

/// The v0.0.8 free functions over JSON, as C# calls them: the local-network rule (Android's
/// definition) and the download name preview; NULL and bad JSON are errors, never a crash.
#[test]
fn stateless_v0_0_8_functions_over_json() {
    let call = |f: unsafe extern "C" fn(*const c_char) -> *mut c_char, args: Value| -> Value {
        let args = c(&args.to_string());
        // SAFETY: `args` outlives the call; the result is taken (and freed) at once.
        serde_json::from_str(&take(unsafe { f(args.as_ptr()) })).unwrap()
    };
    for (address, local) in [
        ("http://192.168.1.20:4533", true),
        ("http://[fe80::1]:8096", true),
        ("nas.local", true),
        ("100.64.0.1", true),
        ("http://127.0.0.1:4533", false),
        ("https://demo.navidrome.org", false),
    ] {
        assert_eq!(
            call(jf_address_is_local, json!({ "address": address })),
            json!({ "ok": local }),
            "{address}"
        );
        assert_eq!(
            jf_ffi::address_is_local(address.into()),
            local,
            "UniFFI: {address}"
        );
    }
    assert_eq!(
        call(jf_download_preview, json!({ "template": "{artist} - {title}" })),
        json!({ "ok": "Artist - Title.flac" })
    );
    assert_eq!(
        jf_ffi::download_preview(String::new()),
        "Album Artist/Album/01 Title.flac"
    );
    for f in [jf_address_is_local, jf_download_preview] {
        // SAFETY: NULL is allowed and answered with an error.
        let null: Value = serde_json::from_str(&take(unsafe { f(ptr::null()) })).unwrap();
        assert_eq!(null["error"]["kind"], "InvalidArgument", "{null}");
        let bad = call(f, json!({ "nothing": 1 }));
        assert_eq!(bad["error"]["kind"], "InvalidArgument", "{bad}");
    }
}
