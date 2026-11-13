# ADR-0011: Spotify in Remote mode, Web API only

* Status: **Deferred to the commercial phase** (2026-11-05: developer accounts wait until then; ADR-0016)
* Date: 2026-11-05
* Findings: [`../sources-report.md`](../sources-report.md#spotify)

## Context

The brief limits Spotify to Remote (Connect) and the Web API. What the
platform allows today:

* Web API with Authorization Code + PKCE; loopback redirects must use
  `127.0.0.1` (implicit grant and `localhost` redirects removed in 2025).
  (Secondary sources; developer.spotify.com was unreachable from here.)
* **Every player endpoint works only for Premium users**, and Spotify does
  not guarantee the order commands execute in (official OpenAPI schema).
* Endpoints removed for new apps in Nov 2024 (recommendations,
  audio features, related artists, featured playlists) and Feb 2026
  (multi-get, artist top tracks, new releases, other users' playlists,
  follow endpoints); search returns at most 10 items per page (schema).
* **Development mode is capped at about 5 users and the app owner needs
  Premium; Extended Quota is only for organisations with at least 250 000
  monthly active users** (secondary sources, ⚑ re-verify).
* Developer Terms and Policy (secondary, ⚑ re-verify): controlling a
  background Spotify app counts as "streaming", which makes the app a
  Streaming SDA (no ads, not sold); "do not create any product or service
  which is integrated with streams or content from another service"; do not
  replicate Spotify's core experience; do not synchronise Spotify audio
  with lyrics or other recordings; cache metadata only temporarily; show
  Spotify attribution and link back.

## Decision

1. **Spotify is a WebAssembly plugin in Remote mode**, not native code.
   Capabilities: `auth` (OAuth PKCE in the system browser, loopback
   redirect), `catalog` (Spotify search and the user's library and
   playlists), and `remote` (device list, transfer, play, pause, seek, skip,
   volume, state polling with backoff). No `resolve_stream`.
2. **A `content_policy` block in the manifest** (added to ADR-0004) declares
   isolation, and the host enforces it:

   ```toml
   [content_policy]
   unified_search = false     # results never merge with other sources
   mixed_queue = false        # Spotify items only in the Spotify device's queue
   library_storage = "none"   # nothing persisted in our library database
   metadata_ttl = "24h"       # in-memory/disk cache lifetime
   lyrics_sync = false        # no synced lyrics while this source plays
   attribution = "spotify"    # required logo and link-back in its views
   ```

   Apple Music (ADR-0010) uses the same block for its own rules.
3. **Spotify gets its own view**, branded per Spotify's guidelines, not a
   tab inside our unified Library or Search.
4. **Access model ⚑:** a shared Jellyfesh client ID could serve only about
   five people. The realistic option is "bring your own client ID": the
   person registers a Spotify app and pastes its client ID; the plugin
   explains the steps. Whether that is within the terms is unconfirmed, so
   it ships only after a terms review, and never in store builds until then.
5. Android's App Remote SDK (last release 0.8.0, 2023) is not used; the Web
   API covers remote control on every platform.

## Consequences

* Spotify in Jellyfesh is a remote control and a browser of the person's
  Spotify catalogue, not a source of audio, and it stays visibly separate.
* Remote mode needs Premium; the plugin says so before login.
* If the terms review rejects bring-your-own-client-ID, Spotify drops out
  of v1 entirely.
