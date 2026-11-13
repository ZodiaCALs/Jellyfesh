#!/usr/bin/env python3
"""Home-page hero wordmarks and formal one-colour wordmarks (2026-11-05).

Brief: spaced capitals for formal use; a fancier wordmark for the website's
home page. Three hero options, each a 1600 x 720 banner:

  hero-biolume   the monoline wordmark as glowing glass tubes in a dark sea
  hero-tentacle  the monoline wordmark whose j and y trail long tentacles
  hero-glass     the spaced capitals as frosted glass over an aurora field

Formal one-colour files: formal-caps-ink.svg and formal-caps-white.svg.

Usage: python3 gen_hero.py <output-dir>
Needs fontTools and uharfbuzz for the capitals (see gen_wordmark.py).
"""
import pathlib
import random
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent.parent / "icons" / "tools"))
from gen_concepts import oklch, taper  # noqa: E402
from gen_wordmark import W, monoline, type_outline  # noqa: E402

HW, HH = 1600, 720
MEDIUM = "/usr/share/fonts/opentype/inter/InterDisplay-Medium.otf"
ICE = oklch(0.95, 0.04, 205)
CYAN = oklch(0.74, 0.125, 208)
CYAN_HI = oklch(0.84, 0.10, 205)


def doc(body, defs, title):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {HW} {HH}" width="{HW}" height="{HH}">'
            f'<title>{title}</title><defs>{defs}</defs>{body}</svg>\n')


def vgrad(id_, y0, y1, stops):
    s = "".join(f'<stop offset="{o}" stop-color="{c}" stop-opacity="{a}"/>' for o, c, a in stops)
    return (f'<linearGradient id="{id_}" x1="0" y1="{y0}" x2="0" y2="{y1}" '
            f'gradientUnits="userSpaceOnUse">{s}</linearGradient>')


def glow(id_, cx, cy, r, col, a):
    return (f'<radialGradient id="{id_}" cx="{cx}" cy="{cy}" r="{r}" gradientUnits="userSpaceOnUse">'
            f'<stop offset="0" stop-color="{col}" stop-opacity="{a}"/>'
            f'<stop offset="0.6" stop-color="{col}" stop-opacity="{a * 0.35:.3f}"/>'
            f'<stop offset="1" stop-color="{col}" stop-opacity="0"/></radialGradient>')


GRAIN = ('<filter id="grain" x="0" y="0" width="100%" height="100%">'
         '<feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" seed="11" result="n"/>'
         '<feColorMatrix in="n" type="matrix" values="0 0 0 0 .5 0 0 0 0 .5 0 0 0 0 .5 0 0 0 0.05 0"/></filter>')


def field(top, bottom, glows):
    defs = vgrad("base", 0, HH, [(0, top, 1), (1, bottom, 1)]) + GRAIN
    body = f'<rect width="{HW}" height="{HH}" fill="url(#base)"/>'
    for i, (cx, cy, r, col, a) in enumerate(glows):
        defs += glow(f"g{i}", cx, cy, r, col, a)
        body += f'<rect width="{HW}" height="{HH}" fill="url(#g{i})"/>'
    return defs, body


def word_transform(width_px, baseline_y):
    _, _, w = monoline(True)
    s = width_px / w
    x = (HW - width_px) / 2
    return s, f"translate({x:.1f},{baseline_y}) scale({s:.4f})"


