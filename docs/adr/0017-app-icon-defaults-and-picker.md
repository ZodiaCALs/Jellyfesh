# ADR-0017: App icon defaults and the icon picker

* Status: Accepted (your decision of 2026-11-05)
* Related: ADR-0016 (distribution), `design/icons/round4/`

## Context

You chose icon variant B and asked to keep all ten colour combinations as
options in Settings, with **Ice** as the default light icon and **Steel** as
the default dark icon. Each platform lets an app change its own icon in a
different, limited way.

## Decision

1. **Ten icons ship in the app:** Ice, Steel, Original, Navy Tide, Royal,
   Teal Deep, Sea Glass, Electric, Abyss, Lagoon (`design/icons/round4/`).
2. **Settings → Appearance → App icon** offers **Automatic** (default) and
   the ten. Automatic means Ice in light appearance and Steel in dark.
3. Per platform:

| Platform | Automatic | Another choice | Limits |
| --- | --- | --- | --- |
| macOS 26+ | Layered icon with Default = Ice and Dark = Steel; the system follows the appearance everywhere (Dock, Finder, Launchpad) and derives the clear and tinted looks from the layers | Dock icon set at run time (`NSApplication.applicationIconImage`) while Jellyfesh runs | Finder and Launchpad keep the bundle icon; we never modify the app bundle, because that breaks its signature |
| macOS 14–15 | Bundle icon is Ice; the Dock icon switches to Steel at run time in dark appearance | Same as above | Finder shows Ice |
| Windows | Taskbar and window icon follow the app's theme (`WM_SETICON`) | Same, and the app rewrites the icon of the Start menu and desktop shortcuts it installed | A pinned taskbar shortcut may show the old icon until Explorer refreshes |
| Android | Ice: launchers do not reliably follow dark mode for app icons | One `activity-alias` per icon, switched with `PackageManager.setComponentEnabledSetting` after a short confirmation | Some launchers take seconds to refresh or drop home-screen shortcuts; Android 13+ themed icons use the monochrome layer instead |
| Linux | Ice: the Flatpak exports one icon from the app metadata | Not offered; the picker is hidden | GNOME has no per-appearance app icons |

4. **Assets:** Ice and Steel ship as full native icon sets (macOS `.icon`
   and `.icns`, Windows `.ico` 16–256 px, Android adaptive layers with
   monochrome, Linux SVG for Ice). The other eight ship as vectors and are
   rasterised on first selection, so the download grows by kilobytes, not
   megabytes. Every option uses its simplified master at 16 and 32 px.
5. The choice is stored in settings and applied by the shell; the core only
   stores the value (shells own platform icon APIs, ADR-0001).

## Consequences

* The icon a person picks is honoured everywhere the platform allows, and
  the limits are stated next to the picker instead of failing silently.
* Two more shell adapters per platform (Dock tile, shortcut rewriting,
  activity aliases); each gets a smoke test in Milestone 4.
