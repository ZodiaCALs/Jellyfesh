//! A well-behaved test plugin: every capability, every login kind, and
//! `do:<command>` searches that exercise the host imports. It talks to the
//! test server named by the `server_url` setting.

use std::cell::RefCell;

use jf_plugin_sdk::prelude::*;
use serde::Deserialize;

struct Echo;

thread_local! {
    static SERVER: RefCell<String> = const { RefCell::new(String::new()) };
    /// A password seen in `auth_submit`, to prove the host restarts the
    /// instance afterwards (`do:seen` reads it back).
    static SEEN: RefCell<String> = const { RefCell::new(String::new()) };
}

fn server() -> Result<String> {
    let url = SERVER.with(|s| s.borrow().clone());
    if url.is_empty() {
        Err(PluginError::invalid_input("no server configured"))
    } else {
        Ok(url.trim_end_matches('/').to_string())
    }
}

fn titles(titles: Vec<String>) -> ItemPage {
    ItemPage::new(
        titles
            .into_iter()
            .enumerate()
            .map(|(i, title)| Item::new(ItemKind::Track, i.to_string(), title))
            .collect(),
    )
}

#[derive(Deserialize)]
struct Hit {
    id: String,
    title: String,
    artist: Option<String>,
    duration_ms: Option<u32>,
    isrc: Option<String>,
    artwork: Option<String>,
    playable: Option<bool>,
}

/// `ok:<hex signature>` or `err:<crypto error>`, for the signing tests.
fn signed(result: core::result::Result<Vec<u8>, crypto::CryptoError>) -> String {
    match result {
        Ok(signature) => format!("ok:{}", crypto::hex(&signature)),
        Err(error) => format!("err:{error:?}"),
    }
}

fn command(command: &str) -> Result<ItemPage> {
    let out = match command {
        "kv" => {
            kv::set("greeting", b"hello")?;
            kv::set_json("answer", &42u32)?;
            let greeting = String::from_utf8(kv::get("greeting")?.unwrap_or_default()).unwrap_or_default();
            let answer = kv::get_json::<u32>("answer")?.unwrap_or_default();
            kv::delete("answer")?;
            vec![greeting, answer.to_string(), format!("{:?}", kv::get("answer")?)]
        }
        "kv-quota" => {
            let big = vec![1u8; 600 * 1024];
            let first = kv::set("big-1", &big).is_ok();
            let second = kv::set("big-2", &big);
            vec![first.to_string(), format!("{second:?}")]
        }
        "secret-set" => {
            kv::secret::set("token", "s3cret-token")?;
            vec!["stored".into()]
        }
        "secret-get" => vec![kv::secret::get("token")?.unwrap_or_else(|| "none".into())],
        "crypto" => {
            use jf_plugin_sdk::crypto::{AesMode, GcmParams, HashAlgorithm};
            let key = [7u8; 16];
            let gcm = AesMode::Gcm(GcmParams {
                nonce: vec![1; 12],
                aad: Vec::new(),
            });
            let sealed = crypto::aes_encrypt(&gcm, &key, b"plain")?;
            let opened = crypto::aes_decrypt(&gcm, &key, &sealed)?;
            vec![
                crypto::md5_hex("abc")?,
                crypto::hex(&crypto::sha256(b"abc")?),
                crypto::hex(&crypto::hmac(
                    HashAlgorithm::Sha256,
                    b"key",
                    b"The quick brown fox jumps over the lazy dog",
                )?),
                String::from_utf8(opened).unwrap_or_default(),
                crypto::random_hex(8)?,
            ]
        }
        "rsa-sign-big" => {
            // One byte over the 16 MiB input limit.
            let data = vec![0u8; 16 * 1024 * 1024 + 1];
            vec![signed(crypto::rsa_sign_pkcs1_sha256("signing-key", &data))]
        }
        "clock" => vec![clock::now_utc_ms().to_string(), clock::monotonic_ms().to_string()],
        "random" => {
            let mut bytes = [0u8; 16];
            getrandom::fill(&mut bytes).map_err(|e| PluginError::internal(e.to_string()))?;
            vec![crypto::hex(&bytes)]
        }
        "log" => {
            for i in 0..300 {
                jf_plugin_sdk::info!("echo log line {i}");
            }
            vec!["logged".into()]
        }
        "cookies" => {
            let response = http::Request::get(format!("{}/cookies", server()?)).send()?;
            vec![response.text()]
        }
        "headers" => {
            let response = http::Request::post(format!("{}/headers", server()?))
                .header("x-plugin", "echo")
                .json(&serde_json::json!({ "hello": "world" }))?
                .send()?;
            vec![response.status.to_string(), response.text()]
        }
        "seen" => vec![SEEN.with(|s| s.borrow().clone())],
        // `rsa-sign:<secret>`: signs "jellyfesh" with the key in that secret.
        other if other.starts_with("rsa-sign:") => {
            let secret = &other["rsa-sign:".len()..];
            vec![signed(crypto::rsa_sign_pkcs1_sha256(secret, b"jellyfesh"))]
        }
        other => return Err(PluginError::invalid_input(format!("unknown command {other}"))),
    };
    Ok(titles(out))
}

