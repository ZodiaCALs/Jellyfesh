//! The hostile-plugin suite (Milestone 3): every sandbox limit must hold.

mod common;

use std::time::Duration;

use common::*;
use jf_plugin_host::{CallError, HostError, Limits};
use serde_json::json;

const SPIN: &str = "org.jellyfesh.test.spin";
const BOMB: &str = "org.jellyfesh.test.bomb";
const NET: &str = "org.jellyfesh.test.net";
const THIEF: &str = "org.jellyfesh.test.thief";
const PANIC: &str = "org.jellyfesh.test.panic";

fn spin_manifest() -> String {
    manifest(
        SPIN,
        &["search", "browse", "lyrics", "recommendations"],
        &[],
        &["${settings.server_url}"],
        20,
    )
}

fn slow_server() -> TestServer {
    TestServer::start(|request| match request.path() {
        "/slow" => {
            let mut response = Response::new(200, "slow");
            response.delay = Duration::from_millis(800);
            response
        }
        _ => Response::new(200, "ok"),
    })
}

/// An infinite loop is stopped by the CPU budget (200 ms), well before the
/// wall-clock deadline; a loop that keeps calling a host import is stopped
/// too, so time spent in the host cannot hide the guest's own CPU use.
#[test]
fn infinite_loops_are_stopped_by_the_cpu_budget() {
    let env = Env::new();
    env.install("spin", &spin_manifest(), SPIN, None);

    let (result, elapsed) = timed(|| recv(env.host.search(SPIN, "x", any(), page(10))));
    assert_eq!(result, Err(CallError::CpuLimit));
    assert!(elapsed < Duration::from_millis(1500), "took {elapsed:?}");

    let (result, elapsed) = timed(|| recv(env.host.browse(SPIN, None, page(10))));
    assert_eq!(result, Err(CallError::CpuLimit));
    assert!(elapsed < Duration::from_millis(1500), "took {elapsed:?}");

    // The plugin restarts lazily and answers the next call.
    let page = recv(env.host.recommendations_for_user(SPIN, page(10))).unwrap();
    assert_eq!(page.items[0].title, "Still alive");
}

/// A plugin that only waits (endless slow requests) uses no CPU but runs
/// into the wall-clock deadline (3 s in these tests).
#[test]
fn endless_waiting_hits_the_wall_clock_deadline() {
    let env = Env::new();
    let server = slow_server();
    env.install("spin", &spin_manifest(), SPIN, Some(&server.url()));
    let (result, elapsed) = timed(|| recv(env.host.lyrics(SPIN, track("t", "x"))));
    assert_eq!(result, Err(CallError::Timeout));
    assert!(
        elapsed >= Duration::from_millis(2900),
        "stopped early: {elapsed:?}"
    );
    assert!(elapsed < Duration::from_millis(4500), "took {elapsed:?}");
    assert!(server.count("/slow") >= 2, "the requests did run");
}

/// Allocating past 64 MiB traps, at once or by growing; the plugin restarts.
#[test]
fn memory_bombs_hit_the_memory_limit() {
    let env = Env::new();
    let manifest = manifest(BOMB, &["search", "browse", "recommendations"], &[], &[], 5);
    env.install("memory-bomb", &manifest, BOMB, None);
    assert_eq!(
        recv(env.host.search(BOMB, "x", any(), page(10))),
        Err(CallError::MemoryLimit)
    );
    assert_eq!(
        recv(env.host.browse(BOMB, None, page(10))),
        Err(CallError::MemoryLimit)
    );
    let page = recv(env.host.recommendations_for_user(BOMB, page(10))).unwrap();
    assert_eq!(page.items[0].title, "Still alive");
}

/// Panics, aborts and stack overflows become errors (with the panic message
/// when there is one), and the instance restarts lazily.
#[test]
fn panics_and_traps_become_errors_and_restart_the_plugin() {
    let env = Env::new();
    let manifest = manifest(
        PANIC,
        &["search", "browse", "lyrics", "recommendations"],
        &[],
        &[],
        5,
    );
    env.install("panicker", &manifest, PANIC, None);
    let calls = |env: &Env| {
        recv(env.host.recommendations_for_user(PANIC, page(1)))
            .unwrap()
            .items[0]
            .title
            .clone()
    };

    assert_eq!(calls(&env), "1");
    assert_eq!(calls(&env), "2", "the instance lives between calls");

    match recv(env.host.search(PANIC, "hello", any(), page(10))) {
        Err(CallError::Crashed(message)) => assert!(message.contains("boom: hello"), "{message}"),
        other => panic!("expected a crash, got {other:?}"),
    }
    assert_eq!(calls(&env), "1", "a fresh instance after the panic");

    assert!(matches!(
        recv(env.host.browse(PANIC, None, page(10))),
        Err(CallError::Crashed(_))
    ));
    match recv(env.host.lyrics(PANIC, track("t", "x"))) {
        Err(CallError::Crashed(message)) => assert!(message.contains("stack"), "{message}"),
        other => panic!("expected a stack overflow, got {other:?}"),
    }
    assert_eq!(calls(&env), "1");
}

