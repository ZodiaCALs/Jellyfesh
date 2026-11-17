//! The store against a local HTTP server. Every byte served here is test
//! data, not a model.

use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use std::thread;

use super::*;

/// Test data, not a model: `len` bytes of a fixed pseudo-random pattern.
fn test_bytes(len: usize) -> Vec<u8> {
    let mut state: u32 = 0x9e37_79b9;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state.to_le_bytes()[0]
        })
        .collect()
}

/// A request the server saw: its path and `Range` header.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    path: String,
    range: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Behaviour {
    /// Answers `Range` requests with 206; otherwise always the whole body with 200.
    ignore_ranges: bool,
    /// Closes the first body after this many bytes (with the full length announced).
    break_first_after: Option<usize>,
}

/// A local HTTP/1.1 server in a thread serving one file, one connection at a time.
struct TestServer {
    base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl TestServer {
    fn start(file: &str, body: Vec<u8>, behaviour: Behaviour) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("binding a local port");
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let path = format!("/{file}");
        thread::spawn(move || {
            let mut break_after = behaviour.break_first_after;
            for stream in listener.incoming().flatten() {
                let Some(request) = read_request(&stream) else {
                    continue;
                };
                log.lock().unwrap().push(request.clone());
                let range = if behaviour.ignore_ranges {
                    None
                } else {
                    request.range.as_deref()
                };
                let cut = if request.path == path {
                    break_after.take()
                } else {
                    None
                };
                let _ = respond(stream, request.path == path, &body, range, cut);
            }
        });
        TestServer { base, seen }
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

fn read_request(stream: &TcpStream) -> Option<Seen> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let path = line.split(' ').nth(1)?.to_owned();
    let mut range = None;
    loop {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("range")
        {
            range = Some(value.trim().to_owned());
        }
    }
    Some(Seen { path, range })
}

fn respond(
    mut stream: TcpStream,
    found: bool,
    body: &[u8],
    range: Option<&str>,
    cut: Option<usize>,
) -> io::Result<()> {
    if !found {
        return stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    }
    let len = body.len();
    let start = range
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.strip_suffix('-'))
        .and_then(|s| s.parse::<usize>().ok());
    let (status, from, extra) = match start {
        Some(start) if start >= len => {
            let head = format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{len}\r\n\
                 Content-Length: 0\r\nConnection: close\r\n\r\n"
            );
            return stream.write_all(head.as_bytes());
        }
        Some(start) => (
            "206 Partial Content",
            start,
            format!("Content-Range: bytes {start}-{}/{len}\r\n", len - 1),
        ),
        None => ("200 OK", 0, String::new()),
    };
    let rest = &body[from..];
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n{extra}\
         Connection: close\r\n\r\n",
        rest.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&rest[..cut.unwrap_or(rest.len()).min(rest.len())])?;
    stream.flush()
}

const FILE: &str = "test-data.bin";

/// A store whose only model is `body` (test data) served by `server`.
fn store_for(dir: &Path, server: &TestServer, body: &[u8]) -> ModelStore {
    let sha1: &'static str = Box::leak(hex(&sha1::Sha1::digest(body)).into_boxed_str());
    let sha256: &'static str = Box::leak(hex(&sha2::Sha256::digest(body)).into_boxed_str());
    let models: &'static [ModelInfo] = Box::leak(Box::new([ModelInfo {
        id: "test",
        name: "Test data",
        file: FILE,
        url: "http://test.invalid/test-data.bin",
        approx_bytes: 1000,
        sha1,
        sha256: Some(sha256),
        multilingual: false,
    }]));
    ModelStore {
        dir: dir.to_owned(),
        base_url: Some(server.base.clone()),
        models,
    }
}

fn no_cancel() -> AtomicBool {
    AtomicBool::new(false)
}

/// Downloads `test`, recording progress.
fn download(store: &ModelStore, cancel: &AtomicBool) -> (Result<PathBuf, SpeechError>, Vec<(u64, u64)>) {
    let mut calls = Vec::new();
    let result = store.download("test", &mut |done, total| calls.push((done, total)), cancel);
    (result, calls)
}

