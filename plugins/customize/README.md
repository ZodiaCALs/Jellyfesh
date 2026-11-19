# Customize for Jellyfesh

Changes how Jellyfesh looks and is laid out, in every app at once. The plugin
(`org.jellyfesh.customize`) ships inside every Jellyfesh app and is also
published as a separate `.jfpkg`, signed with the Jellyfesh release key. It
is part of **Jellyfesh Pro** (ADR-0020, ADR-0021). For people using it:
`docs/plugins/customize.md`.

## What it does

Its settings are the whole plugin; it never uses the network. The app asks
it for the look when it loads and whenever the settings change, and applies
the answer live in every window.

| Group | Setting | Values (default first) |
| --- | --- | --- |
| Look | Corners | 1 (Luminous); 0 (square) to 2 |
| Look | Glass | 1; 0 (solid panels) to 1 |
| Look | Aurora | 0.75; 0 to 1 |
| Look | Typeface | System, Rounded, Serif, Monospaced (the platform's own fonts) |
| Look | Text Size | 100%; 85% to 130% |
| Look | Density | Standard, Compact, Roomy |
| Lyrics | Weight | Bold, Regular, Medium, Semibold, Heavy |
| Lyrics | Glow | 0.6; 0 to 1 |
| Lyrics | Highlight | Artwork, Accent Colour, Custom Colour |
| Lyrics | Highlight Colour | #FF8A3D (for Custom Colour) |
| Layout | Sidebar | Home, Albums, Artists, Songs, Playlists, Favourites, Search, Sources: shown ones in order |
| Layout | Home | Library Summary, Recently Added, Recently Played, Most Played (and Favourites): shown ones in order |
| Layout | Now Playing | Stage, Artwork, Lyrics |
| Layout | Mini-Player | Bar, Compact, Floating |

Everything left at its default is left to the app, so the apps keep their
own defaults (the v0.0.6 Luminous look). The app still checks every answer:
numbers are kept within their ranges, a place shown twice is shown once, at
least one place of the library stays in the sidebar, and Settings is always
there. Reduce Transparency and Reduce Motion still win over Glass and the
aurora. **Reset to Defaults** under the plugin's settings restores them.

The settings come in English, Simplified and Traditional Chinese and
Japanese (`l10n/`).

## Tests

```sh
cargo test -p jf-customize
```

Unit tests for the settings-to-look mapping (every setting, the defaults,
the schema's defaults matching the plugin's); the real component in the
real host for Pro gating, the translations and the checked look.
