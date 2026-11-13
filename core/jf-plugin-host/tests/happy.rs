//! Happy-path tests with the `echo` test plugin: every call, every host
//! import, settings, all five login flows, logout, persistence, uninstall,
//! signatures and community gating, and timings.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL;
use common::*;
use jf_plugin_host::package::PackageBuilder;
use jf_plugin_host::trust::{key_id, public_key, test_key_seed};
use jf_plugin_host::{
    AuthKind, AuthStep, CallError, Capability, FieldKind, FieldValue, HostError, LyricFormat, PageRequest,
    PlaybackMode, PluginError, Quality, SignatureStatus, UnplayableReason,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const PASSWORD: &str = "correct horse battery staple";

fn echo_server() -> TestServer {
    TestServer::start(|request| match request.path() {
        "/search" => Response::json(json!([
            {
                "id": "t1",
                "title": format!("Result for {}", request.query("q").unwrap_or_default()),
                "artist": "Artist",
                "duration_ms": 200000,
                "isrc": "us-rc1-76-07839",
                "artwork": "http://evil.example/art.jpg"
            },
            { "id": "t2", "title": "Blocked here", "playable": false },
            { "id": "", "title": "no id" }
        ])),
        "/cookies" => Response::new(200, request.header("cookie").unwrap_or_default().to_string()),
        "/headers" => Response::new(
            201,
            format!(
                "{} {} {}",
                request.header("x-plugin").unwrap_or_default(),
                request.header("content-type").unwrap_or_default(),
                String::from_utf8_lossy(&request.body)
            ),
        ),
        "/token" if request.form("code").as_deref() == Some("good-code") => {
            Response::json(json!({ "access_token": "at-1", "name": "OAuth User" }))
        }
        "/me"
            if request
                .header("cookie")
                .is_some_and(|c| c.contains("session=abc123")) =>
        {
            Response::json(json!({ "name": "Cookie User" }))
        }
        "/login"
            if request.form("username").as_deref() == Some("alice")
                && request.form("password").as_deref() == Some(PASSWORD) =>
        {
            Response::json(json!({ "token": "tok-alice" }))
        }
        "/ping" if request.header("x-api-key") == Some("key-123") => Response::new(200, "pong"),
        "/logout" => Response::new(200, "bye"),
        "/token" | "/me" | "/login" | "/ping" => Response::new(401, "no"),
        _ => Response::new(404, "not found"),
    })
}

fn echo_env() -> (Env, TestServer) {
    let env = Env::new();
    let server = echo_server();
    env.install("echo", &echo_manifest(), ECHO, Some(&server.url()));
    (env, server)
}

/// The next step of a login flow (at most 10 s).
fn next(flow: &jf_plugin_host::AuthFlow) -> AuthStep {
    flow.steps()
        .recv_timeout(Duration::from_secs(10))
        .expect("no login step within 10 s")
}

fn fields(pairs: &[(&str, &str)]) -> Vec<FieldValue> {
    pairs
        .iter()
        .map(|(name, value)| FieldValue {
            name: name.to_string(),
            value: value.to_string(),
        })
        .collect()
}

/// Every file under `dir` whose bytes contain `needle`.
fn files_containing(dir: &Path, needle: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                stack.push(entry.unwrap().path());
            }
        } else if let Ok(bytes) = std::fs::read(&path)
            && bytes.windows(needle.len()).any(|w| w == needle.as_bytes())
        {
            found.push(path);
        }
    }
    found
}

/// A raw `GET` to the loopback listener, as the browser would send it.
fn browser_get(url: &str) -> String {
    let url = url::Url::parse(url).unwrap();
    let mut stream = TcpStream::connect((url.host_str().unwrap(), url.port().unwrap())).unwrap();
    let target = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_string(),
    };
    write!(
        stream,
        "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    answer
}

