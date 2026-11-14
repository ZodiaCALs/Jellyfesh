# One Jellyfesh in five apps

The macOS, Windows, Linux and Android apps and Jellyfesh Web are one app:
the same places, in the same order, with the same words, features and
steps. Each keeps its platform's own look and mechanics: the macOS Settings
window and menus, the Windows navigation pane, GNOME's preferences and main
menu, Android's bottom tabs, the web's sidebar and phone tabs. Rules and
lists (presets, sorting, what a setting does) come from the core, never from
an app's own copy (ADR-0001).

This page is the reference. When an app differs from it, the app is wrong,
unless a difference is listed under [Allowed differences](#allowed-differences).

## Words

Titles, buttons, menu items, switches, tabs and group headings are in
**Title Case** in every app, Windows included. Descriptions, hints and
messages are sentences. British spelling: Favourites, Colour, Centre,
Levelling, Licences. An ellipsis (…) ends a label that asks for more before
acting.

| Use | Not |
|---|---|
| Settings | Preferences |
| Add Music Folder… (menus, Home, empty pages) · Add Folder… (in Settings › Library) | Add a music folder, + |
| Remove Folder… | Remove |
| Rescan Library | Scan again |
| Play Next · Play Last | Play Later, Add to queue |
| Add to Playlist… · Add to Favourites · Remove from Favourites | |
| Go to Album · Go to Artist | |
| New Playlist… · Import Playlist… · Export Playlist… · Rename… · Delete Playlist… | |
| Up Next · Clear Queue · Nothing Up Next | Clear, The Queue Is Empty |
| About This Song (Now Playing › Info) · Length | About this track, Duration |
| Recently Added · Recently Played · Most Played | Date Added |
| No Results | Nothing found. |
| Add a Source · Installed · Install from File… · Uninstall… | Installed sources, Install from a file… |
| Server Address | Server address |
| Sign In · Sign Out · Sign In with QR Code · Sign In in Browser · Paste Cookies · Enter API Key | Sign in, Sign in with a QR code |
| Official · Community · unofficial | |
| Sources Settings… | Manage sources |
| Allow Community Plugins | |
| System (theme) | Follow System |
| Now Playing Backdrop | Background, Now Playing background |
| Volume Levelling (group) · ReplayGain (its control) | ReplayGain over Volume Levelling |
| Length (crossfade) | Length in seconds, Seconds |
| Reset to Flat | Flat (as a button) |
| Violet (last accent) | Purple |

## Navigation

In this order everywhere: **Home · Albums · Artists · Songs · Playlists ·
Favourites**, then **Search · Sources**, then **Settings**.

| App | How |
|---|---|
| macOS | Sidebar groups Library (the first six), Discover (Search, Sources) and Playlists (the person's playlists, with +). Settings is the Settings window (⌘,). |
| Windows | Navigation pane with the first six, a separator, Search and Sources; Settings at the bottom. |
| Linux | Sidebar with the first six, a separator, Search and Sources. The main menu holds Add Music Folder…, Rescan Library, Settings, Keyboard Shortcuts and About Jellyfesh. |
| Android | Bottom tabs Home, Library, Search, Sources; Library switches between Albums, Artists, Songs, Playlists and Favourites. Settings is the gear on Home, Library and Sources. |
| Web | Sidebar (wide) or bottom tabs (phones): Home, Albums, Artists, Songs, Search, Sources, Settings. Phones put Albums, Artists and Songs under one Library tab, as on Android. |

## Pages

**Home.** The library size ("1,204 songs · 96 albums · 41 artists"), then
Recently Added (albums), Recently Played (songs) and Most Played (songs).
With nothing in the library and no source signed in, a welcome instead:
"Welcome to Jellyfesh", "Add your music folders, or connect a server in
Sources.", buttons **Add Music Folder…** and **Add a Source**.

**Albums.** A grid. Sort menu: Title, Artist, Year, Recently Added, and
**Descending**. Empty: "No Albums Yet", "Add a music folder to see your
albums here.", **Add Music Folder…**.

**Artists.** A list. Empty: "No Artists Yet", same text and button.

**Songs.** **Play** and **Shuffle**, then the list. Sort menu: Title, Artist,
Album, Year, Recently Added, and **Descending**. Empty: "No Songs Yet", same
text and button.

**Album, artist and playlist pages.** Artwork and facts, **Play**,
**Shuffle**, and a More menu (Play Next, Play Last, Add to Playlist…; on a
playlist also Rename… and Delete Playlist…).

**Playlists.** **New Playlist…** asks for a name first. Import Playlist… and
Export Playlist… (M3U8) in every app. Songs reorder by dragging (a handle on
Android). Delete Playlist… asks first. Empty: "No Playlists Yet",
"Make a playlist to keep songs together.", **New Playlist…**.

**Favourites.** Songs marked with the heart. Empty: "No Favourites Yet",
"Songs you mark with the heart appear here."

**Song menu** (right click, long press or More): Play Next, Play Last, Add to
Playlist…, Add to Favourites or Remove from Favourites, Go to Album, Go to
Artist, and on desktops Show in Finder / Show in Explorer / Show in Files.

**Search.** One field ("Songs, albums and artists"). Library results first
(Songs, Albums, Artists), then **From Your Sources**, one group per source.
Nothing found: "No Results".

**Sources.** "Sources bring music from servers and services into
Jellyfesh." Then **Add a Source** (the services from the core's presets,
each "Your Jellyfin server", "Your Navidrome server" or "Airsonic, Gonic and
other Subsonic servers", or "Signed in as …"), then **Your Sources** (each
installed source with its status; choosing one browses it), then
**Sources Settings…**, which opens Settings › Sources.

**Now Playing.** Artwork or the Misty field, the controls (shuffle,
previous, play, next, repeat, volume with mute), and three panes: **Lyrics**,
**Queue** and **Info**. Queue: "Up Next", **Clear Queue** (asks first), and
"Nothing Up Next" when empty. Info: "About This Song" with Title, Artist,
Album, Year, Length, Format, Lyrics and File. Desktops close it with
**Library**; phones with the ⌄ button.

## Settings

Sections in this order: **Appearance · Playback · Library · Sources**, plus
**About** where the platform keeps About in settings (Windows, Android,
Web). Groups and rows in this order:

**Appearance**
1. **Theme**: System, Light, Dark (with pictures). "Light, dark, or the same
   as <system>."
2. **Accent Colour**: Follow the Artwork; the ten swatches; Custom with a hue
   slider.
3. **Transparency**: Solid … Clear; Reduce Motion and Transparency.
4. **Now Playing Backdrop**: Artwork, Misty.
5. **Lyrics**: Lyrics Size (Small, Medium, Large), Lyrics Alignment (Start,
   Centre), Show Translations, Show Romanization. Lyrics settings live only
   here: no separate Lyrics section.
6. **App Icon** (macOS, Windows, Android).
7. **Language** (Web only).

**Playback**
1. **Output**: Output Device (System Default, then the devices).
2. **Crossfade**: Crossfade, Length (1–12 s), Album Continuity.
3. **Volume Levelling**: ReplayGain (Off, Track, Album), Preamp. "Evens out
   loudness using the ReplayGain tags in your files."
4. **Equalizer**: Equalizer (on/off), Preset (Flat, Bass Boost, Vocal,
   Treble from the core's `eq_presets()`; Custom once a band moves), Preamp,
   the ten bands, **Reset to Flat**.
5. **Streaming** (Web only): Streaming Quality.
6. **Startup**: Resume Playback, "Start playing where you left off when
   Jellyfesh opens."

**Library**
1. **Music Folders**: the folders, each with Show in Finder / Explorer /
   Files where the platform has one and **Remove Folder…**, which asks
   "Remove “Name” from the library?" ("Its songs leave the library and its
   playlists. The files stay where they are.", Cancel, Remove Folder). Then
   **Add Folder…**.
2. **Watch Folders**: "Update the library when files change."
3. **Rescan Library**: "Look for new, changed and removed files.", with the
   scan's progress and Last Scan.

**Sources**
1. **Add a Source**: as on the Sources page. Choosing a service asks for the
   Server Address (the core's example as placeholder) and how to sign in.
2. **Installed**, with **Install from File…** in its heading. Each source:
   its name, the Official or Community · unofficial badge, "Signed in as …"
   or "Not signed in"; then Enabled, the sign-in buttons for each method it
   offers (or Sign Out), its own settings, "Version 1.2.0 · Author", and
   **Uninstall…**, which asks "Uninstall Name?" ("Its settings and sign-in
   are removed.", Cancel, Uninstall). With none: "No Sources Installed",
   "Install a source plugin from a .jfpkg file."
3. **Community Plugins**: "Official sources are signed by Jellyfesh.
   Community plugins are made by others and are off unless you allow them."
   and **Allow Community Plugins**, which shows the disclosure first.

**About**: the version (from the build, never typed in), the website, and
the third-party licences.

## Steps that ask first

Remove Folder…, Uninstall…, Delete Playlist… and Clear Queue ask for
confirmation in every app. Turning on a community plugin, or Allow
Community Plugins, shows the disclosure (ADR-0012). New Playlist… asks for
the name before it creates anything.

## Keyboard

The same keys on every desktop app and the web (⌘ instead of Ctrl on
macOS): Space Play or Pause · Ctrl+→ / Ctrl+← Next / Previous Song ·
Shift+→ / Shift+← Skip 5 Seconds · Ctrl+↑ / Ctrl+↓ Volume · Ctrl+F Search ·
Esc Back (and closes Now Playing). Anything more follows the platform's own
conventions and must not reuse these keys.

## Allowed differences

* **Web:** no Playlists, Favourites or history (so Home has no Recently
  Played or Most Played), and no Output Device (a browser decides where
  sound goes). Its Settings also list the keyboard shortcuts. Its library is folders the browser may
  read (re-picked each visit where the browser cannot keep access), with no
  Watch Folders. **Open Files…** plays files for one visit without adding
  them. Streaming Quality and Language are its own settings.
* **Linux:** no App Icon (GNOME takes the icon from the installed files); a
  note says so.
* **Android:** the library is one tab; it asks for access to music and offers
  "Find Music on This Phone" before folders.
* **macOS:** About Jellyfesh is in the app menu. **Linux:** About and Keyboard
  Shortcuts are in the main menu.
* Plugin settings apply as each field is committed on macOS and Linux, and
  with **Save** on Windows, Android and the web.