/// Three violations in a row turn a plugin off with a visible reason;
/// turning it back on clears the strikes.
#[test]
fn three_strikes_turn_a_plugin_off() {
    let env = Env::new();
    env.install("spin", &spin_manifest(), SPIN, None);
    for _ in 0..3 {
        assert_eq!(
            recv(env.host.search(SPIN, "x", any(), page(10))),
            Err(CallError::CpuLimit)
        );
    }
    let info = env.host.plugin(SPIN).unwrap();
    assert!(!info.enabled);
    let reason = info.disabled_reason.unwrap();
    assert!(reason.contains("3 times"), "{reason}");
    assert!(matches!(
        recv(env.host.search(SPIN, "x", any(), page(10))),
        Err(CallError::Disabled(_))
    ));

    env.host.set_enabled(SPIN, true).unwrap();
    assert!(env.host.plugin(SPIN).unwrap().disabled_reason.is_none());
    assert!(recv(env.host.recommendations_for_user(SPIN, page(10))).is_ok());
}

/// Calls return at once; a stuck plugin holds at most one of the two
/// workers, so another plugin keeps answering while it spins or waits.
#[test]
fn one_plugin_cannot_block_another_or_the_caller() {
    let env = Env::new();
    let slow = slow_server();
    let echo_server = TestServer::start(|_| Response::json(json!([{ "id": "1", "title": "fast" }])));
    env.install("spin", &spin_manifest(), SPIN, Some(&slow.url()));
    env.install("echo", &echo_manifest(), ECHO, Some(&echo_server.url()));

    // Queue several calls that each run to their deadline.
    let (stuck, queued) = timed(|| {
        (0..3)
            .map(|_| env.host.lyrics(SPIN, track("t", "x")))
            .collect::<Vec<_>>()
    });
    assert!(queued < Duration::from_millis(50), "the caller waited {queued:?}");
    std::thread::sleep(Duration::from_millis(200));

    for _ in 0..5 {
        let (result, elapsed) = timed(|| recv(env.host.search(ECHO, "q", any(), page(5))));
        assert_eq!(result.unwrap().items[0].title, "fast");
        assert!(
            elapsed < Duration::from_millis(1000),
            "echo waited {elapsed:?} behind spin"
        );
    }
    // The spinning calls are still running (one at a time) or queued.
    assert!(stuck[2].try_recv().is_err());
    for reply in stuck {
        assert_eq!(recv(reply), Err(CallError::Timeout));
    }
}

fn net_manifest(max_rps: u32) -> String {
    manifest(
        NET,
        &["search", "browse"],
        &[],
        &[
            "${settings.server_url}",
            "rebind.test",
            "*.private.test",
            "evil.example",
        ],
        max_rps,
    )
}

fn attack(env: &Env, url: &str) -> String {
    titles(&env.host, NET, url).remove(0)
}

/// Like [`attack`], for a request whose answer depends on the network: when
/// nothing answers at the address (an unroutable one on CI runners), the
/// call runs out of time, and that is an outcome too.
fn attack_somewhere(env: &Env, url: &str) -> String {
    match recv(env.host.search(NET, url, any(), page(50))) {
        Ok(page) => page.items.into_iter().map(|i| i.title).next().unwrap_or_default(),
        Err(err) => format!("the call ended: {err:?}"),
    }
}

/// Responses over 16 MiB are refused, with or without a content-length.
#[test]
fn oversized_responses_are_refused() {
    let env = Env::new();
    let server = TestServer::start(|request| {
        let big = vec![b'x'; 20 * 1024 * 1024];
        match request.path() {
            "/big" => Response::new(200, big),
            "/big-chunked" => {
                let mut response = Response::new(200, big);
                response.chunked = true;
                response
            }
            _ => Response::new(200, vec![b'y'; 1024 * 1024]),
        }
    });
    env.install("net-attacker", &net_manifest(20), NET, Some(&server.url()));
    assert_eq!(attack(&env, "{server}/big"), "err TooLarge");
    assert_eq!(attack(&env, "{server}/big-chunked"), "err TooLarge");
    assert_eq!(attack(&env, "{server}/fine"), format!("ok 200 {}", 1024 * 1024));
}

