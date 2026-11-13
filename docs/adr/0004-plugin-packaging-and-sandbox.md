# ADR-0004: Plugin package, WIT world, permissions and signing

* Status: Proposed
* Date: 2026-11-05
* Related: ADR-0003 (runtime), ADR-0012 (community plugins), ADR-0015 (secrets)

## Context

Every online service is a plugin. Plugins come from three trust levels
(first-party official, third-party official, community-unofficial), run
in-process, and handle account credentials. The brief fixes the manifest
fields, capabilities, playback modes and host imports; this record turns
them into a package format and a sandbox contract.

## Decision

### Package

`<id>-<version>.jfpkg`, a zip with:

| File | Required | Notes |
| --- | --- | --- |
| `manifest.toml` | yes | schema below; unknown keys are an error |
| `plugin.wasm` | yes | a component targeting `jellyfesh:plugin/source@1.x` |
| `icon.svg` or `icon.png` | no | ≤ 64 KB; SVG is sanitised (no scripts, no external refs) |
| `SIGNATURE` | no | ed25519 over the SHA-256 of the other files, with the signer's key id |

```toml
id = "org.jellyfesh.subsonic"      # reverse-DNS, unique
name = "Subsonic"
version = "1.0.0"                  # semver of the plugin
api_version = "1.0"                # WIT package major.minor it targets
author = "Jellyfesh"
official = true                    # true only for first-party, signed with our key
capabilities = ["auth", "search", "browse", "resolve_stream", "lyrics", "artwork"]
playback_mode = "stream"           # stream | vendor | remote
auth = ["password-exchange", "api-key"]   # qr | oauth | cookie-import | password-exchange | api-key

[network]
allow = ["${settings.server_url}"] # user-entered origin; or fixed hosts like "api.example.com"
max_rps = 10

[settings]                         # JSON Schema subset (draft 2020-12), rendered natively
schema = "settings.schema.json"
```

Services whose terms forbid mixing their content with others (Spotify,
Apple Music) declare a `[content_policy]` block that the host enforces:
`unified_search`, `mixed_queue`, `library_storage`, `metadata_ttl`,
`lyrics_sync` and `attribution` (ADR-0011 has the full example). The
default is permissive, which suits self-hosted servers.

`official` is display metadata, not trust: the host shows the **Official**
badge only when the package is signed by a key in the app's built-in trust
list. Community packages show **Community · unofficial**; vendor-mode
packages also show **Vendor playback**.

### WIT world (shape, v1)

```wit
package jellyfesh:plugin@1.0.0;

world source {
  import http;    // request(req) -> result<response, http-error>; allowlisted
  import kv;      // get/set/delete; set-secret/get-secret go to the OS keystore
  import crypto;  // digest, hmac, aes-cbc/gcm/ecb, rsa-pkcs1/oaep, random-bytes
  import clock;   // now-utc-ms, monotonic-ms
  import log;     // log(level, message), rate-limited

  export meta;            // capabilities(), settings-changed(json)
  export auth;            // begin(kind) -> flow-step; poll(flow) -> flow-step; logout()
  export catalog;         // search(query, filters, page); browse(node, page)
  export playback;        // resolve-stream(item, quality) -> stream-info
  export lyrics;          // lyrics(item) -> option<lyric-payload>
  export artwork;         // artwork(item, size) -> option<image-ref>
  export recommendations; // for-item(item, page); for-user(page)
}
```

* `stream-info` = URL, headers, `expires-at`, codec hint, bitrate, and
  whether the host may cache it. `lyric-payload` is raw text plus a format
  tag (lrc, elrc, ttml, vtt, json-syllables) that `jf-lyrics` converts into
  `LyricDoc`, so plugin lyrics become the same model as local ones.
* Unimplemented capabilities are stubs that return `unsupported`; the host
  never calls a capability the manifest does not declare.
* No WASI imports at all: no filesystem, sockets, environment or arguments.
  The SDK provides `getrandom` and clock shims over our imports, so plugins
  built from `wasm32-unknown-unknown` or `wasm32-wasip2` link cleanly
  (ADR-0003 confirms the target).
* `api_version` major must match; a newer minor runs if the host's world is
  a superset.

### Sandbox contract (enforced by the host, tested in M3)

| Limit | Default | Enforcement |
| --- | --- | --- |
| CPU per call | epoch budget ≈ 200 ms of CPU; 2 s for `auth.begin` | ADR-0003 (epoch interruption) |
| Wall clock per call | 10 s, 30 s for login steps | host timer, then instance dropped |
| Memory | 64 MB linear memory per instance; 1 table, ≤ 10 000 elements | runtime resource limiter |
| Response size | 16 MB per HTTP response, 64 MB per call | streaming check in `http` |
| Requests | `max_rps` from the manifest, capped at 20; 6 concurrent | token bucket per plugin |
| Network | Allowlist after redirects; scheme https unless the user entered http for a LAN server; private, loopback and link-local ranges blocked unless they came from a user-entered URL; DNS answers re-checked (anti-rebinding) | `http` import |
| Cookies | One jar per plugin, persisted encrypted, never shared | `http` import |
| Storage | kv quota 1 MB plain values per plugin; secrets only via keystore keys namespaced by plugin id | `kv` import |
| Strikes | 3 limit violations in a row disable the plugin with a visible reason | host |

### Signing and trust

* First-party plugins are signed with the Jellyfesh release key (ed25519,
  offline). The public key is compiled into the app.
* Community authors may sign with their own key. The host shows the key
  fingerprint and remembers it (trust on first use); a later package with a
  different key needs explicit confirmation.
* Store builds load only packages signed with the release key
  (ADR-0012).

### Auth flows

The host owns the state machines; the UI renders steps natively:

* `qr`: plugin returns a payload string; host renders the QR natively and
  calls `auth.poll` on the plugin's requested interval.
* `oauth`: authorisation code with PKCE in the **system browser**, redirect to
  a loopback port (desktop) or an app link/custom scheme (Android). No
  embedded webviews.
* `cookie-import`: the person pastes a cookie header; the host validates it
  against the plugin's declared cookie names and allowlist before storing it
  in the plugin's jar.
* `password-exchange`: the password goes to the plugin once and must come
  back as a token in the same call; the host wipes the buffer and never
  persists it. A plugin that asks to store a password is rejected at review
  for first-party and flagged in the UI for community packages.
* `api-key`: stored as a secret.

## Consequences

* A plugin cannot read files, open sockets, or reach hosts outside its
  allowlist; the worst a malicious plugin can do is misuse its own service
  account and burn its own CPU and memory budget.
* Settings-dependent allowlists make self-hosted servers work without
  wildcards.
* Supporting JavaScript or Python plugins later would need a WASI shim
  (clocks, random, stdio) without filesystem; out of scope for v1.