#[test]
fn installs_and_searches() {
    let (env, server) = echo_env();
    let info = env.host.plugin(ECHO).unwrap();
    assert!(info.official);
    assert!(matches!(info.signature, SignatureStatus::Trusted { .. }));
    assert!(info.enabled);
    assert_eq!(info.capabilities.len(), 7);
    assert_eq!(info.auth_kinds.len(), 5);
    assert_eq!(info.playback_mode, PlaybackMode::Stream);
    assert_eq!(info.api_version, "1.0");
    assert_eq!(info.settings["server_url"], server.url());
    assert_eq!(info.settings_schema["properties"]["server_url"]["format"], "uri");
    assert!(info.account.is_none());
    assert_eq!(env.host.plugins().len(), 1);

    let page = recv(env.host.search(ECHO, "hello", any(), page(10))).unwrap();
    assert_eq!(page.items.len(), 2, "the item without an id is dropped");
    let first = &page.items[0];
    assert_eq!(first.title, "Result for hello");
    assert_eq!(first.isrc.as_deref(), Some("USRC17607839"));
    assert_eq!(first.artwork, None, "artwork outside the allowlist is dropped");
    let second = &page.items[1];
    assert!(!second.playable);
    assert_eq!(
        second.unplayable.as_ref().unwrap().reason,
        UnplayableReason::Region
    );
    assert_eq!(server.count("/search"), 1);

    let runtime = recv(env.host.capabilities(ECHO)).unwrap();
    assert!(runtime.contains(&Capability::ResolveStream));
}

#[test]
fn browse_resolve_lyrics_artwork_and_recommendations() {
    let (env, server) = echo_env();
    let root = recv(env.host.browse(ECHO, None, page(10))).unwrap();
    assert_eq!(root.items[0].id, "albums");
    let albums = recv(env.host.browse(ECHO, Some(root.items[0].clone()), page(3))).unwrap();
    assert_eq!(albums.items.len(), 3);
    assert_eq!(albums.next.as_deref(), Some("3"));
    let more = recv(env.host.browse(
        ECHO,
        Some(root.items[0].clone()),
        PageRequest {
            cursor: albums.next.clone(),
            limit: 2,
        },
    ))
    .unwrap();
    assert_eq!(more.items[0].id, "al3");
    let missing = recv(env.host.browse(ECHO, Some(track("nope", "x")), page(3)));
    assert_eq!(missing, Err(CallError::Plugin(PluginError::NotFound)));

    let stream = recv(env.host.resolve_stream(ECHO, track("t1", "Song"), Quality::High)).unwrap();
    assert_eq!(stream.url, format!("{}/stream/t1.flac", server.url()));
    assert_eq!(stream.codec.as_deref(), Some("flac"));
    assert_eq!(stream.bitrate_kbps, Some(320));
    assert!(stream.cacheable);
    assert_eq!(stream.headers[0].name, "x-token");
    let outside = recv(env.host.resolve_stream(ECHO, track("outside", "x"), Quality::Low));
    assert!(
        matches!(outside, Err(CallError::InvalidResponse(_))),
        "{outside:?}"
    );

    let lyrics = recv(env.host.lyrics(ECHO, track("t1", "Song"))).unwrap().unwrap();
    assert_eq!(lyrics.format, LyricFormat::Lrc);
    assert_eq!(lyrics.text, "[00:01.00]Song");
    assert_eq!(recv(env.host.lyrics(ECHO, track("t1", "silent"))).unwrap(), None);

    let art = recv(env.host.artwork(ECHO, track("t1", "Song"), 10))
        .unwrap()
        .unwrap();
    assert_eq!(
        art.url,
        format!("{}/art/t1?size=64", server.url()),
        "size clamped to 64"
    );

    let like = recv(
        env.host
            .recommendations_for_item(ECHO, track("t1", "Song"), page(5)),
    )
    .unwrap();
    assert_eq!(like.items[0].id, "like-t1");
    assert_eq!(
        recv(env.host.recommendations_for_user(ECHO, page(5))),
        Err(CallError::Plugin(PluginError::AuthRequired))
    );
}

