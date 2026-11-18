# A relay for Jellyfesh Web

Jellyfesh Web runs in a browser, and a browser lets a page talk to another
site only on that site's terms: the site must answer with CORS headers that
allow the page, and the page may not set some headers at all (`Cookie`,
`Referer`, `User-Agent`, `Origin`). Your own Subsonic, Navidrome or Jellyfin
server can allow Jellyfesh Web ([web.md](web.md), "Connecting to your
server"). Some services cannot be asked to: the web players of QQ Music and
NetEase Cloud Music answer without CORS headers, and the unofficial plugins
for them must send the services' own `Referer`, a browser's `User-Agent` and
the account's session in `Cookie`.

For those, Jellyfesh Web says so before you sign in:

> QQ Music doesn't let browsers connect to it directly. Use the Jellyfesh app
> on your computer or phone, or set up a relay in Settings › Sources › Relay.

A **relay** is a small program you run yourself, on your computer or on a
server of yours: `jf-relay` (in `tools/jf-relay`). Jellyfesh Web hands it each
request of such a source, wrapped; the relay makes the request with all its
headers and hands the answer back, with CORS headers for Jellyfesh Web. **The
Jellyfesh project runs no relay**, and Jellyfesh Web never uses one you did
not set up.

Which sources go through the relay: the services the core knows refuse
browsers (`jf_web::relay::REFUSING`: the unofficial QQ Music and NetEase
Cloud Music plugins, by their id or the hosts in their allowlist), and any
other source whose service Jellyfesh Web finds answering without letting the
page read it (it checks once a request failed, and remembers). Everything
else, your own servers included, is reached directly as before.

## What the relay can see

Everything the service sees, because it makes the requests: the plugin's
requests and the service's answers (lists, lyrics, covers, streams), and your
sign-in to the service (its cookies and tokens travel through it). Run it
where only you control it. It keeps nothing: no cache, no cookies (Jellyfesh
Web keeps the plugin's cookies, encrypted, with its other secrets), and its
log has only the start-up line, plus with `--verbose` each request's method,
host and status, never a path, query, header, body or the token.

What protects the relay from being used by others:

* it listens on `127.0.0.1` (this computer only) unless told otherwise;
* every request needs the **token** you set, compared in constant time;
* every request needs an **`Origin`** you allowed with `--origin` (the address
  you open Jellyfesh Web at), so other sites you visit cannot use it from your
  browser; answers to other origins carry no CORS headers;
* it forwards only to the **hosts the bundled plugins' allowlists name**
  (official and community: `jf-relay hosts` lists them, read from the
  manifests when it is built) and the ones you add with `--allow`; fixed hosts
  only over https and only at public addresses, checked after the name is
  resolved, as the apps' plugin host does;
* it never follows a redirect: Jellyfesh Web checks every hop against the
  plugin's own allowlist before asking the relay for it;
* it refuses the headers the plugin host sets itself (`Host`,
  `Content-Length`, `Transfer-Encoding`, …), request bodies over 16 MiB, and
  anything that is not plain HTTP/1.1.

## Running it on your computer

You need Rust (the repository's toolchain). From a checkout:

```sh
cargo build --release -p jf-relay           # target/release/jf-relay
target/release/jf-relay token               # prints a new random token
export JF_RELAY_TOKEN=<that token>
target/release/jf-relay --origin https://you.github.io
```

`--origin` is the address you open Jellyfesh Web at, as the browser shows it
up to the host and port (`https://you.github.io`, or `http://localhost:4173`
for a copy you serve yourself); give it once per address. The relay prints
where it listens (`http://127.0.0.1:8790` by default; `--listen` changes it).

Then in Jellyfesh Web: **Settings › Sources › Relay**, Relay Address
`http://127.0.0.1:8790`, Relay Token the same token, **Save**. It says
"Connected to the relay." when the relay answers; otherwise why not (a wrong
token, a missing `--origin`, nothing listening). Sign in to the source again.

Notes for browsers:

* An https page may reach `http://127.0.0.1` and `http://localhost`: browsers
  treat them as secure. Other plain-http addresses are mixed content, which
  the page refuses before saving.
* Chrome and Edge may ask whether the site may reach apps and services on this
  device or your local network (their local network access permission). Allow
  it for Jellyfesh Web's address, or requests to the relay fail.
* A browser that does not let an https page reach `http://127.0.0.1` needs the
  relay on https: put a reverse proxy with a certificate in front of it, as on
  a server (below).

## Running it on your own server

Keep the relay on `127.0.0.1` and put your web server in front of it with
https. With Caddy:

```caddyfile
relay.example.com {
    reverse_proxy 127.0.0.1:8790
}
```

With nginx:

```nginx
server {
    listen 443 ssl;
    server_name relay.example.com;
    # ssl_certificate … (Let's Encrypt or your own)
    location / {
        proxy_pass http://127.0.0.1:8790;
        proxy_buffering off;          # streams pass through as they come
        proxy_read_timeout 120s;      # WeChat's sign-in waits up to 30 s per poll
        client_max_body_size 16m;
    }
}
```

Start the relay as a service with the token in its environment (or
`--token-file`, a file only that user can read), for example with systemd:

```ini
[Service]
Environment=JF_RELAY_TOKEN=<token>
ExecStart=/usr/local/bin/jf-relay --origin https://you.github.io
DynamicUser=yes
Restart=on-failure
```

and enter `https://relay.example.com` as the Relay Address. A relay mounted
under a path (`https://example.com/relay/`) works if the proxy removes the path
before passing requests on (`location /relay/ { proxy_pass
http://127.0.0.1:8790/; }`); enter `https://example.com/relay` then.

Anyone with the token and your Jellyfesh Web origin could send requests
through the relay to the hosts it allows; keep the token secret, and change it
(restart with a new one, enter it in Settings) if it leaks.

## Options

| Option | |
| --- | --- |
| `--listen ADDRESS:PORT` | Where to listen; `127.0.0.1:8790` by default. Listening beyond this computer prints a reminder to put https in front. |
| `--origin ORIGIN` | An address Jellyfesh Web is opened at; required, may be repeated. |
| `--allow ENTRY` | Also forward to a host written as in a manifest (`music.example.com`, `*.example.com`, `music.example.com:8443`; https only, public addresses only), or to your own server's origin (`http://192.168.1.20:4533`, which may be on your network). May be repeated. |
| `--token-file FILE` | Read the token from a file instead of `JF_RELAY_TOKEN`. At least 16 characters, no spaces. |
| `--verbose` | Log each request's method, host and status. |
| `jf-relay token` | Print a new random token. |
| `jf-relay hosts [--allow …]` | List the hosts and origins the relay forwards to. |

## The protocol

For developers (`tools/jf-relay/src/relay.rs`, and on the page
`platforms/web/src/lib/plugins/relay.ts`):

* `POST /v1/forward` with `Authorization: Bearer <token>`, `Jf-Relay-Url` (the
  absolute URL), `Jf-Relay-Method` (`GET`, `HEAD`, `POST`, `PUT`, `PATCH`,
  `DELETE` or `OPTIONS`) and `Jf-Relay-Headers` (the request's headers as a
  JSON array of `[name, value]` pairs, non-ASCII escaped); the body is the
  request's body. `Range` and `If-Range` pass through when the wrapped headers
  have none, for media elements.
* The answer is `200` with `Jf-Relay-Status` (the service's status, redirects
  included), `Jf-Relay-Headers` (the service's headers as JSON, `Set-Cookie`
  and `Location` included) and the service's body, streamed.
* A refusal is a 4xx or 5xx with `Jf-Relay-Error`: `origin`, `token`, `host`,
  `address`, `request`, `dns`, `upstream`, `timeout` or `busy`, and a JSON body
  `{"error", "message"}`.
* `GET /v1/ping` answers `{"relay": "jf-relay", "version", "protocol": 1}` once
  the origin and token pass; Settings › Sources › Relay uses it.
* `OPTIONS` answers CORS preflights for allowed origins (and Chromium's
  private network preflights).

Tests: `cargo test -p jf-relay` (the allowlist from the bundled manifests,
address checks, HTTP parsing, and the relay end to end on loopback: origin,
token and host checks, the headers a page may not set forwarded, answers
wrapped, nothing secret logged), and Jellyfesh Web's
`tests/e2e/relay.spec.ts` (the unofficial QQ Music plugin's QR sign-in through
the relay against a mock of WeChat and QQ Music that sends no CORS headers).
