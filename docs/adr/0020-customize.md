# ADR-0020: Customize: appearance plugins and the Look slice

* Status: **Accepted**
* Date: 2026-11-17
* Amends: ADR-0004 (settings schema), ADR-0013 (design language)

## Context

The owner asked for "more settings in a new plugin for customizing the UI",
covering both look and feel and layout (answer "Both", 2026-11-17):

* **Look and feel:** corner roundness, glass and aurora strength, typeface
  and text size, lyrics style (weight, glow, highlight colour) and density,
  applied live in every app.
* **Layout:** which places the sidebar shows and in what order, which Home
  sections and in what order, the Now Playing layout and the mini-player
  style.

Customize is part of Jellyfesh Pro (ADR-0021). Platform UI code must stay
free of business logic, so the apps must receive resolved values, not the
plugin's settings. The 1.1 settings schema has only flat scalars, which
cannot express ordered lists, colours or groups, and its titles are English
only while the apps speak four languages.

## Decision

1. **Capability `appearance`** (API 1.2, a Pro capability) with the export

   ```wit
   interface appearance {
       use types.{plugin-error};
       enum typeface { system, rounded, serif, mono }
       enum density { compact, standard, roomy }
       enum lyrics-weight { regular, medium, semibold, bold, heavy }
       variant highlight { artwork, accent, custom(u32) }   // 0xRRGGBB
       enum place { home, albums, artists, songs, playlists, favourites, search, sources }
       enum home-section { summary, recently-added, recently-played, most-played, favourites }
       enum now-playing-layout { stage, artwork, lyrics }
       enum mini-player-style { bar, compact, floating }
       record look {
           corner-scale: option<f32>,      // 0–2; 1 is Luminous
           glass: option<f32>,             // 0–1; 0 is solid panels
           aurora: option<f32>,            // 0–1; field.aurora-strength
           typeface: option<typeface>,
           text-scale: option<f32>,        // 0.85–1.30
           density: option<density>,
           lyrics-weight: option<lyrics-weight>,
           lyrics-glow: option<f32>,       // 0–1
           lyrics-highlight: option<highlight>,
           sidebar: option<list<place>>,   // shown places, in order
           home: option<list<home-section>>,
           now-playing: option<now-playing-layout>,
           mini-player: option<mini-player-style>,
       }
       look: func() -> result<look, plugin-error>;
   }
   ```

   The host calls `look` after the plugin's settings change and when it
   loads, then sanitises the answer: numbers clamped to their ranges (NaN
   ignored), duplicate places and sections dropped, at least one library
   place kept (Settings is never part of the list and always shown). One
   appearance plugin is active: the first enabled one in install order.

2. **The `Look` slice** in the core holds resolved values, defaults filled
   in, so the apps apply one thing: `corner_scale`, `glass`, `aurora`,
   `typeface`, `text_scale`, `density`, `lyrics_weight`, `lyrics_glow`,
   `lyrics_highlight`, `sidebar`, `home`, `now_playing`, `mini_player`, and
   `customized` (false when no appearance plugin is active or Pro is off;
   then every value is the Luminous default). Defaults: 1.0, 1.0, the
   token's `field.aurora-strength` (0.75), system, 1.0, standard, bold, 0.6,
   artwork, all eight places in the docs/ui.md order, summary, recently
   added, recently played, most played; stage; bar. Reduce Transparency and
   the Transparency setting still win over `glass`; Reduce Motion still wins
   over motion.

   How each app applies it: radii × `corner_scale`; glass alpha and blur ×
   `glass`; the aurora field at `aurora`; the platform's own faces for
   `typeface` (no bundled fonts: SF, SF Rounded, New York and SF Mono on
   Apple; Segoe UI Variable, then Cascadia Mono for mono and Georgia for
   serif on Windows; the system's sans, serif and monospace on Linux and
   Android; `system-ui`, `ui-rounded`, `ui-serif`, `ui-monospace` on the
   web); every type size × `text_scale`; spacing × 0.8, 1.0 or 1.2 for
   density; the lyrics' weight, glow (a soft shadow in the highlight colour)
   and highlight colour; the sidebar, navigation pane or tabs and Home in
   the given order; the three Now Playing layouts (stage: the v0.0.6 stage;
   artwork: a large cover with the current line below; lyrics: lyrics full
   width with a small cover in the corner); the mini-player as the bar, a
   compact bar (cover, title, play) or a floating capsule. Android keeps its
   four bottom tabs and orders the Library switcher by `sidebar`.

3. **Settings schema profile 1.2** (packages that use it declare
   `api_version = "1.2"`):
   * `"format": "color"`: a string `#RRGGBB`, shown as a colour well;
   * a `number` or `integer` with both `minimum` and `maximum` is a slider,
     with `multipleOf` as its step;
   * `"type": "array"` with `"items": { "type": "string", "enum": [...] }`
     (or `oneOf`) and `"uniqueItems": true`: an ordered checklist; the value
     lists the shown items in order;
   * `"oneOf": [{ "const": "stage", "title": "Stage" }, ...]` for choices
     with titles;
   * `"x-jf-group": "Lyrics"` on a property: consecutive properties with the
     same group are shown together under that heading;
   * translations in the package: `l10n/{zh-Hans,zh-Hant,ja}.json`, mapping
     `title`, `description`, `properties.{name}.title`,
     `properties.{name}.description`, `properties.{name}.x-jf-group` and
     `choices.{value}` to the language's words. The core merges them:
     `Core::localized_settings_schema(plugin, language)` returns the schema
     in that language (English where a word is missing).
   Packages may hold up to 16 files.

4. **The Customize plugin** (`org.jellyfesh.customize`, official, signed,
   bundled with every app): groups **Look** (Corners, Glass, Aurora,
   Typeface, Text Size, Density), **Lyrics** (Weight, Glow, Highlight,
   Highlight Colour) and **Layout** (Sidebar, Home, Now Playing,
   Mini-Player), with translations in every app language. Its `look` maps
   the settings to a `look`, leaving out what is at its default.
   `Command::ResetPluginSettings { plugin }` restores the defaults (**Reset
   to Defaults** under its settings).

5. **Where it lives in the apps.** A new Settings section **Extensions**
   (after Sources) holds Jellyfesh Pro (ADR-0021) and every extension:
   plugins whose capabilities are only `make_lyrics` or `appearance`
   (`PluginSummary.kind == Extension`; they are not listed under Sources).
   Appearance ends with **Customize…**, which opens Extensions › Customize.

## Consequences

* Theme packs can be plugins too, without code paths of their own.
* Five apps implement three Now Playing layouts and three mini-player
  styles; the defaults stay exactly v0.0.6.

## Tests

* Schema profile 1.2: parsing, validation and defaults of every new keyword;
  1.1 packages refused when they use them; translation merging.
* Look sanitising (ranges, NaN, duplicates, the library place rule) and
  resolution with defaults, Pro off and two appearance plugins.
* The Customize plugin's settings-to-look mapping; sandbox tests for the new
  export (not declared, Pro required).
* FFI contract vectors for the `Look` slice.