/// Redirects are re-checked on every hop: a redirect to another origin is
/// refused before it is followed; one within the allowlist works.
#[test]
fn redirects_cannot_escape_the_allowlist() {
    let env = Env::new();
    let victim = TestServer::start(|_| Response::new(200, "internal secret"));
    let victim_url = victim.url();
    let server = TestServer::start(move |request| match request.path() {
        "/to-victim" => Response::redirect(&format!("{victim_url}/admin")),
        "/to-evil" => Response::redirect("https://evil.example.org/"),
        "/to-metadata" => Response::redirect("http://169.254.169.254/latest/meta-data/"),
        "/inside" => Response::redirect("/landing"),
        "/loop" => Response::redirect("/loop"),
        _ => Response::new(200, "landed"),
    });
    env.install("net-attacker", &net_manifest(20), NET, Some(&server.url()));
    for path in ["/to-victim", "/to-evil", "/to-metadata"] {
        let outcome = attack(&env, &format!("{{server}}{path}"));
        assert!(outcome.starts_with("err NotAllowed"), "{path}: {outcome}");
    }
    assert_eq!(
        victim.requests().len(),
        0,
        "the redirect target was never contacted"
    );
    assert_eq!(attack(&env, "{server}/inside"), "ok 200 6");
    assert_eq!(attack(&env, "{server}/loop"), "err TooManyRedirects");
}

/// Only http(s) URLs get as far as an origin check, as requests and as
/// redirects: for a `blob:` URL, `Url::origin` parses the URL inside it, and
/// url 2.5.8 and older did so without a limit, so a deeply nested one
/// overflowed the stack and took the whole app down (RUSTSEC-2026-0592).
#[test]
fn nested_blob_urls_are_refused_and_the_host_carries_on() {
    let env = Env::new();
    let nested = format!("{}https://evil.example/", "blob:".repeat(10_000));
    let location = nested.clone();
    let server = TestServer::start(move |request| match request.path() {
        "/to-blob" => Response::redirect(&location),
        _ => Response::new(200, "landed"),
    });
    env.install("net-attacker", &net_manifest(20), NET, Some(&server.url()));
    let outcome = attack(&env, &nested);
    assert!(outcome.starts_with("err InvalidRequest"), "{outcome}");
    let outcome = attack(&env, "{server}/to-blob");
    assert!(outcome.starts_with("err NotAllowed"), "{outcome}");
    assert_eq!(attack(&env, "{server}/fine"), "ok 200 6");
}

/// A fixed host whose DNS answer changes cannot be steered to a local
/// address: the name is resolved once per request, an answer containing a
/// private address is refused as a whole, and the connection goes to the
/// address that was checked.
#[test]
fn dns_rebinding_is_defeated() {
    let env = Env::new();
    let local = TestServer::start(|_| Response::new(200, "local service"));
    env.install("net-attacker", &net_manifest(20), NET, Some(&local.url()));

    // First lookup: a public (unroutable documentation) address; every later
    // lookup: loopback. The connection must go to the first, checked answer.
    // What answers there depends on the network (nothing, or an
    // intercepting proxy); it is never the local service.
    env.resolver
        .answer("rebind.test", &[&["203.0.113.7"], &["127.0.0.1"]]);
    let outcome = attack_somewhere(&env, "https://rebind.test/");
    eprintln!("connecting to the checked public address gave: {outcome}");
    assert_eq!(env.resolver.calls("rebind.test"), 1, "resolved exactly once");
    assert_eq!(local.requests().len(), 0, "the local service was never reached");

    // The next request gets the rebinding answer and is refused outright.
    let outcome = attack(&env, "https://rebind.test/");
    assert!(outcome.starts_with("err BlockedAddress"), "{outcome}");
    assert_eq!(env.resolver.calls("rebind.test"), 2);

    // A mixed answer is refused as a whole.
    env.resolver
        .answer("mixed.private.test", &[&["203.0.113.8", "127.0.0.1"]]);
    let outcome = attack(&env, "https://mixed.private.test/");
    assert!(outcome.starts_with("err BlockedAddress"), "{outcome}");

    assert_eq!(local.requests().len(), 0, "the local service was never reached");
}