#[test]
fn host_imports_work() {
    let (env, server) = echo_env();
    assert_eq!(titles(&env.host, ECHO, "do:kv"), vec!["hello", "42", "None"]);
    let quota = titles(&env.host, ECHO, "do:kv-quota");
    assert_eq!(quota[0], "true");
    assert!(quota[1].contains("quota exceeded"), "{quota:?}");

    assert_eq!(titles(&env.host, ECHO, "do:secret-set"), vec!["stored"]);
    assert_eq!(titles(&env.host, ECHO, "do:secret-get"), vec!["s3cret-token"]);
    assert!(env.secrets.keys().contains(&format!("jellyfesh/{ECHO}/token")));

    let crypto = titles(&env.host, ECHO, "do:crypto");
    assert_eq!(crypto[0], "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(
        crypto[1],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        crypto[2],
        "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
    );
    assert_eq!(crypto[3], "plain");
    assert_eq!(crypto[4].len(), 16);

    let clock = titles(&env.host, ECHO, "do:clock");
    let now: u64 = clock[0].parse().unwrap();
    let host_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!(now.abs_diff(host_now) < 5_000);
    assert!(clock[1].parse::<u64>().is_ok());

    // getrandom goes through the SDK's backend over crypto.random-bytes.
    let a = titles(&env.host, ECHO, "do:random");
    let b = titles(&env.host, ECHO, "do:random");
    assert_eq!(a[0].len(), 32);
    assert_ne!(a, b);

    // 300 lines: the rate limit drops some, the call still succeeds.
    assert_eq!(titles(&env.host, ECHO, "do:log"), vec!["logged"]);

    let headers = titles(&env.host, ECHO, "do:headers");
    assert_eq!(headers[0], "201");
    assert_eq!(headers[1], r#"echo application/json {"hello":"world"}"#);
    let posted = server
        .requests()
        .into_iter()
        .find(|r| r.path() == "/headers")
        .unwrap();
    assert_eq!(posted.method, "POST");
    assert_eq!(posted.header("user-agent"), Some("Jellyfesh"));
}

#[test]
fn settings_are_validated_stored_and_delivered() {
    let env = Env::new();
    let first = echo_server();
    let second = echo_server();
    env.install("echo", &echo_manifest(), ECHO, None);
    // No server yet: the plugin says so, and nothing is allowed.
    assert!(matches!(
        recv(env.host.search(ECHO, "x", any(), page(5))),
        Err(CallError::Plugin(PluginError::InvalidInput(_)))
    ));
    for bad in [
        "not json",
        "[]",
        r#"{"server_url": "ftp://x.example"}"#,
        r#"{"other": 1}"#,
    ] {
        assert!(
            matches!(env.host.set_settings(ECHO, bad), Err(HostError::Settings(_))),
            "{bad}"
        );
    }
    env.host
        .set_settings(ECHO, &json!({ "server_url": first.url() }).to_string())
        .unwrap();
    recv(env.host.search(ECHO, "a", any(), page(5))).unwrap();
    env.host
        .set_settings(ECHO, &json!({ "server_url": second.url() }).to_string())
        .unwrap();
    recv(env.host.search(ECHO, "b", any(), page(5))).unwrap();
    assert_eq!((first.count("/search"), second.count("/search")), (1, 1));
    assert_eq!(
        env.host.plugin(ECHO).unwrap().settings["server_url"],
        second.url()
    );
}

#[test]
fn qr_login() {
    let (env, _server) = echo_env();
    let flow = env.host.begin_auth(ECHO, AuthKind::Qr).unwrap();
    match next(&flow) {
        AuthStep::ShowQr {
            data,
            hint,
            expires_at_ms,
        } => {
            assert!(data.starts_with("jf-test://login/"), "{data}");
            assert_eq!(hint.as_deref(), Some("Scan with the test app"));
            assert!(expires_at_ms.is_some());
        }
        other => panic!("expected a QR code, got {other:?}"),
    }
    assert_eq!(
        next(&flow),
        AuthStep::Waiting {
            message: "Confirm on your phone".into()
        }
    );
    assert_eq!(
        next(&flow),
        AuthStep::Done {
            account: "QR User".into()
        }
    );
    assert_eq!(env.host.plugin(ECHO).unwrap().account.as_deref(), Some("QR User"));
    // Logged in: personal recommendations work now.
    assert!(recv(env.host.recommendations_for_user(ECHO, page(5))).is_ok());
}

#[test]
fn oauth_login_with_pkce_and_loopback() {
    let (env, server) = echo_env();
    let flow = env.host.begin_auth(ECHO, AuthKind::Oauth).unwrap();
    let AuthStep::OpenUrl { url } = next(&flow) else {
        panic!("expected a browser page");
    };
    let url = url::Url::parse(&url).unwrap();
    assert_eq!(url.path(), "/authorize");
    let param = |key: &str| url.query_pairs().find(|(k, _)| k == key).unwrap().1.into_owned();
    let redirect_uri = param("redirect_uri");
    assert!(redirect_uri.starts_with("http://127.0.0.1:"), "{redirect_uri}");
    assert_eq!(param("code_challenge_method"), "S256");

    // A forged redirect with the wrong state is refused and the flow waits on.
    let forged = browser_get(&format!("{redirect_uri}?code=evil&state=wrong"));
    assert!(forged.starts_with("HTTP/1.1 400"), "{forged}");
    let answer = browser_get(&format!("{redirect_uri}?code=good-code&state={}", param("state")));
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(answer.contains("You are logged in"));

    assert_eq!(
        next(&flow),
        AuthStep::Done {
            account: "OAuth User".into()
        }
    );
    // The token request carried a verifier that matches the challenge.
    let token = server
        .requests()
        .into_iter()
        .find(|r| r.path() == "/token")
        .unwrap();
    let verifier = token.form("code_verifier").unwrap();
    assert_eq!(
        BASE64URL.encode(Sha256::digest(verifier.as_bytes())),
        param("code_challenge")
    );
    assert_eq!(token.form("redirect_uri").unwrap(), redirect_uri);
    assert!(
        env.secrets
            .keys()
            .contains(&format!("jellyfesh/{ECHO}/access-token"))
    );
}

#[test]
fn cookie_import_login() {
    let (env, _server) = echo_env();
    let flow = env.host.begin_auth(ECHO, AuthKind::CookieImport).unwrap();
    let AuthStep::AskFields { fields: form } = next(&flow) else {
        panic!("expected a form");
    };
    assert_eq!(form[0].kind, FieldKind::CookieHeader);
    assert!(form[0].secret);
    flow.submit(fields(&[(
        "cookie",
        "Cookie: session=abc123; tracking=zzz; other=1",
    )]));
    assert_eq!(
        next(&flow),
        AuthStep::Done {
            account: "Cookie User".into()
        }
    );
    // Only the declared cookie was imported, and it is sent from now on.
    assert_eq!(titles(&env.host, ECHO, "do:cookies"), vec!["session=abc123"]);
    // The jar on disk is encrypted.
    assert!(files_containing(env.dir.path(), "abc123").is_empty());
}

#[test]
fn password_exchange_never_stores_the_password() {
    let (env, server) = echo_env();
    let flow = env.host.begin_auth(ECHO, AuthKind::PasswordExchange).unwrap();
    let AuthStep::AskFields { fields: form } = next(&flow) else {
        panic!("expected a form");
    };
    assert!(form.iter().any(|f| f.name == "password" && f.secret));
    flow.submit(fields(&[("username", "alice"), ("password", PASSWORD)]));
    assert_eq!(
        next(&flow),
        AuthStep::Done {
            account: "alice".into()
        }
    );
    let login = server
        .requests()
        .into_iter()
        .find(|r| r.path() == "/login")
        .unwrap();
    assert_eq!(
        login.form("password").as_deref(),
        Some(PASSWORD),
        "it reached the service once"
    );
    assert_eq!(titles(&env.host, ECHO, "do:secret-get"), vec!["tok-alice"]);

    assert!(
        !env.secrets.any_value_contains(PASSWORD),
        "a password is in the keystore"
    );
    assert!(
        files_containing(env.dir.path(), PASSWORD).is_empty(),
        "a password is on disk"
    );
    // The instance that saw the password was restarted.
    assert!(
        titles(&env.host, ECHO, "do:seen").is_empty(),
        "the password lingers in plugin memory"
    );

    // A plugin that tries to keep the password is refused, in kv and in the keystore.
    let flow = env.host.begin_auth(ECHO, AuthKind::PasswordExchange).unwrap();
    let _form = next(&flow);
    flow.submit(fields(&[
        ("username", "hoarder"),
        ("password", "hunter22-secret"),
    ]));
    assert_eq!(
        next(&flow),
        AuthStep::Failed {
            message: "the host refused to store the password".into()
        }
    );
    assert!(!env.secrets.any_value_contains("hunter22-secret"));
    assert!(files_containing(env.dir.path(), "hunter22-secret").is_empty());
}

#[test]
fn api_key_login() {
    let (env, server) = echo_env();
    let flow = env.host.begin_auth(ECHO, AuthKind::ApiKey).unwrap();
    let _form = next(&flow);
    flow.submit(fields(&[("api-key", "key-123")]));
    assert_eq!(
        next(&flow),
        AuthStep::Done {
            account: "API User".into()
        }
    );
    assert_eq!(server.count("/ping"), 1);
    // An API key is meant to be kept, as a secret.
    assert!(env.secrets.any_value_contains("key-123"));

    let flow = env.host.begin_auth(ECHO, AuthKind::ApiKey).unwrap();
    let _form = next(&flow);
    flow.submit(fields(&[("api-key", "wrong")]));
    assert!(matches!(next(&flow), AuthStep::Failed { .. }));
}

#[test]
fn logins_can_be_cancelled_and_must_be_declared() {
    let (env, _server) = echo_env();
    let flow = env.host.begin_auth(ECHO, AuthKind::PasswordExchange).unwrap();
    let _form = next(&flow);
    flow.cancel();
    assert_eq!(
        next(&flow),
        AuthStep::Failed {
            message: "cancelled".into()
        }
    );
    assert!(
        flow.steps().recv_timeout(Duration::from_secs(2)).is_err(),
        "the flow ended"
    );

    // The same plugin with a manifest that only lists QR logins.
    const QR_ONLY: &str = "org.jellyfesh.test.qr-only";
    let qr_only = manifest(
        QR_ONLY,
        &[
            "auth",
            "search",
            "browse",
            "resolve_stream",
            "lyrics",
            "artwork",
            "recommendations",
        ],
        &["qr"],
        &["${settings.server_url}"],
        5,
    );
    env.install("echo", &qr_only, QR_ONLY, None);
    assert!(env.host.begin_auth(QR_ONLY, AuthKind::Qr).is_ok());
    assert!(matches!(
        env.host.begin_auth(QR_ONLY, AuthKind::PasswordExchange),
        Err(CallError::NotDeclared(_))
    ));
    assert!(matches!(
        env.host.begin_auth("org.jellyfesh.test.nothing", AuthKind::Qr),
        Err(CallError::NotInstalled(_))
    ));
}

#[test]
fn logout_forgets_credentials() {
    let (env, server) = echo_env();
    let flow = env.host.begin_auth(ECHO, AuthKind::PasswordExchange).unwrap();
    let _form = next(&flow);
    flow.submit(fields(&[("username", "alice"), ("password", PASSWORD)]));
    assert!(matches!(next(&flow), AuthStep::Done { .. }));
    let _ = titles(&env.host, ECHO, "do:cookies"); // creates the jar's key
    assert!(!env.secrets.keys().is_empty());

    recv(env.host.logout(ECHO)).unwrap();
    assert_eq!(server.count("/logout"), 1, "the plugin could revoke the session");
    assert!(env.secrets.keys().is_empty(), "{:?}", env.secrets.keys());
    assert!(env.host.plugin(ECHO).unwrap().account.is_none());
    assert_eq!(titles(&env.host, ECHO, "do:secret-get"), vec!["none"]);
}

#[test]
fn logout_of_a_plugin_that_is_turned_off_still_forgets_credentials() {
    let (env, server) = echo_env();
    assert_eq!(titles(&env.host, ECHO, "do:secret-set"), vec!["stored"]);
    assert!(!env.secrets.keys().is_empty());
    env.host.set_enabled(ECHO, false).unwrap();

    recv(env.host.logout(ECHO)).unwrap();
    assert_eq!(server.count("/logout"), 0, "a plugin that is off is not called");
    assert!(env.secrets.keys().is_empty(), "{:?}", env.secrets.keys());
    env.host.set_enabled(ECHO, true).unwrap();
    assert_eq!(titles(&env.host, ECHO, "do:secret-get"), vec!["none"]);
}

#[test]
fn plugins_survive_a_restart() {
    let (env, server) = echo_env();
    assert_eq!(titles(&env.host, ECHO, "do:secret-set"), vec!["stored"]);
    let flow = env.host.begin_auth(ECHO, AuthKind::Qr).unwrap();
    while !next(&flow).is_final() {}
    drop(flow);

    let host = env.reopen();
    let info = host.plugin(ECHO).unwrap();
    assert_eq!(info.settings["server_url"], server.url());
    assert_eq!(info.account.as_deref(), Some("QR User"));
    assert!(info.official);
    // Compiled lazily on the first call.
    assert_eq!(titles(&host, ECHO, "do:secret-get"), vec!["qr-token"]);
}

#[test]
fn uninstall_removes_everything() {
    let (env, _server) = echo_env();
    assert_eq!(titles(&env.host, ECHO, "do:secret-set"), vec!["stored"]);
    assert_eq!(titles(&env.host, ECHO, "do:kv")[0], "hello");
    env.host.uninstall(ECHO).unwrap();
    assert!(env.host.plugins().is_empty());
    assert!(env.secrets.keys().is_empty());
    let plugins = env.dir.path().join("plugins");
    assert!(!plugins.join("data").join(ECHO).exists());
    assert!(!plugins.join("packages").join(format!("{ECHO}.jfpkg")).exists());
    assert!(matches!(
        recv(env.host.search(ECHO, "x", any(), page(1))),
        Err(CallError::NotInstalled(_))
    ));
    assert!(matches!(
        env.host.uninstall(ECHO),
        Err(HostError::NotInstalled(_))
    ));
}

#[test]
fn community_plugins_and_signatures() {
    let env = Env::new();
    let server = echo_server();
    let unsigned = package(
        &env.incoming(),
        "unsigned.jfpkg",
        &echo_manifest(),
        component("echo"),
        false,
    );
    assert!(matches!(
        env.host.install(&unsigned),
        Err(HostError::CommunityDisabled)
    ));

    env.host.set_allow_community(true);
    let info = env.host.install(&unsigned).unwrap();
    assert!(!info.official);
    assert_eq!(info.signature, SignatureStatus::Unsigned);
    assert!(!info.enabled, "community plugins start turned off");
    assert!(matches!(
        recv(env.host.search(ECHO, "x", any(), page(1))),
        Err(CallError::Disabled(_))
    ));
    env.host.set_enabled(ECHO, true).unwrap();
    env.host
        .set_settings(ECHO, &json!({ "server_url": server.url() }).to_string())
        .unwrap();
    assert!(recv(env.host.search(ECHO, "x", any(), page(1))).is_ok());

    env.host.set_allow_community(false);
    assert!(matches!(
        recv(env.host.search(ECHO, "x", any(), page(1))),
        Err(CallError::CommunityDisabled)
    ));
    assert!(matches!(
        env.host.set_enabled(ECHO, true),
        Err(HostError::CommunityDisabled)
    ));
    env.host.set_allow_community(true);

    // An update signed by a key (where the first install was unsigned) is
    // refused until the plugin is uninstalled (trust on first use).
    let signed = package(
        &env.incoming(),
        "signed.jfpkg",
        &echo_manifest(),
        component("echo"),
        true,
    );
    assert!(matches!(
        env.host.install(&signed),
        Err(HostError::KeyChanged(..))
    ));
    env.host.uninstall(ECHO).unwrap();
    let info = env.host.install(&signed).unwrap();
    assert!(info.official, "the test key is in this host's trust list");

    // A host with production options (no test key in its trust list) sees
    // the same package as a community package with the key's fingerprint.
    let plain = jf_plugin_host::PluginHost::new(jf_plugin_host::HostConfig {
        dir: env.dir.path().join("plain"),
        cache_dir: env.dir.path().join("plain-cache"),
        secrets: env.secrets.clone(),
        allow_community: true,
    })
    .unwrap();
    let info = plain.install(&signed).unwrap();
    assert!(!info.official);
    assert_eq!(
        info.signature,
        SignatureStatus::Community {
            key_id: key_id(&public_key(&test_key_seed()))
        }
    );

    // A package whose contents changed after signing is refused.
    let bytes = PackageBuilder::new()
        .manifest(&echo_manifest())
        .wasm(component("echo"))
        .file("settings.schema.json", SCHEMA.as_bytes().to_vec())
        .sign(test_key_seed())
        .build()
        .unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut signature = String::new();
    archive
        .by_name("SIGNATURE")
        .unwrap()
        .read_to_string(&mut signature)
        .unwrap();
    let tampered = PackageBuilder::new()
        .manifest(&echo_manifest().replace("max_rps = 20", "max_rps = 19"))
        .wasm(component("echo"))
        .file("settings.schema.json", SCHEMA.as_bytes().to_vec())
        .file("SIGNATURE", signature.into_bytes())
        .build()
        .unwrap();
    let path = env.incoming().join("tampered.jfpkg");
    std::fs::write(&path, tampered).unwrap();
    assert!(matches!(plain.install(&path), Err(HostError::Signature(_))));
}

/// Compile, instantiate and call times (printed; run with `--nocapture`).
#[test]
fn timings() {
    let env = Env::new();
    let server = echo_server();
    let path = package(
        &env.incoming(),
        "echo.jfpkg",
        &echo_manifest(),
        component("echo"),
        true,
    );
    let (_, install) = timed(|| env.host.install(&path).unwrap());
    env.host
        .set_settings(ECHO, &json!({ "server_url": server.url() }).to_string())
        .unwrap();

    // Warm calls without network: the host's round trip plus a tiny export.
    let mut warm: Vec<Duration> = (0..200)
        .map(|_| timed(|| recv(env.host.lyrics(ECHO, track("t", "Song"))).unwrap()).1)
        .collect();
    warm.sort();
    // Search against a local server: host overhead plus a loopback request.
    // Spaced out so the plugin's rate limit (20 requests a second) is not
    // what gets measured.
    let mut search: Vec<Duration> = (0..30)
        .map(|_| {
            std::thread::sleep(Duration::from_millis(60));
            timed(|| recv(env.host.search(ECHO, "q", any(), page(20))).unwrap()).1
        })
        .collect();
    search.sort();

    // A fresh host: the first call compiles, instantiates and delivers settings.
    let host = env.reopen();
    let (_, cold) = timed(|| recv(host.lyrics(ECHO, track("t", "Song"))).unwrap());
    // A password login restarts the instance: the next call instantiates
    // the already-compiled component, delivers settings and runs.
    let flow = host.begin_auth(ECHO, AuthKind::PasswordExchange).unwrap();
    let _form = next(&flow);
    flow.submit(fields(&[("username", "nobody"), ("password", "wrong")]));
    while !next(&flow).is_final() {}
    let (_, restart) = timed(|| recv(host.lyrics(ECHO, track("t", "Song"))).unwrap());
    let size = component("echo").len();
    eprintln!(
        "echo component {} KiB (debug build): install (compile + link + start) {install:?}; \
         cold first call (compile + instantiate + settings + call) {cold:?}; \
         call after a restart (instantiate + settings + call) {restart:?}; \
         warm call median {:?} p95 {:?}; search via loopback HTTP median {:?} p95 {:?}",
        size / 1024,
        warm[warm.len() / 2],
        warm[warm.len() * 95 / 100],
        search[search.len() / 2],
        search[search.len() * 95 / 100],
    );
    assert!(
        warm[warm.len() / 2] < Duration::from_millis(50),
        "{:?}",
        warm[warm.len() / 2]
    );
    assert!(
        search[search.len() / 2] < Duration::from_millis(50),
        "{:?}",
        search[search.len() / 2]
    );
}

/// The same measurements for any build of the echo plugin, for example a
/// release build: `cargo xtask plugin build jf-test-echo --release --out
/// /tmp/echo.wasm`, then `JF_TIMING_COMPONENT=/tmp/echo.wasm cargo test -p
/// jf-plugin-host --test happy -- --ignored --nocapture measure_component`.
#[test]
#[ignore = "manual measurement; needs JF_TIMING_COMPONENT"]
fn measure_component() {
    let path = std::env::var("JF_TIMING_COMPONENT").expect("JF_TIMING_COMPONENT");
    let wasm = std::fs::read(&path).unwrap();
    let env = Env::new();
    let server = echo_server();
    let package_path = package(
        &env.incoming(),
        "echo.jfpkg",
        &echo_manifest(),
        wasm.clone(),
        true,
    );
    let (_, install) = timed(|| env.host.install(&package_path).unwrap());
    env.host
        .set_settings(ECHO, &json!({ "server_url": server.url() }).to_string())
        .unwrap();
    let mut colds = Vec::new();
    for _ in 0..5 {
        let host = env.reopen();
        colds.push(timed(|| recv(host.lyrics(ECHO, track("t", "Song"))).unwrap()).1);
    }
    colds.sort();
    let mut warm: Vec<Duration> = (0..200)
        .map(|_| timed(|| recv(env.host.lyrics(ECHO, track("t", "Song"))).unwrap()).1)
        .collect();
    warm.sort();
    eprintln!(
        "{path}: {} KiB; install {install:?}; cold first call median of 5 {:?}; warm call median {:?} p95 {:?}",
        wasm.len() / 1024,
        colds[2],
        warm[warm.len() / 2],
        warm[warm.len() * 95 / 100],
    );
}
