# ADR-0022: Discover, details, comments, covers and the lyrics bar

* Status: **Accepted**
* Date: 2026-11-17
* Amends: ADR-0018 (API 1.1), ADR-0019 (API 1.2)

## Context

While v0.0.7 was being built, the owner asked for more (2026-11-17):

* a **Discover page for each source**;
* covers that load after signing in to an account (they did not);
* to **check the singer, the album, comments and detailed information**
  (version, sample rate, sound quality and so on);
* a **word-by-word lyrics bar**;
* the same views and functions in every app, and a working sign-in in
  Jellyfesh Web.

Plugin API 1.2 is still being written, so these join it. Existing 1.0 and 1.1
types cannot change: component types are checked structurally, so a changed
record or enum would stop older plugins from loading.

## Decision

1. **Three more optional exports in API 1.2**, each with its own manifest
   capability, looked up like `maker` and `appearance`:
   * `discover`: shelves for a source's Discover page (`shelf { id, title,
     style: row | grid | chart, items, more }`, at most 12 shelves of 50
     items);
   * `details`: an item's description (album notes, an artist's biography),
     a large image, facts with known keys (version, genre, language,
     released, label, composer, lyricist, arranger, producer, popularity,
     quality, or other) and a comment count;
   * `comments`: pages of comments, with the hot ones on the first page;
     read-only, with the source's attribution.
2. **Discover in the core** (`Command::Discover { source }`,
   `Event::Discover`): the plugin's shelves, or, for plugins without the
   export, shelves made from `recommendations.for-user` ("For You") and the
   top level of `browse`. The library has one too: Recently Added, Most
   Played and Rediscover (albums not played for a long time).
3. **Details** (`Command::Details { item }`, `Event::Details`): for library
   items from the tags and the file; for source items from `details` and the
   stream. Technical information (codec, container, sample rate, bit depth,
   channels, bitrate, size, ReplayGain) and a **quality label the core
   decides**: Hi-Res Lossless (lossless above 16-bit or 48 kHz), Lossless,
   High (lossy at 256 kbit/s or more), Standard. What is playing reports its
   decoded format the same way.
4. **Comments** (`Command::Comments { item, page }`, `Event::Comments`), for
   sources that declare them.
5. **Covers through the core.** The core fetches a source item's picture
   through the plugin host (its allowlist and the picture's headers) into
   the artwork cache, and the apps show the cached file, so every app loads
   covers the same way, including pictures that need headers.
6. **The lyrics bar.** Every app shows the current line word by word when
   the lyrics have word timing: in Now Playing, in a one-line lyrics bar in
   the player bar or mini-player, and in a floating **Desktop Lyrics** window
   where the platform allows one (macOS, Windows, Linux on X11, and Chromium
   through Document Picture-in-Picture). A core timing helper gives the
   current line, the next line and each word's progress, with contract
   vectors, so the five apps agree to the frame. Services' word timing
   arrives as `elrc` or `json-syllables`; plugins convert their own formats
   (NetEase's `yrc`) and test the conversion with golden files. Encrypted
   lyric formats are not decrypted.
7. **Jellyfesh Web and services that refuse browsers.** A browser cannot
   reach services that send no CORS headers, and cannot set `Cookie`,
   `Referer` or `User-Agent` at all. Jellyfesh Web says so before sign-in
   ("{Service} doesn't let browsers connect to it directly …") and can use a
   **relay** the person runs themselves (`tools/jf-relay`: localhost or
   their own server; a token they set; only the hosts the installed plugins'
   allowlists name; nothing logged). No relay is operated by the project.

## Consequences

* Plugins gain work: every official and community plugin implements
  `discover`, and `details` and `comments` where the service offers them.
* Comments are the services' users' words: shown read-only with
  attribution, never stored.

## Tests

* Sanitising of shelves, facts and comments; a test plugin for each export;
  Discover synthesis for 1.0 and 1.1 plugins.
* Quality labels and technical information from generated WAV, FLAC and MP3
  fixtures; details from tags.
* Lyrics bar contract vectors; golden tests for `yrc` conversion.
* `jf-relay`: token, origin and host checks, forbidden headers forwarded,
  nothing else reachable.