/// Fixed hosts may not resolve to private, loopback or link-local
/// addresses; direct IP URLs are never on the allowlist; only the person's
/// own configured server may be local.
#[test]
fn private_addresses_are_blocked() {
    let env = Env::new();
    let own = TestServer::start(|_| Response::new(200, "own server"));
    let other = TestServer::start(|_| Response::new(200, "someone else"));
    env.install("net-attacker", &net_manifest(20), NET, Some(&own.url()));

    for (host, address) in [
        ("ten.private.test", "10.0.0.1"),
        ("home.private.test", "192.168.1.1"),
        ("meta.private.test", "169.254.169.254"),
        ("loop.private.test", "127.0.0.1"),
        ("cgnat.private.test", "100.64.0.1"),
        ("v6loop.private.test", "::1"),
        ("mapped.private.test", "::ffff:127.0.0.1"),
        ("ula.private.test", "fd00::1"),
        ("zero.private.test", "0.0.0.0"),
    ] {
        env.resolver.answer(host, &[&[address]]);
        let outcome = attack(&env, &format!("https://{host}/"));
        assert!(
            outcome.starts_with("err BlockedAddress"),
            "{host} ({address}): {outcome}"
        );
    }
    for url in [
        other.url(),
        format!("http://localhost:{}/", other.port()),
        "http://169.254.169.254/latest/meta-data/".to_string(),
        "https://[::1]/".to_string(),
        "http://evil.example/".to_string(),
        "file:///etc/passwd".to_string(),
    ] {
        let outcome = attack(&env, &url);
        assert!(
            outcome.starts_with("err NotAllowed") || outcome.starts_with("err InvalidRequest"),
            "{url}: {outcome}"
        );
    }
    assert_eq!(other.requests().len(), 0);
    // The configured server itself is on loopback and allowed.
    assert_eq!(attack(&env, "{server}/ping"), "ok 200 10");
}

/// A plugin cannot read another plugin's secrets, kv values or cookies, nor
/// the host's own entries; keystore keys stay in their namespace.
#[test]
fn secrets_cannot_cross_plugin_namespaces() {
    let env = Env::new();
    // `/cookies` echoes the Cookie header and sets a cookie for the caller.
    let server = TestServer::start(|request| match request.path() {
        "/cookies" => Response::new(200, request.header("cookie").unwrap_or_default().to_string())
            .header("set-cookie", "victim=1; Max-Age=3600; Path=/"),
        _ => Response::new(404, ""),
    });
    env.install("echo", &echo_manifest(), ECHO, Some(&server.url()));
    assert_eq!(titles(&env.host, ECHO, "do:secret-set"), vec!["stored"]);
    assert_eq!(titles(&env.host, ECHO, "do:kv")[0], "hello");
    // The first request stores the cookie in echo's jar, the second sends it.
    let _ = titles(&env.host, ECHO, "do:cookies");
    assert_eq!(titles(&env.host, ECHO, "do:cookies"), vec!["victim=1"]);

    let thief = manifest(THIEF, &["search"], &[], &["${settings.server_url}"], 20);
    env.install("secret-thief", &thief, THIEF, Some(&server.url()));
    let results = titles(&env.host, THIEF, ECHO);
    for line in &results {
        if line.starts_with("secret \"token\"") {
            assert!(line.ends_with("=> Ok(None)"), "own namespace is empty: {line}");
        } else if line.starts_with("secret ") {
            // The SDK turns the host's invalid-key error into this.
            assert!(line.contains("Err(") && line.contains("storage key"), "{line}");
        } else if line.starts_with("kv ") {
            assert!(line.ends_with("=> Ok(None)"), "{line}");
        } else if line.starts_with("cookies") {
            assert!(
                !line.contains("victim"),
                "the thief got another plugin's cookie: {line}"
            );
        }
    }
    assert!(results.iter().any(|l| l.starts_with("cookies")));
    assert!(!results.iter().any(|l| l.contains("s3cret-token")), "{results:?}");

    // Keystore keys stay in their namespaces: echo's token and jar key, and
    // the jar key of the thief's own cookies (the server set one for it too).
    let keys = env.secrets.keys();
    assert!(keys.contains(&format!("jellyfesh/{ECHO}/token")), "{keys:?}");
    for key in &keys {
        let ok = key.starts_with(&format!("jellyfesh/{ECHO}/"))
            || *key == format!("jellyfesh/{THIEF}/~cookie-key");
        assert!(ok, "unexpected keystore entry {key} in {keys:?}");
    }
}

