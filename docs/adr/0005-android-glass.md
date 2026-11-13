# ADR-0005: Android glass with Cloudy behind `GlassSurface`

* Status: Proposed
* Date: 2026-11-05 (facts from the Cloudy and Haze repositories, Maven
  Central and developer.android.com that day)

## Context

The brief asks for Cloudy, wrapped in our own `GlassSurface` so it can be
swapped, with a pinned version because the newer backdrop API may be alpha.
Misty's Annex A.4 notes that Compose can only blur a composable's own
content, so plain Compose cannot do backdrop glass at all.

**Verified:**

* **Cloudy** (skydoves/Cloudy), **Apache-2.0**, minSdk 23. Latest stable
  **0.7.1** (2026-07-12); pre-release **1.0.0-alpha01** (2026-07-17).
  * Self blur: `Modifier.cloudy(radius)`.
  * Backdrop blur: `Modifier.sky(rememberSky())` on the content behind, and
    `Modifier.cloudy(sky = …, tint, shape, progressive = …, cpuBlurEnabled)` on
    the glass. Progressive directions: `TopToBottom`, `BottomToTop`, `Edges`.
  * `Modifier.liquidGlass()`; refraction needs API 33.
  * Marked experimental: Mirage (`@ExperimentalMirage`, "excluded from the
    stable ABI"), motion-driven lights, the backdrop `light` parameter,
    `liquidGlassTuned`.
  * Below API 31 backdrop blur draws a scrim unless `cpuBlurEnabled = true`;
    progressive blur needs API 33 (uniform on 31–32).
  * The backdrop re-captures only while content scrolls or animates; after
    an async change such as an artwork load, call `Sky.invalidate()`.
  * 0.6.0 → 0.7.1 fixed an ANR and render-thread crashes in the blur path.
* **Haze** (chrisbanes/haze), **Apache-2.0**, API 23+, **2.1.0**
  (2026-11-02): backdrop input, `HazeProgressive`, masks, tint, fallback
  colour; glass module stable in 2.1.0. Scrim by default at API ≤ 30;
  progressive blur approximated on 31–32. A true window-backdrop path exists
  only on Android 17 QPR2, behind a flag, at 23–46% higher P90 CPU frame time.
* Compose BOM **2026.09.00** (Compose 1.12.1).

## Decision

1. **Pin Cloudy 0.7.1** (stable). Do not use 1.0.0-alpha01 or any
   `@Experimental…` API. Renovate opens upgrade PRs; we move only to stable
   releases after S9 re-runs.
2. **`GlassSurface` is our only entry point.** Screens never import Cloudy.
   `GlassSurface(grade = Veil | Mist | Clear, progressive = None | Top | Bottom)`
   picks one of three implementations at run time:

   | Device | Implementation |
   | --- | --- |
   | API ≥ 33, not low-RAM, not battery saver, transparency allowed | Cloudy backdrop blur, progressive where asked, Misty tint and edge |
   | API 31–32 | Cloudy uniform blur; progressive edges become a gradient scrim |
   | API < 31, low-RAM, battery saver, or reduce effects on | Solid Misty Cloud (MAT-08); no CPU blur |

3. `GlassSurface` calls `Sky.invalidate()` when the theme snapshot or
   artwork changes, so the backdrop re-captures once per track and not per frame.
4. **No liquid glass on Android** in v1: refraction needs API 33 and the
   API is new. The one liquid accent (ADR-0013 D2) is macOS-only unless S9
   shows it is cheap.
5. **Haze 2.x is the swap target.** S9 runs the same screens with Cloudy and
   Haze behind `GlassSurface` on a low-end and a mid-range device and
   records frame time, jank and battery; we swap if Haze wins clearly.
6. Minimum SDK: **26** (cpal's AAudio backend, ADR-0009; decided 2026-11-05). Glass
   from 31, progressive from 33.

## Consequences

* Devices on API 26–30 get solid Cloud surfaces. That is Misty's own
  Android default, so the design still holds.
* A library swap touches one file.
* Now Playing's backdrop is a baked image (ADR-0001), so the heaviest
  visual costs nothing per frame on any device.

## Verification sources

github.com/skydoves/Cloudy (README, releases, `buildSrc`), Maven Central
`com.github.skydoves:cloudy`; github.com/chrisbanes/haze (CHANGELOG,
`docs/blur/platforms.md`, `docs/performance.md`); developer.android.com
Compose BOM mapping.
