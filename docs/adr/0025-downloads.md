# ADR-0025: Downloads from your own servers

* Status: **Accepted**
* Date: 2026-11-22
* Amends: ADR-0024 (API 1.3), [`../ui.md`](../ui.md) (Downloads, Settings ›
  Downloads)

## Context

For v0.0.8 the owner asked (2026-11-22) to "add customized download
function". We told the owner how we read it:
* songs, albums and playlists can be downloaded from the person's own
  servers (Jellyfin, Navidrome/Subsonic) when the server allows it;
* downloads are saved as audio files in a folder they choose, with options;
* they show in the Library and play offline;
* Spotify and the official QQ Music and NetEase plugins offer none, because
  those services don't allow it;
* the unofficial community plugins are left out unless the owner decides
  otherwise.

The non-goals stand: no DRM circumvention and no paywall bypass.

**What the research found (2026-11-22):**
* **Jellyfin:** `/Items/{id}/Download` serves the original file, with
  ranges.
  * It needs the account's `EnableContentDownloading` policy, and
    `fields=CanDownload` gives a per-item answer.
  * `/Audio/{id}/stream?static=true` skips that policy, so it must never be
    used to download.
* **Subsonic:** `download` serves the original file. Navidrome answers error
  50 when downloads are off, and needs `format=raw` to skip transcoding.
* **Spotify:** the Developer Terms forbid permanent copies.
* **QQ Music:** its licence bans unauthorised third-party software.
* **NetEase:** its terms could not be read.
* **lofty 0.25.5** writes tags, embedded covers and unsynced lyrics for MP3,
  FLAC, Opus and M4A. Until 0.26 it is safest through the format-specific
  tag types.

## Decision

1. **Which sources.** Only plugins with the new `download` capability (API
   1.3) can download, and the core enforces it. In v0.0.8 these are Jellyfin
   and Subsonic/Navidrome.
   * Their `download` export returns where to fetch the file: URL, headers,
     container, size, and whether it is the original.
   * It refuses when the server doesn't allow downloading: Jellyfin's
     `CanDownload` or policy, Subsonic's download role or error 50. The
     apps say so: "Your Jellyfin server doesn't allow downloads for this
     account."
   * Never Spotify, QQ Music or NetEase, official or community. Never a
     stream URL that skips a server's download policy.
2. **The options**, in Settings › Downloads (every app but the web):
   * **Folder.**
     * The default is a Jellyfesh folder in the person's Music folder:
       `~/Music/Jellyfesh` on macOS, `Music\Jellyfesh` on Windows,
       `$XDG_MUSIC_DIR/Jellyfesh` on Linux (the Flatpak through its music
       folder permission).
     * On Android, `Music/Jellyfesh` in shared storage through MediaStore,
       with no all-files permission.
     * A folder chosen on macOS is kept with a security-scoped bookmark.
   * **Quality:**
     * **Original** (the default) is the file exactly as the server keeps
       it.
     * **High** (320 kbit/s), **Normal** (192) or **Low** (128) is the
       server's conversion, to Opus where it can, otherwise AAC or MP3.
   * **File Names.**
     * A template from {artist}, {album_artist}, {album}, {title},
       {track}, {disc}, {year} and {source}, with a live preview. The
       default is `{album_artist}/{album}/{track} {title}`.
     * Names are made safe on every platform:
       * `<>:"/\|?*` and control characters are replaced;
       * trailing dots and spaces are trimmed;
       * reserved Windows names are avoided;
       * each part is at most 255 bytes.
     * A clash adds " (2)".
   * **Tags.**
     * Original files stay byte for byte unless **Add Missing Cover and
       Lyrics** is on.
     * Converted files always get title, artist, album, album artist,
       track, disc and year tags, and the cover.
     * With **Lyrics** on, the lyrics go in the tag; synced lyrics are
       written as LRC text, which many players read. **Save .lrc Files**
       also writes them beside the song.
   * **Wi-Fi Only** (Android, on by default): downloads wait while the
     connection is metered.
   * **Downloads at Once:** 1 to 4, 2 by default.
3. **The queue, in the core:**
   * downloads resume with HTTP ranges where the server allows;
   * network errors are retried with backoff;
   * downloads pause while offline, can be cancelled, and survive a
     restart;
   * a `Downloads` slice holds each item's progress and error.
4. **The library link and offline playback.**
   * The download folder is a library root. Each finished file joins the
     library linked to its source item, which then plays the file when it
     exists (offline too) and shows a Downloaded mark.
   * The Downloads page lists downloads in progress, then the finished ones
     by album. It has **Remove Download…** (deletes the file; asks first)
     and the space used.
   * Settings › Downloads has **Remove All Downloads…** (asks first).
5. **Jellyfesh Web:** Download on a song saves the original file through the
   browser, named by the template, when the source allows downloads. There
   is no queue, offline library or Downloads page (an allowed difference).
6. **Privacy:** the files stay on the device; nothing is uploaded.

## Consequences

* The Jellyfin and Subsonic plugins gain the export. The core gains a
  download queue and `lofty`, writing through the format-specific tags until
  0.26.
* Two more steps ask first: Remove Download… and Remove All Downloads….

## Tests

* **Templates and safe names:** a golden table covering every platform's
  rules.
* **The queue:** a local HTTP test server with ranges, failures and resume;
  cancel; pause while offline; persistence.
* **Capability:** the host refuses `download` from a plugin without it;
  refusals become the apps' messages.
* **Tags:** MP3, FLAC, Opus and M4A fixtures get the tags, cover and lyrics.
  Originals stay untouched by default.
* **Library link:** a source item with a download plays the file.
