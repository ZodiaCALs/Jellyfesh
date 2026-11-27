# One Jellyfesh in five apps

The macOS, Windows, Linux and Android apps and Jellyfesh Web are one app:
the same places, in the same order, with the same words, features and
steps.

Since v0.0.8 (ADR-0023) they also share **one layout, Jellyfesh Web's**:
* the floating sidebar panel, or the phone tabs on narrow screens;
* Settings as a page in the main window;
* one player bar floating over every page;
* the web's pages, measures and components.

Each app draws it with its own toolkit: SwiftUI, WinUI 3, GTK 4 with
libadwaita, Jetpack Compose. No app shows a webview.

What stays native:
* window frames and title-bar controls;
* the macOS menu bar and Dock menu;
* system file pickers and share sheets;
* media controls and notifications;
* accessibility;
* scrolling;
* the system font.

The reference screenshots of every page are in `screenshots/web/`. Rules and
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
| Downloads · Download · Remove Download… · Remove All Downloads… | Offline, Save Offline, Delete Download |
| Save as Playlist… (a source playlist) | Import, Copy Playlist |
| Show More (a search tab's kind) | See More, Load More |
| Results from {Service} (a separate source on the All tab) | |
| Auto · Lossless · High · Normal · Low (Streaming Quality) | Hi-Fi, Standard Quality |
| Open Mini Player · Keep on Top | Compact Mode, Floating Window |
| Updated {time} ago (a source's cached likes or playlists) | Last synced |

## Navigation

In this order everywhere: **Home · Albums · Artists · Songs · Playlists ·
Favourites · Downloads**, then **Search · Sources**, then **Open Files… ·
Settings**. One layout in every app (ADR-0023):

| Width | Layout |
|---|---|
| 761 px (or dp) and wider | The floating sidebar panel: the wordmark, the seven library places, a separator, Search and Sources; Open Files… and Settings at its foot. Every app, and Android tablets. |
| Up to 760 px | Tabs **Home · Library · Search · Sources · Settings**. Library switches between Albums, Artists, Songs, Playlists, Favourites and Downloads. Android phones, and narrow windows. |

Settings is a page in the main window everywhere. On macOS, ⌘, and the app
menu's Settings… open it, and About Jellyfesh opens Settings › About; the
menu bar's other items go to the same places. Jellyfesh Web has no Downloads
place (see Allowed differences).

## Pages

**Home.** The library size ("1,204 songs · 96 albums · 41 artists"), then
Recently Added (albums), Recently Played (songs) and Most Played (songs),
every row with its cover.
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
playlist also Rename… and Delete Playlist…). Shuffle starts on a random
song.
* A **source playlist** (ADR-0024) has **Save as Playlist…** and Open in
  {Service} in its More menu. Its songs show as soon as the first page
  arrives.
* A source playlist whose songs the service doesn't list (`songs-listed`
  false, such as a Spotify playlist the person follows) says "{Service}
  lists this playlist's songs only in {Service}.". Play and Shuffle still
  start it there.
* Albums, playlists and songs from a source that allows downloads have
  **Download** in their More menu (ADR-0025). A downloaded song shows the
  Downloaded mark.

**Playlists.** **New Playlist…** asks for a name first. Import Playlist… and
Export Playlist… (M3U8) in every app. Songs reorder by dragging (a handle on
Android). Delete Playlist… asks first. Empty: "No Playlists Yet",
"Make a playlist to keep songs together.", **New Playlist…**.
* After the Jellyfesh playlists come the sources' playlists, one section
  per signed-in source (the `SourceLibrary` slice). They refresh when the
  page opens. A cached list says "Updated {time} ago".

**Favourites.** Songs marked with the heart. Empty: "No Favourites Yet",
"Songs you mark with the heart appear here."
* After them comes one section per signed-in source, in the source's own
  words ("Liked Songs", "Favourites", "Starred"), with Songs, Albums and
  Artists where the source keeps them.
* A source whose likes came back empty still shows its name, with "Nothing
  liked in {Service} yet."; one whose likes failed shows the reason.
* They are read-only: the heart marks a song in Jellyfesh only.
* Spotify's sections stand on their own, attributed "Content from
  Spotify".

**Downloads** (ADR-0025).
* At the top: **Play** and **Shuffle**, and "{n} songs · {size} used".
* **In Progress**: each download with "Waiting", its percentage, or, after
  a failure, the reason and **Try Again**; **Cancel** on each.
* While the core pauses downloads: "Downloads resume when you're back
  online." (offline) or "Downloads resume when you're off a metered
  connection." (metered, with Pause on Metered Connections on).
* Then the downloaded songs by album. **Remove Download…** (asks first) is
  on each song and on a whole album, which removes all its downloaded songs.
* Empty: "No Downloads Yet", "Download albums and playlists from your
  servers to play them offline."

**Song menu** (right click, long press or More): Play Next, Play Last, Add to
Playlist…, Add to Favourites or Remove from Favourites, Go to Album, Go to
Artist, and on desktops Show in Finder / Show in Explorer / Show in Files.

