#!/usr/bin/env python3
"""Round 2 icon variants: the Wavebell shape drawn in Tide's style.

Brief (2026-11-05): use the Wavebell shape with Tide's transparent lines and
background, in combinations of deep blue and cyan.

Each variant is a Tide-style tile (gradient, soft glow, grain) with the
Wavebell glyph (two wave arcs over a core, five level-meter tentacles).
"Transparent lines" are explored three ways:

  translucent  the glyph is white at partial opacity, so the tile shows through
  glass        each stroke is a frosted tube: a bright rim and a see-through core
  hollow       the core is an outline instead of a solid half-disc, like Tide

The glyph is drawn as a luminance mask over a paint (white, or white to cyan),
so transparency is real: whatever sits behind the foreground layer shows
through it on every platform that composites layers.

Layers per variant (1024 x 1024): background.svg, foreground.svg,
monochrome.svg, full.svg. Usage: python3 gen_round2.py <output-dir>
"""
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from gen_concepts import SIZE, C, oklch, radial, vgrad, grain, svg  # noqa: E402

CY = 478
ARCS = (246, 168)
CORE_R = 92
BARS = [(352, 110), (432, 196), (512, 262), (592, 176), (672, 104)]
W = 36  # stroke width, the same as round 1's Wavebell


def glyph_paths():
    """The Wavebell glyph as (kind, data) primitives."""
    prims = []
    for r in ARCS:
        prims.append(("arc", f"M{C - r},{CY} A{r},{r} 0 0 1 {C + r},{CY}"))
    prims.append(("core", f"M{C - CORE_R},{CY} A{CORE_R},{CORE_R} 0 0 1 {C + CORE_R},{CY} Z"))
    for x, h in BARS:
        prims.append(("bar", f"M{x},{CY + 46} L{x},{CY + 46 + h}"))
    return prims


def glyph_mask(mode, opacity=1.0, rim=5.0, inner=0.28):
    """Mask content (white = opaque) for the glyph in a given line mode."""
    out = []
    lum = lambda v: "#" + f"{int(round(v * 255)):02x}" * 3  # noqa: E731
    for kind, d in glyph_paths():
        if kind == "core":
            if mode == "hollow":
                # outline core: an arc closed by its base, like Tide's outline bell
                inset = W / 2
                r = CORE_R - inset / 2
                dd = f"M{C - r},{CY} A{r},{r} 0 0 1 {C + r},{CY} Z"
                out.append(f'<path d="{dd}" fill="none" stroke="{lum(opacity)}" stroke-width="{W * 0.8}" '
                           f'stroke-linejoin="round"/>')
            elif mode == "glass":
                out.append(f'<path d="{d}" fill="{lum(inner)}"/>')
                out.append(f'<path d="{d}" fill="none" stroke="{lum(1.0)}" stroke-width="{rim * 2}" '
                           f'stroke-linejoin="round"/>')
            else:
                out.append(f'<path d="{d}" fill="{lum(opacity)}"/>')
            continue
        if mode == "glass":
            out.append(f'<path d="{d}" fill="none" stroke="#ffffff" stroke-width="{W}" stroke-linecap="round"/>')
            out.append(f'<path d="{d}" fill="none" stroke="{lum(inner)}" stroke-width="{W - 2 * rim}" '
                       f'stroke-linecap="round"/>')
        else:
            out.append(f'<path d="{d}" fill="none" stroke="{lum(opacity)}" stroke-width="{W}" '
                       f'stroke-linecap="round"/>')
    return "".join(out)


def glyph_solid(color, mode="solid", width_boost=0):
    """Plain single-colour glyph (shadows and monochrome layers)."""
    out = []
    for kind, d in glyph_paths():
        if kind == "core":
            if mode == "hollow":
                r = CORE_R - W / 4
                dd = f"M{C - r},{CY} A{r},{r} 0 0 1 {C + r},{CY} Z"
                out.append(f'<path d="{dd}" fill="none" stroke="{color}" stroke-width="{W * 0.8 + width_boost}" '
                           f'stroke-linejoin="round"/>')
            else:
                out.append(f'<path d="{d}" fill="{color}"/>')
        else:
            out.append(f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{W + width_boost}" '
                       f'stroke-linecap="round"/>')
    return "".join(out)


