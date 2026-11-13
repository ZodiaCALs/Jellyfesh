#!/usr/bin/env python3
"""Generate the Jellyfesh app-icon concept layers (Step 0, for approval).

Each concept is drawn on a 1024 x 1024 canvas as three layers, the way
Android adaptive icons and macOS 26 Icon Composer expect them:

  background.svg  full bleed, no transparency
  foreground.svg  the glyph, kept inside a circle of radius 310 around the
                  centre so it survives every platform mask
  monochrome.svg  the glyph in one colour, for Android themed icons, GNOME
                  symbolic icons and the Windows tray

plus full.svg (background + foreground, unmasked) for quick viewing.

Colours are OKLCH values resolved to sRGB with the CSS Color 4 OKLab
matrices and per-channel clipping, the same conversion the Misty tokens
use, so icon colours and UI colours agree.

Usage: python3 gen_concepts.py <output-dir>
"""
import math
import pathlib
import sys

SIZE = 1024
C = SIZE / 2


# --------------------------------------------------------------------------
# Colour
# --------------------------------------------------------------------------
def oklch(l, c, h):
    """OKLCH -> '#rrggbb' (CSS Color 4 OKLab matrices, clipped per channel)."""
    a = c * math.cos(math.radians(h % 360))
    b = c * math.sin(math.radians(h % 360))
    l_ = l + 0.3963377774 * a + 0.2158037573 * b
    m_ = l - 0.1055613458 * a - 0.0638541728 * b
    s_ = l - 0.0894841775 * a - 1.2914855480 * b
    lc, mc, sc = l_ ** 3, m_ ** 3, s_ ** 3
    lin = (
        4.0767416621 * lc - 3.3077115913 * mc + 0.2309699292 * sc,
        -1.2684380046 * lc + 2.6097574011 * mc - 0.3413193965 * sc,
        -0.0041960863 * lc - 0.7034186147 * mc + 1.7076147010 * sc,
    )

    def enc(v):
        v = min(1.0, max(0.0, v))
        return 12.92 * v if v <= 0.0031308 else 1.055 * v ** (1 / 2.4) - 0.055

    return "#" + "".join(f"{int(math.floor(enc(v) * 255 + 0.5)):02x}" for v in lin)


# --------------------------------------------------------------------------
# SVG helpers
# --------------------------------------------------------------------------
def svg(body, defs="", title=""):
    t = f"<title>{title}</title>" if title else ""
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}" '
        f'width="{SIZE}" height="{SIZE}">{t}<defs>{defs}</defs>{body}</svg>\n'
    )


def radial(id_, cx, cy, r, color, opacity):
    return (
        f'<radialGradient id="{id_}" cx="{cx}" cy="{cy}" r="{r}" gradientUnits="userSpaceOnUse">'
        f'<stop offset="0" stop-color="{color}" stop-opacity="{opacity}"/>'
        f'<stop offset="0.55" stop-color="{color}" stop-opacity="{opacity * 0.42:.3f}"/>'
        f'<stop offset="1" stop-color="{color}" stop-opacity="0"/></radialGradient>'
    )


def vgrad(id_, y0, y1, stops):
    s = "".join(
        f'<stop offset="{o}" stop-color="{c}" stop-opacity="{a}"/>' for o, c, a in stops
    )
    return (
        f'<linearGradient id="{id_}" x1="0" y1="{y0}" x2="0" y2="{y1}" '
        f'gradientUnits="userSpaceOnUse">{s}</linearGradient>'
    )


def grain(id_, opacity):
    """Misty's fractal grain, very faint; invisible at small sizes."""
    return (
        f'<filter id="{id_}" x="0" y="0" width="100%" height="100%">'
        f'<feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" seed="7" result="n"/>'
        f'<feColorMatrix in="n" type="matrix" values="0 0 0 0 0.5  0 0 0 0 0.5  0 0 0 0 0.5  0 0 0 {opacity} 0"/>'
        f"</filter>"
    )


