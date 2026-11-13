#!/usr/bin/env python3
"""Jellyfesh wordmark options (Step 0, 2026-11-05).

1. Monoline: custom lowercase lettering drawn with the same round-capped
   strokes as the icon's lines. The l's end in a small curl and the y and j
   hook like tentacles. Optional "bell" tittle: the j's dot becomes a tiny
   dome, the jellyfish bell. No font is involved, so there is nothing to
   license.
2. Type: "Jellyfesh" set in Inter Display (SIL Open Font License 1.1) and
   converted to outlines, with kerning from HarfBuzz. Needs fontTools and
   uharfbuzz; skipped with a message when they are missing.
3. Lockups: icon B (original colours) with each wordmark, horizontal and
   stacked.

Usage: python3 gen_wordmark.py <output-dir> [<icon-option-dir>]
"""
import math
import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent.parent / "icons" / "tools"))
from gen_concepts import oklch  # noqa: E402

# ---------------------------------------------------------------------------
# Monoline lettering. Units: x-height 100, ascender 150, descender 55.
# y grows downwards; baseline at y = 0.
# ---------------------------------------------------------------------------
W = 18  # stroke width


def letter_j(x, bell=False):
    out = [f"M{x},-100 V24 Q{x},55 {x - 30},55"]
    dots = []
    if bell:
        # a small dome: the jellyfish bell standing in for the tittle
        r = 19
        dots.append(f'<path d="M{x - r},-128 A{r},{r} 0 0 1 {x + r},-128 Z"/>')
    else:
        dots.append(f'<circle cx="{x}" cy="-130" r="{W * 0.62:.1f}"/>')
    return out, dots, x


def letter_e(x):
    cx, cy, rx, ry = x + 48, -50, 48, 50
    a = math.radians(38)
    ex, ey = cx + rx * math.cos(a), cy + ry * math.sin(a)
    return [f"M{x + 2},-50 H{x + 96} A{rx},{ry} 0 1 0 {ex:.1f},{ey:.1f}"], [], x + 96


def letter_l(x):
    return [f"M{x},-150 V-20 Q{x},0 {x + 18},0"], [], x + 18


def letter_y(x):
    return [f"M{x},-100 L{x + 52},-10",
            f"M{x + 92},-100 L{x + 44},28 Q{x + 34},55 {x + 8},55"], [], x + 92


def letter_f(x):
    return [f"M{x + 18},0 V-116 Q{x + 18},-150 {x + 52},-150", f"M{x},-100 H{x + 46}"], [], x + 52


def letter_s(x):
    return [f"M{x + 78},-84 C{x + 70},-96 {x + 56},-100 {x + 40},-100 "
            f"C{x + 18},-100 {x + 4},-90 {x + 4},-75 C{x + 4},-60 {x + 18},-54 {x + 40},-50 "
            f"C{x + 64},-46 {x + 80},-38 {x + 80},-24 C{x + 80},-8 {x + 62},0 {x + 40},0 "
            f"C{x + 22},0 {x + 8},-6 {x + 2},-18"], [], x + 80


def letter_h(x):
    return [f"M{x},-150 V0", f"M{x},-62 C{x},-86 {x + 18},-100 {x + 42},-100 "
            f"C{x + 66},-100 {x + 84},-86 {x + 84},-62 V0"], [], x + 84


# glyph, left side bearing gap from the previous glyph's right edge
SEQUENCE = [("j", 30), ("e", 40), ("l", 34), ("l", 26), ("y", 26), ("f", 20), ("e", 30), ("s", 32), ("h", 36)]
MAKERS = {"j": letter_j, "e": letter_e, "l": letter_l, "y": letter_y, "f": letter_f, "s": letter_s, "h": letter_h}


def monoline(bell=False):
    """Return (stroke paths, filled dots, width) of the word, starting at x = 0."""
    paths, dots, cursor = [], [], 0
    for glyph, gap in SEQUENCE:
        x = cursor + gap
        if glyph == "j":
            p, d, right = letter_j(x, bell)
        else:
            p, d, right = MAKERS[glyph](x)
        paths += p
        dots += d
        cursor = right
    return paths, dots, cursor + 4


def monoline_group(paint, bell=False, shadow=None):
    paths, dots, width = monoline(bell)
    d = " ".join(paths)
    body = ""
    if shadow:
        col, op = shadow
        body += (f'<g transform="translate(0 6)" filter="url(#wmshadow)" opacity="{op}">'
                 f'<path d="{d}" fill="none" stroke="{col}" stroke-width="{W}" stroke-linecap="round" '
                 f'stroke-linejoin="round"/><g fill="{col}">{"".join(dots)}</g></g>')
    body += (f'<path d="{d}" fill="none" stroke="{paint}" stroke-width="{W}" stroke-linecap="round" '
             f'stroke-linejoin="round"/><g fill="{paint}">{"".join(dots)}</g>')
    return body, width


