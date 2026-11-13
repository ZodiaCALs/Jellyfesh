# ADR-0006: Linux glass with GTK render nodes

* Status: Proposed
* Date: 2026-11-05 (GTK and libadwaita NEWS and sources, Ubuntu package
  pages, crates.io)

## Context

The brief asks to verify "GTK blur render nodes". Until recently GTK could
blur only a widget's own content (`gtk_snapshot_push_blur`), not what lies
behind it.

**Verified:**

* **GTK 4.24.1** (2026-10-01) and **libadwaita 1.10.0** (2026-09-14) are current.
* **GTK 4.22 added CSS `backdrop-filter`**, built on new render nodes that
  can read what is behind: `GtkSnapshot` gained `push_copy`, `append_paste`
  and `push_composite`. Backdrop-filter bugs were still being fixed in 4.21.3,
  so the code is young.
* GTK CSS has **no mask properties**, so a progressive (gradient) blur needs
  a custom widget: copy the background, blur it, then mask it with a gradient
  (`push_mask`, available since 4.10).
* **GTK 4.24 requests compositor blur behind the window by itself** through
  the staging Wayland protocol `ext-background-effect-v1`, working out the
  region from what the window draws. Reported compositor support: KWin
  (Plasma 6.7), Mutter (GNOME 51), niri 26.04; Hyprland merged it; not found
  for sway/wlroots or COSMIC (secondary sources).
* Vulkan is GTK's default renderer on Wayland since 4.16.
* Distribution versions: Ubuntu 24.04 ships GTK 4.14 and libadwaita 1.5;
  Ubuntu 26.04 ships GTK 4.22 and libadwaita 1.9; Debian 13 ships GTK 4.18
  and libadwaita 1.7. GNOME 51 (with GTK 4.24) was released 2026-09-16.
* Rust crates: `gtk4` **0.11.5** (feature `v4_24`, binds the 4.22 snapshot
  functions, Rust ≥ 1.92); `libadwaita` **0.9.2** (feature `v1_10`).
* libadwaita: `AdwNavigationSplitView`, `AdwBreakpoint` (1.4),
  `AdwMultiLayoutView` and `AdwStyleManager:accent-color` (1.6).

## Decision

1. **Ship as a Flatpak on the GNOME 51 runtime** (GTK 4.24, libadwaita 1.10).
   This is the only way to get the same GTK on every distribution. Native
   distro packages are best-effort and need GTK ≥ 4.22 and libadwaita ≥ 1.6.
2. **In-app glass** (Veil top bar, mini-player) uses CSS `backdrop-filter`
   on 4.22+, styled from the generated token CSS plus the run-time theme.
3. **Progressive edge blur** is a small custom widget (`JfEdgeBlur`) that
   snapshots `copy → blur → mask(gradient) → paste` over the edge band.
   Spike S7 measures its GPU cost on Intel integrated graphics with the
   Vulkan and GL renderers; budget 2 ms per frame at 60 Hz for both edges.
4. **Window-behind blur is off by default.** The main window is opaque; we
   do not depend on compositor blur, which is not portable. GTK 4.24 will
   request it automatically for any translucent area, so we draw opaque
   backgrounds except where we deliberately opt in later.
5. **Fallback to solid Cloud** when the person turns on high contrast or
   reduced animations, when GTK falls back to the Cairo renderer, or when
   the GTK version is older than 4.22 (distro builds).
6. libadwaita supplies the adaptive layout and follows the system's dark
   style and accent colour. The accent colour seeds our hue only when the
   person chooses "system accent" in Appearance; otherwise the artwork hue
   wins (ADR-0013).

## Consequences

* Linux is the only platform whose glass depends on a toolkit feature that
  is barely a year old; S7 can still downgrade the edge blur to a gradient
  scrim without touching the rest of the design.
* MPRIS (zbus 6 via `mpris-server` 0.11) lives in the Linux shell crate.
* X11 sessions still work, but compositor blur does not apply there.
  Plasma 6.8 reportedly no longer ships an X11 session.

## Verification sources

GTK NEWS (gtk-4-24 branch), `gtk/gtksnapshot.c`,
`gtk/gtkcssstylepropertyimpl.c`, `gdk/wayland/gdksurface-wayland.c`;
libadwaita NEWS and tags; packages.ubuntu.com (noble, resolute);
crates.io `gtk4`, `libadwaita`. Compositor support for
`ext-background-effect-v1` is from secondary reports and must be
re-checked in S7.