def field(prefix, base_top, base_bottom, glows, grain_opacity):
    """A Misty-style lit field: base gradient, soft radial glows, grain."""
    defs = vgrad(f"{prefix}base", 0, SIZE, [(0, base_top, 1), (1, base_bottom, 1)])
    body = f'<rect width="{SIZE}" height="{SIZE}" fill="url(#{prefix}base)"/>'
    for i, (cx, cy, r, color, op) in enumerate(glows):
        defs += radial(f"{prefix}g{i}", cx, cy, r, color, op)
        body += f'<rect width="{SIZE}" height="{SIZE}" fill="url(#{prefix}g{i})"/>'
    if grain_opacity:
        defs += grain(f"{prefix}grain", grain_opacity)
        body += f'<rect width="{SIZE}" height="{SIZE}" filter="url(#{prefix}grain)"/>'
    return defs, body


def dome(x0, x1, top, hem, scallops=4, dip=24, shoulder=0.30):
    """Jellyfish bell: a dome from x0 to x1, scalloped hem drawn right-to-left."""
    w = x1 - x0
    mid = (x0 + x1) / 2
    d = (
        f"M{x0:.1f},{hem:.1f} "
        f"C{x0:.1f},{hem - (hem - top) * 0.62:.1f} {mid - w * shoulder:.1f},{top:.1f} {mid:.1f},{top:.1f} "
        f"C{mid + w * shoulder:.1f},{top:.1f} {x1:.1f},{hem - (hem - top) * 0.62:.1f} {x1:.1f},{hem:.1f} "
    )
    step = w / scallops
    for i in range(scallops):
        xa = x1 - step * i
        xb = xa - step
        d += f"Q{(xa + xb) / 2:.1f},{hem + dip:.1f} {xb:.1f},{hem:.1f} "
    return d + "Z"


def dome_open(x0, x1, top, hem, shoulder=0.30):
    """Dome outline only (no hem), for monoline drawings."""
    w = x1 - x0
    mid = (x0 + x1) / 2
    return (
        f"M{x0:.1f},{hem:.1f} "
        f"C{x0:.1f},{hem - (hem - top) * 0.62:.1f} {mid - w * shoulder:.1f},{top:.1f} {mid:.1f},{top:.1f} "
        f"C{mid + w * shoulder:.1f},{top:.1f} {x1:.1f},{hem - (hem - top) * 0.62:.1f} {x1:.1f},{hem:.1f}"
    )


def strand(x, y, length, amp, drift=0.0):
    """A drifting tentacle: an S-curve whose sway grows towards the tip."""
    return (
        f"M{x:.1f},{y:.1f} "
        f"C{x + amp * 0.55 + drift * 0.2:.1f},{y + length * 0.34:.1f} "
        f"{x - amp + drift * 0.6:.1f},{y + length * 0.68:.1f} "
        f"{x + amp * 0.35 + drift:.1f},{y + length:.1f}"
    )


def cubic_pts(p0, p1, p2, p3, n=48):
    pts = []
    for i in range(n + 1):
        t = i / n
        mt = 1 - t
        x = mt**3 * p0[0] + 3 * mt**2 * t * p1[0] + 3 * mt * t**2 * p2[0] + t**3 * p3[0]
        y = mt**3 * p0[1] + 3 * mt**2 * t * p1[1] + 3 * mt * t**2 * p2[1] + t**3 * p3[1]
        dx = 3 * mt**2 * (p1[0] - p0[0]) + 6 * mt * t * (p2[0] - p1[0]) + 3 * t**2 * (p3[0] - p2[0])
        dy = 3 * mt**2 * (p1[1] - p0[1]) + 6 * mt * t * (p2[1] - p1[1]) + 3 * t**2 * (p3[1] - p2[1])
        pts.append((x, y, dx, dy, t))
    return pts


def taper(p0, p1, p2, p3, w0, w1, ease=0.8):
    """A filled path that follows a cubic Bezier and narrows from w0 to w1."""
    left, right = [], []
    for x, y, dx, dy, t in cubic_pts(p0, p1, p2, p3):
        ln = math.hypot(dx, dy) or 1.0
        nx, ny = -dy / ln, dx / ln
        w = (w0 + (w1 - w0) * t**ease) / 2
        left.append((x + nx * w, y + ny * w))
        right.append((x - nx * w, y - ny * w))
    tip = right[-1]
    d = f"M{left[0][0]:.1f},{left[0][1]:.1f} " + " ".join(f"L{a:.1f},{b:.1f}" for a, b in left[1:])
    # round the tip with a small arc
    r = max(w1 / 2, 0.5)
    d += f" A{r:.1f},{r:.1f} 0 0 1 {tip[0]:.1f},{tip[1]:.1f} "
    d += " ".join(f"L{a:.1f},{b:.1f}" for a, b in reversed(right[:-1])) + " Z"
    return d


