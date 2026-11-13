//! HTTP streaming against a local server written here on `std::net`: range requests and
//! seeking, a dropped connection that resumes, expired and refused URLs resolved again, a slow
//! server (buffering events and rate-0 anchors), a missing item, gapless joins and MP4 with its
//! index at the end.

mod common;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use common::*;
use jf_engine::{Engine, EngineConfig, EngineEvent, HttpSource, OutputKind, PlayItem, StreamResolver};
use parking_lot::Mutex;

// ----- the test server --------------------------------------------------------------------------

/// How the server answers one path.
#[derive(Clone, Default)]
struct Route {
    body: Arc<Vec<u8>>,
    /// Answer every request with this status and no body.
    status: Option<u16>,
    /// The first response closes the connection after this many body bytes.
    drop_after: Option<usize>,
    /// Pause once for this long before sending this resource offset.
    stall: Option<(usize, Duration)>,
    /// Accepted `t=` tokens; other tokens get `refusal`.
    tokens: Option<Arc<Mutex<Vec<String>>>>,
    refusal: u16,
    /// Answer 200 with the whole body even to a range request.
    ignore_range: bool,
    /// Sleep this long after every 4 KiB sent.
    pace: Option<Duration>,
    content_type: &'static str,
}

#[derive(Debug, Clone)]
struct Logged {
    path: String,
    token: Option<String>,
    range_start: Option<u64>,
    status: u16,
}

struct Server {
    addr: SocketAddr,
    routes: Arc<Mutex<HashMap<String, Route>>>,
    log: Arc<Mutex<Vec<Logged>>>,
    stop: Arc<AtomicBool>,
}

impl Server {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let routes: Arc<Mutex<HashMap<String, Route>>> = Arc::default();
        let log: Arc<Mutex<Vec<Logged>>> = Arc::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (r, l, s) = (Arc::clone(&routes), Arc::clone(&log), Arc::clone(&stop));
        std::thread::spawn(move || {
            let dropped = Arc::new(AtomicUsize::new(0));
            while !s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (r, l, d) = (Arc::clone(&r), Arc::clone(&l), Arc::clone(&dropped));
                        std::thread::spawn(move || serve(stream, &r, &l, &d));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(2)),
                }
            }
        });
        Self {
            addr,
            routes,
            log,
            stop,
        }
    }

    fn route(&self, path: &str, route: Route) {
        self.routes.lock().insert(path.to_string(), route);
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    fn log(&self) -> Vec<Logged> {
        self.log.lock().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn serve(
    mut stream: TcpStream,
    routes: &Mutex<HashMap<String, Route>>,
    log: &Mutex<Vec<Logged>>,
    dropped: &AtomicUsize,
) {
    stream.set_nonblocking(false).unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
    }
    let head = String::from_utf8_lossy(&head).to_string();
    let target = head.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let token = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("t="))
        .map(str::to_string);
    let range_start = head.lines().find_map(|line| {
        let lower = line.to_ascii_lowercase();
        let value = lower
            .strip_prefix("range:")?
            .trim()
            .strip_prefix("bytes=")?
            .to_string();
        value.split('-').next()?.trim().parse::<u64>().ok()
    });
    let route = routes.lock().get(path).cloned();
    let status = match &route {
        None => 404,
        Some(route) => {
            if let Some(status) = route.status {
                status
            } else if let Some(tokens) = &route.tokens
                && !token.as_ref().is_some_and(|t| tokens.lock().contains(t))
            {
                route.refusal
            } else if route.ignore_range || range_start.is_none() {
                200
            } else {
                206
            }
        }
    };
    log.lock().push(Logged {
        path: path.to_string(),
        token,
        range_start,
        status,
    });
    let Some(route) = route.filter(|_| status == 200 || status == 206) else {
        let _ = write!(
            stream,
            "HTTP/1.1 {status} No\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        return;
    };
    let total = route.body.len();
    let start = if status == 206 {
        (range_start.unwrap_or(0) as usize).min(total)
    } else {
        0
    };
    let mut header = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n",
        route.content_type,
        total - start
    );
    if status == 206 {
        header += &format!("Content-Range: bytes {start}-{}/{total}\r\n", total.max(1) - 1);
    }
    header += "\r\n";
    if stream.write_all(header.as_bytes()).is_err() {
        return;
    }
    let drop_after = route
        .drop_after
        .filter(|_| dropped.fetch_add(1, Ordering::SeqCst) == 0);
    let mut pos = start;
    let mut sent = 0;
    while pos < total {
        if let Some((at, pause)) = route.stall
            && pos <= at
            && at < pos + 4096
        {
            let first = at - pos;
            if first > 0 && stream.write_all(&route.body[pos..at]).is_err() {
                return;
            }
            sent += first;
            pos = at;
            let _ = stream.flush();
            std::thread::sleep(pause);
        }
        let mut end = (pos + 4096).min(total);
        if let Some(limit) = drop_after {
            end = end.min(start + limit);
            if pos >= start + limit {
                let _ = stream.shutdown(std::net::Shutdown::Both);
                return;
            }
        }
        if stream.write_all(&route.body[pos..end]).is_err() {
            return;
        }
        sent += end - pos;
        pos = end;
        if let Some(pace) = route.pace {
            std::thread::sleep(pace);
        }
    }
    let _ = sent;
}