def tile(prefix, kind, stops, glows, grain_op=0.05):
    """Tide-style background: a gradient plus soft glows and grain."""
    defs, body = "", ""
    if kind == "diag":
        s = "".join(f'<stop offset="{o}" stop-color="{c}"/>' for o, c in stops)
        defs += (f'<linearGradient id="{prefix}g" x1="0" y1="0" x2="{SIZE}" y2="{SIZE}" '
                 f'gradientUnits="userSpaceOnUse">{s}</linearGradient>')
    elif kind == "vert":
        defs += vgrad(f"{prefix}g", 0, SIZE, [(o, c, 1) for o, c in stops])
    elif kind == "radial":
        s = "".join(f'<stop offset="{o}" stop-color="{c}"/>' for o, c in stops)
        defs += (f'<radialGradient id="{prefix}g" cx="512" cy="160" r="900" gradientUnits="userSpaceOnUse">'
                 f'{s}</radialGradient>')
    body += f'<rect width="{SIZE}" height="{SIZE}" fill="url(#{prefix}g)"/>'
    for i, (cx, cy, r, col, op) in enumerate(glows):
        defs += radial(f"{prefix}w{i}", cx, cy, r, col, op)
        body += f'<rect width="{SIZE}" height="{SIZE}" fill="url(#{prefix}w{i})"/>'
    if grain_op:
        defs += grain(f"{prefix}n", grain_op)
        body += f'<rect width="{SIZE}" height="{SIZE}" filter="url(#{prefix}n)"/>'
    return defs, body


# Palette: deep blues and cyans (OKLCH), named for review.
CYAN_HI = oklch(0.84, 0.10, 205)
CYAN = oklch(0.74, 0.125, 208)
AQUA = oklch(0.66, 0.12, 212)
AZURE = oklch(0.56, 0.15, 242)
DEEP = oklch(0.40, 0.16, 262)
DEEPER = oklch(0.33, 0.15, 264)
NAVY = oklch(0.25, 0.10, 262)
ABYSS = oklch(0.19, 0.06, 258)
ICE = oklch(0.95, 0.04, 205)

VARIANTS = [
    dict(id="classic", name="Tide Classic",
         note="Tide's tile recoloured cyan to deep blue; solid white lines. The baseline to compare against.",
         bg=("diag", [(0, CYAN), (1, DEEP)], [(260, 200, 600, CYAN_HI, 0.45)]),
         line=dict(mode="solid", paint=("flat", "#ffffff"), shadow=0.30)),
    dict(id="clearwater", name="Clearwater",
         note="Same tile; the lines are 75% white, so the gradient shows through them.",
         bg=("diag", [(0, CYAN), (1, DEEP)], [(260, 200, 600, CYAN_HI, 0.45)]),
         line=dict(mode="translucent", opacity=0.75, paint=("flat", "#ffffff"), shadow=0.16)),
    dict(id="glass-tide", name="Glass Tide",
         note="Deep blue to cyan, reversed; every line is a frosted tube with a bright rim and a clear core.",
         bg=("diag", [(0, DEEPER), (1, CYAN)], [(820, 860, 560, CYAN_HI, 0.35)]),
         line=dict(mode="glass", rim=5.5, inner=0.30, paint=("flat", "#ffffff"), shadow=0.12)),
    dict(id="deep-sea", name="Deep Sea",
         note="Navy lit by a cyan glow from above; glass lines tinted from white to cyan.",
         bg=("radial", [(0, AQUA), (0.55, DEEPER), (1, NAVY)], []),
         line=dict(mode="glass", rim=5.5, inner=0.34, paint=("grad", ICE, CYAN_HI), shadow=0.10)),
    dict(id="depth", name="Depth",
         note="Cyan surface falling to deep blue; translucent lines with an open, outlined core like Tide's bell.",
         bg=("vert", [(0, CYAN), (1, DEEPER)], [(512, 120, 520, CYAN_HI, 0.4)]),
         line=dict(mode="hollow", opacity=0.82, paint=("flat", "#ffffff"), shadow=0.14)),
    dict(id="midnight", name="Midnight",
         note="Near-black navy with a cyan rim of light; cyan-tinted translucent lines.",
         bg=("diag", [(0, ABYSS), (1, DEEP)], [(120, 960, 560, AQUA, 0.6), (980, 80, 380, AZURE, 0.35)]),
         line=dict(mode="translucent", opacity=0.9, paint=("grad", ICE, CYAN), shadow=0.0)),
    dict(id="aurora", name="Aurora",
         note="Deep blue with cyan and azure glows in the corners, as Misty's field does; glass lines.",
         bg=("diag", [(0, DEEPER), (1, NAVY)], [(150, 140, 620, CYAN, 0.75), (930, 900, 600, AZURE, 0.7),
                                                 (80, 980, 420, AQUA, 0.45)]),
         line=dict(mode="glass", rim=5.5, inner=0.30, paint=("flat", "#ffffff"), shadow=0.10)),
    dict(id="lagoon", name="Lagoon",
         note="The brightest: cyan to azure; 88% white lines with a deeper shadow for lift.",
         bg=("diag", [(0, CYAN_HI), (1, AZURE)], [(240, 180, 560, ICE, 0.35)]),
         line=dict(mode="translucent", opacity=0.88, paint=("flat", "#ffffff"), shadow=0.26)),
]