def tentacle(x, y, length, amp, drift, w0, w1):
    """Tapered version of strand()."""
    return taper(
        (x, y),
        (x + amp * 0.55 + drift * 0.2, y + length * 0.34),
        (x - amp + drift * 0.6, y + length * 0.68),
        (x + amp * 0.35 + drift, y + length),
        w0,
        w1,
    )


# --------------------------------------------------------------------------
# Concepts
# --------------------------------------------------------------------------
CONCEPTS = []


def concept(fn):
    CONCEPTS.append(fn)
    return fn


def blur_filter(id_, std, pad="40%"):
    return (
        f'<filter id="{id_}" x="-{pad}" y="-{pad}" width="{100 + 2 * int(pad[:-1])}%" '
        f'height="{100 + 2 * int(pad[:-1])}%"><feGaussianBlur stdDeviation="{std}"/></filter>'
    )


@concept
def lumen():
    """Glass bell over Misty's light field; tentacles fall like a soft waveform."""
    hue = 250
    bg_defs, bg = field(
        "lb",
        oklch(0.985, 0.006, hue),
        oklch(0.955, 0.012, hue),
        [
            (330, 150, 640, oklch(0.80, 0.12, hue), 0.95),
            (930, 160, 420, oklch(0.845, 0.105, hue + 20), 0.5),
            (910, 900, 660, oklch(0.885, 0.08, hue + 55), 0.95),
            (90, 960, 540, oklch(0.915, 0.052, hue - 70), 0.9),
        ],
        0.05,
    )
    bell = dome(296, 728, 226, 486, scallops=4, dip=26)
    ink = oklch(0.42, 0.15, hue)
    violet = oklch(0.62, 0.12, hue + 55)
    xs = [356, 434, 512, 590, 668]
    lens = [196, 268, 318, 262, 186]
    fg_defs = (
        vgrad("lfill", 226, 500, [(0, "#ffffff", 0.92), (0.7, "#ffffff", 0.62), (1, oklch(0.93, 0.045, hue), 0.7)])
        + radial("lheart", 512, 340, 180, oklch(0.80, 0.11, hue), 0.7)
        + blur_filter("lshadow", 22)
    )
    tentacles = ""
    for i, (x, ln) in enumerate(zip(xs, lens)):
        fg_defs += vgrad(f"lt{i}", 490, 490 + ln, [(0, ink, 0.95), (0.55, ink, 0.7), (1, violet, 0.05)])
        tentacles += (
            f'<path d="{tentacle(x, 488, ln, 14 + ln * 0.035, 10, 26 if i == 2 else 22, 4)}" '
            f'fill="url(#lt{i})"/>'
        )
    fg = (
        f'<path d="{bell}" transform="translate(0 30)" fill="{oklch(0.30, 0.04, hue)}" '
        f'fill-opacity="0.18" filter="url(#lshadow)"/>'
        + tentacles
        + f'<path d="{bell}" fill="url(#lfill)"/>'
        + f'<path d="{bell}" fill="url(#lheart)"/>'
        + f'<path d="{bell}" fill="none" stroke="#ffffff" stroke-opacity="0.96" stroke-width="7" '
        f'stroke-linejoin="round"/>'
    )
    mono = f'<path d="{bell}" fill="#000"/>' + "".join(
        f'<path d="{tentacle(x, 480, ln, 14 + ln * 0.035, 10, 34, 12)}" fill="#000"/>'
        for x, ln in zip(xs, lens)
    )
    return dict(
        id="lumen",
        name="Lumen",
        idea="A frosted-glass bell lit from behind by Misty's aurora. Its five tapered tentacles fall like a soft waveform.",
        palette=f"Light field at hue {hue} (Misty “Mist”), aurora at +55° and −70°; ink-blue tentacles fading to lilac.",
        notes="The most direct expression of the UI itself: glass over a lit field. Pairs with Abyss as its dark appearance.",
        bg=(bg_defs, bg),
        fg=(fg_defs, fg),
        mono=("", mono),
    )