// ----- helpers ---------------------------------------------------------------------------------

fn wav_bytes(rate: u32, channels: u16, samples: &[f64], float: bool) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: if float { 32 } else { 16 },
        sample_format: if float {
            hound::SampleFormat::Float
        } else {
            hound::SampleFormat::Int
        },
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
    for &s in samples {
        if float {
            writer.write_sample(s as f32).unwrap();
        } else {
            writer
                .write_sample((s * 32_768.0).round().clamp(-32_768.0, 32_767.0) as i16)
                .unwrap();
        }
    }
    writer.finalize().unwrap();
    cursor.into_inner()
}

fn http_item(id: u64, url: String) -> PlayItem {
    PlayItem::http(id, HttpSource::new(url))
}

fn offline_engine(dir: &std::path::Path, resolver: Option<Arc<dyn StreamResolver>>) -> (Engine, PathBuf) {
    let out = dir.join("render.wav");
    let config = EngineConfig {
        output: OutputKind::WavFile {
            path: out.clone(),
            realtime: false,
        },
        buffer_ms: 200,
        cache_dir: Some(dir.to_path_buf()),
        resolver,
    };
    (Engine::new(config).unwrap(), out)
}

fn run_offline(
    dir: &std::path::Path,
    items: Vec<PlayItem>,
    resolver: Option<Arc<dyn StreamResolver>>,
) -> Render {
    let (engine, out) = offline_engine(dir, resolver);
    let events = engine.events();
    engine.set_queue(items, 0);
    engine.play();
    assert!(engine.wait_until_idle(IDLE_TIMEOUT), "render did not finish");
    drop(engine);
    let (rate, samples) = read_wav(&out);
    Render {
        rate,
        samples,
        events: events.try_iter().collect(),
    }
}

// ----- tests ------------------------------------------------------------------------------------

/// A 16-bit "position code": every frame's value names its index modulo 32 768.
fn position_code(frames: usize) -> Vec<f64> {
    (0..frames)
        .flat_map(|n| {
            let v = ((n % 32_768) as f64 - 16_384.0) / 32_768.0;
            [v, v]
        })
        .collect()
}

fn decode_position(sample: f32) -> usize {
    ((sample * 32_768.0).round() as i64 + 16_384) as usize
}

