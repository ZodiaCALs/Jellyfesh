# Writing a Jellyfesh plugin

A Jellyfesh source plugin is a WebAssembly component for the
`jellyfesh:plugin@1.1` world `remote-source` (`plugins/wit`): the `source`
world of API 1.0 plus the `remote` interface. It has no WASI at all: no
files, sockets, environment or clock of its own. Everything goes through the
host's imports (HTTP limited to the manifest's allowlist, small private
storage, the system keystore, crypto, time, logging), and the host calls its
exports (sign-in, search, browse, streams, lyrics, artwork,
recommendations) under a CPU, memory and time budget. ADR-0003 and ADR-0004
explain why; this guide is about doing it.

You need Rust (the version in `rust-toolchain.toml`) with the
`wasm32-unknown-unknown` target, and three pieces of this repository:

| Piece | What it is |
| --- | --- |
| `plugins/sdk` (`jf-plugin-sdk`) | the guest crate: bindings, the `Plugin` trait, `export_plugin!`, HTTP, kv and secrets, crypto, clock, logging |
| `tools/jf-plugin` (`jf-plugin`) | the tool: `new`, `build`, `pack`, `keygen`, `sign`, `verify`, `inspect` |
| `plugins/testkit` (`jf-plugin-testkit`) | runs your component in the real host against a mock of your service, for `cargo test` |

The official plugins are complete examples of everything below:
`plugins/subsonic` and `plugins/jellyfin` (servers the person runs),
`plugins/qqmusic` (links only), `plugins/spotify` (remote control, OAuth) and
`plugins/netease` (signed requests, QR sign-in).

Two **community plugins** live here too, in `plugins/community/`: NetEase
Cloud Music (Unofficial) and QQ Music (Unofficial)
(`community.unofficial.netease` and `community.unofficial.qqmusic`, crates
`jf-community-netease` and `jf-community-qqmusic`, with the shared
`jf-community-common` and the test-only `jf-community-testsupport`). They
keep ADR-0012's rules for community plugins (the services' web endpoints
and web QR sign-in only, nothing taken from the services' apps, no paywall
bypass), ship inside the apps but are never signed with the release key, so
the apps install them only once the person allows community plugins
(ADR-0012, amended for v0.0.6), and show how to test a service on fixed
hosts through a `mock-service` cargo feature (`plugins/community/README.md`).
Other community plugins live outside this repository.

Build the tool once from a checkout, or run it through cargo:

```sh
cargo install --path tools/jf-plugin      # then: jf-plugin --help
cargo run -p jf-plugin -- --help          # without installing
```

## 1. Start a crate

```sh
jf-plugin new radio-paradise --id org.example.radio-paradise --author "Ada Lovelace"
```

This writes `radio-paradise/` with `Cargo.toml`, `src/lib.rs` (a working
plugin that searches a JSON API), `manifest.toml`, `settings.schema.json`,
`README.md` and `.gitignore`. The crate is a workspace of its own and finds
the SDK by path (`--sdk <checkout>/plugins/sdk` if the tool cannot find it).
The SDK is not on crates.io; keep a Jellyfesh checkout next to your plugin.

A plugin that lives in this repository is a workspace member instead, with
`jf-plugin-sdk.workspace = true` (see `plugins/subsonic/Cargo.toml`).

## 2. Write it

```rust
use jf_plugin_sdk::prelude::*;

struct Radio;

impl Plugin for Radio {
    fn capabilities() -> Vec<Capability> {
        vec![Capability::Search, Capability::ResolveStream]
    }

    fn search(query: String, _filters: SearchFilters, page: PageRequest) -> Result<ItemPage> {
        // Ask the service with http::Request::get(…).query(…).send_json()?,
        // then map each hit to Item::new(ItemKind::Track, id, title).
        let _ = (query, page);
        Ok(ItemPage::new(Vec::new()))
    }
}

jf_plugin_sdk::export_plugin!(Radio);
```

Every method you leave out answers `unsupported`, and the host only calls
what the manifest declares. Things to know:

* **State.** Methods are associated functions; the host may restart an
  instance at any time (after a trap, a limit, or a password sign-in). Keep
  caches in `thread_local!` cells, anything that must last in `kv` (at most
  1 MiB), and credentials in `kv::secret` (the system keystore). Never store
  a password: during a password-exchange sign-in the host refuses to store
  any value that contains it. Tie stored credentials to the server address
  they were made for, so a changed address never receives them (both
  official plugins store a short hash of the address with the secret).
* **Settings.** `settings_changed` receives the person's settings as JSON
  matching `settings.schema.json`, before any other call. The schema is a
  small profile of JSON Schema that the app renders natively
  (`core/jf-plugin-host/src/schema.rs`): flat, scalar properties, `enum`,
  `format: "uri"`, defaults. Secrets do not belong there.
* **Network.** `[network] allow` lists fixed host names (https only) or
  `${settings.<key>}`, the origin the person entered, which may be on the
  local network (`core/jf-plugin-host/src/manifest.rs`). Redirects are
  re-checked; `max_rps` is capped at 20.
* **Items.** Ids are yours and opaque to the host: make them stable and tell
  your kinds apart (the official plugins use `tr:`, `al:`, `ar:`, `pl:`, `f:`
  prefixes). Lyrics, artwork and recommendation calls may get items from
  other sources: match those by title, artist and length. Mark tracks the
  account cannot play with `unplayable` and a reason; never work around a
  paywall or region lock.
* **Paging.** `page.cursor` is whatever your previous page put in `next`;
  return `next: None` on the last page and `total` when the service says.
* **Errors.** `auth-required` makes the app offer to sign in again,
  `not-found`, `network`, `service(text)` (shown to the person), `internal`
  for bugs. HTTP failures convert with `?`.
* **Streams and images.** Put credentials in `StreamInfo::headers` (and
  `ImageRef::headers`) rather than in URLs where the service allows it; set
  `expires_at_ms` when the URL stops working. Item artwork may be stored with
  the item, so it must not carry credentials.
* **Sign-in kinds.** `password-exchange` (fields, exchanged for a token in
  `submit`), `api-key`, `qr` (a QR code or, with `show-code`, a code to type
  elsewhere, then `poll`), `oauth` (PKCE in the system browser) and
  `cookie-import`. The WIT `auth` interface documents each flow.
* **Limits.** About 200 ms of CPU per call (2 s for `auth.begin`), 10 s of
  wall time (30 s for sign-in steps), 64 MiB of memory, 16 MiB per HTTP
  response. Waiting for HTTP does not count as CPU.

### API 1.1: links and remote control

These need `api_version = "1.1"` in the manifest (ADR-0018); without
them, keep `"1.0"` so the plugin also loads in apps that implement 1.0.

* **Links** open an item in its service. Add a `[links]` table with an
  `https` URL template per item kind, holding `{id}` once after the host;
  the host fills in the item's id (percent-encoded) and the apps offer
  **Open in {Service}**, the main action for items that cannot play here.
  The person's browser opens them, never the plugin, so their host need not
  be in `[network] allow`.

  ```toml
  [links]
  track = "https://music.example.com/song/{id}"
  playlist = "https://music.example.com/playlist?id={id}"
  ```