**Rows and cards** (v0.0.9):
* A song row shows a **play button** over its artwork (or its number) and a
  **More** button at its end while the pointer is over it or it has keyboard
  focus. The play button plays from that row, as a double-click, Return or
  a tap on a phone does.
  * Phones have no hover: they show More on every row.
* Right-click opens the same menu as More, in every app and in the web; so
  does a long press on touch screens.
* Album and playlist cards show a play button on the cover and More, on
  hover or focus. Right-click opens the card's menu: Play, Shuffle, Play
  Next, Play Last, Add to Playlist…, Go to Artist, Download (where the
  source allows it) and Open in {Service}.
* A source's song keeps its album, artist and link wherever it shows (Home,
  Favourites, playlists, the queue). So its menu offers Go to Album, Go to
  Artist and Open in {Service} whenever the source has them, not only in
  search and browse.
* Download appears only for sources that allow downloads (ADR-0025).

**Search** (ADR-0024 §4). One field ("Songs, albums and artists"), then the
tabs **All**, **Library**, and one per source that answered.
* **All:** library results first (Songs, Albums, Artists), then **From Your
  Sources**, one group per source that allows merging, then a link to each
  separate source's tab ("Results from Spotify").
* **A source's tab:** its Songs, Albums, Artists and Playlists, each with
  **Show More**.
* Only sources that are ready are asked: switched on, and signed in when
  they need a sign-in. A source that isn't set up yet is left out of search;
  Settings › Sources says what it needs.
* A source that fails shows its reason in plain words, never a code's
  prefix ("invalid input: …").
* Nothing found: "No Results".

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
capsule and Now Playing show its track instead of the engine's, with
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
  desktops, two on phones); the player bar, its capsule and the phone
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
player bar's style (Bar, Compact, Floating; the Look calls it the
mini-player, which is not the mini player window). With `customized` false every value is
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
player bar on desktops and the web, and above the capsule on Android.
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

## The player bar and the mini player (v0.0.8)

**The bar** (ADR-0023 §2) floats over every page, Settings included. It is
the full bar from 900 px wide and the capsule below that, and hides only
while Now Playing is open.