@concept
def abyss():
    """Bioluminescent bell drifting in a dark field."""
    hue = 250
    bg_defs, bg = field(
        "ab",
        oklch(0.215, 0.02, hue),
        oklch(0.155, 0.016, hue),
        [
            (260, 150, 660, oklch(0.46, 0.11, hue), 0.95),
            (900, 900, 660, oklch(0.40, 0.085, hue + 55), 0.9),
            (80, 980, 540, oklch(0.34, 0.065, hue - 70), 0.85),
        ],
        0.065,
    )
    glow = oklch(0.88, 0.085, hue)
    lilac = oklch(0.88, 0.07, hue + 55)
    mint = oklch(0.90, 0.06, hue - 70)
    bell = dome(310, 714, 212, 468, scallops=5, dip=22, shoulder=0.25)
    fg_defs = radial("aheart", 512, 320, 250, oklch(0.80, 0.12, hue), 0.7) + blur_filter("aglow", 18)
    xs = [366, 439, 512, 585, 658]
    lens = [236, 296, 340, 284, 222]
    strands = ""
    for i, (x, ln) in enumerate(zip(xs, lens)):
        fg_defs += vgrad(f"at{i}", 476, 476 + ln, [(0, glow, 1), (0.5, lilac, 0.75), (1, mint, 0.05)])
        strands += f'<path d="{tentacle(x, 474, ln, 20, -40, 17, 3)}" fill="url(#at{i})"/>'
    fg = (
        '<g transform="rotate(-6 512 512)">'
        f'<g filter="url(#aglow)" opacity="0.85">{strands}'
        f'<path d="{bell}" fill="none" stroke="{glow}" stroke-width="20"/></g>'
        + strands
        + f'<path d="{bell}" fill="url(#aheart)"/>'
        + f'<path d="{bell}" fill="{glow}" fill-opacity="0.1" stroke="{glow}" stroke-width="10" '
        f'stroke-linejoin="round"/>'
        + f'<path d="{dome_open(392, 632, 292, 468, 0.25)}" fill="none" stroke="{lilac}" '
        f'stroke-opacity="0.6" stroke-width="6" stroke-linecap="round"/>'
        + "</g>"
    )
    mono = (
        '<g transform="rotate(-6 512 512)">'
        f'<path d="{bell}" fill="none" stroke="#000" stroke-width="32" stroke-linejoin="round"/>'
        + "".join(
            f'<path d="{tentacle(x, 486, ln, 20, -40, 30, 12)}" fill="#000"/>'
            for x, ln in ((366, 236), (512, 340), (658, 222))
        )
        + "</g>"
    )
    return dict(
        id="abyss",
        name="Abyss",
        idea="The bell after dark: a luminous outline drifting in a deep field, tentacles trailing in the current.",
        palette=f"Dark field at hue {hue}; light from the hue, lilac (+55°) and mint (−70°).",
        notes="Strongest mood of the set. Thin light lines fade below 32 px, so it needs a hand-hinted small size.",
        bg=(bg_defs, bg),
        fg=(fg_defs, fg),
        mono=("", mono),
    )