def hero_biolume(out):
    defs, bg = field(oklch(0.24, 0.08, 258), oklch(0.13, 0.04, 258), [
        (320, 140, 760, oklch(0.62, 0.12, 210), 0.55),
        (1340, 620, 700, oklch(0.45, 0.14, 245), 0.5),
        (900, 760, 520, oklch(0.50, 0.09, 190), 0.35)])
    rnd = random.Random(7)
    snow = "".join(
        f'<circle cx="{rnd.uniform(0, HW):.0f}" cy="{rnd.uniform(0, HH):.0f}" r="{rnd.uniform(1.2, 3.2):.1f}" '
        f'fill="#fff" fill-opacity="{rnd.uniform(0.12, 0.45):.2f}"/>' for _ in range(46))
    paths, dots, _ = monoline(True)
    d = " ".join(paths)
    s, tf = word_transform(1040, 420)
    rim = 4.2 / s  # rim width in wordmark units
    mask = (f'<mask id="tube" maskUnits="userSpaceOnUse" x="0" y="0" width="{HW}" height="{HH}"><g transform="{tf}">'
            f'<path d="{d}" fill="none" stroke="#fff" stroke-width="{W}" stroke-linecap="round" stroke-linejoin="round"/>'
            f'<path d="{d}" fill="none" stroke="#5c5c5c" stroke-width="{W - 2 * rim:.2f}" stroke-linecap="round" '
            f'stroke-linejoin="round"/><g fill="#fff">{"".join(dots)}</g></g></mask>')
    defs += (mask + vgrad("paint", 200, 500, [(0, ICE, 1), (1, CYAN, 1)])
             + '<filter id="bloom" x="-20%" y="-60%" width="140%" height="220%"><feGaussianBlur stdDeviation="16"/></filter>')
    word_glow = (f'<g filter="url(#bloom)" opacity="0.75"><g transform="{tf}">'
                 f'<path d="{d}" fill="none" stroke="{CYAN}" stroke-width="{W * 1.4}" stroke-linecap="round" '
                 f'stroke-linejoin="round"/><g fill="{CYAN_HI}">{"".join(dots)}</g></g></g>')
    body = (bg + f'<rect width="{HW}" height="{HH}" filter="url(#grain)"/>' + snow + word_glow
            + f'<rect width="{HW}" height="{HH}" fill="url(#paint)" mask="url(#tube)"/>')
    (out / "hero-biolume.svg").write_text(doc(body, defs, "Jellyfesh hero: bioluminescent glass"))


def hero_tentacle(out):
    defs, bg = field(oklch(0.74, 0.125, 208), oklch(0.33, 0.15, 264), [
        (800, 60, 700, oklch(0.84, 0.10, 205), 0.45),
        (1500, 700, 600, oklch(0.40, 0.15, 258), 0.4)])
    paths, dots, w = monoline(True)
    d = " ".join(paths)
    s, tf = word_transform(960, 300)
    # tentacles trailing from the j (x = 30) and the y (descender end near x = 300) in wordmark units
    j_x = 30 - 30
    y_end = 0
    cursor = 0
    for glyph, gap in (("j", 30), ("e", 40), ("l", 34), ("l", 26), ("y", 26)):
        x = cursor + gap
        if glyph == "y":
            y_end = x + 8
        widths = {"j": 0, "e": 96, "l": 18, "y": 92}
        cursor = x + widths[glyph]
    tentacles = [
        # from the j's hook: one long, one shorter, drifting right with an S-curve
        ((j_x + 2, 55), (j_x - 30, 140), (j_x + 90, 190), (j_x + 120, 300), W, 3, 0.95),
        ((j_x + 8, 52), (j_x + 10, 110), (j_x + 120, 140), (j_x + 190, 235), W * 0.75, 2.5, 0.75),
        # from the y's descender
        ((y_end + 2, 55), (y_end - 30, 145), (y_end + 100, 200), (y_end + 140, 320), W, 3, 0.95),
        ((y_end + 8, 52), (y_end + 14, 112), (y_end + 130, 150), (y_end + 210, 250), W * 0.75, 2.5, 0.75),
        # short ones from the l curls
        ((218, 2), (238, 40), (226, 80), (250, 128), W * 0.6, 2, 0.6),
        ((262, 2), (282, 46), (268, 96), (296, 150), W * 0.6, 2, 0.6),
    ]
    tent_paths = "".join(
        f'<path d="{taper(p0, p1, p2, p3, w0, w1)}" fill="url(#tpaint)" fill-opacity="{op}"/>'
        for p0, p1, p2, p3, w0, w1, op in tentacles)
    defs += (vgrad("paint", -150, 60, [(0, ICE, 1), (1, CYAN, 1)])  # wordmark units
             + vgrad("tpaint", 30, 320, [(0, ICE, 1), (0.5, CYAN_HI, 0.75), (1, CYAN, 0)])
             + '<filter id="lift" x="-10%" y="-30%" width="120%" height="160%"><feGaussianBlur stdDeviation="8"/></filter>')
    shadow = (f'<g transform="translate(0 10)" filter="url(#lift)" opacity="0.3"><g transform="{tf}">'
              f'<path d="{d}" fill="none" stroke="{oklch(0.22, 0.09, 262)}" stroke-width="{W}" stroke-linecap="round" '
              f'stroke-linejoin="round"/></g></g>')
    word = (f'<g transform="{tf}" opacity="0.94">{tent_paths}'
            f'<path d="{d}" fill="none" stroke="url(#paint)" stroke-width="{W}" stroke-linecap="round" '
            f'stroke-linejoin="round"/><g fill="url(#paint)">{"".join(dots)}</g></g>')
    body = bg + f'<rect width="{HW}" height="{HH}" filter="url(#grain)"/>' + shadow + word
    (out / "hero-tentacle.svg").write_text(doc(body, defs, "Jellyfesh hero: tentacle flourish"))