The full bar holds, in this order:
* the artwork and two lines (or the lyrics bar's line);
* shuffle, previous, play, next, repeat, and the scrubber with times;
* the **quality badge**;
* Lyrics, Queue, the device button (while a remote source plays) and
  volume;
* the **mini player button** and More.

The capsule holds the artwork, the title, play and next.

Every part of the bar keeps to its own column at any width; nothing in it
overlaps (v0.0.9):
* The title, the artist and the translation end with "…" when too long.
* The sung line fades out at its column's edge.
* The times stay beside the scrubber.
* The scrubber shows keyboard focus by thickening and showing its thumb,
  never with the system's focus ring.

**The quality menu** (ADR-0026 §3) opens from the badge, which shows the
playing quality's label (the core's `QualityLabel`).
* It lists the levels `NowPlaying.qualities` offers, with the chosen one
  checked, and the format line ("FLAC · 24-bit · 96 kHz", "AAC · 256
  kbit/s").
* With no levels it shows the format line and "Files play as they are." or
  "Quality is set in {Service}.".
* A new level applies at once when the song can seek, and from the next
  song otherwise.

**The mini player** (ADR-0026 §4) opens with the bar's button or **Open
Mini Player**. It is a small window above other windows with:
* the cover, the title and the artist;
* previous, play and next;
* a progress line;
* the lyrics bar's line, when Show Lyrics Line is on.

It closes from its own button. It is a window on macOS, Windows, Linux and
in the web (Document Picture-in-Picture), and picture-in-picture on Android
when the setting is on.

## Settings

Sections in this order in every app: **Appearance · Playback · Library ·
Downloads · Sources · Extensions · About**. Downloads isn't in Jellyfesh Web.
Groups and rows in this order:

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
6. **Mini Player** (v0.0.8):
   * **Keep on Top** (on);
   * **Open When the Window Is Closed** (off): "Keep the mini player when
     you close the window while music plays.";
   * **Show Lyrics Line** (on);
   * on Android instead, **Mini Player in Picture-in-Picture** (off):
     "Show the mini player when you leave Jellyfesh while music plays.".
7. **App Icon** (macOS, Windows, Android).
8. **Language** (Web only).
9. **Customize…**: "Corners, glass, type, lyrics style and layout." Opens
   Extensions › Customize (with a Pro badge while Pro is inactive).

**Playback**
1. **Output**: Output Device (System Default, then the devices).
2. **Crossfade**: Crossfade, Length (1–12 s), Album Continuity.
3. **Volume Levelling**: ReplayGain (Off, Track, Album), Preamp. "Evens out
   loudness using the ReplayGain tags in your files."
4. **Equalizer**: Equalizer (on/off), Preset (Flat, Bass Boost, Vocal,
   Treble from the core's `eq_presets()`; Custom once a band moves), Preamp,
   the ten bands, **Reset to Flat**.
5. **Streaming**: **Streaming Quality**: Auto, Lossless, High, Normal, Low.
   "Auto plays lossless, or High on a metered connection." The same setting
   as the bar's quality menu.
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

**Downloads** (v0.0.8, ADR-0025; not in Jellyfesh Web)
1. **Folder**: the folder, with **Change…** and Show in Finder / Explorer /
   Files. Android saves to Music/Jellyfesh.
2. **Quality**: Original ("The file exactly as your server keeps it."),
   High, Normal, Low.
3. **File Names**: the template, with its fields and a live preview of one
   song's path.
4. **Tags**:
   * **Add Missing Cover and Lyrics** (off): "Original files are kept as
     they are unless this is on.";
   * **Lyrics** (on);
   * **Save .lrc Files** (off).
5. **Network**: **Pause on Metered Connections** (on): "Downloads wait while
   you're on mobile data, a hotspot or another metered connection." Every app
   reports its network to the core, so every app but the web shows it.
   **Downloads at Once** (1–4).
6. **Space Used** and **Remove All Downloads…** (asks first).

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
   section heading. Every extension ends with "Version · Author" and
   **Uninstall…**, which asks as sources do ("Uninstall Name?", "Its
   settings and sign-in are removed."); a bundled extension the person
   uninstalls stays uninstalled until they install it again.

**About**: **Keyboard Shortcuts** on desktops and the web (the shared keys
listed under Keyboard, with the platform's modifier); the version (from the
build, never typed in), and the website and the third-party licences where
the build knows where they are published;
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
| Remove Download… | Remove the Download of “Name”? | The file is deleted from this device. You can download it again. | Remove Download |
| Remove All Downloads… | Remove All Downloads? | Every downloaded file is deleted from this device. | Remove All |
 Turning on a community plugin, or Allow
Community Plugins, shows the disclosure (ADR-0012). New Playlist… asks for
the name before it creates anything.

## Keyboard

The same keys on every desktop app and the web (⌘ instead of Ctrl on
macOS): Space Play or Pause · Ctrl+→ / Ctrl+← Next / Previous Song ·
Shift+→ / Shift+← Skip 5 Seconds · Ctrl+↑ / Ctrl+↓ Volume · Ctrl+F Search ·
Ctrl+M Open Mini Player (⌥⌘M on macOS, where ⌘M minimises) · Ctrl+, Settings
· Esc Back (and closes Now Playing). Anything more follows the platform's
own conventions and must not reuse these keys.

## Allowed differences

* **Web:**
  * Remove Folder… says only "Its songs leave the library.".
  * Hearts, playlists and history are kept in the browser.
  * No Output Device: a browser decides where sound goes.
  * No Downloads place or Settings section: Download saves one file through
    the browser.
  * The mini player needs Document Picture-in-Picture (Chrome and Edge 116+,
    Firefox 151+, desktops).
    * Elsewhere its button is hidden and Ctrl+M does nothing.
    * Its Settings group has only Show Lyrics Line: a picture-in-picture
      window always stays on top and closes with the page.
    * A page has one picture-in-picture window at a time, so opening the
      mini player closes Desktop Lyrics and the other way round. The mini
      player's lyrics line shows the line being sung.
  * Its library is folders the browser may read, picked again on each visit
    before their songs play, with no Watch Folders. **Open Files…** plays
    files for one visit without adding them.
  * Language is its own setting.
* **Linux:**
  * No App Icon: GNOME takes the icon from the installed files; a note says
    so.
  * On Wayland, Desktop Lyrics and the mini player are normal windows,
    because Wayland lets no app keep its window above the others. The mini
    player's **Keep on Top…** opens the desktop's window menu, where Always
    on Top may be offered. On X11 both stay above other windows.
  * Customize's Rounded typeface is the system's sans: GNOME has no rounded
    face.
* **Android:**
  * It asks for access to music and offers "Find Music on This Phone" before
    folders.
  * It asks for **Local Network** access before connecting to a server on
    the home network (Android 17).
  * No Desktop Lyrics: the lyrics bar above the capsule shows the line being
    sung.
  * The mini player is picture-in-picture, off until turned on.
  * Its Rounded typeface is the system's: Android has no rounded system
    face.
  * The Google Play build sells Pro through Google Play Billing only and has
    no **Install from File…**, **Allow Community Plugins**, **Enter Licence
    Key…** or **Check for Updates**.
* **macOS:** the menu bar's Jellyfesh menu also holds About Jellyfesh and
  Settings…, which open those Settings pages.
* Plugin settings apply as each field is committed on macOS and Linux, and
  with **Save** on Windows, Android and the web.
* **Store builds** (App Store, Microsoft Store, Google Play): no **Install
  from File…** and no **Allow Community Plugins** (the core's
  `can_install_plugins` is false); App Store and Google Play builds have no
  **Enter Licence Key…** (Pro through the store only). Store builds have no
  Check for Updates (the store updates them). ADR-0021 §4.
* **On-device AI Lyrics:** not in Jellyfesh Web and not on Windows on Arm
  (whisper.cpp does not build there with MSVC); both offer the online mode.