@concept
def wavebell():
    """Bell made of sound-wave arcs; tentacles become a level meter."""
    h0, h1 = 32, 78
    bg_defs = vgrad("wb", 0, SIZE, [(0, oklch(0.66, 0.15, h0), 1), (1, oklch(0.74, 0.14, h1), 1)])
    bg_defs += radial("wbg", 180, 160, 620, oklch(0.80, 0.10, h0 + 20), 0.55)
    bg_defs += radial("wbg2", 900, 940, 560, oklch(0.60, 0.14, h0 - 20), 0.5)
    bg_defs += grain("wbgrain", 0.05)
    bg = (
        f'<rect width="{SIZE}" height="{SIZE}" fill="url(#wb)"/>'
        f'<rect width="{SIZE}" height="{SIZE}" fill="url(#wbg)"/>'
        f'<rect width="{SIZE}" height="{SIZE}" fill="url(#wbg2)"/>'
        f'<rect width="{SIZE}" height="{SIZE}" filter="url(#wbgrain)"/>'
    )
    cy = 478

    def arcs(color):
        out = ""
        for r in (246, 168):
            out += (
                f'<path d="M{C - r},{cy} A{r},{r} 0 0 1 {C + r},{cy}" fill="none" stroke="{color}" '
                f'stroke-width="36" stroke-linecap="round"/>'
            )
        return out + f'<path d="M{C - 92},{cy} A92,92 0 0 1 {C + 92},{cy} Z" fill="{color}"/>'

    bars = [(352, 110), (432, 196), (512, 262), (592, 176), (672, 104)]

    def meter(color):
        return "".join(
            f'<line x1="{x}" y1="{cy + 46}" x2="{x}" y2="{cy + 46 + h}" stroke="{color}" '
            f'stroke-width="36" stroke-linecap="round"/>'
            for x, h in bars
        )

    shadow_col = oklch(0.35, 0.10, h0)
    fg_defs = blur_filter("wshadow", 14, "25%")
    fg = (
        f'<g transform="translate(0 18)" filter="url(#wshadow)" opacity="0.32">'
        f"{arcs(shadow_col)}{meter(shadow_col)}</g>" + arcs("#ffffff") + meter("#ffffff")
    )
    return dict(
        id="wavebell",
        name="Wavebell",
        idea="The bell drawn as sound: two wave arcs over a solid core, tentacles as a level meter. Built to stay legible at 16 px.",
        palette=f"Warm coral to amber (hues {h0}° to {h1}°), after the sea nettle; white glyph.",
        notes="Most legible and most distinctive in a dock of blue icons. It drifts towards a broadcast or fingerprint sign, so check it against podcast and radio app icons.",
        bg=(bg_defs, bg),
        fg=(fg_defs, fg),
        mono=("", arcs("#000") + meter("#000")),
    )


@concept
def verse():
    """A jellyfish swimming bell-first; its tentacles trail behind as lyric lines."""
    hue = 300
    bg_defs, bg = field(
        "vb",
        oklch(0.985, 0.008, hue),
        oklch(0.95, 0.016, hue),
        [
            (150, 200, 600, oklch(0.86, 0.09, hue), 0.9),
            (920, 860, 640, oklch(0.88, 0.07, hue + 55), 0.85),
            (900, 120, 420, oklch(0.90, 0.05, hue - 70), 0.7),
        ],
        0.05,
    )
    ink = oklch(0.30, 0.05, hue)
    accent = oklch(0.52, 0.17, hue)
    # the bell points left (the direction of travel); hem at x = 410
    bell_up = dome(356, 668, 214, 418, scallops=3, dip=24, shoulder=0.30)
    bell = f'<path d="{bell_up}" transform="rotate(-90 {C} {C}) translate(0 0)"/>'
    lines = [(-96, 300, 0.45, 17), (-30, 392, 1.0, 27), (36, 336, 0.45, 17), (100, 246, 0.3, 15)]

    def lyric(dy, length, op, w, color, mono=False):
        x0, y0 = 420, C + dy
        p = taper(
            (x0, y0),
            (x0 + length * 0.33, y0 - 14),
            (x0 + length * 0.66, y0 + 14),
            (x0 + length, y0 - 4),
            w if not mono else w + 8,
            w * 0.6 if not mono else w + 4,
            ease=1.0,
        )
        return f'<path d="{p}" fill="{color}" fill-opacity="{1 if mono else op}"/>'

    fg_defs = vgrad("vfill", 226, 410, [(0, "#ffffff", 0.97), (1, oklch(0.95, 0.04, hue), 0.85)])
    fg_defs += radial("vheart", 512, 330, 160, oklch(0.80, 0.10, hue), 0.65)
    fg_defs += blur_filter("vshadow", 20)
    edge = oklch(0.45, 0.12, hue)
    # rotate() turns the upright dome so it faces left, the way it swims
    rot = f'transform="rotate(-90 {C} {C})"'
    fg = (
        f'<g transform="translate(0 26)"><path d="{bell_up}" {rot} fill="{ink}" fill-opacity="0.16" '
        f'filter="url(#vshadow)"/></g>'
        + "".join(
            lyric(dy, ln, op, w, accent if op == 1.0 else ink) for dy, ln, op, w in lines
        )
        + f'<path d="{bell_up}" {rot} fill="url(#vfill)"/>'
        + f'<path d="{bell_up}" {rot} fill="url(#vheart)"/>'
        + f'<path d="{bell_up}" {rot} fill="none" stroke="{edge}" stroke-opacity="0.9" '
        f'stroke-width="8" stroke-linejoin="round"/>'
    )
    mono = f'<path d="{bell_up}" {rot} fill="#000"/>' + "".join(
        lyric(dy, ln, 1, w, "#000", mono=True) for dy, ln, _, w in lines[:3]
    )
    return dict(
        id="verse",
        name="Verse",
        idea="A jellyfish swims bell-first, and its tentacles trail behind it as lines of lyrics. The current line is the one accent.",
        palette=f"Pale field at hue {hue} (Misty “Violet”); ink lines; one accent line.",
        notes="Puts synced lyrics at the centre of the brand. At 16 px it can read as a list, so the bell must stay heavy.",
        bg=(bg_defs, bg),
        fg=(fg_defs, fg),
        mono=("", mono),
    )