def hero_glass(out):
    res = type_outline("JELLYFESH", MEDIUM, 0.16)
    if res is None:
        print("hero-glass skipped (needs fontTools and uharfbuzz)")
        return
    paths, adv, upem, cap = res
    target_w = 1240
    s = target_w / adv
    x0, base = (HW - target_w) / 2, 360 + cap * s / 2
    glows = [(260, 140, 700, oklch(0.80, 0.12, 205), 0.85), (1380, 600, 720, oklch(0.52, 0.17, 252), 0.8),
             (760, 720, 560, oklch(0.72, 0.10, 185), 0.65), (1320, 60, 460, oklch(0.84, 0.08, 225), 0.55)]
    defs, bg = field(oklch(0.62, 0.11, 225), oklch(0.40, 0.13, 256), glows)
    letters = f'<g transform="translate({x0:.1f},{base:.1f}) scale({s:.5f})">{paths}</g>'
    defs += (f'<mask id="letters" maskUnits="userSpaceOnUse" x="0" y="0" width="{HW}" height="{HH}">'
             f'<g fill="#fff">{letters}</g></mask>'
             '<filter id="frost" x="0" y="0" width="100%" height="100%"><feGaussianBlur stdDeviation="22"/>'
             '<feColorMatrix type="saturate" values="1.7"/></filter>'
             '<filter id="drop" x="-10%" y="-30%" width="120%" height="160%"><feGaussianBlur stdDeviation="14"/></filter>'
             + vgrad("sheen", base - cap * s, base, [(0, "#ffffff", 0.62), (1, "#ffffff", 0.30)]))
    glass = (f'<g filter="url(#drop)" opacity="0.32" transform="translate(0 14)"><g fill="{oklch(0.30, 0.08, 250)}">'
             f'{letters}</g></g>'
             f'<g mask="url(#letters)"><g filter="url(#frost)">{bg}</g>'
             f'<rect width="{HW}" height="{HH}" fill="url(#sheen)"/></g>'
             f'<g fill="none" stroke="#fff" stroke-opacity="0.9" stroke-width="{2.6 / s:.1f}">{letters}</g>')
    body = bg + f'<rect width="{HW}" height="{HH}" filter="url(#grain)"/>' + glass
    (out / "hero-glass.svg").write_text(doc(body, defs, "Jellyfesh hero: aurora glass capitals"))


def formal(out):
    res = type_outline("JELLYFESH", MEDIUM, 0.16)
    if res is None:
        print("formal capitals skipped (needs fontTools and uharfbuzz)")
        return
    paths, adv, upem, cap = res
    pad = 24
    s = 110 / cap
    w, h = adv * s + 2 * pad, 110 + 2 * pad
    for name, col in (("ink", oklch(0.205, 0.02, 250)), ("white", "#ffffff")):
        body = f'<g transform="translate({pad},{pad + 110}) scale({s:.5f})" fill="{col}">{paths}</g>'
        (out / f"formal-caps-{name}.svg").write_text(
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w:.0f} {h:.0f}" width="{w:.0f}" height="{h:.0f}">'
            f'<title>JELLYFESH, formal wordmark, {name}</title>{body}</svg>\n')


def main(out):
    out = pathlib.Path(out)
    out.mkdir(parents=True, exist_ok=True)
    hero_biolume(out)
    hero_tentacle(out)
    hero_glass(out)
    formal(out)
    print("heroes and formal wordmarks written to", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "design/brand/wordmark")
