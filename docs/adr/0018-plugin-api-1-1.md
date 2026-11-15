# ADR-0018: Plugin API 1.1: links and remote sessions

* Status: **Accepted**
* Date: 2026-11-15
* Amends: ADR-0003 (runtime), ADR-0004 (packaging), ADR-0010 (Apple Music), ADR-0011 (Spotify)

## Context

For v0.0.6 the owner asked for four official integrations that API 1.0 cannot
express:

* **QQ Music's official API** (`qqmusic-skills`) returns metadata and links
  that open a song in QQ Music, never audio. Its items are unplayable in
  Jellyfesh but should open in the service.
* **Spotify** allows only remote control of the person's own Spotify devices
  (ADR-0011): no stream, and its items never enter Jellyfesh's queue.
* **Apple Music** plays only through MusicKit on macOS (ADR-0010), a player
  that lives in the macOS app, not in a plugin.
* **NetEase Cloud Music's open platform** streams, which API 1.0 covers.

ADR-0010 and ADR-0011 were deferred until developer accounts exist; the owner
now wants them built and released as a public beta, each switched on only
where its account, key or terms allow.

Version 1.0 plugins must keep working unchanged.

## Decision

1. **WIT package `jellyfesh:plugin@1.1.0`.** Every 1.0 interface and type
   stays exactly as it is. wasmtime matches interface names within a major
   version in both directions (a host's `@1.1.0` lookup finds a plugin's
   `@1.0.0` export and the reverse), so 1.0 plugins load in a 1.1 host, and
   the manifest check (same major, minor not newer) keeps 1.1 plugins out of
   1.0 hosts with a clear error.
2. **A new `remote` interface**, exported by a new world `remote-source`
   (`source` plus `remote`): devices, playback state, play (tracks or a
   container from an index, on a device), resume, pause, next, previous,
   seek, volume, shuffle, repeat and transfer. The SDK always builds
   `remote-source` and answers `unsupported` for what a plugin does not
   implement, as it does for the other capabilities. The meta `capability`
   enum cannot grow without breaking 1.0 plugins, so `remote` is a manifest
   capability only; the host looks up the `remote` export only for plugins
   whose manifest declares it, with `playback_mode = "remote"`.
3. **Links are manifest data, not calls.** A 1.1 manifest may carry
   `[links]` with an `https` URL template per item kind (`track`, `album`,
   `artist`, `playlist`) holding `{id}`. The host checks each template (https,
   a host in the plugin's allowlist, `{id}` once), fills in the item's id
   percent-encoded, and the core adds `link` to the source items and Now
   Playing. The apps offer **Open in {Service}** for items with a link, and
   use it as the main action for items that cannot play here. No plugin call
   per item.
4. **Remote sessions live in the core.** Playing an item of a remote source
   (from its own views; with `mixed_queue = false` it never joins the local
   queue) pauses local playback and starts a remote session: the core calls
   `remote.play`, polls `remote.state` (every second while playing, every five
   while paused, backing off after errors, stopping when nothing plays),
   routes the transport commands to the session, and publishes it as a
   `Remote` slice (state, device, devices). Starting local playback ends the
   session (it stays a remote control; Spotify keeps playing until paused).
   Content policy still applies: no synced lyrics while a source with
   `lyrics_sync = false` plays.
5. **App-provided remote backends.** The same session accepts a backend
   that an app implements through the FFI (a callback interface with the
   shape of `remote`), registered under a reserved source id. The macOS app
   uses it for Apple Music with `ApplicationMusicPlayer` (macOS 14+). Without
   a MusicKit-enabled App ID and signing, MusicKit refuses authorisation, so
   such builds hide Apple Music rather than show something that cannot work.
6. **Access models**, each stated in the plugin's own sign-in screen:
   QQ Music's personal key per person; Spotify's bring-your-own client ID per
   person (Spotify caps a shared development app at a handful of users; a
   terms review is still owed, so it is marked beta); NetEase's open-platform
   credentials as the research shows they must be handled; Apple Music
   through the person's own subscription.

## Consequences

* Plugins built with SDK 1.1 export `remote` (unsupported by default) and
  need a 1.1 host; 1.0 packages keep loading everywhere.
* The core gains a second kind of playback (remote sessions) beside the
  engine, and every app gains a remote Now Playing state, a device picker and
  Open in {Service}. Jellyfesh Web shows links but has no remote sessions in
  this version (OAuth sign-in needs a loopback redirect a page cannot catch).
* Services whose terms forbid mixing (Spotify, Apple Music) stay in their own
  views, as ADR-0010 and ADR-0011 require.
