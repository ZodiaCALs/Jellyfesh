# Customize

The Customize plugin changes how Jellyfesh looks and is laid out: corners,
glass, the aurora behind the artwork, the typeface and text size, how
lyrics are highlighted, and which places the sidebar and Home show, in what
order. It comes with every Jellyfesh app from v0.0.7 and updates with it;
the same plugin is also published as a separate download
(`org.jellyfesh.customize`, signed with the Jellyfesh release key).
Customize is part of **Jellyfesh Pro**. How it works inside:
`plugins/customize/README.md` and ADR-0020.

## What it is

The plugin is only its settings: it never uses the network and keeps
nothing else. When its settings change, every Jellyfesh window takes the
new look at once. What you leave at its default stays as the app has it
(the Luminous look of v0.0.6).

## Set it up

1. You need **Jellyfesh Pro**; without it, the app keeps its own look.
2. In Jellyfesh's settings, open the plugin **Customize** (among the
   extensions) and change what you like. **Reset to Defaults** puts
   everything back.

## Settings

| Group | Setting | Choices (default first) |
| --- | --- | --- |
| Look | Corners | 1; from 0 (square) to 2 (very round) |
| Look | Glass | 1; from 0 (solid panels) to 1 |
| Look | Aurora | 0.75; from 0 (none) to 1 |
| Look | Typeface | System, Rounded, Serif, Monospaced (your system's own fonts) |
| Look | Text Size | 100%; from 85% to 130% |
| Look | Density | Standard, Compact, Roomy |
| Lyrics | Weight | Bold, Regular, Medium, Semibold, Heavy |
| Lyrics | Glow | 0.6; from 0 to 1 |
| Lyrics | Highlight | Artwork (colours from the cover), Accent Colour, Custom Colour |
| Lyrics | Highlight Colour | #FF8A3D, for Custom Colour |
| Layout | Sidebar | which of Home, Albums, Artists, Songs, Playlists, Favourites, Search and Sources show, in your order |
| Layout | Home | which of Library Summary, Recently Added, Recently Played, Most Played and Favourites show, in your order |
| Layout | Now Playing | Stage, Artwork or Lyrics |
| Layout | Mini-Player | Bar, Compact or Floating |

The settings are in English, Simplified Chinese, Traditional Chinese and
Japanese.

## What the app keeps in its own hands

* Settings is always in the sidebar, and at least one place of your library
  stays there; a place listed twice shows once.
* Numbers stay within their ranges whatever the plugin says.
* **Reduce Transparency** and **Reduce Motion** in your system settings
  still win over Glass and the aurora.
* Each app uses its platform's own fonts for the typeface (no fonts are
  bundled). Android keeps its four bottom tabs and orders the Library
  switcher by your sidebar order.
* One appearance plugin is active at a time: the first one turned on.

## Privacy

Customize sends nothing anywhere. Its settings are stored with Jellyfesh's
other settings on this device.
