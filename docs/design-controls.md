# Control styles: bars and buttons

Status: **D chosen; refined in v0.0.4** (see "v0.0.4: the controls as
built", below, which wins where it differs from the options). The Step 0
options (2026-11-05) are kept for the record. Mock-ups are on the design
canvas, page "Controls · four styles"; their sources are in
[`design/canvas/`](../design/canvas/) (`Controls-A…D.dc.html`).

The brief: keep Misty consistent, make the bars and buttons more beautiful,
and draw on Microsoft, Apple and Google. Each option below keeps Misty's
colours, glass grades, type, radii, motion tokens and accessibility rules,
and changes only how the bars and buttons are shaped and behave.

## The options

| Part | A · Glass capsules (after Apple) | B · Fluent (after Microsoft) | C · Expressive (after Google) | D · Blend (recommended) |
| --- | --- | --- | --- | --- |
| Top bar | No bar; floating glass circle (back) and a glass capsule of icon buttons | Bar attached to the window edge; tabs with a short accent underline; labelled "Speakers" button | Large back button, left-aligned title, tonal "Lyrics" pill, icon buttons | Veil bar with Misty's segmented control and a "Speakers" chip |
| Seek bar | 6 px, no thumb; thickens to 12 px with a time bubble while dragging | 4 px with times at both ends; ringed thumb whose centre grows on hover and shrinks on press | Wavy line for the played part, a vertical handle with gaps, a stop dot at the end | Times at both ends, a calm wave for the played part, a ringed thumb; thickens while dragging |
| Volume | Thin bar between two speaker glyphs | Mute button, ringed-thumb slider, value | Thick 16 px track with the speaker glyph inside and a vertical handle | Thin bar between two speaker glyphs |
| Transport | Glyph-only; large pause glyph; filled previous/next glyphs | Quiet square buttons; accent circle for play/pause | Separate tonal pills; play is a rounded square when paused and a wide pill when playing | Previous, play and next grouped in one glass capsule; accent circle play |
| Toggles (shuffle, repeat) | Accent glyph with a small dot under it | Accent-soft square with accent glyph | Accent-soft circle with accent glyph | Accent-soft circle with accent glyph |
| Play button feedback | Press scales to 92% | Hover lightens, press darkens and scales to 96% | Shape change on state; press scales to 96% | Hover adds a soft halo; press scales to 92% |
| Mini-player | Floating glass capsule; progress line along the bottom | Bar attached to the bottom edge with progress along its top | Solid capsule; progress as a ring around the play button | Floating glass capsule; progress ring around the play button |

A fifth way is **native flavours**: A on macOS, B on Windows, C on Android
and D on Linux, all on the same tokens. It fits each platform's conventions
best but makes Jellyfesh look slightly different on each.

## Misty rules each option respects (and the one it bends)

| Rule | How the options handle it |
| --- | --- |
| COL-04: at most one accent-filled action per view | Play/pause is the only accent fill on Now Playing; the mini-player's play button is never accent-filled, so a Library screen keeps its own primary action |
| COL-05, COL-06: accent text for glyphs; accent-soft only behind accent glyphs | Toggles use accent-soft with accent-text glyphs |
| ICO-01, ICO-03: 1.75-unit strokes; outline at rest, filled only for active | Pause is filled because media is playing. **A's filled previous/next glyphs bend ICO-03**; keeping them outlined removes the conflict |
| SHP-01 to SHP-04: radii from the scale, one radius per shape, nesting, no squircles | C uses separate pills, not Material's joined segments with mixed corners, and its shape change moves only between scale radii (16 px and full) |
| MAT-05, MAT-06, MAT-10: floating layers in Veil; no glass on glass beyond two; no animated blur | Capsules and bars are Veil; nothing animates blur or opacity of glass |
| MOT-05, MOT-06, MOT-11, MOT-12 | Fluid for movement, Spring only for small presses and the play button's shape change, Linear for progress; the wave is a loop allowed only while media plays, and it stops when paused, in reduced motion and in Still mode |
| ACC-10, ACC-12, PLT-09 | Visible focus ring; 44 pt targets (48 dp on Android) |
| CMP-02: a player is art, two lines of type, one progress bar, one control | Every mini-player has exactly one control and one progress indicator |

## Mapping to native controls (Misty PLT-10)

| Option | macOS (SwiftUI) | Windows (WinUI 3) | Linux (GTK 4) | Android (Compose) |
| --- | --- | --- | --- | --- |
| A | `GlassEffectContainer` + `glassEffect` on 26, materials on 14–15 | Acrylic capsules, custom slider template | `backdrop-filter` capsules | Cloudy `GlassSurface` capsules |
| B | Custom `Slider` style | `Slider` restyled (thumb ring), `SelectorBar` | Custom `GtkScale` CSS | Custom slider |
| C | Custom shapes and slider | Custom templates | Custom widgets | Material 3 `Slider`, button groups and wavy progress, restyled with Misty tokens |
| D | Mix of A and B | Mix of B and A | Custom `GtkScale` + capsule | Mix of C and A |

The choice becomes a `controls.style` token group once you pick.

## v0.0.4: the controls as built

Feedback on v0.0.3: the progress bar should be a normal straight line, the
design should draw more on Apple Music and Windows Media Player, the motion
should feel better, and Preferences should offer more customization. Misty
stays: its colours, glass grades, type, radii, motion tokens and
accessibility rules are unchanged. Sizes below are tokens
(`design/tokens.json`); every app follows this section.

### Seek bar and sliders: a straight line (replaces D's wave)

* **Track:** a straight line with round caps, `player.scrubber-track` (4 px)
  on pointer screens, `player.scrubber-track-touch` (6 px) on touch screens.
  Played part `ink-90`; the rest `ink-30` on glass or a backdrop
  (`line-strong` on plain surfaces). The accent stays on the play button
  (COL-04). On streams, the buffered part may show as `ink-50`.
* **Times** at both ends under the line: elapsed on the start side,
  remaining (`−1:23`) on the end side, 12 px tabular figures, `ink-70`.
* **Thumb:** none at rest (after Apple Music). Pointer screens: hovering or
  focusing the bar thickens it to `player.scrubber-track-hover` (6 px) and
  shows a round `player.scrubber-thumb` (14 px) thumb, `ink-90` with a
  `glass-edge` ring and `shadow-1` (after Media Player); dragging thickens
  it to `player.scrubber-track-drag` (8 px) and the times follow the thumb.
  Touch screens: no thumb; dragging thickens the line to
  `player.scrubber-track-touch-drag` (12 px). Changes of thickness and the
  thumb's appearance use `dur-1` Fluid; progress itself is Linear (MOT-06).
* **No loop animation:** the bar redraws only when its fill moves by at
  least one device pixel (the drawing-time `DisplayClock`), and never while
  paused or hidden.
* **Keyboard and screen readers:** a slider with Left/Right ±5 s,
  PageUp/PageDown ±30 s, Home/End; its value reads "1:23 of 3:45".
* **Volume** uses the same line between two speaker glyphs, with the same
  hover thumb.

### Mini-player and the desktop player bar

* **Phones and narrow windows** (under 900 px): the floating Veil capsule
  stays (art, two lines of type, one play button: CMP-02), and its progress
  becomes a straight `player.mini-progress` (2 px) line along the capsule's
  bottom edge, inset by `player.mini-radius`, instead of the ring.
* **Windows 900 px wide and wider:** the capsule grows into a **player bar**
  after Media Player and Apple Music, a Veil bar `player.bar-height` (80 px)
  high, floating at the bottom of the content column with `r-xl` corners:
  art (48 px) and two lines of type on the start side (they open Now
  Playing); in the middle, the transport capsule (shuffle, previous, play,
  next, repeat; play is the only accent fill) above the seek bar with its
  times; on the end side, Lyrics, Queue and the volume slider. This bends
  CMP-02 (one control) on purpose, for desktop use; phones keep CMP-02.

### Now Playing (after Apple Music)

* **Artwork:** `artwork.radius-hero`, `shadow-3`; it scales to
  `player.artwork-paused-scale` (0.88) when paused and springs back to 1
  when playing (`dur-3` Spring).
* **Backdrop:** the baked backdrop crossfades to the next song's
  (`backdrop.crossfade`, `dur-4` Fluid); nothing animates blur (MAT-10).
* **Lyrics:** the active line sits at `lyrics.anchor-position`; when it
  changes, the lines move up with `dur-3` Fluid and each following line
  starts `lyrics.stagger` (30 ms) after the one above, so they settle in a
  cascade. The active line is `ink-90` at full size; the others are
  `lyrics.ink-inactive` (sung ones `ink-30`) at `lyrics.scale-inactive` (0.96), scaled
  from their start edge. The word sweep is unchanged (Linear). Sizes follow
  the Lyrics size setting (`lyrics.scale-small`, 1, `lyrics.scale-large`)
  and the alignment setting (start or centre). Tapping a line seeks to it.
* **Transport:** play and pause morph into each other (`dur-2` Spring);
  presses scale to 92 %; hover adds the soft halo (D).

### Library (after Apple Music and Media Player)

* **Large titles:** each page starts with its title at `type.text-h2`
  (about 30 px, bold; `text-h1` on wide windows), which scrolls with the
  content.
* **Album grid:** covers at `artwork.radius-grid`; hovering lifts a cover by
  2 px with `shadow-2` and shows a 36 px glass play button in its bottom
  start corner that plays the album; presses scale to 97 %.
* **Album page:** large artwork (240 px on desktop, full width on phones),
  the title (30 px bold), the artist in accent text (opens the artist),
  genre, year and song count, then **Play** (accent fill) and **Shuffle**
  (glass); track numbers turn into a play glyph on hover; the playing row
  shows a three-bar level glyph that moves only while playing (still in
  reduced motion); a footer gives "12 songs, 48 minutes".
* **Rows:** hover `fill`, selected or playing `accent-soft` with accent text.

### Motion

All within Misty's tokens: pages fade in and rise 8 px (`dur-2` Fluid);
going back fades (`dur-1`); Now Playing opens from the mini-player or bar
(the artwork grows from the mini-player's where the toolkit allows, else a
slide and fade, `player.open-now-playing`) and closes with Exit
(`player.close-now-playing`); sheets and popovers scale from 96 % and fade
(`dur-2`); theme colours crossfade (`dur-4`). With reduced motion (the
system's or **Reduce motion and transparency**), everything becomes a short
fade, the artwork does not scale, the level glyph stands still and lyrics
jump.

### Preferences → Appearance (more customization)

Grouped cards after Windows Settings and macOS System Settings; every
change applies at once and the core keeps it (`SetSettings`):

| Setting | Control | Core field |
| --- | --- | --- |
| Theme | System · Light · Dark, a segmented control with small previews | `appearance` |
| Accent colour | "Follow the artwork" switch; when off, ten swatches (hues 250, 210, 180, 150, 120, 60, 30, 0, 330, 290) and a Custom hue slider (0–360 on a hue-gradient track) with a preview | `hue_source` (`Artwork` or `Fixed { hue }`) |
| Transparency | Slider from Solid to Clear (0–100 %, default 50 %) with a live preview | `transparency` (0–1) |
| Reduce motion and transparency | Switch | `reduce_effects` |
| Now Playing background | Artwork · Misty, with previews | `style` |
| Lyrics size | Small · Medium · Large | `lyrics_size` |
| Lyrics alignment | Start · Centre | `lyrics_align` |
| Translation, Romanization | Switches | `show_translation`, `show_romanization` |
| App icon | The existing picker | `app_icon` |

**Transparency** scales the alpha of the glass grades (Clear, Mist, Veil):
`glass_alpha(alpha, t)` in `jf-palette` (contract vectors in
`tests/contract/glass-alpha.json`). At 50 % the tokens' own alphas apply;
toward Solid the glass becomes opaque (at 0 % it is the solid Cloud look);
toward Clear the alphas halve. Apps whose platform materials have no alpha
(SwiftUI materials, Acrylic) map it to the nearest material thickness or
tint opacity. Reduce motion and transparency still makes glass solid.
