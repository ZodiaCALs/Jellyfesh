# ADR-0027: Source songs keep their album, artist and link; search asks ready sources; one Keychain item on macOS

* Status: **Accepted**
* Date: 2026-11-27
* Amends: ADR-0015 §2 (one keystore entry per secret, on macOS), ADR-0018
  (links on rows), ADR-0024 §4 (the sources a search asks)

## Context

After v0.0.8 the owner sent screenshots of the macOS app (2026-11-27):
* a keychain prompt, "Jellyfesh wants to access key "Jellyfesh" in your
  keychain";
* the player bar's lines overlapping its controls;
* Favourites with nothing from the source;
* Home rows needing a play button and a right-click menu ("play next,
  download, view artist");
* search not reaching the source.

**What we found:**
* **The keychain.**
  * The direct download is ad-hoc signed until there is a Developer ID.
    The login keychain ties each item to the exact build that saved it.
    For code without a team ID, macOS ties the item to that build's code
    hash.
  * So after every update macOS asks for the login password once per item
    before the new build may read it.
  * Plugin secrets are one item each (`jellyfesh/<plugin>/<name>`). The
    official NetEase plugin alone keeps three: App ID, private key and
    tokens.
  * The core read the keychain on every request, so answering Allow
    (rather than Always Allow) brought the prompt back on the next request.
  * The data protection keychain needs a provisioning profile, which an
    ad-hoc signed app cannot have.
* **Rows.**
  * `TrackRow` (history, favourites, playlists) has a source song's plugin
    and ID, but not its album, artist or link. The queue's `SourceTrack`
    keeps only the plugin, ID, title, artist name, album name, length,
    artwork and ISRC.
  * So Go to Album, Go to Artist and Open in {Service} appeared for source
    songs only in search and browse.
* **Search.**
  * Every switched-on plugin with `search` was asked, signed in or not.
  * An unset Jellyfin answered "invalid input: Enter your server's
    address…", and the app showed it as is. `plugin_error_text` puts
    "invalid input:" before text that plugins write for people.
* **NetEase search.**
  * The official NetEase plugin had no search: its note said the path was
    not in the official client.
  * That client's README documents `ncm-cli search song --keyword`.
    Independent public clients of the open platform all call
    `GET /openapi/music/basic/search/song/get/v3` with `keyword`, `limit`
    and `offset`, and `search/album/get/v2`, `search/artists/get/v2` and
    `search/playlist/get/v2`.
  * NetEase's documentation cannot be reached from our build environment,
    so these follow those clients and are not tested against the live
    service.
* **NetEase likes.** The plugin answered an empty page when it could not
  find the liked songs' playlist in NetEase's answer. Favourites showed
  nothing and no reason.

## Decision

1. **One Keychain item on macOS.**
   * The core keeps every secret in one item: service "Jellyfesh", account
     `jellyfesh/secrets`, a JSON map.
   * It reads the item once per run, at the keystore check, and keeps it in
     memory. On macOS the check waits up to ten minutes, because it may be
     waiting for the person to answer the prompt.
   * Each write reads the item again and merges into it, so two processes
     don't drop each other's keys.
   * **Older items.** Items saved by v0.0.8 and earlier move into the item
     the first time each is read, and are then deleted. A deleted secret is
     remembered as removed, so a leftover old item never brings it back.
   * **Denied access.** If the person denies access, that run keeps
     sign-ins in memory (the existing session-only state) and never
     overwrites the stored item.
   * **Windows and Linux** keep one entry per secret. Credential Manager
     limits a credential to 2,560 bytes, and neither system asks per app.
   * **The result:** after an update, at most one prompt, which Always Allow
     ends until the next update. A Developer ID signature removes the
     prompts entirely; the release workflow uses one when its secrets are
     set.
2. **Source songs keep their album, artist and link.**
   * `SourceTrack` gains `artist_id` and `album_id`. Both are optional, so
     queues, histories and playlists stored before still read.
   * They are filled whenever a source item is played, queued or added to a
     playlist.
   * `TrackRow` gains `artist_id`, `album_id` and `link`. The core fills
     `link` from the plugin's link templates, as it does for `SourceItem`.
   * The apps' song menus use them wherever a row shows.
3. **Search asks ready sources only.** A source is asked when it is
   switched on, allowed (official, or community when allowed), has
   `search`, and is signed in when it needs a sign-in. This is the rule
   `SourceLibrary` already uses.
4. **Plain error text.** `plugin_error_text` shows a plugin's
   `invalid-input` text without the prefix, as it already does for
   `service`.
5. **NetEase search and likes.**
   * The official NetEase plugin gains `search` (songs, albums, artists,
     playlists) on the paths above.
   * A liked-songs answer it cannot read becomes an error instead of an
     empty list.

## Consequences

* FFI contract change: new `SourceTrack` and `TrackRow` fields in Swift,
  Kotlin and C#, with contract and golden tests.
* macOS: the first update to this version asks once more for each item saved
  before (the move). After that, at most once per update.

## Tests

* **Secrets.**
  * One keychain read serves every secret in a run.
  * Old items move in once and are deleted.
  * A denied read is not retried in that run, and nothing is overwritten.
  * A deleted secret stays deleted when an old item is left behind.
  * Two stores writing different keys keep both.
  * A corrupt item counts as empty.
* **Rows.**
  * A source song played from search keeps its artist, album and link in
    history, favourites and playlists.
  * Rows stored before still read.
* **Search.** A plugin that needs a sign-in and has none is not asked.
* **Error text:** golden strings.
* **NetEase.**
  * Search requests and answers against the mock server.
  * An unreadable liked-songs answer is an error.