#[test]
fn seeking_fetches_the_target_with_a_range_request() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start();
    // 4 s of 16-bit stereo (706 kB) from a server sending about 400 kB/s: the target is not
    // downloaded yet when the seek asks for it.
    let frames = 4 * 44_100;
    let body = wav_bytes(44_100, 2, &position_code(frames), false);
    let size = body.len();
    server.route(
        "/code.wav",
        Route {
            body: Arc::new(body),
            pace: Some(Duration::from_millis(10)),
            content_type: "audio/wav",
            ..Route::default()
        },
    );

    let render = render(
        dir.path(),
        vec![http_item(1, server.url("/code.wav"))],
        None,
        |e| e.seek(3_000),
    );

    let target = 3 * 44_100;
    let landed = decode_position(render.left()[0]);
    assert_eq!(landed, target % 32_768, "seek lands on the target frame");
    assert_eq!(render.frames(), frames - target);
    let log = server.log();
    println!("requests for a {size}-byte file: {log:?}");
    assert!(log.iter().all(|r| r.status == 206), "{log:?}");
    let seek_request = log.iter().find(|r| r.range_start.is_some_and(|s| s > 0));
    assert!(
        seek_request.is_some(),
        "a range request for the seek target: {log:?}"
    );
    assert!(render.errors().is_empty());
}

#[test]
fn a_dropped_connection_resumes_where_it_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start();
    let input = stereo(&sine(440.0, 44_100, 0, 2 * 44_100, 0.5));
    server.route(
        "/tone.wav",
        Route {
            body: Arc::new(wav_bytes(44_100, 2, &input, true)),
            drop_after: Some(100_000),
            content_type: "audio/wav",
            ..Route::default()
        },
    );

    let render = run_offline(dir.path(), vec![http_item(1, server.url("/tone.wav"))], None);

    let log = server.log();
    println!("requests: {log:?}");
    assert!(log.len() >= 2, "{log:?}");
    assert_eq!(
        log[1].range_start,
        Some(100_000),
        "resumed at the first missing byte"
    );
    assert_eq!(render.frames(), 2 * 44_100);
    assert!(
        render.samples.iter().zip(&input).all(|(&a, &b)| a == b as f32),
        "bit-exact across the reconnect"
    );
    assert!(render.errors().is_empty());
}

struct Tokens {
    url: String,
    next: Mutex<u32>,
    issued: Arc<Mutex<Vec<String>>>,
    calls: AtomicUsize,
}

impl StreamResolver for Tokens {
    fn resolve(&self, id: u64) -> Result<HttpSource, String> {
        assert_eq!(id, 7);
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut next = self.next.lock();
        *next += 1;
        let token = format!("fresh{next}");
        self.issued.lock().push(token.clone());
        Ok(HttpSource {
            expires_at_ms: None,
            ..HttpSource::new(format!("{}?t={token}", self.url))
        })
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[test]
fn an_expired_url_is_resolved_before_it_is_used() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start();
    let input = stereo(&sine(440.0, 44_100, 0, 44_100, 0.5));
    let issued = Arc::new(Mutex::new(Vec::new()));
    server.route(
        "/tone.wav",
        Route {
            body: Arc::new(wav_bytes(44_100, 2, &input, true)),
            tokens: Some(Arc::clone(&issued)),
            refusal: 403,
            content_type: "audio/wav",
            ..Route::default()
        },
    );
    let resolver = Arc::new(Tokens {
        url: server.url("/tone.wav"),
        next: Mutex::new(0),
        issued,
        calls: AtomicUsize::new(0),
    });
    let item = PlayItem::http(
        7,
        HttpSource {
            // Expired a second ago: it must not even be tried.
            expires_at_ms: Some(now_ms() - 1_000),
            ..HttpSource::new(server.url("/tone.wav?t=stale"))
        },
    );

    let render = run_offline(dir.path(), vec![item], Some(resolver.clone()));

    let log = server.log();
    println!("requests: {log:?}");
    assert!(log.iter().all(|r| r.token.as_deref() != Some("stale")), "{log:?}");
    assert!(resolver.calls.load(Ordering::SeqCst) >= 1);
    assert_eq!(render.frames(), 44_100);
    assert!(render.samples.iter().zip(&input).all(|(&a, &b)| a == b as f32));
}

#[test]
fn a_refused_url_is_resolved_again_and_the_stream_continues() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start();
    let input = stereo(&sine(440.0, 44_100, 0, 2 * 44_100, 0.5));
    let issued = Arc::new(Mutex::new(vec!["first".to_string()]));
    server.route(
        "/tone.wav",
        Route {
            body: Arc::new(wav_bytes(44_100, 2, &input, true)),
            tokens: Some(Arc::clone(&issued)),
            refusal: 410,
            // The first connection ends early; by then the server no longer accepts the token.
            drop_after: Some(150_000),
            content_type: "audio/wav",
            ..Route::default()
        },
    );
    let resolver = Arc::new(Tokens {
        url: server.url("/tone.wav"),
        next: Mutex::new(0),
        issued: Arc::clone(&issued),
        calls: AtomicUsize::new(0),
    });
    // Revoke "first" as soon as the first request has been logged.
    let revoker = {
        let issued = Arc::clone(&issued);
        let log = Arc::clone(&server.log);
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            while log.lock().is_empty() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            issued.lock().retain(|t| t != "first");
        })
    };
    let item = http_item(7, server.url("/tone.wav?t=first"));

    let render = run_offline(dir.path(), vec![item], Some(resolver.clone()));
    revoker.join().unwrap();

    let log = server.log();
    let statuses: Vec<u16> = log.iter().map(|r| r.status).collect();
    println!("requests: {log:?}");
    assert!(statuses.contains(&410), "{statuses:?}");
    assert_eq!(resolver.calls.load(Ordering::SeqCst), 1);
    assert_eq!(log.last().map(|r| r.token.as_deref()), Some(Some("fresh1")));
    assert_eq!(render.frames(), 2 * 44_100);
    assert!(render.samples.iter().zip(&input).all(|(&a, &b)| a == b as f32));
    assert!(render.errors().is_empty());
}