def build(v):
    pid = v["id"].replace("-", "")
    kind, stops, glows = v["bg"]
    bg_defs, bg = tile(pid, kind, stops, glows)
    line = v["line"]
    mode = line["mode"]
    mask = glyph_mask(mode, opacity=line.get("opacity", 1.0), rim=line.get("rim", 5.0), inner=line.get("inner", 0.28))
    fg_defs = (f'<mask id="{pid}m" maskUnits="userSpaceOnUse" x="0" y="0" width="{SIZE}" height="{SIZE}">'
               f'<rect width="{SIZE}" height="{SIZE}" fill="#000"/>{mask}</mask>')
    ptype = line["paint"][0]
    if ptype == "flat":
        paint = line["paint"][1]
    else:
        fg_defs += vgrad(f"{pid}p", 200, 820, [(0, line["paint"][1], 1), (1, line["paint"][2], 1)])
        paint = f"url(#{pid}p)"
    fg = ""
    if line.get("shadow"):
        fg_defs += (f'<filter id="{pid}s" x="-20%" y="-20%" width="140%" height="150%">'
                    f'<feGaussianBlur stdDeviation="14"/></filter>')
        fg += (f'<g transform="translate(0 16)" filter="url(#{pid}s)" opacity="{line["shadow"]}">'
               f'{glyph_solid(oklch(0.22, 0.09, 262), "hollow" if mode == "hollow" else "solid")}</g>')
    fg += f'<rect width="{SIZE}" height="{SIZE}" fill="{paint}" mask="url(#{pid}m)"/>'
    mono = glyph_solid("#000", "hollow" if mode == "hollow" else "solid", width_boost=6)
    return bg_defs, bg, fg_defs, fg, mono


def main(out):
    out = pathlib.Path(out)
    meta = []
    for v in VARIANTS:
        bg_defs, bg, fg_defs, fg, mono = build(v)
        d = out / v["id"]
        d.mkdir(parents=True, exist_ok=True)
        (d / "background.svg").write_text(svg(bg, bg_defs, f"{v['name']} background"))
        (d / "foreground.svg").write_text(svg(fg, fg_defs, f"{v['name']} foreground"))
        (d / "monochrome.svg").write_text(svg(mono, "", f"{v['name']} monochrome"))
        (d / "full.svg").write_text(svg(bg + fg, bg_defs + fg_defs, f"{v['name']} (unmasked)"))
        meta.append({"id": v["id"], "name": v["name"], "idea": v["note"], "palette": v["line"]["mode"],
                     "notes": ""})
    (out / "concepts.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False) + "\n")
    print("wrote", len(meta), "variants to", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "design/icons/round2")
