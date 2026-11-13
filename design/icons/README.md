# App icon concepts

Status: **decided (2026-11-05)**: variant B's construction; **Ice** is the
default light icon and **Steel** the default dark icon; all ten colour
options in `round4/` ship as choices in Settings → Appearance → App icon
(ADR-0017). Earlier rounds stay for the record.

Five jellyfish-inspired directions, each drawn as separate layers so the
chosen one can become every platform's icon format without redrawing:

| Layer | Used for |
| --- | --- |
| `background.svg` | Android adaptive background, macOS 26 Icon Composer back layer, Windows/GNOME plate |
| `foreground.svg` | Android adaptive foreground, Icon Composer front layer; the glyph stays inside a circle of radius 310 on the 1024 canvas, so every Android mask keeps it whole |
| `monochrome.svg` | Android 13+ themed icon, GNOME symbolic icon, Windows tray and taskbar badge |
| `full.svg` | background + foreground, unmasked, for quick viewing |

| # | Concept | Idea |
| --- | --- | --- |
| 1 | Lumen | Frosted-glass bell lit by Misty's aurora; five tapered tentacles fall like a soft waveform |
| 2 | Abyss | The same bell after dark: a luminous outline drifting in a deep field |
| 3 | Wavebell | The bell drawn as sound-wave arcs over a solid core; tentacles as a level meter; built for 16 px |
| 4 | Verse | Swimming bell-first, its tentacles trail behind as lines of lyrics; the current line is the accent |
| 5 | Tide | Misty's identity-glyph recipe: one outline glyph on a hue-to-hue+55° tile |

Explored and dropped: *Flagnote* (bell as note head, tentacle as stem and
flag) and *J-Drift* (bell falling into a J) both read as umbrellas at small
sizes; *Aurelia* (moon jelly from above) read as the ⌘ key, and its rotated
variant formed hooks.

## Round 2: Wavebell's shape in Tide's style

Brief (2026-11-05): keep Wavebell's shape, use Tide's transparent lines and
tile, and try combinations of deep blue and cyan. Sources in `round2/`.

| # | Variant | Tile | Lines |
| --- | --- | --- | --- |
| 1 | Tide Classic | cyan → deep blue, diagonal | solid white (baseline) |
| 2 | Clearwater | cyan → deep blue, diagonal | 75% white, translucent |
| 3 | Glass Tide | deep blue → cyan, diagonal | glass tubes: bright rim, clear core |
| 4 | Deep Sea | navy with a cyan glow from above | glass, white to cyan |
| 5 | Depth | cyan → deep blue, vertical | translucent, outlined core like Tide's bell |
| 6 | Midnight | near-black navy, cyan rim light | cyan-tinted translucent |
| 7 | Aurora | deep blue with cyan and azure corner glows | glass |
| 8 | Lagoon | bright cyan → azure | 88% white, deeper shadow |

The glyph is a luminance mask over a paint, so the transparency is real:
the foreground layer lets the background layer show through on every
platform that composites icon layers (Android adaptive, macOS 26 Icon
Composer). Translucent and glass lines fade at 16–32 px; the chosen variant
gets hand-tuned small sizes with solid lines.

## Round 3: Depth's tile with Midnight's lines

Brief (2026-11-05): use Midnight's lines on Depth's background. Sources in
`round3/`.

| Variant | Difference |
| --- | --- |
| A · Depth × Midnight (`final`) | Exactly as asked: cyan-to-deep-blue tile, cyan-tinted 90% translucent lines |
| B · With lift (`final-lift`) | Adds a soft shadow under the glyph for the bright top of the tile |
| C · With outlined core (`final-outline`) | Depth's open core instead of the solid one |
| D · With a deeper top (`final-deep`) | Tile starts at aqua, so the lines read more clearly |

Each variant has `foreground-small.svg`, a simplified master for 16 and
32 px: one arc, the core and three tentacles at full opacity.

## Round 4: colour options for B

Brief (2026-11-05): keep variant B, try more colour combinations. Sources in
`round4/` (each with `foreground-small.svg`).

| # | Option | Tile (top → bottom) | Lines |
| --- | --- | --- | --- |
| 1 | Original B | cyan → deep blue | ice → cyan |
| 2 | Navy Tide | aqua → navy | ice → light cyan |
| 3 | Royal | sky cyan → royal blue | ice → sky |
| 4 | Teal Deep | teal → deep sea blue | ice → teal |
| 5 | Sea Glass | pale aqua → teal-blue | white → aqua |
| 6 | Electric | vivid cyan → electric blue | white → cyan |
| 7 | Steel | blue-grey → slate | ice → cyan |
| 8 | Abyss (dark) | navy → near-black, cyan glow | light cyan → cyan |
| 9 | Ice (light) | ice white → pale cyan | deep blue |
| 10 | Lagoon | bright lagoon → azure | white → ice |

8 and 9 are intended as the dark and light icon appearances (macOS 26
and Android can follow the system appearance).

## Regenerate

```bash
python3 design/icons/tools/gen_concepts.py design/icons/concepts
python3 design/icons/tools/gen_round2.py design/icons/round2
python3 design/icons/tools/gen_round3.py design/icons/round3
python3 design/icons/tools/gen_round4.py design/icons/round4
```

The script has no dependencies. Colours are written in OKLCH and converted
with the CSS Color 4 OKLab matrices and per-channel clipping, the same
conversion the Misty tokens use, so icon and interface colours agree.

## After a concept is chosen

1. Hand-tune 16 and 32 px masters (thin strokes merge or vanish at those sizes).
2. macOS 26: build a layered `.icon` in Icon Composer (default, dark and
   tinted appearances); export the classic `.icns` set for macOS 14–15.
3. Android: adaptive icon (108 dp layers) with a monochrome layer; Play Store 512 px.
4. Windows: MSIX asset set (Square44x44 with target-size and unplated variants,
   Square150x150, Wide310x150, StoreLogo, SplashScreen) at all scale factors.
5. Linux: `hicolor` scalable SVG plus a `-symbolic` SVG, following the GNOME app
   icon guidelines.
6. Check the final mark against Jellyfin's (purple-to-cyan arch) and the
   Misty mark (gradient square, sun and mist lines), which we must not resemble.