# ---------------------------------------------------------------------------
# Type wordmark (Inter Display, outlined, kerned).
# ---------------------------------------------------------------------------
def type_outline(text, font_path, tracking=-0.02, bell_j=False):
    try:
        import uharfbuzz as hb
        from fontTools.pens.svgPathPen import SVGPathPen
        from fontTools.ttLib import TTFont
    except ImportError:
        return None
    data = pathlib.Path(font_path).read_bytes()
    face = hb.Face(data)
    font = hb.Font(face)
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    hb.shape(font, buf, {"kern": True, "liga": True})
    tt = TTFont(font_path)
    gs = tt.getGlyphSet()
    order = tt.getGlyphOrder()
    upem = tt["head"].unitsPerEm
    x = 0
    parts = []
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        name = order[info.codepoint]
        pen = SVGPathPen(gs)
        gs[name].draw(pen)
        d = pen.getCommands()
        if bell_j and name == "j":
            d = bell_tittle(d, tt["OS/2"].sxHeight)
        parts.append(f'<path transform="translate({x + pos.x_offset},0) scale(1,-1)" d="{d}"/>')
        x += pos.x_advance + tracking * upem
    cap = tt["OS/2"].sCapHeight
    return "".join(parts), x, upem, cap


def bell_tittle(d, x_height):
    """Swap the j's dot for a dome (font units, y up)."""
    import re
    subs = [m.group(0) for m in re.finditer(r"M[^M]*", d)]

    def ys(sub):
        nums = [float(v) for v in re.findall(r"-?\d+(?:\.\d+)?", sub)]
        return nums[1::2], nums[0::2]

    keep, dot = [], None
    for sub in subs:
        y, x = ys(sub)
        if y and min(y) > x_height:
            dot = (min(x), max(x), min(y), max(y))
        else:
            keep.append(sub)
    if not dot:
        return d
    x0, x1, y0, y1 = dot
    cx, r = (x0 + x1) / 2, (x1 - x0) / 2 * 1.35
    base = y0 + (y1 - y0) * 0.15
    k = 1.3333 * r
    dome = (f"M{cx - r:.1f} {base:.1f}C{cx - r:.1f} {base + k:.1f} {cx + r:.1f} {base + k:.1f} "
            f"{cx + r:.1f} {base:.1f}Z")
    return "".join(keep) + dome


# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------
def svg_doc(view_w, view_h, body, defs="", title=""):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {view_w:.0f} {view_h:.0f}" '
            f'width="{view_w:.0f}" height="{view_h:.0f}"><title>{title}</title><defs>{defs}</defs>{body}</svg>\n')


DEEP = oklch(0.33, 0.15, 264)
AZURE = oklch(0.56, 0.15, 242)
CYAN = oklch(0.74, 0.125, 208)
ICE = oklch(0.95, 0.04, 205)
SHADOW = oklch(0.22, 0.09, 262)


def grad(id_, y0, y1, c0, c1):
    return (f'<linearGradient id="{id_}" x1="0" y1="{y0}" x2="0" y2="{y1}" gradientUnits="userSpaceOnUse">'
            f'<stop offset="0" stop-color="{c0}"/><stop offset="1" stop-color="{c1}"/></linearGradient>')


SHADOW_FILTER = '<filter id="wmshadow" x="-10%" y="-30%" width="120%" height="160%"><feGaussianBlur stdDeviation="5"/></filter>'


def icon_inline(icon_dir, x, y, size):
    """Inline icon B (background + foreground) as a nested, rounded tile."""
    import re
    bg = (icon_dir / "background.svg").read_text()
    fg = (icon_dir / "foreground.svg").read_text()

    def inner(s):
        s = re.sub(r"<title>.*?</title>", "", s)
        return s[s.index(">") + 1: s.rindex("</svg>")]

    r = size * 0.225
    return (f'<defs><clipPath id="tileclip"><rect x="{x}" y="{y}" width="{size}" height="{size}" rx="{r:.1f}"/>'
            f'</clipPath></defs><g clip-path="url(#tileclip)"><svg x="{x}" y="{y}" width="{size}" height="{size}" '
            f'viewBox="0 0 1024 1024">{inner(bg)}{inner(fg)}</svg></g>')


def write_type(out, stem, res, pad, caps=False):
    paths, adv, upem, cap = res
    scale = (110 if caps else 150) / cap
    tw = adv * scale
    asc, desc = cap * scale * 1.12, (0.08 if caps else 0.26) * upem * scale
    th = asc + desc
    view_w, view_h = tw + 2 * pad, th + pad
    base_y = asc + pad / 2
    mask = (f'<mask id="tmask" maskUnits="userSpaceOnUse" x="0" y="0" width="{view_w:.0f}" height="{view_h:.0f}">'
            f'<g transform="translate({pad},{base_y:.1f}) scale({scale:.5f})" fill="#fff">{paths}</g></mask>')
    for mode, (c0, c1) in (("light", (AZURE, DEEP)), ("dark", (ICE, CYAN))):
        defs = mask + grad("tpaint", base_y - cap * scale, base_y + 10, c0, c1)
        body = f'<rect width="{view_w:.0f}" height="{view_h:.0f}" fill="url(#tpaint)" mask="url(#tmask)"/>'
        (out / f"{stem}-{mode}.svg").write_text(svg_doc(view_w, view_h, body, defs,
                                                       f"Jellyfesh wordmark ({stem}, Inter Display, outlined), {mode}"))