#[test]
fn a_missing_item_is_reported_and_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start();
    let input = stereo(&sine(660.0, 44_100, 0, 22_050, 0.5));
    server.route(
        "/b.wav",
        Route {
            body: Arc::new(wav_bytes(44_100, 2, &input, true)),
            content_type: "audio/wav",
            ..Route::default()
        },
    );
    let items = vec![
        http_item(1, server.url("/missing.wav")),
        http_item(2, server.url("/b.wav")),
    ];

    let render = run_offline(dir.path(), items, None);

    println!(
        "errors: {:?}",
        render
            .events
            .iter()
            .filter(|e| matches!(e, EngineEvent::Error { .. }))
            .collect::<Vec<_>>()
    );
    assert_eq!(render.errors(), [Some(1)]);
    assert_eq!(render.started(), [2]);
    assert_eq!(render.frames(), 22_050);
    assert!(render.samples.iter().zip(&input).all(|(&a, &b)| a == b as f32));
    let log = server.log();
    assert_eq!(
        log.iter().filter(|r| r.path == "/missing.wav").count(),
        1,
        "404 is not retried"
    );
}

#[test]
fn http_items_join_gaplessly() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start();
    let (freq, amplitude) = (1_000.0, 0.5);
    let (split, total) = (30_001, 66_000);
    let first = dir.path().join("part1.flac");
    let second = dir.path().join("part2.flac");
    let mut expected = sine_flac(&first, freq, 44_100, 0, split, amplitude);
    expected.extend(sine_flac(&second, freq, 44_100, split, total - split, amplitude));
    for (path, file) in [("/1.flac", &first), ("/2.flac", &second)] {
        server.route(
            path,
            Route {
                body: Arc::new(std::fs::read(file).unwrap()),
                content_type: "audio/flac",
                ..Route::default()
            },
        );
    }
    let items = vec![
        http_item(1, server.url("/1.flac")),
        http_item(2, server.url("/2.flac")),
    ];

    let render = run_offline(dir.path(), items, None);

    assert_eq!(render.frames(), total);
    assert_eq!(render.left(), expected, "sample-exact across two HTTP items");
    assert_eq!(render.started(), [1, 2]);
}

#[test]
fn mp4_with_its_index_at_the_end_streams_gaplessly() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/aac-part1.m4a");
    server.route(
        "/a.m4a",
        Route {
            body: Arc::new(std::fs::read(fixture).unwrap()),
            content_type: "audio/mp4",
            ..Route::default()
        },
    );

    let render = run_offline(dir.path(), vec![http_item(1, server.url("/a.m4a"))], None);

    let log = server.log();
    println!("requests: {log:?}");
    assert_eq!(
        render.frames(),
        26_461,
        "priming and padding trimmed over HTTP too"
    );
    assert!(render.errors().is_empty());
}

