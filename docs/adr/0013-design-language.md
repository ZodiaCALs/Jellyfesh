# ADR-0013: Misty design language, tokens and deviations

* Status: Proposed
* Date: 2026-11-05 (Misty 1.1, commit `4e667a5` of github.com/ZodiaCALs/Misty)

## Context

The brief says the final look follows the Misty design language, with
Folia and Cloudy as references for ideas only. I read Misty 1.1: the
standard (248 requirements, 32 clauses, Annex A platforms, Annex B
internationalisation), its token file and its licences.

What matters for Jellyfesh:

* **Licences.** Tokens, platform packages and scripts are Apache-2.0; the
  standard's text, the marks and the sample pages are CC BY-ND 4.0. We may
  derive tokens with attribution (NOTICE) and a change notice.
* **Trademark.** We may say "designed with the Misty design language". We may
  not use "Misty" in a product, package or plug-in name, may not use the Misty
  mark in the app, and may claim conformance ("Misty 1.1 Core on macOS,
  reviewed …") only if every MUST at that level is met.
* **One hue.** Every tinted colour is OKLCH from one hue; contrast is
  guaranteed at all 360 hues in both appearances (COL-10). That is a natural
  fit for an artwork-derived accent.
* **Glass.** Four grades (Clear, Mist, Veil, Cloud), one edge, one shadow,
  no simulated thickness (MAT-04); fallback to solid Cloud for reduced
  transparency, increased contrast and missing blur (MAT-08); blur, saturation
  and fill never animate (MAT-10); at most two overlapping glass layers
  (MAT-06).
* **Field.** Every view sits on the lit field (sky, aurora glows, grain)
  that MUST NOT be replaced with a photograph (MAT-02).
* **Native type.** Platform faces only, no downloaded fonts (TYP-01); CJK
  without negative tracking and with generous line height (TYP-08).
* **Motion.** Five durations, Fluid/Spring/Exit/Linear curves; motion only
  shows origin, destination or response (MOT-01); reduced motion completes
  instantly (MOT-12).
* **Annex A** maps tokens to SwiftUI, Compose and WinUI; there is no GTK
  package (Flutter covers Linux there), so we generate GTK CSS ourselves.
  Annex A.4 says Compose cannot blur what lies behind a composable and maps
  Android glass to Cloud.

## Decision

1. Misty 1.1 is the base. `design/tools/import_misty.py` turns Misty's token
   file into our single source, `design/tokens.json`, adding Jellyfesh groups
   (lyrics, artwork, backdrop, edge blur, glass budget, player). Everything
   Jellyfesh adds is `status: proposed` until you approve it.
2. Theme colours are resolved **in the core** from the artwork hue and sent
   as sRGB, so four platforms render identical colours (Misty PLT-03 asks
   for run-time resolution rather than baked colours).
3. Codegen targets Rust, Swift, Kotlin, C# (+ XAML) and GTK CSS.
4. We follow your brief where it conflicts with a Misty MUST, and record the
   deviation:

| ID | Brief | Misty | Default chosen | Effect |
| --- | --- | --- | --- | --- |
| D1 | Blurred artwork as Now Playing backdrop | MAT-02 | Now Playing only, tone-mapped for contrast; the Misty field (at the artwork hue) everywhere else | No Misty Core claim while D1 stands |
| D2 | One liquid-glass accent | MAT-04 | Scrubber thumb only, via the platform material where one exists | No Misty Core claim while D2 stands |
| D3 | Cloudy blur on Android | Annex A.4 (informative) | Live glass where capable, Cloud otherwise | Compatible: MAT-08 is still met |

**Decision (2026-11-05): build both and compare.** Appearance gets a
**Style** setting:

| Style | Now Playing backdrop | Liquid accent | Elsewhere | Misty claim |
| --- | --- | --- | --- | --- |
| Artwork (your brief) | Baked blurred artwork, tone-mapped for contrast (D1) | Scrubber thumb (D2) | Misty field at the artwork hue | None while selected |
| Misty | Misty field with its aurora at the artwork hue; artwork as hero media on a Clear frame | None (plain Veil thumb) | Same | "Misty 1.1 Core" possible |

Static mock-ups of both are on the design canvas now; the live switch ships
in Milestone 2. The default and whether the switch stays in the product are
decided after you compare them.

## Consequences

* Misty's contrast guarantees cover our UI colours; the baked backdrop needs
  its own guarantee, provided by tone-mapping (ADR-0001, `jf-palette` tests).
* We must not name anything "Misty-…" (for example the token package is
  `jf-tokens`, not `misty-tokens`).
* Our icon must not resemble the Misty mark (a gradient square with a sun
  and mist lines); see `design/icons/README.md`.
* Upgrading Misty is a re-run of the import script plus a review of the diff.
