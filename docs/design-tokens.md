# Design tokens: proposed defaults

Status: **proposed (Step 0)**. Your token list was blank, so every value
below is a default I propose. Items marked **⚑** need your decision; the rest
follow Misty 1.1 unchanged.

Source of truth: [`design/tokens.json`](../design/tokens.json), built by
[`design/tools/import_misty.py`](../design/tools/import_misty.py) from the
Misty 1.1 token file (Apache-2.0; attribution in [`NOTICE`](../NOTICE)) plus
the Jellyfesh groups `brand`, `type-roles`, `lyrics`, `artwork`, `backdrop`,
`edge-blur`, `glass-budget` and `player`.

## How colour works

Misty defines every theme colour in OKLCH from **one hue**. That fits
"accent derived from artwork" well: `jf-palette` finds one hue in the
artwork, and Misty derives the field, ink, accent, lines and glass from it.
Misty's contrast table holds every text pair at WCAG AA or better at all 360
hues in both appearances, so a hue from any artwork is safe. The core
resolves the colours (CSS Color 4 OKLab matrices, per-channel clipping, the
same conversion as the Misty packages) and sends sRGB to every UI.

## Your categories

| Category | Proposed default | |
| --- | --- | --- |
| Mood keywords | calm, precise, luminous, unhurried, tidal | ⚑ |
| Style | Two styles built for comparison: **Artwork** (blurred-artwork backdrop, one liquid accent) and **Misty** (conformant field at the artwork hue) | Decided 2026-11-05: build both |
| Light neutrals (hue 250) | field `sky` #eef3f9, `sky-high` #f7fafe; surfaces `cloud` #f9fbfe, `cloud-soft` #f2f6fb; ink #101820 at 92/72/62/35/10/6% | Misty |
| Dark neutrals (hue 250) | field `sky` #0a1016, `sky-high` #12181e; surfaces `cloud` #191f25, `cloud-soft` #1f262e; ink #f6f9fc at 92/72/55/35/10/6% | Misty |
| Accent | Hue from artwork. Light accent L 0.46 C 0.15 (#0059a7 at 250); dark L 0.78 C 0.12 (#7abdff at 250). Fallback hue **250** (Misty “Mist”) when artwork is missing or its accent chroma is under 0.04 | ⚑ fallback hue |
| Typefaces | Native interface faces: SF Pro (macOS), Segoe UI Variable (Windows), Adwaita Sans or Cantarell per GNOME version (Linux), Roboto (Android). CJK: PingFang / Hiragino / Apple SD Gothic Neo; Microsoft YaHei UI / JhengHei UI / Yu Gothic UI / Malgun Gothic; Noto Sans CJK. No bundled fonts (Misty TYP-01) | Misty |
| Interface type scale | Display 56/0.98/700 · Title 28/1.1/700 · Heading 20/1.2/700 · Body 16/1.7/400 · Small 14/1.6/400 · Label 13/1.4/600 · Caption 12/1.45/600 · Micro 11/1.4/700 | Misty |
| Player type roles | Now Playing title 28/700; artist 16/600; list title 14/1.3/500; list secondary 12/1.3/400 | ⚑ list leading tightened from Misty's 1.6 for 52 pt rows |
| Lyrics type | Line 30 pt (fluid 24 to 40 by window width), 1.3, 700; CJK line height 1.6; translation 18/500; romanization 15/500 | ⚑ |
| Corner radii | 4 · 6 · 8 · 12 · 16 · 24 · full. List art 6, grid covers 12, Now Playing art 24, mini-player 16, scrubber full | Misty + ⚑ mapping |
| Spacing | 4 px base: 4 · 8 · 12 · 16 · 20 · 24 · 32 · 40 · 48 · 64 · 80 · 96 | Misty |
| Glass: blur radii | Clear 24 · Mist 26 · Veil 32 px (CSS σ); compact windows 14 · 18 · 22 | Misty |
| Glass: tint alpha | Clear 30% · Mist 42% · Veil 62% light; 36 · 50 · 68% dark; dialog 86%; saturation boost 175% | Misty |
| Glass: border/highlight | One 1 px inset edge, white at 55% light / 10% dark (soft: 30% / 5%); one shadow; no gradients, glows or bevels | Misty |
| Motion: durations | 80 · 160 · 240 · 380 · 600 ms | Misty |
| Motion: easing | Fluid (0.22, 1, 0.36, 1) for entering and moving; Spring (0.34, 1.36, 0.64, 1) for small objects only; Exit (0.4, 0, 1, 1); Linear for progress and loops only | Misty |

## Jellyfesh additions (all ⚑)

| Token group | Proposal |
| --- | --- |
| Backdrop bake | Downsample to 1/4 (long edge ≤ 256 px), Gaussian σ ≈ 8.5% of the short edge, chroma ×1.2, OKLab lightness clamped to 0.16–0.34 (dark) or 0.86–0.95 (light), 1-step dither. Crossfade 600 ms Fluid on track change |
| Edge blur | Top 56 pt under chrome, bottom 48 pt above the mini-player; Veil blur (32) and tint (62%) at the edge, falling to 0 on a smoothstep ramp |
| Glass budget | ≤ 3 live blur layers per screen on desktop, ≤ 2 on Android |
| Lyrics motion | Line advance 380 ms Fluid, active line at 38% of the viewport; word sweep is progress, so Linear. Inactive lines change colour only (ink-50; unsung words in the active line ink-70): no blur, no scale |
| Player | Mini-player 64 pt (72 dp Android), track rows 52 pt, scrubber track 4, thumb 14 with a 44 pt hit area; Now Playing opens with Misty's Deeper (320 ms), closes with Back (300 ms) |

## Where your brief and Misty disagree

Misty is strict, and three of your requirements break a Misty MUST. Decision
(2026-11-05): build both styles and compare them (ADR-0013); the
deviations below apply only to the **Artwork** style. Because they are MUSTs, Jellyfesh could not claim "Misty 1.1
Core" conformance while they stand. We can still say, as a factual
reference, "designed with the Misty design language".

| Your requirement | Misty rule | Proposed resolution |
| --- | --- | --- |
| Now Playing backdrop is the blurred artwork | MAT-02: the field MUST NOT be replaced with a photograph | Keep the blurred artwork on Now Playing only, tone-mapped into a contrast-safe band. Every other screen uses the Misty field at the artwork hue. Alternative that stays conformant: a Misty field whose aurora takes the artwork hue, with the artwork itself only as hero media |
| One liquid-glass accent (scrubber thumb or mini-player) | MAT-04: glass MUST NOT simulate thickness | Use it only on the scrubber thumb, and only through the platform's own material where one exists (macOS 26 `glassEffect`); elsewhere a plain Veil thumb. Or drop it and stay conformant |
| Cloudy blur on Android | Annex A.4 maps Android glass to Cloud (Compose cannot blur what lies behind) | Use Cloudy behind `GlassSurface` where the device is capable; fall back to Cloud everywhere else. A library that re-renders the backdrop makes this possible, so it is an enhancement, not a MUST breach |

## Codegen (Milestone 1 builds it)

`xtask tokens` reads `design/tokens.json` and writes:

| Target | Output | Notes |
| --- | --- | --- |
| Rust | `core/jf-tokens/src/generated.rs` | Static tokens as `const`; theme colour definitions as tables `jf-palette` resolves at run time |
| Swift | `platforms/macos/Generated/Tokens.swift` | `CGFloat` metrics, `Animation.timingCurve` curves |
| Kotlin | `platforms/android/.../generated/Tokens.kt` | `Dp`, `CubicBezierEasing` |
| C# | `platforms/windows/Generated/Tokens.cs` + `Tokens.xaml` | XAML resource dictionary; Spring needs the composition API (`KeySpline` cannot overshoot) |
| GTK CSS | `platforms/linux/data/tokens.css` | Static metrics as CSS; colours set at run time from the theme snapshot |

Theme colours never ship baked for one hue (Misty PLT-03); every platform
receives them from the core's theme snapshot, with a generated hue-250
snapshot as the first-frame fallback.