#[test]
fn a_server_that_ignores_ranges_still_seeks() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start();
    let frames = 2 * 44_100;
    server.route(
        "/code.wav",
        Route {
            body: Arc::new(wav_bytes(44_100, 2, &position_code(frames), false)),
            ignore_range: true,
            content_type: "audio/wav",
            ..Route::default()
        },
    );

    let render = render(
        dir.path(),
        vec![http_item(1, server.url("/code.wav"))],
        None,
        |e| e.seek(1_500),
    );

    let target = 66_150;
    assert_eq!(decode_position(render.left()[0]), target % 32_768);
    assert_eq!(render.frames(), frames - target);
}

#[test]
fn a_slow_server_reports_buffering_with_a_stopped_clock() {
    let server = Server::start();
    // 2 s of 16-bit stereo at 44.1 kHz (176 kB/s); the server stalls for 1.5 s after 0.6 s.
    let input = stereo(&sine(440.0, 44_100, 0, 2 * 44_100, 0.5));
    let body = wav_bytes(44_100, 2, &input, false);
    let stall_at = 44 + (0.6 * 44_100.0) as usize * 4;
    server.route(
        "/slow.wav",
        Route {
            body: Arc::new(body),
            stall: Some((stall_at, Duration::from_millis(1_500))),
            content_type: "audio/wav",
            ..Route::default()
        },
    );
    let config = EngineConfig {
        output: OutputKind::Null { realtime: true },
        buffer_ms: 100,
        ..EngineConfig::default()
    };
    let engine = Engine::new(config).unwrap();
    let events = engine.events();
    engine.set_queue(vec![http_item(5, server.url("/slow.wav"))], 0);
    let started = Instant::now();
    engine.play();

    // Sample the published anchor while the server stalls.
    let mut seen_stopped = false;
    while started.elapsed() < Duration::from_millis(1_800) {
        std::thread::sleep(Duration::from_millis(50));
        let anchor = engine.anchor();
        if started.elapsed() > Duration::from_millis(1_100) && anchor.rate == 0.0 {
            seen_stopped = true;
        }
    }
    assert!(engine.wait_until_idle(IDLE_TIMEOUT));
    let elapsed = started.elapsed();
    let events: Vec<EngineEvent> = events.try_iter().collect();

    let buffering: Vec<(usize, bool)> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            EngineEvent::Buffering { id: 5, buffering } => Some((i, *buffering)),
            _ => None,
        })
        .collect();
    println!("buffering events {buffering:?} over {elapsed:?}");
    let flags: Vec<bool> = buffering.iter().map(|(_, b)| *b).collect();
    assert_eq!(flags, [true, false], "one stall, one buffering episode");
    let begin = buffering
        .iter()
        .find(|(_, b)| *b)
        .map(|(i, _)| *i)
        .expect("Buffering true");
    let end = buffering
        .iter()
        .find(|(i, b)| !*b && *i > begin)
        .map(|(i, _)| *i)
        .expect("Buffering false");
    let between: Vec<f32> = events[begin..end]
        .iter()
        .filter_map(|e| match e {
            EngineEvent::Anchor(a) => Some(a.rate),
            _ => None,
        })
        .collect();
    println!("anchor rates while buffering: {between:?}");
    assert!(!between.is_empty() && between.iter().all(|&r| r == 0.0));
    assert!(seen_stopped, "the published anchor stood still during the stall");
    // After buffering the clock runs again.
    let resumed = events[end..]
        .iter()
        .any(|e| matches!(e, EngineEvent::Anchor(a) if a.rate == 1.0));
    assert!(resumed);
    // 2 s of audio plus the part of the 1.5 s stall the 0.6 s already sent could not cover.
    assert!(elapsed >= Duration::from_millis(2_600), "{elapsed:?}");
}