* **Remote control** is for services that only let an app control the
  person's own players. Declare the `remote` capability with
  `playback_mode = "remote"` (and no `resolve_stream`), then implement the
  `remote_*` methods: `remote_devices`, `remote_state` (`None` when nothing
  plays), `remote_play` (tracks from an index, or an album or playlist as
  `context`), `remote_resume`, `remote_pause`, `remote_next`,
  `remote_previous`, `remote_seek`, `remote_set_volume`, `remote_set_shuffle`,
  `remote_set_repeat` and `remote_transfer`. The host checks every answer:
  devices without an id or name are dropped, volumes cut to 0–100, one
  device at most is active, positions stay within the song. `meta`'s
  `capabilities` cannot report `remote` (its enum is 1.0's); the host lists
  it whenever the manifest declares it. Jellyfesh Web does not run remote
  sources yet.

* **A fixed OAuth redirect port.** An oauth login normally listens on a
  random loopback port. Services where each person registers their own app
  (and with it the exact redirect URI) need it fixed:

  ```toml
  [oauth]
  redirect_port = 29817   # 1024–65535; the redirect URI is http://127.0.0.1:29817/callback
  ```

  `flow.oauth.redirect-uri` then always holds that URI; show it in your
  sign-in help. If another program holds the port, the login ends with a
  message that names it.

* **Signing with a private key.** `crypto::rsa_sign_pkcs1_sha256(secret,
  data)` signs with RSASSA-PKCS1-v1_5 and SHA-256 using the RSA private key
  stored in your secret `secret` (PEM, PKCS #8 or #1, or the DER in base64).
  The host reads the key from the keystore itself: store it once when the
  person enters it and never read it back. A plugin that calls it needs
  `api_version = "1.1"`; Jellyfesh Web cannot sign and answers
  `invalid-input` ("not supported in the browser").

Plugins built with this SDK always export `remote` (answering `unsupported`
unless implemented); a 1.1 host looks it up only when the manifest declares
`remote`. Packages built for 1.0 keep working in 1.1 hosts unchanged.

## 3. Build

```sh
jf-plugin build                    # target/plugins/<package>.wasm
jf-plugin build --release          # smaller and faster
jf-plugin build --path plugins/subsonic --out /tmp/subsonic.wasm
```

`build` runs `cargo build --target wasm32-unknown-unknown` with
`--cfg getrandom_backend="custom"` (the SDK provides randomness from the
host), wraps the module into a component with `wit-component`, and refuses it
unless it imports only `jellyfesh:plugin/*@1` and exports the whole `source`
world (and `remote` when the manifest declares it, checked by `pack` and
`verify`). `cargo xtask plugin build <package>` and the plugin tests use the same
code (`tools/jf-plugin/plugin-build`). `opt-level = "s"` in the release
profile gives noticeably smaller components (the official plugins: about
440 KiB).

## 4. Test

The crate compiles for the host too, so plain unit tests work. For the real
thing, `jf-plugin-testkit` builds your component, packs it with your
manifest, schema and icon, signs it with the published test key and installs
it into a real `PluginHost` with production limits, then talks to a mock of
your service on 127.0.0.1 (the host lets the plugin reach it because it is
the configured server). A plugin outside this repository adds it by path,
like the SDK:

```toml
[dev-dependencies]
jf-plugin-testkit = { path = "../jellyfesh/plugins/testkit" }
serde_json = "1.0.154"
```

```rust
use jf_plugin_testkit::*;

#[test]
fn searches() {
    let server = TestServer::start(|request| match request.path() {
        "/api/search" => Response::json(&serde_json::json!([{ "id": "1", "title": "Blue" }])),
        _ => Response::text(404, "not found"),
    });
    let env = Env::new();
    let info = env.install_crate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")), "radio-paradise");
    env.settings(&info.id, serde_json::json!({ "server_url": server.url() }));
    let found = recv(env.host.search(&info.id, "blue", kinds(&[]), page(20))).unwrap();
    assert_eq!(found.items[0].title, "Blue");
}
```

The testkit also drives sign-in flows (`login`, `next_step`, `final_step`,
`fields`), fetches the URLs you hand out the way the engine would (`fetch`),
shows what was requested (`server.requests()`), and finds files containing a
secret (`env.files_containing`). For a service on fixed hosts, the test
package can name a mock instead: `PluginFiles::read(dir)
.with_test_server_setting("test_server")` adds that setting and its origin
to a test copy of the manifest and schema, `env.install_files` installs it,
and your plugin uses the setting when it is present (published packages do
not have it, and the host refuses settings their schema does not name).
Write the mock from the service's official
API documentation, with invented data, never from anyone's account; keep
expected output in golden files. The official plugins' `tests/` folders are
full examples (mock servers, fixtures, golden lyrics).

## 5. Pack

```sh
jf-plugin pack --release                         # target/plugins/<id>-<version>.jfpkg
jf-plugin pack --path plugins/jellyfin --out jellyfin.jfpkg
jf-plugin pack --wasm my.wasm --out my.jfpkg     # an already built component
```

A `.jfpkg` is a zip with `manifest.toml`, `plugin.wasm`, the settings schema
the manifest names, an optional `icon.png` (at most 64 KiB) and an optional
`SIGNATURE` (`core/jf-plugin-host/src/package.rs`). `pack` checks the result
with the host's own package reader, so a package it writes installs: unknown
manifest keys, a schema the allowlist cannot use or a component with WASI
imports are errors here, not in the app.

## 6. Sign and check

```sh
jf-plugin keygen ~/keys/jellyfesh-signing.key    # mode 0600; prints the public key and key id only
jf-plugin sign radio.jfpkg --key ~/keys/jellyfesh-signing.key
JF_PLUGIN_SIGNING_KEY=… jf-plugin sign radio.jfpkg   # in CI, from a secret
jf-plugin verify radio.jfpkg --require-signature
jf-plugin inspect radio.jfpkg                    # or --json
```

A signing key is an ed25519 seed written as 64 hex characters (a trailing
newline is fine), in a file only you can read or in the
`JF_PLUGIN_SIGNING_KEY` environment variable. The tool never prints it.
`sign` replaces any earlier signature; `verify` runs the app's install checks
plus the component check and prints who signed; `inspect` shows the
manifest, capabilities, sign-in kinds, allowlist, settings, signature and key
id, entries and the component's imports and exports, even for a broken
package.

The app shows a package as **Official** only when a key in its trust list
signed it: the Jellyfesh release key (key id `0e446b8fb5b07d74`), whose
private half is kept outside the repository. Any other valid signature makes
a community package, and the app remembers its key on first install. The
published test key (`--trust-test-key`) is for tests only.

## 7. Release packages of the bundled plugins

```sh
plugins/release.sh
```

Run from the repository root, it builds the plugins the apps bundle in
release mode and writes exactly

* `dist/plugins/org.jellyfesh.subsonic.jfpkg`
* `dist/plugins/org.jellyfesh.jellyfin.jfpkg`
* `dist/plugins/org.jellyfesh.qqmusic.jfpkg`
* `dist/plugins/org.jellyfesh.spotify.jfpkg`
* `dist/plugins/org.jellyfesh.netease.jfpkg`
* `dist/plugins/community.unofficial.netease.jfpkg` (community, unsigned)
* `dist/plugins/community.unofficial.qqmusic.jfpkg` (community, unsigned)

When `JF_PLUGIN_SIGNING_KEY` is set and not empty it must hold the Jellyfesh
release key (64 hex characters): the official packages are signed with it
and must then verify as Official, or the script fails and removes the
package, so a wrong secret cannot ship packages the apps refuse.
`JF_PLUGIN_ALLOW_UNTRUSTED_KEY=1` accepts another key (a fork, a dry run).
Without the variable the packages are unsigned. The script says which, never
prints the key, and needs only Rust with the `wasm32-unknown-unknown`
target (it builds `jf-plugin` on the way), so it runs on a plain
`ubuntu-24.04` runner. The release workflow (`.github/workflows/release.yml`)
runs it with the secret; the apps bundle the files from `dist/plugins/`, and
the same packages are published as separate downloads.

The two community packages never get a signature: the script packs them
without `JF_PLUGIN_SIGNING_KEY` in their environment and fails if one comes
out signed, or with the tests' `mock_origin` setting. Jellyfesh Web bundles
only the Subsonic and Jellyfin plugins (`platforms/web/scripts/build-plugins.mjs`
lists them): the community plugins' services do not let a web page on
another site call their web interfaces (CORS).

## Command reference

| Command | Does |
| --- | --- |
| `jf-plugin new <name> [--path DIR] [--id ID] [--author NAME] [--sdk DIR]` | a plugin crate from the template |
| `jf-plugin build [--release] [--path DIR] [--out FILE] [--target-dir DIR]` | the crate → a checked component |
| `jf-plugin pack [--release] [--path DIR] [--wasm FILE] [--out FILE] [--key FILE]` | manifest + component + schema + icon → `.jfpkg` |
| `jf-plugin keygen <FILE> [--force]` | a new signing key, mode 0600, never printed |
| `jf-plugin sign <PACKAGE> [--key FILE] [--out FILE]` | signs with the key file or `JF_PLUGIN_SIGNING_KEY` |
| `jf-plugin verify <PACKAGE> [--require-signature] [--require-trusted] [--trust-key HEX] [--trust-test-key]` | the app's install checks; exit status 1 when they fail |
| `jf-plugin inspect <PACKAGE> [--json]` | manifest, capabilities, signature, key id, entries, imports |
