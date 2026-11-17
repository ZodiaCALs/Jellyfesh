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
| Open in {Service} (the plugin's name: Open in QQ Music) | Open in Browser, Open Link |
| Playing on {Device} | Connected to, Casting to |
| Devices (the picker's title) | Speakers, Outputs |
| Stop Controlling | Disconnect, Stop Remote |

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

**Artists.** A list. Empty: "No Artists Yet", "Add a music folder to see
your artists here.", **Add Music Folder…**.

**Songs.** **Play** and **Shuffle**, then the list. Sort menu: Title, Artist,
Album, Year, Recently Added, and **Descending**. Empty: "No Songs Yet", "Add a
music folder to see your songs here.", **Add Music Folder…**.

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
each "Your Jellyfin server", "Your Navidrome server", "Airsonic, Gonic and
other Subsonic servers", "Your QQ Music account" or "Your Spotify devices",
or "Signed in as …"), then **Your Sources** (each
installed source with its status; choosing one browses it), then
**Sources Settings…**, which opens Settings › Sources.

**Now Playing.** Artwork or the Misty field, the controls (shuffle,
previous, play, next, repeat, volume with mute), and three panes: **Lyrics**,
**Queue** and **Info**. Queue: "Up Next", **Clear Queue** (asks first), and
"Nothing Up Next" when empty. Info: "About This Song" with Title, Artist,
Album, Year, Length, Format, Lyrics and File. Desktops close it with
**Library**; phones with the ⌄ button.

**Links and remote playback** (v0.0.6, ADR-0018). Every item with a
`link` (search and browse rows, album, artist and playlist pages, Now
Playing) offers **Open in {Service}**: in the song menu and the More
menus, after Go to Artist, and in Now Playing's Info pane under About This
Song. An item that cannot play here but has a link opens it as its main
action (click, Enter or tap), next to its unplayable reason.

A remote source (`PluginSummary.remote_playback`, such as Spotify) plays on
the person's own devices: Play and Shuffle work as usual and start a remote
session; Play Next, Play Last and Add to Playlist are not offered for its
songs. While the `Remote` slice has a `source`, the player bar, the
mini-player and Now Playing show its track instead of the engine's, with
**Playing on {Device}** under the title (and `error`, when set, in a
quieter line below). The transport controls send the usual commands (the
core routes them). The device button opens **Devices** (it sends
`Remote { Refresh }` when it opens): every device with its kind's icon, the
playing one checked; choosing one sends `Transfer`. The volume control sets
the playing device's volume (`SetVolume { percent }`) and is hidden when the
device has none. More holds Open in {Service} and **Stop Controlling**
(`Remote { End }`). The Lyrics pane says "Lyrics aren't shown while
{Service} plays." and the Queue pane says "Up Next is in {Service}." When a
session ends with an `error` ("Sign in to Spotify again."), show it once as
a notice. A source with `attribution` names its service in its views
("Content from Spotify"); no logos until they are approved.

## Luminous (v0.0.6 design)

The owner chose direction C, **Luminous: the cover's colour fills the
room**. Misty's two signatures, the aurora and the glass, are turned up on
every screen; Now Playing becomes a stage.

* **Field.** The aurora at about 75 % (`field.aurora-strength`, was about
  45 %) with four larger glows at the artwork's hue, on every screen, light
  and dark; colours crossfade on every song change (`dur-4`, Fluid).
* **Glass.** The sidebar is Clear glass; grouped surfaces (Settings groups,
  lists, cards) take the Mist tint **without live blur** plus `shadow-1`, so
  a screen stays within the glass budget (three live blur layers on
  desktops, two on phones); the player bar, the mini-player and the phone
  tab bar are Veil capsules floating above the content, which fades out
  just above them. Platforms use their own materials: the macOS sidebar
  material, Mica and Acrylic on Windows, `backdrop-filter` on GTK 4.22 and
  later, blur on Android 12 and later. Older Android and GTK fall back to
  solid Cloud.
* **Now Playing.** The Artwork backdrop (the core's bake) behind
  everything; artwork 380 px with `shadow-3`; title 32 px
  (`type-roles.now-playing-title`); lyrics 40 px bold, line height 1.18
  (`lyrics.line`), inactive lines at 92 % (`lyrics.scale-inactive`), the
  cascade 40 ms apart (`lyrics.stagger`); every control in one floating
  Veil **dock** (radius 40, 80 high on desktops; a rounded dock card on
  phones): shuffle, previous, play, next, repeat, the scrubber with times,
  volume, and on the right the device button and More.
* **Type and covers.** Page titles 48 px (`type.text-h1`); grid covers with
  radius 16 (`artwork.radius-grid`), `shadow-2`, lifting 4 px on hover or
  focus; four large covers per row on wide windows.
* **Contrast.** Text keeps WCAG AA on the stronger field: ink on glass
  never drops below `ink-70` for secondary text, and the dark backdrop bake
  keeps its lightness band (0.16–0.34) so white ink holds on any cover.
  Reduce Motion and Transparency turns blur and the cascade off (solid
  Cloud, no stagger), as before.
* The Artwork backdrop stays the default, so no Misty Core claim while it is
  selected (as in v0.0.5).

## AI Lyrics, Customize and Pro (v0.0.7)

**Lyrics pane.** With no lyrics: "No Lyrics" and, when the `AiLyrics`
slice is `available` (or Pro is inactive, which opens the Pro sheet),
**Create Synced Lyrics**. With plain lyrics: **Time These Lyrics** below
them. While the current song has a job: its stage ("Reading Audio…",
"Sending to {origin}…", "Recognising… 42%", "Timing Words…") with a
progress bar and **Cancel**. When the slice says what is needed first:
"Download the {Model} model ({size}) to make lyrics on this device." with
**Download** (then "Downloading {Model}… 35%" and **Cancel**); "Add your
API key to make lyrics online." with **Open AI Lyrics Settings**. A failed
job: "Couldn't make lyrics: {message}" and **Try Again**. Made lyrics carry
"Made by AI Lyrics · {engine}" under the last line and in the Info pane's
Lyrics row, and the More menu adds **Make Again** and **Delete Made
Lyrics** (asks first).

**Sending audio** asks once per plugin and origin before the first online
job: "Send This Song to {origin}?", "AI Lyrics will send the song's audio to
{origin} to make its lyrics. The provider's terms and privacy policy apply,
and it may charge your account.", Cancel, **Allow**.

**Song menu** adds **Create Synced Lyrics** after Go to Artist for library
songs while AI Lyrics is available.

**The Pro sheet** opens from any Pro action while Pro is inactive: "Jellyfesh
Pro", "AI Lyrics makes synced lyrics for any song. Customize makes
Jellyfesh yours: corners, glass, type, lyrics and layout.", **Buy Jellyfesh
Pro – {price}** and **Restore Purchases** (store builds), **Enter Licence
Key…**, **Not Now**, and the links Licence Agreement and Privacy Policy.
**Enter Licence Key** asks for the key ("JFP1-…"; Cancel, **Activate**).
Its errors: "That doesn't look like a Jellyfesh licence key." (malformed),
"This licence key isn't valid." (unknown key or bad signature), "This key is
for another product." and "This licence key has been withdrawn."

**The Look slice** (ADR-0020) applies live everywhere: corner radii, glass,
aurora, typeface, text size, density, the lyrics' weight, glow and
highlight, the order of the sidebar (navigation pane, Library switcher) and
of Home, the Now Playing layout (Stage, Artwork, Lyrics) and the
mini-player (Bar, Compact, Floating). With `customized` false every value is
the v0.0.6 default.

## Discover, details, comments and the lyrics bar (v0.0.7)

**Discover.** Choosing a source on the Sources page opens its **Discover**
page: the source's name and status, then its shelves from the core
(`Event::Discover`), each with its title and **See All** when it has more
(which browses it). Row shelves scroll sideways, grid shelves wrap, chart
shelves are numbered lists. **Browse** (the source's own folders) is a
button in the header. The library's Discover page is Home's sections.
While loading: placeholders the shape of the shelves; on failure: the
error and **Try Again**.

**Singer and album.** Go to Artist and Go to Album work for source items
too and open the same artist and album pages as the library's: the
picture, the biography or notes (`Details`, "More" expands it), Play and
Shuffle, the songs, and for artists the albums.

**Details.** The song menu, album and artist More menus and Now Playing's
Info pane offer **Get Info**: About This Song with Title, Artist, Album,
Version, Year, Genre, Composer, Lyricist, Label, Length, then **Audio**:
Quality (the core's label, such as "Hi-Res Lossless"), Format ("FLAC ·
24-bit · 96 kHz · Stereo"), Bitrate, Size, ReplayGain, and File or Source.
Rows without a value are left out. Now Playing's Info pane shows the same
for what is playing, with the format actually being decoded.

**Comments.** For sources whose `PluginSummary.comments` is true, Now
Playing has a fourth pane, **Comments**: "Popular" (the hot comments) then
"Latest", each with the author, picture, time, likes and text; more load
at the end of the list. "Comments from {Service}" at the top. Read-only.
Empty: "No Comments Yet".

**Lyrics, word by word.** With word-timed lyrics, the current line fills
word by word in the highlight colour, in Now Playing and in the **lyrics
bar**: one line (and its translation when Show Translations is on) in the
player bar on desktops and the web, and above the mini-player on Android.
**Desktop Lyrics** (macOS, Windows, Linux on X11, the web in Chromium)
opens a floating, always-on-top window with the current and next line; it
can be locked so clicks pass through, and closed from its own button or
the More menu. Settings › Appearance › Lyrics adds **Show Lyrics Bar** and
**Desktop Lyrics**.

**Jellyfesh Web and services that refuse browsers.** Before signing in to
a source the browser cannot reach: "{Service} doesn't let browsers connect
to it directly. Use the Jellyfesh app on your computer or phone, or set up
a relay in Settings › Sources › Relay." Settings › Sources › **Relay**:
Relay Address and Relay Token, with "A relay you run yourself lets
Jellyfesh Web reach services that browsers can't. Jellyfesh doesn't run
one for you."

## Settings

Sections in this order: **Appearance · Playback · Library · Sources ·
Extensions**, plus **About** where the platform keeps About in settings
(Windows, Android, Web). Groups and rows in this order:

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
8. **Customize…**: "Corners, glass, type, lyrics style and layout." Opens
   Extensions › Customize (with a Pro badge while Pro is inactive).

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

**Extensions** (v0.0.7; ADR-0019, ADR-0020, ADR-0021)
1. **Jellyfesh Pro**: with Pro Preview, "Pro features are free during the
   public beta."; with Pro, "Jellyfesh Pro · Purchased" or "Jellyfesh Pro ·
   Licence …ABCD" and **Remove Licence…** (asks first); without Pro, "AI
   Lyrics and Customize are part of Jellyfesh Pro.", **Buy Jellyfesh Pro –
   {price}** and **Restore Purchases** (store builds only), and **Enter
   Licence Key…**. Then the links Licence Agreement, Privacy Policy and
   Terms of Sale.
2. **AI Lyrics**: Enabled; its settings (the localised schema, by group);
   **Sign In** or Sign Out for the online mode's API key ("API Key Added");
   **On-Device Models**, each "Name · Size" with **Download**, "Downloading…
   35%" with **Cancel**, or "Ready" with **Delete…**; **Sending Audio**,
   each allowed origin with **Stop Allowing**; "Version 1.2.0 · Author".
3. **Customize**: Enabled; its settings by group (Look, Lyrics, Layout);
   **Reset to Defaults**.
4. Any other extension the same way, and **Install from File…** in the
   section heading.

**About**: the version (from the build, never typed in), and the website and
the third-party licences where the build knows where they are published;
**Licence Agreement** and **Privacy Policy**. Direct downloads also have
**Check for Updates** and **Check for Updates Automatically**; when a newer
version exists, "Jellyfesh {version} Is Available" and **View Release**.

## Steps that ask first

Remove Folder…, Uninstall…, Delete Playlist… and Clear Queue ask for
confirmation in every app, with these words (Cancel, then the action):

| Step | Title | Message | Action |
|---|---|---|---|
| Remove Folder… | Remove “Name” from the library? | Its songs leave the library and its playlists. The files stay where they are. | Remove Folder |
| Uninstall… | Uninstall Name? | Its settings and sign-in are removed. | Uninstall |
| Delete Playlist… | Delete “Name”? | The songs stay in your library. | Delete Playlist |
| Clear Queue | Clear the Queue? | Playback stops and every song is removed from the queue. | Clear Queue |
| Delete Made Lyrics | Delete These Lyrics? | You can make them again. | Delete |
| Remove Licence… | Remove the Licence Key? | Pro features stop until you enter a key or buy Jellyfesh Pro. | Remove Licence |
| Delete… (a speech model) | Delete the {Model} Model? | You can download it again. | Delete |
 Turning on a community plugin, or Allow
Community Plugins, shows the disclosure (ADR-0012). New Playlist… asks for
the name before it creates anything.

## Keyboard

The same keys on every desktop app and the web (⌘ instead of Ctrl on
macOS): Space Play or Pause · Ctrl+→ / Ctrl+← Next / Previous Song ·
Shift+→ / Shift+← Skip 5 Seconds · Ctrl+↑ / Ctrl+↓ Volume · Ctrl+F Search ·
Esc Back (and closes Now Playing). Anything more follows the platform's own
conventions and must not reuse these keys.

## Allowed differences

* **Web:** Remove Folder… says only "Its songs leave the library." (it has
  no playlists). No Playlists, Favourites or history (so Home has no Recently
  Played or Most Played; it shows each signed-in source's start page
  instead), and no Output Device (a browser decides where sound goes). Its
  Settings also list the keyboard shortcuts. Its library is folders the browser may
  read, picked again on each visit before their songs play, with no Watch
  Folders. **Open Files…** plays files for one visit without adding
  them. Streaming Quality and Language are its own settings.
* **Linux:** no App Icon (GNOME takes the icon from the installed files); a
  note says so.
* **Android:** the library is one tab; it asks for access to music and offers
  "Find Music on This Phone" before folders.
* **macOS:** About Jellyfesh is in the app menu. **Linux:** About and Keyboard
  Shortcuts are in the main menu.
* Plugin settings apply as each field is committed on macOS and Linux, and
  with **Save** on Windows, Android and the web.