impl Plugin for Echo {
    fn capabilities() -> Vec<Capability> {
        vec![
            Capability::Auth,
            Capability::Search,
            Capability::Browse,
            Capability::ResolveStream,
            Capability::Lyrics,
            Capability::Artwork,
            Capability::Recommendations,
        ]
    }

    fn settings_changed(settings: &serde_json::Value) -> Result<()> {
        let url = settings["server_url"].as_str().unwrap_or_default().to_string();
        SERVER.with(|s| *s.borrow_mut() = url);
        Ok(())
    }

    fn search(query: String, _filters: SearchFilters, page: PageRequest) -> Result<ItemPage> {
        if let Some(rest) = query.strip_prefix("do:") {
            return command(rest);
        }
        let hits: Vec<Hit> = http::Request::get(format!("{}/search", server()?))
            .query(&[("q", &query), ("limit", &page.limit.to_string())])
            .send_json()?;
        let items = hits
            .into_iter()
            .map(|h| {
                let mut item = Item::new(ItemKind::Track, h.id, h.title);
                item.artist = h.artist;
                item.duration_ms = h.duration_ms;
                item.isrc = h.isrc;
                if let Some(url) = h.artwork {
                    item = item.artwork(url);
                }
                if h.playable == Some(false) {
                    item = item.unplayable(UnplayableReason::Region, None);
                }
                item
            })
            .collect();
        Ok(ItemPage::new(items))
    }

    fn browse(node: Option<Item>, page: PageRequest) -> Result<ItemPage> {
        match node {
            None => Ok(ItemPage::new(vec![Item::new(
                ItemKind::Folder,
                "albums",
                "Albums",
            )])),
            Some(node) if node.id == "albums" => {
                let start: u32 = page.cursor.as_deref().and_then(|c| c.parse().ok()).unwrap_or(0);
                let items = (start..start + page.limit)
                    .map(|i| Item::new(ItemKind::Album, format!("al{i}"), format!("Album {i}")))
                    .collect();
                Ok(ItemPage::with_next(items, (start + page.limit).to_string()))
            }
            Some(_) => Err(PluginError::NotFound),
        }
    }

    fn resolve_stream(item: Item, quality: Quality) -> Result<StreamInfo> {
        if item.id == "outside" {
            return Ok(StreamInfo::new("https://evil.example/stream.flac"));
        }
        let mut info = StreamInfo::new(format!("{}/stream/{}.flac", server()?, item.id));
        info.headers.push(Header::new("x-token", "abc"));
        info.codec = Some("FLAC".into());
        info.bitrate_kbps = Some(match quality {
            Quality::Low => 96,
            Quality::Normal => 160,
            Quality::High => 320,
            Quality::Lossless => 1000,
        });
        info.cacheable = true;
        Ok(info)
    }

    fn lyrics(item: Item) -> Result<Option<LyricPayload>> {
        if item.title == "silent" {
            return Ok(None);
        }
        Ok(Some(LyricPayload::new(
            LyricFormat::Lrc,
            format!("[00:01.00]{}", item.title),
        )))
    }

    fn artwork(item: Item, size: u32) -> Result<Option<ImageRef>> {
        Ok(Some(ImageRef::new(format!(
            "{}/art/{}?size={size}",
            server()?,
            item.id
        ))))
    }

    fn recommendations_for_item(item: Item, _page: PageRequest) -> Result<ItemPage> {
        Ok(ItemPage::new(vec![Item::new(
            ItemKind::Track,
            format!("like-{}", item.id),
            "Similar",
        )]))
    }

    fn recommendations_for_user(_page: PageRequest) -> Result<ItemPage> {
        match kv::secret::get("token")? {
            Some(_) => Ok(ItemPage::new(vec![Item::new(
                ItemKind::Playlist,
                "daily",
                "Daily mix",
            )])),
            None => Err(PluginError::AuthRequired),
        }
    }

