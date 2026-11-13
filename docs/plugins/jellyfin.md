# Jellyfin servers

The Jellyfin plugin plays your music from your own Jellyfin server. It
comes with every Jellyfesh app and updates with it; the same plugin is also
published as a separate download (`org.jellyfesh.jellyfin`, signed with the
Jellyfesh release key). What it stores and where: `plugins/jellyfin/README.md`.

## Which servers

* **Jellyfin 10.9 or newer**: 10.9, 10.10, 10.11 and the 12.x releases.
  Version 10.9 brought what the plugin relies on (lyrics, the library views
  and the user taken from the sign-in token). Older servers get a message
  asking for an update.
* Emby is a different server and is not supported.

The plugin was written from Jellyfin's own sources (version 12.4) and tested
against a mock server built from them, not yet against a live server.

## Before you start

You need the address you open Jellyfin at, for example
`http://192.168.1.20:8096` or `https://jellyfin.example.com`, including the
base path if your server has one (`https://example.com/jellyfin`). Pasting
the address of a page of the web app (`…/web/#/home`) works too. A server on
your home network works: Jellyfesh lets the plugin reach that one address.

For Quick Connect, it must be turned on in the server's dashboard (General,
Quick Connect).

## Set it up

1. In Jellyfesh's Sources settings, open **Jellyfin** and enter the server
   address.
2. Sign in:
   * **Quick Connect** (no password typed in Jellyfesh): Jellyfesh shows a
     six-digit code. In Jellyfin (the web page or an app where you are signed
     in), open your profile, choose **Quick Connect**, enter the code and
     confirm. Jellyfesh notices within two seconds. The code works for ten
     minutes.
   * **User name and password**: sent once to your server, which answers with
     an access token; only the token is kept. Leave the password empty if
     your account has none.

Jellyfin then lists Jellyfesh among its devices (with the device name from
the settings) until you sign out.

## Settings

| Setting | Values | What it does |
| --- | --- | --- |
| Server address | a URL | where your server is |
| Streaming quality | `app` (default), `lossless`, `high`, `normal`, `low` | `app` follows the quality you chose in Jellyfesh for each network; the others always use that quality on this server |
| Format when the server converts | `aac` (default), `mp3`, `opus` | what Jellyfin converts to when it has to |
| Device name | text, `Jellyfesh` by default | how this device appears in Jellyfin's devices and sessions |

| Quality | Limit | You get |
| --- | --- | --- |
| lossless | none | the original file, if Jellyfesh plays its format; otherwise a 320 kbit/s conversion |
| high | 320 kbit/s | the original file if Jellyfesh plays its format and it is within the limit; otherwise a conversion at the limit |
| normal | 192 kbit/s | the same |
| low | 96 kbit/s | the same |

Jellyfesh plays FLAC, MP3, AAC and ALAC (M4A, MP4), Ogg Vorbis and Opus, WAV,
AIFF, CAF and Matroska audio. Anything else, such as WMA, is converted by
Jellyfin. The original file arrives with seeking support; conversions are
sent as they are made. Limits your administrator sets for remote
connections still apply.

## What you can do

* **Search** tracks, albums, artists and playlists.
* **Browse** your music libraries: artists → albums → tracks, all albums, and
  recently added albums. With one music library its sections are at the
  top; with several, each library is a folder. Also your audio playlists and
  your favourite songs, albums and artists.
* **Lyrics** from Jellyfin (embedded lyrics or lyric files next to the music):
  synced lyrics as timed lines, others as plain text. Lyrics for tracks from
  other sources are found by title, artist and length.
* **Instant mix**: suggestions based on a track, album, artist or playlist.

## Signing out

Signing out tells Jellyfin the session ended, so the token stops working on
the server too, and deletes it on this device. The device id stays, so
signing in again shows the same device in Jellyfin.

## When something goes wrong

| Message | What to do |
| --- | --- |
| Cannot reach the server at … | Check the address and port, that Jellyfin runs, and that this device can reach it (same network, VPN). |
| No Jellyfin server answers at this address (HTTP 404) | Use the address you open Jellyfin at, with its base path if it has one. |
| The server at this address is Emby Server, not Jellyfin | The plugin works with Jellyfin only. |
| This server runs Jellyfin 10.8 …; Jellyfesh needs Jellyfin 10.9 or newer | Update the server. |
| Quick Connect is turned off on this server | Ask an administrator to turn it on (dashboard, General), or sign in with your password. |
| The Quick Connect code expired | Start again for a new code; codes last ten minutes. |
| Wrong user name or password. | Check them on Jellyfin's web page. |
| The server refused this sign-in … | The account may be disabled, not allowed on new devices, or at its limit of sessions; an administrator can change that in the user's settings. |
| Your account on this server is not allowed to play media | An administrator can allow media playback for your user. |
| The file of this track is missing on the server | Jellyfin lists the track but has no file for it. |
| Sign in again. | The token stopped working (signed out from Jellyfin's dashboard, for example). Sign in again. |

## Privacy

The plugin talks only to your server. The access token travels in the
`Authorization` header of each request, never in a URL; cover art needs no
sign-in on Jellyfin, so cover URLs carry nothing secret. Use `https` for a
server you reach over the internet.
