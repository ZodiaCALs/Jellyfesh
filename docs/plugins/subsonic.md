# Subsonic and OpenSubsonic servers

The Subsonic plugin plays your music from your own server: Navidrome, Gonic,
Airsonic, LMS, Ampache and the other servers that speak the Subsonic API.
It comes with every Jellyfesh app and updates with it; the same plugin is
also published as a separate download (`org.jellyfesh.subsonic`, signed with
the Jellyfesh release key). What it stores and where: `plugins/subsonic/README.md`.

## Which servers

* Any server with the **Subsonic API 1.13.0 or newer** (the version that
  introduced token sign-in) and JSON answers.
* On **OpenSubsonic** servers the plugin also uses, when the server offers
  them: **API keys** (`apiKeyAuthentication`), **form POST** (requests in the
  body, so your sign-in stays out of web-server logs) and **structured
  lyrics** (`songLyrics`, synced lyrics by song).
* Navidrome offers form POST and structured lyrics since 0.51; its API-key
  support was merged after 0.64.2 and was not in a release when this plugin
  was written.

The plugin was tested against a mock server written from the OpenSubsonic
documentation, not yet against a live server.

## Before you start

You need the address you open your server at, for example
`http://192.168.1.20:4533` (Navidrome's usual port) or
`https://music.example.com`. If the server runs under a path behind a
reverse proxy, include it (`https://example.com/navidrome`), but not a page
of its web app. A server on your home network works: Jellyfesh lets the
plugin reach that one address.

If your server offers API keys, create one for Jellyfesh in your server's
settings (see its documentation for where); you can revoke it there at any
time.

## Set it up

1. In Jellyfesh's Sources settings, open **Subsonic** and enter the server
   address.
2. Sign in:
   * **API key**: paste the key. If your server offers API keys, this is the
     only way offered, as the OpenSubsonic API recommends.
   * **User name and password** (servers without API keys): Jellyfesh makes
     one token from your password on your device, checks it with the server
     and keeps only the token. It works like a password for this server, so
     it stays in your system's keystore.

If the address uses plain `http` and is not on your local network, the form
says so: your sign-in would travel unencrypted, so use `https` if the
server offers it.

## Settings

| Setting | Values | What it does |
| --- | --- | --- |
| Server address | a URL | where your server is |
| Streaming quality | `app` (default), `lossless`, `high`, `normal`, `low` | `app` follows the quality you chose in Jellyfesh for each network; the others always use that quality on this server |
| Format when the server converts | `mp3` (default), `opus`, `aac` | what the server converts to when it has to |

| Quality | Limit | You get |
| --- | --- | --- |
| lossless | none | the original file, if Jellyfesh plays its format; otherwise a 320 kbit/s conversion |
| high | 320 kbit/s | the original file if Jellyfesh plays its format and it is within the limit; otherwise a conversion at the limit |
| normal | 192 kbit/s | the same |
| low | 96 kbit/s | the same |

Jellyfesh plays FLAC, MP3, AAC and ALAC (M4A, MP4), Ogg Vorbis and Opus, WAV,
AIFF, CAF and Matroska audio. Anything else, such as WMA, is converted.

## What you can do

* **Search** artists, albums and tracks.
* **Browse** artists → albums → tracks; album lists (recently added, recently
  played, most played, random, A to Z, by artist); playlists; starred songs,
  albums and artists.
* **Lyrics** come from the server: synced lyrics need a server with
  structured lyrics; otherwise plain lyrics are looked up by artist and
  title. Lyrics for tracks from other sources are found by title, artist and
  length.
* **Similar songs** need a server that finds them (often through an external
  service it is configured with); otherwise the list is empty.

## Signing out

Signing out deletes the stored API key or token on this device. A Subsonic
sign-in is not a session on the server; to make an API key stop working
everywhere, revoke it in your server's settings.

## When something goes wrong

| Message | What to do |
| --- | --- |
| Cannot reach the server at … | Check the address and port, that the server runs, and that this device can reach it (same network, VPN). |
| No Subsonic server answers at this address (HTTP 404) | Use the address you open the server at, without a page path. |
| The address answered HTTP 401 (or 403) before reaching a Subsonic server | A proxy in front of the server asks for its own sign-in. Let `/rest/` through to the server. |
| Wrong user name or password. | Check them on the server's web page. |
| … supports API keys … sign in with "API key" | Create an API key in the server's settings and use it. |
| This server does not accept token sign-in | Common for LDAP accounts. Create an API key on the server. Jellyfesh never keeps your password, so without token sign-in or API keys it cannot sign in. |
| The server speaks Subsonic API 1.12.0; Jellyfesh needs 1.13.0 or newer | Update the server. |
| Your password is part of your user name … | Change the password on the server, or use an API key. |
| Your account on this server is not allowed to stream music | Ask the server's administrator for the streaming permission. |
| Videos can't be played in Jellyfesh | Videos in a music library are listed but not played. |
| Sign in again. | The server no longer accepts the stored sign-in (a revoked key, a changed password). Sign in again. |

## Privacy

The plugin talks only to your server. Subsonic has no other way to sign in a
stream or cover request, so those URLs carry your API key or token in their
query; the player keeps them in memory, and they are not written to disk.
Use `https` for a server you reach over the internet.
