# Brand: wordmark and lockups

Status: **formal wordmark decided; home-page hero awaiting your choice
(2026-11-05)**. Review on the design canvas, pages "Wordmark · formal and
home page" and "Wordmark · options and lockups".

## Usage (decided 2026-11-05)

| Where | Wordmark | Files |
| --- | --- | --- |
| Formal: About window, installers, documents, legal pages, press | **Spaced capitals**, one colour (ink on light, white on dark) | `wordmark/formal-caps-{ink,white}.svg`, `wordmark/type-caps-{light,dark}.svg` |
| Website home page hero | **One fancy option** (choose): A bioluminescent glass, B tentacle flourish, C aurora glass capitals | `wordmark/hero-{biolume,tentacle,glass}.svg` |
| Everything inside the app | System font text (Misty TYP-01); the wordmark appears only as artwork | — |

The other options below stay in the kit for social cards, stickers and
merchandise.

## All options

| # | Option | Files | Notes |
| --- | --- | --- | --- |
| 1 | Monoline | `wordmark/monoline-{light,dark,mono}.svg` | Custom lowercase lettering in the icon's round strokes; the j, y and l's curl like tentacles. No font, nothing to license |
| 2 | Monoline with bell | `wordmark/monoline-bell-{light,dark}.svg` | The j's dot is a small dome: the jellyfish's bell |
| 3 | On the icon's tile | `wordmark/monoline{,-bell}-banner.svg` | Icon-style translucent lines on the icon's gradient, for website header, splash and social cards |
| 4 | Type | `wordmark/type-{light,dark}.svg` | "Jellyfesh" in Inter Display SemiBold, kerned with HarfBuzz and converted to outlines |
| 5 | Type with bell | `wordmark/type-bell-{light,dark}.svg` | Lowercase, with the j's dot swapped for the dome |
| 6 | Spaced capitals | `wordmark/type-caps-{light,dark}.svg` | Inter Display Medium, tracking +16% |

Lockups: `wordmark/lockup-horizontal-{light,dark}.svg` and
`wordmark/lockup-stacked-light.svg` (icon B, original colours, with option 2).

## Rules

* Inside the app, text stays in the platform's system font (Misty TYP-01).
  The wordmark is artwork: About screen, website, installers, splash.
* Inter Display is under the SIL Open Font License 1.1, which allows using
  it in artwork such as a logo; the outlines are in the SVGs, so no font
  ships with them.
* Clear space around a lockup: at least the icon's corner radius on every
  side. Minimum size: wordmark x-height 8 px; below that, use the icon alone.

## Regenerate

```bash
python3 design/brand/tools/gen_wordmark.py design/brand/wordmark design/icons/round4/original
python3 design/brand/tools/gen_hero.py design/brand/wordmark
```

On the website, hero B's tentacles may sway slowly (CSS transform, about
6 s per cycle); the animation is off under `prefers-reduced-motion`.

Options 4–6 need `fonttools` and `uharfbuzz` (Python) and Inter Display
installed; options 1–3 and the lockups need nothing extra.