@concept
def tide():
    """Misty's identity-glyph recipe: one outline glyph on a hue-to-hue+55 tile."""
    hue = 196
    bg_defs = (
        f'<linearGradient id="tb" x1="0" y1="0" x2="1024" y2="1024" gradientUnits="userSpaceOnUse">'
        f'<stop offset="0" stop-color="{oklch(0.64, 0.12, hue)}"/>'
        f'<stop offset="1" stop-color="{oklch(0.56, 0.13, hue + 55)}"/></linearGradient>'
        + radial("tg", 260, 200, 560, oklch(0.80, 0.08, hue), 0.5)
        + grain("tgrain", 0.05)
    )
    bg = (
        f'<rect width="{SIZE}" height="{SIZE}" fill="url(#tb)"/>'
        f'<rect width="{SIZE}" height="{SIZE}" fill="url(#tg)"/>'
        f'<rect width="{SIZE}" height="{SIZE}" filter="url(#tgrain)"/>'
    )
    bell = dome(318, 706, 250, 486, scallops=4, dip=26, shoulder=0.28)

    def glyph(color, w):
        out = f'<path d="{bell}" fill="none" stroke="{color}" stroke-width="{w}" stroke-linejoin="round"/>'
        for x, ln in ((415, 220), (512, 290), (609, 205)):
            out += (
                f'<path d="{strand(x, 530, ln, 20, 6)}" fill="none" stroke="{color}" '
                f'stroke-width="{w}" stroke-linecap="round"/>'
            )
        return out

    fg_defs = blur_filter("tshadow", 12, "25%")
    fg = (
        f'<g transform="translate(0 14)" filter="url(#tshadow)" opacity="0.3">'
        f'{glyph(oklch(0.30, 0.08, hue + 30), 40)}</g>' + glyph("#ffffff", 40)
    )
    return dict(
        id="tide",
        name="Tide",
        idea="Misty's own recipe for a product face: one outline glyph with a 2-unit stroke on a tile tinted from the hue to the hue + 55°.",
        palette=f"Tile from hue {hue} to {hue + 55}; white glyph.",
        notes="The most systematic choice and the easiest to keep consistent. Also the least ownable: it looks like a well-made system glyph.",
        bg=(bg_defs, bg),
        fg=(fg_defs, fg),
        mono=("", glyph("#000", 46)),
    )


def main(out):
    out = pathlib.Path(out)
    meta = []
    for fn in CONCEPTS:
        c = fn()
        d = out / c["id"]
        d.mkdir(parents=True, exist_ok=True)
        (d / "background.svg").write_text(svg(c["bg"][1], c["bg"][0], f"{c['name']} background"))
        (d / "foreground.svg").write_text(svg(c["fg"][1], c["fg"][0], f"{c['name']} foreground"))
        (d / "monochrome.svg").write_text(svg(c["mono"][1], c["mono"][0], f"{c['name']} monochrome"))
        (d / "full.svg").write_text(
            svg(c["bg"][1] + c["fg"][1], c["bg"][0] + c["fg"][0], f"{c['name']} (unmasked)")
        )
        meta.append({k: c[k] for k in ("id", "name", "idea", "palette", "notes")})
    import json

    (out / "concepts.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False) + "\n")
    print("wrote", len(meta), "concepts to", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "design/icons/concepts")