/// The per-plugin request rate (`max_rps`) holds: requests beyond it wait
/// for a token and are refused when none comes before the deadline.
#[test]
fn the_rate_limit_holds() {
    let env = Env::new();
    let server = TestServer::start(|_| Response::new(200, "pong"));
    env.install("net-attacker", &net_manifest(2), NET, Some(&server.url()));
    let page = recv(env.host.browse(NET, None, page(1))).unwrap();
    let text = &page.items[0].title;
    let numbers: Vec<u32> = text.split(' ').filter_map(|w| w.parse().ok()).collect();
    let (sent, refused) = (numbers[0], numbers[1]);
    let wall = test_limits().wall.as_secs() as u32;
    assert!(sent <= 2 + 2 * wall, "{text}");
    assert_eq!(sent + refused, 50, "{text}");
    assert_eq!(
        server.count("/ping") as u32,
        sent,
        "the server saw only the admitted requests"
    );
}

/// Bytes that are not a component, a core module, a truncated component
/// and a component for another API version are all refused at install;
/// nothing is installed.
#[test]
fn malformed_components_are_refused() {
    let env = Env::new();
    let echo = component("echo");
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("garbage", b"definitely not wasm".to_vec()),
        ("empty", Vec::new()),
        ("core module", core_module("echo")),
        ("truncated", echo[..echo.len() / 2].to_vec()),
        ("2.0 component", component("api-mismatch")),
    ];
    for (what, wasm) in cases {
        let path = package(
            &env.incoming(),
            &format!("{what}.jfpkg"),
            &echo_manifest(),
            wasm,
            true,
        );
        let error = env.host.install(&path).unwrap_err();
        assert!(
            matches!(error, HostError::InvalidComponent(_) | HostError::Incompatible(_)),
            "{what}: {error}"
        );
    }
    assert!(env.host.plugins().is_empty());
    let installed = std::fs::read_dir(env.dir.path().join("plugins/packages"))
        .unwrap()
        .count();
    assert_eq!(installed, 0);
}

/// `api_version` must name this host's major version and no newer minor
/// (the host implements 1.3); a component built for another major version
/// is refused even when the manifest claims 1.0.
#[test]
fn api_version_mismatches_are_refused() {
    let env = Env::new();
    for version in ["2.0", "1.4", "0.9"] {
        let text = echo_manifest().replace("api_version = \"1.0\"", &format!("api_version = {version:?}"));
        let path = package(
            &env.incoming(),
            &format!("v{version}.jfpkg"),
            &text,
            component("echo"),
            true,
        );
        let error = env.host.install(&path).unwrap_err();
        assert!(
            matches!(error, HostError::ApiVersion { .. }),
            "{version}: {error}"
        );
    }
    let path = package(
        &env.incoming(),
        "mismatch.jfpkg",
        &echo_manifest(),
        component("api-mismatch"),
        true,
    );
    match env.host.install(&path).unwrap_err() {
        HostError::Incompatible(message) => assert!(message.contains("jellyfesh:plugin"), "{message}"),
        other => panic!("expected an incompatible component, got {other}"),
    }
}

/// The host never calls a capability the manifest does not declare, and a
/// plugin that reports more than its manifest declares is refused.
#[test]
fn undeclared_capabilities_are_refused() {
    let env = Env::new();
    let narrow = manifest(ECHO, &["search"], &[], &["${settings.server_url}"], 5);
    let path = package(&env.incoming(), "narrow.jfpkg", &narrow, component("echo"), true);
    match env.host.install(&path).unwrap_err() {
        HostError::Manifest(message) => assert!(message.contains("does not declare"), "{message}"),
        other => panic!("expected a manifest error, got {other}"),
    }
    env.install("spin", &spin_manifest(), SPIN, None);
    assert!(matches!(
        recv(
            env.host
                .resolve_stream(SPIN, track("t", "x"), jf_plugin_host::Quality::High)
        ),
        Err(CallError::NotDeclared(_))
    ));
}

/// Limits come from the options, so they can be tightened further: a
/// 20 ms CPU budget still stops the loop.
#[test]
fn limits_are_configurable() {
    let env = Env::with_limits(Limits {
        cpu: Duration::from_millis(20),
        ..test_limits()
    });
    env.install("spin", &spin_manifest(), SPIN, None);
    let (result, elapsed) = timed(|| recv(env.host.search(SPIN, "x", any(), page(10))));
    assert_eq!(result, Err(CallError::CpuLimit));
    assert!(elapsed < Duration::from_millis(1000), "took {elapsed:?}");
}