def main(out, icon_dir=None):
    out = pathlib.Path(out)
    out.mkdir(parents=True, exist_ok=True)
    pad = 40
    top, bottom = -175, 75  # vertical extent incl. tittle and descenders
    h = bottom - top

    # Monoline on light: deep blue gradient strokes
    for bell in (False, True):
        tag = "-bell" if bell else ""
        body, width = monoline_group("url(#wmlight)", bell)
        view_w = width + 2 * pad
        g = f'<g transform="translate({pad},{-top + pad / 2})">{body}</g>'
        (out / f"monoline{tag}-light.svg").write_text(svg_doc(
            view_w, h + pad, g, grad("wmlight", -150, 55, AZURE, DEEP), f"Jellyfesh monoline wordmark{tag}, light"))
        # dark: Midnight lines with lift
        body, width = monoline_group("url(#wmdark)", bell, shadow=(SHADOW, 0.35))
        g = f'<g transform="translate({pad},{-top + pad / 2})">{body}</g>'
        (out / f"monoline{tag}-dark.svg").write_text(svg_doc(
            view_w, h + pad, g, grad("wmdark", -150, 55, ICE, CYAN) + SHADOW_FILTER,
            f"Jellyfesh monoline wordmark{tag}, dark"))
        # banner: on Depth's tile gradient
        bw, bh = view_w + 80, h + 120
        banner = (grad("wmbanner", 0, bh, oklch(0.74, 0.125, 208), oklch(0.33, 0.15, 264))
                  + grad("wmdark", -150, 55, ICE, CYAN) + SHADOW_FILTER)
        body, _ = monoline_group("url(#wmdark)", bell, shadow=(SHADOW, 0.35))
        g = (f'<rect width="{bw:.0f}" height="{bh:.0f}" rx="28" fill="url(#wmbanner)"/>'
             f'<g transform="translate({pad + 40},{-top + 60 - 10})" opacity="0.92">{body}</g>')
        (out / f"monoline{tag}-banner.svg").write_text(svg_doc(bw, bh, g, banner, f"Jellyfesh monoline wordmark{tag}, banner"))
    body, width = monoline_group("#000", False)
    (out / "monoline-mono.svg").write_text(svg_doc(width + 2 * pad, h + pad,
                                                     f'<g transform="translate({pad},{-top + pad / 2})">{body}</g>',
                                                     "", "Jellyfesh monoline wordmark, one colour"))

    # Type wordmark
    font = "/usr/share/fonts/opentype/inter/InterDisplay-SemiBold.otf"
    medium = "/usr/share/fonts/opentype/inter/InterDisplay-Medium.otf"
    variants = [("type", "Jellyfesh", font, -0.02, False),
                ("type-bell", "jellyfesh", font, -0.02, True),
                ("type-caps", "JELLYFESH", medium, 0.16, False)]
    for stem, text, fpath, track, bell in variants:
        res = type_outline(text, fpath, track, bell) if pathlib.Path(fpath).exists() else None
        if res is None:
            print("type wordmark skipped (needs fontTools, uharfbuzz and Inter Display)")
            break
        write_type(out, stem, res, pad, caps=(stem == "type-caps"))

    # Lockups with icon B
    if icon_dir:
        icon_dir = pathlib.Path(icon_dir)
        for mode in ("light", "dark"):
            body, width = monoline_group(f"url(#wm{mode})", True,
                                         shadow=(SHADOW, 0.35) if mode == "dark" else None)
            size = 236
            gap = 56
            view_w = size + gap + width + 2 * pad
            view_h = size + 2 * pad
            defs = (grad("wmlight", -150, 55, AZURE, DEEP) if mode == "light"
                    else grad("wmdark", -150, 55, ICE, CYAN) + SHADOW_FILTER)
            base_y = pad + size / 2 + 50  # x-height centred on the tile
            g = icon_inline(icon_dir, pad, pad, size) + f'<g transform="translate({pad + size + gap},{base_y})">{body}</g>'
            (out / f"lockup-horizontal-{mode}.svg").write_text(svg_doc(view_w, view_h, g, defs,
                                                                         f"Jellyfesh lockup, horizontal, {mode}"))
        body, width = monoline_group("url(#wmlight)", True)
        size = 300
        view_w = max(width, size) + 2 * pad
        view_h = size + 60 + h + 2 * pad
        g = (icon_inline(icon_dir, (view_w - size) / 2, pad, size)
             + f'<g transform="translate({(view_w - width) / 2},{pad + size + 60 - top})">{body}</g>')
        (out / "lockup-stacked-light.svg").write_text(svg_doc(view_w, view_h, g, grad("wmlight", -150, 55, AZURE, DEEP),
                                                                "Jellyfesh lockup, stacked, light"))
    print("wordmarks written to", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "design/brand/wordmark",
         sys.argv[2] if len(sys.argv) > 2 else None)