    fn auth_begin(flow: Flow) -> Result<FlowStep> {
        Ok(match flow.kind {
            AuthKind::Qr => FlowStep::new(Step::ShowQr(QrCode {
                data: format!("jf-test://login/{}", flow.id),
                hint: Some("Scan with the test app".into()),
                expires_at_ms: Some(clock::now_utc_ms() + 60_000),
            }))
            .state("scan")
            .poll_after_ms(200),
            AuthKind::Oauth => {
                let oauth = flow
                    .oauth
                    .ok_or_else(|| PluginError::invalid_input("no oauth request"))?;
                let url = format!(
                    "{}/authorize?{}",
                    server()?,
                    http::form_encode(&[
                        ("client_id", "jellyfesh-test"),
                        ("redirect_uri", &oauth.redirect_uri),
                        ("state", &oauth.state),
                        ("code_challenge", &oauth.code_challenge),
                        ("code_challenge_method", &oauth.code_challenge_method),
                    ])
                );
                FlowStep::new(Step::OpenUrl(OpenUrl {
                    url,
                    capture: RedirectCapture::Loopback,
                }))
            }
            AuthKind::CookieImport => FlowStep::new(Step::AskFields(vec![FieldSpec {
                kind: FieldKind::CookieHeader,
                ..FieldSpec::secret("cookie", "Cookie header from your browser")
            }])),
            AuthKind::PasswordExchange => FlowStep::new(Step::AskFields(vec![
                FieldSpec::text("username", "User name"),
                FieldSpec::secret("password", "Password"),
            ])),
            AuthKind::ApiKey => FlowStep::new(Step::AskFields(vec![FieldSpec::secret("api-key", "API key")])),
        })
    }

    fn auth_poll(flow: Flow) -> Result<FlowStep> {
        Ok(match flow.state.as_str() {
            "scan" => FlowStep::new(Step::Waiting("Confirm on your phone".into()))
                .state("confirm")
                .poll_after_ms(200),
            "confirm" => {
                kv::secret::set("token", "qr-token")?;
                FlowStep::done("QR User")
            }
            _ => FlowStep::failed("unexpected poll"),
        })
    }

    fn auth_submit(flow: Flow, fields: Vec<FieldValue>) -> Result<FlowStep> {
        let field = |name: &str| {
            fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.value.clone())
                .unwrap_or_default()
        };
        #[derive(Deserialize)]
        struct Token {
            access_token: String,
            name: String,
        }
        #[derive(Deserialize)]
        struct Me {
            name: String,
        }
        #[derive(Deserialize)]
        struct Login {
            token: String,
        }
        Ok(match flow.kind {
            AuthKind::Oauth => {
                let token: Token = http::Request::post(format!("{}/token", server()?))
                    .form(&[
                        ("grant_type", "authorization_code"),
                        ("code", &field("code")),
                        ("code_verifier", &field("code-verifier")),
                        ("redirect_uri", &field("redirect-uri")),
                    ])
                    .send_json()?;
                kv::secret::set("access-token", &token.access_token)?;
                FlowStep::done(token.name)
            }
            AuthKind::CookieImport => {
                if field("cookie") != "session" {
                    return Ok(FlowStep::failed(format!(
                        "unexpected cookie field {:?}",
                        field("cookie")
                    )));
                }
                let me: Me = http::Request::get(format!("{}/me", server()?)).send_json()?;
                FlowStep::done(me.name)
            }
            AuthKind::PasswordExchange => {
                let (user, password) = (field("username"), field("password"));
                SEEN.with(|s| *s.borrow_mut() = password.clone());
                if user == "hoarder" {
                    let plain = kv::set("pw", password.as_bytes());
                    let secret = kv::secret::set("pw", &password);
                    return Ok(if plain.is_err() && secret.is_err() {
                        FlowStep::failed("the host refused to store the password")
                    } else {
                        FlowStep::done("stored the password")
                    });
                }
                jf_plugin_sdk::info!("logging in {user} with {password}");
                let login: Login = http::Request::post(format!("{}/login", server()?))
                    .form(&[("username", &user), ("password", &password)])
                    .send_json()?;
                kv::secret::set("token", &login.token)?;
                FlowStep::done(user)
            }
            AuthKind::ApiKey => {
                let key = field("api-key");
                http::Request::get(format!("{}/ping", server()?))
                    .header("x-api-key", &key)
                    .send()?
                    .error_for_status()?;
                kv::secret::set("api-key", &key)?;
                FlowStep::done("API User")
            }
            AuthKind::Qr => FlowStep::failed("nothing to submit"),
        })
    }

    fn logout() -> Result<()> {
        let _ = http::Request::post(format!("{}/logout", server()?)).send();
        Ok(())
    }
}

jf_plugin_sdk::export_plugin!(Echo);
