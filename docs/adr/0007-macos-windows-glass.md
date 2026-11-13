# ADR-0007: macOS and Windows materials

* Status: Proposed
* Date: 2026-11-05 (developer.apple.com documentation; MicrosoftDocs and
  Windows App SDK release notes on GitHub; NuGet; dotnet release index)

## Context

Both platforms offer system materials that blur cheaply in the compositor,
but with a fixed blur radius. Misty's Annex A maps its grades onto them:
use the nearest material and lay the grade's tint over it (PLT-05).

**Verified, macOS:**

* macOS 26 added `glassEffect(_:in:)`, `Glass` (regular, clear, identity,
  tint, interactive), `GlassEffectContainer`, `safeAreaBar`,
  `scrollEdgeEffectStyle` (automatic, hard, soft) and
  `backgroundExtensionEffect()` in SwiftUI, and `NSGlassEffectView` in AppKit.
  There is no public variable-blur API before macOS 26.
* `Material` and `.mask(LinearGradient)` exist since macOS 12;
  `accessibilityReduceTransparency` and `accessibilityReduceMotion` since 10.15.
* `MPNowPlayingInfoCenter` and `MPRemoteCommandCenter` work with any audio
  engine; on macOS the app must set `playbackState` on every start and stop.
* Current toolchain: Xcode 27.2 with Swift 6.4 (2026-10-29).

**Verified, Windows:**

* Windows App SDK **2.6.5** (2026-11-03); 2.0 (2026-04-29) moved to semantic
  versioning and added `SystemBackdropElement`, which puts Mica or Acrylic on
  a single element.
* Mica is Windows 11 only and falls back to a solid colour on Windows 10;
  `DesktopAcrylicBackdrop` works from Windows 10 1809 and turns off in
  Battery Saver; `AcrylicBrush` blurs only in-app content.
* NativeAOT is supported for WinUI 3 since Windows App SDK 1.6.
* **.NET 10 LTS** is current; .NET 8 and 9 reach end of support on 2026-11-10.
* `ISystemMediaTransportControlsInterop::GetForWindow` is documented and is
  what Chromium uses with its own audio stack. Taskbar thumbnail buttons
  (`ITaskbarList3`, up to 7) deliver clicks as `WM_COMMAND`, so the WinUI
  window procedure has to be subclassed.
* Windows App SDK 2.6.5 reverted a backdrop change from 2.6.4, and system
  backdrops are not supported under the new System Composition Engine.

## Decision

**macOS**

| Misty grade / effect | macOS 14–15 | macOS 26+ |
| --- | --- | --- |
| Veil (bars, mini-player) | `.regularMaterial` + Veil tint | same, inside a `GlassEffectContainer` only for the mini-player |
| Mist (cards) | `.thinMaterial` + Mist tint | same |
| Clear (frames) | `.ultraThinMaterial` + Clear tint | same |
| Edge blur under chrome | Material with `.mask(LinearGradient)` | `scrollEdgeEffectStyle(.soft)` |
| Sidebar | native `NavigationSplitView` sidebar material | native (Liquid Glass sidebar) |
| Liquid accent (D2) | none (plain Veil thumb) | `glassEffect(.regular.interactive())` on the scrubber thumb only |

* Minimum **macOS 14.2** (cpal, ADR-0009; MusicKit's `ApplicationMusicPlayer`
  needs 14.0); decided 2026-11-05.
* Reduce transparency → every grade becomes Cloud; reduce motion → no
  backdrop crossfade (instant swap) and no lyric sweep.

**Windows**

| Misty grade / effect | Windows 11 | Windows 10 (1809+) |
| --- | --- | --- |
| Window | Mica (`MicaBackdrop`) | solid field colour |
| Veil (bars, mini-player) | `SystemBackdropElement` with Acrylic + Veil tint | `AcrylicBrush` (in-app) + tint |
| Mist / Clear | in-app `AcrylicBrush` + tint | same |
| Edge blur | Acrylic layer with an opacity mask (composition API) | same |
| Fallback | Transparency effects off, Battery Saver, high contrast → solid Cloud | same |

* **.NET 10** and **Windows App SDK 2.6.x**, pinned per release; NativeAOT
  for startup time (cold-start budget), verified in S8.
* Spring easing (overshoot) uses `Compositor.CreateCubicBezierEasingFunction`;
  XAML `KeySpline` cannot overshoot (Misty Annex A.6).

## Consequences

* Minimum OS versions were decided on 2026-11-05: macOS 14.2 and Windows 10 1809.
* Glass on both platforms is compositor-backed and cheap; the S5 spike
  confirms GPU frame cost on macOS 14 and 26 and on an integrated-GPU Windows
  laptop.
* Windows App SDK changes quickly; we update deliberately, not automatically.