const LEN: usize = 1_500_000;

#[test]
fn fresh_download_verifies_and_reports_progress() {
    let body = test_bytes(LEN);
    let server = TestServer::start(FILE, body.clone(), Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    let store = store_for(dir.path(), &server, &body);
    assert_eq!(store.path("test"), None);

    let (result, progress) = download(&store, &no_cancel());
    let path = result.unwrap();
    assert_eq!(path, dir.path().join(FILE));
    assert_eq!(fs::read(&path).unwrap(), body);
    assert_eq!(store.path("test"), Some(path));
    assert!(!dir.path().join("test-data.bin.part").exists());
    assert!(dir.path().join("test-data.bin.verified").exists());

    let len = LEN as u64;
    assert_eq!(progress.first(), Some(&(0, len)));
    assert_eq!(progress.last(), Some(&(len, len)));
    assert!(progress.len() > 3, "{progress:?}");
    assert!(progress.windows(2).all(|w| w[0].0 <= w[1].0 && w[1].1 == len));
    assert_eq!(
        server.seen(),
        [Seen {
            path: format!("/{FILE}"),
            range: None
        }]
    );
}

#[test]
fn a_second_download_needs_no_network() {
    let body = test_bytes(10_000);
    let server = TestServer::start(FILE, body.clone(), Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    let store = store_for(dir.path(), &server, &body);
    download(&store, &no_cancel()).0.unwrap();
    let (again, progress) = download(&store, &no_cancel());
    assert_eq!(again.unwrap(), dir.path().join(FILE));
    assert_eq!(progress, [(10_000, 10_000)]);
    assert_eq!(server.seen().len(), 1);
}

#[test]
fn resumes_a_partial_download_with_a_range() {
    let body = test_bytes(LEN);
    let server = TestServer::start(FILE, body.clone(), Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("test-data.bin.part"), &body[..600_000]).unwrap();
    let store = store_for(dir.path(), &server, &body);

    let (result, progress) = download(&store, &no_cancel());
    assert_eq!(fs::read(result.unwrap()).unwrap(), body);
    assert_eq!(server.seen()[0].range.as_deref(), Some("bytes=600000-"));
    assert_eq!(server.seen().len(), 1);
    assert_eq!(progress.first(), Some(&(600_000, LEN as u64)));
    assert_eq!(progress.last(), Some(&(LEN as u64, LEN as u64)));
}

#[test]
fn starts_again_when_the_server_ignores_ranges() {
    let body = test_bytes(LEN);
    let server = TestServer::start(
        FILE,
        body.clone(),
        Behaviour {
            ignore_ranges: true,
            ..Behaviour::default()
        },
    );
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("test-data.bin.part"), &body[..600_000]).unwrap();
    let store = store_for(dir.path(), &server, &body);

    let (result, progress) = download(&store, &no_cancel());
    assert_eq!(fs::read(result.unwrap()).unwrap(), body);
    assert_eq!(server.seen()[0].range.as_deref(), Some("bytes=600000-"));
    // The 200 answer restarts the count from zero.
    assert_eq!(progress.first(), Some(&(0, LEN as u64)));
}

#[test]
fn continues_after_the_connection_breaks() {
    let body = test_bytes(LEN);
    let behaviour = Behaviour {
        break_first_after: Some(700_000),
        ..Behaviour::default()
    };
    let server = TestServer::start(FILE, body.clone(), behaviour);
    let dir = tempfile::tempdir().unwrap();
    let store = store_for(dir.path(), &server, &body);

    let (result, _) = download(&store, &no_cancel());
    assert_eq!(fs::read(result.unwrap()).unwrap(), body);
    let seen = server.seen();
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert_eq!(seen[1].range.as_deref(), Some("bytes=700000-"));
}

#[test]
fn a_whole_partial_file_is_finished_without_a_body() {
    let body = test_bytes(50_000);
    let server = TestServer::start(FILE, body.clone(), Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("test-data.bin.part"), &body).unwrap();
    let store = store_for(dir.path(), &server, &body);

    let (result, progress) = download(&store, &no_cancel());
    assert_eq!(fs::read(result.unwrap()).unwrap(), body);
    assert_eq!(server.seen()[0].range.as_deref(), Some("bytes=50000-"));
    assert_eq!(progress.last(), Some(&(50_000, 50_000)));
}

#[test]
fn a_bad_checksum_is_refused_and_the_partial_deleted() {
    // The real catalogue's Tiny entry, served test data: the SHA-1 cannot match.
    let body = test_bytes(100_000);
    let server = TestServer::start("ggml-tiny.bin", body, Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    let store = ModelStore::with_base_url(dir.path(), &format!("{}/", server.base));

    let err = store.download("tiny", &mut |_, _| {}, &no_cancel()).unwrap_err();
    let SpeechError::Download(message) = &err else {
        panic!("{err:?}")
    };
    assert!(message.contains("failed verification"), "{message}");
    assert!(
        message.contains("bd577a113a864445d4c299885e0cb97d4ba92b5f"),
        "{message}"
    );
    assert_eq!(store.path("tiny"), None);
    assert!(!dir.path().join("ggml-tiny.bin").exists());
    assert!(!dir.path().join("ggml-tiny.bin.part").exists());
    assert_eq!(server.seen()[0].path, "/ggml-tiny.bin");
}

#[test]
fn cancelling_keeps_the_partial_for_the_next_download() {
    let body = test_bytes(LEN);
    let server = TestServer::start(FILE, body.clone(), Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    let store = store_for(dir.path(), &server, &body);

    let cancel = AtomicBool::new(false);
    let result = store.download(
        "test",
        &mut |done, _| {
            if done > 0 {
                cancel.store(true, Ordering::Relaxed);
            }
        },
        &cancel,
    );
    assert_eq!(result, Err(SpeechError::Cancelled));
    let kept = fs::metadata(dir.path().join("test-data.bin.part")).unwrap().len();
    assert!(kept > 0 && kept < LEN as u64, "{kept}");
    assert_eq!(store.path("test"), None);

    // Cancelled before it starts: nothing is requested.
    assert_eq!(
        download(&store, &AtomicBool::new(true)).0,
        Err(SpeechError::Cancelled)
    );
    assert_eq!(server.seen().len(), 1);

    let (result, _) = download(&store, &no_cancel());
    assert_eq!(fs::read(result.unwrap()).unwrap(), body);
    assert_eq!(server.seen()[1].range, Some(format!("bytes={kept}-")));
}

#[test]
fn delete_removes_the_model_its_record_and_a_partial() {
    let body = test_bytes(20_000);
    let server = TestServer::start(FILE, body.clone(), Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    let store = store_for(dir.path(), &server, &body);
    download(&store, &no_cancel()).0.unwrap();
    fs::write(dir.path().join("test-data.bin.part"), b"test data").unwrap();

    store.delete("test").unwrap();
    assert_eq!(store.path("test"), None);
    for name in ["test-data.bin", "test-data.bin.part", "test-data.bin.verified"] {
        assert!(!dir.path().join(name).exists(), "{name}");
    }
    // Deleting what is not there succeeds.
    store.delete("test").unwrap();
    assert_eq!(
        store.delete("nope").unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
}

#[test]
fn delete_refuses_a_model_that_is_downloading() {
    let dir = tempfile::tempdir().unwrap();
    let store = ModelStore::new(dir.path());
    let _busy = Busy::claim(&dir.path().join("ggml-base.bin")).unwrap();
    assert_eq!(
        store.delete("base").unwrap_err().kind(),
        io::ErrorKind::ResourceBusy
    );
    assert_eq!(
        store.download("base", &mut |_, _| {}, &no_cancel()),
        Err(SpeechError::Download(
            "this speech model is downloading already".into()
        ))
    );
}

#[test]
fn unknown_ids_are_refused() {
    let body = test_bytes(1_000);
    let server = TestServer::start(FILE, body, Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    let store = ModelStore::with_base_url(dir.path(), &server.base);
    let err = store
        .download("medium", &mut |_, _| {}, &no_cancel())
        .unwrap_err();
    assert!(
        matches!(&err, SpeechError::Download(m) if m.contains("\"medium\"")),
        "{err:?}"
    );
    assert_eq!(store.path("medium"), None);
    assert_eq!(store.path("../ggml-tiny"), None);
    assert!(store.delete("medium").is_err());
    assert!(server.seen().is_empty());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn http_errors_are_reported() {
    let server = TestServer::start("elsewhere.bin", Vec::new(), Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    let store = ModelStore::with_base_url(dir.path(), &server.base);
    let err = store.download("base", &mut |_, _| {}, &no_cancel()).unwrap_err();
    assert_eq!(
        err,
        SpeechError::Download("the model is not on the server (HTTP 404)".into())
    );
    assert_eq!(store.path("base"), None);

    // Nothing listens on the port any more.
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let store = ModelStore::with_base_url(dir.path(), &base);
    assert!(matches!(
        store.download("base", &mut |_, _| {}, &no_cancel()),
        Err(SpeechError::Download(_))
    ));
}

#[test]
fn path_trusts_its_record_until_the_file_changes() {
    let body = test_bytes(30_000);
    let server = TestServer::start(FILE, body.clone(), Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    let store = store_for(dir.path(), &server, &body);
    let path = download(&store, &no_cancel()).0.unwrap();
    let mtime = fs::metadata(&path).unwrap().modified().unwrap();

    // Other bytes of the same size with the same time: the record is trusted, no hashing.
    let mut other = body.clone();
    other[0] ^= 0xff;
    fs::write(&path, &other).unwrap();
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(mtime)
        .unwrap();
    assert_eq!(store.path("test"), Some(path.clone()));

    // A new modification time: hashed again, and refused.
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(mtime + Duration::from_secs(2))
        .unwrap();
    assert_eq!(store.path("test"), None);
    assert_eq!(store.path("test"), None);

    // A download replaces the bad file.
    let (result, _) = download(&store, &no_cancel());
    assert_eq!(fs::read(result.unwrap()).unwrap(), body);
    assert_eq!(store.path("test"), Some(path));
}

#[test]
fn path_verifies_a_file_put_in_place() {
    let body = test_bytes(30_000);
    let server = TestServer::start(FILE, body.clone(), Behaviour::default());
    let dir = tempfile::tempdir().unwrap();
    let store = store_for(dir.path(), &server, &body);

    fs::write(dir.path().join(FILE), &body[..29_999]).unwrap();
    assert_eq!(store.path("test"), None);
    fs::write(dir.path().join(FILE), &body).unwrap();
    assert_eq!(store.path("test"), Some(dir.path().join(FILE)));
    let record = fs::read_to_string(dir.path().join("test-data.bin.verified")).unwrap();
    assert!(
        record.contains(&format!("sha1 {}", hex(&sha1::Sha1::digest(&body)))),
        "{record}"
    );
    assert!(record.contains("size 30000"), "{record}");
    assert!(server.seen().is_empty());
}

#[test]
fn content_ranges() {
    let range = |start, total| Some(ContentRange { start, total });
    assert_eq!(parse_content_range("bytes 0-99/1000"), range(Some(0), Some(1000)));
    assert_eq!(
        parse_content_range(" bytes 600000-1499999/1500000 "),
        range(Some(600_000), Some(1_500_000))
    );
    assert_eq!(parse_content_range("bytes 5-9/*"), range(Some(5), None));
    assert_eq!(parse_content_range("bytes */1000"), range(None, Some(1000)));
    assert_eq!(parse_content_range("items 0-1/2"), None);
    assert_eq!(parse_content_range("bytes 0-1"), None);
    assert_eq!(parse_content_range("bytes x-1/2"), None);
}

#[test]
fn digests_compare_with_the_catalogue() {
    let tiny = crate::model("tiny").unwrap();
    let digests = Digests {
        sha1: tiny.sha1.to_uppercase(),
        sha256: "00".repeat(32),
    };
    assert_eq!(digests.mismatch(tiny), None);
    let pinned = ModelInfo {
        sha256: Some("ab"),
        ..*tiny
    };
    assert!(digests.mismatch(&pinned).unwrap().contains("SHA-256"));
    assert_eq!(hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
}
