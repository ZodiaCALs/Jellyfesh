#!/usr/bin/env python3
"""Round 3: Depth's tile with Midnight's lines (brief of 2026-11-05).

Variants:
  final           Depth tile + Midnight lines exactly (cyan-tinted, 90% translucent)
  final-lift      the same with a soft shadow under the glyph, for the bright top
  final-outline   Midnight lines with Depth's outlined core
  final-deep      a slightly deeper top (aqua instead of cyan) for more line contrast

Each variant also gets foreground-small.svg: a simplified glyph for 16 and
32 px (one arc, core, three tentacles, heavier strokes, full opacity),
because thin translucent lines vanish at those sizes.

Usage: python3 gen_round3.py <output-dir>
"""
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from gen_concepts import SIZE, C, oklch, vgrad, svg  # noqa: E402
from gen_round2 import (  # noqa: E402
    AQUA, CYAN, CYAN_HI, DEEPER, ICE, CY, glyph_mask, glyph_solid, tile,
)

DEPTH_BG = ("vert", [(0, CYAN), (1, DEEPER)], [(512, 120, 520, CYAN_HI, 0.4)])
DEEP_BG = ("vert", [(0, AQUA), (1, DEEPER)], [(512, 120, 520, CYAN, 0.35)])
MIDNIGHT_LINES = dict(mode="translucent", opacity=0.9, paint=(ICE, CYAN))

VARIANTS = [
    dict(id="final", name="Depth × Midnight", bg=DEPTH_BG, mode="translucent", shadow=0.0,
         note="Exactly as asked: Depth's tile with Midnight's cyan-tinted translucent lines."),
    dict(id="final-lift", name="… with lift", bg=DEPTH_BG, mode="translucent", shadow=0.18,
         note="The same, with a soft shadow under the glyph so the lines hold on the bright top."),
    dict(id="final-outline", name="… with outlined core", bg=DEPTH_BG, mode="hollow", shadow=0.12,
         note="Midnight's lines with Depth's outlined core, closer to Tide's open bell."),
    dict(id="final-deep", name="… with a deeper top", bg=DEEP_BG, mode="translucent", shadow=0.0,
         note="The tile starts at aqua instead of cyan, so the lines read more clearly."),
]


def small_mask():
    """Simplified glyph for 16 and 32 px: one arc, a core, three tentacles."""
    w = 72
    r = 236
    core = 120
    out = [
        f'<path d="M{C - r},{CY} A{r},{r} 0 0 1 {C + r},{CY}" fill="none" stroke="#fff" '
        f'stroke-width="{w}" stroke-linecap="round"/>',
        f'<path d="M{C - core},{CY} A{core},{core} 0 0 1 {C + core},{CY} Z" fill="#fff"/>',
    ]
    for x, h in ((402, 150), (512, 236), (622, 140)):
        out.append(f'<path d="M{x},{CY + 70} L{x},{CY + 70 + h}" fill="none" stroke="#fff" '
                   f'stroke-width="{w}" stroke-linecap="round"/>')
    return "".join(out)


def build(v):
    pid = v["id"].replace("-", "")
    kind, stops, glows = v["bg"]
    bg_defs, bg = tile(pid, kind, stops, glows)
    paint_defs = vgrad(f"{pid}p", 200, 820, [(0, ICE, 1), (1, CYAN, 1)])

    def fg_layer(mask_body, mid, shadow_body=""):
        defs = (paint_defs + f'<mask id="{mid}" maskUnits="userSpaceOnUse" x="0" y="0" width="{SIZE}" '
                f'height="{SIZE}"><rect width="{SIZE}" height="{SIZE}" fill="#000"/>{mask_body}</mask>')
        body = ""
        if shadow_body:
            defs += (f'<filter id="{mid}s" x="-20%" y="-20%" width="140%" height="150%">'
                     f'<feGaussianBlur stdDeviation="14"/></filter>')
            body += (f'<g transform="translate(0 16)" filter="url(#{mid}s)" opacity="{v["shadow"]}">'
                     f'{shadow_body}</g>')
        body += f'<rect width="{SIZE}" height="{SIZE}" fill="url(#{pid}p)" mask="url(#{mid})"/>'
        return defs, body

    shadow = glyph_solid(oklch(0.22, 0.09, 262), "hollow" if v["mode"] == "hollow" else "solid") if v["shadow"] else ""
    fg_defs, fg = fg_layer(glyph_mask(v["mode"], opacity=MIDNIGHT_LINES["opacity"]), f"{pid}m", shadow)
    small_defs, small = fg_layer(small_mask(), f"{pid}sm")
    mono = glyph_solid("#000", "hollow" if v["mode"] == "hollow" else "solid", width_boost=6)
    return bg_defs, bg, fg_defs, fg, small_defs, small, mono


def main(out):
    out = pathlib.Path(out)
    meta = []
    for v in VARIANTS:
        bg_defs, bg, fg_defs, fg, small_defs, small, mono = build(v)
        d = out / v["id"]
        d.mkdir(parents=True, exist_ok=True)
        (d / "background.svg").write_text(svg(bg, bg_defs, f"{v['name']} background"))
        (d / "foreground.svg").write_text(svg(fg, fg_defs, f"{v['name']} foreground"))
        (d / "foreground-small.svg").write_text(svg(small, small_defs, f"{v['name']} foreground, 16-32 px"))
        (d / "monochrome.svg").write_text(svg(mono, "", f"{v['name']} monochrome"))
        (d / "full.svg").write_text(svg(bg + fg, bg_defs + fg_defs, f"{v['name']} (unmasked)"))
        meta.append({"id": v["id"], "name": v["name"], "idea": v["note"], "palette": v["mode"], "notes": ""})
    (out / "concepts.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False) + "\n")
    print("wrote", len(meta), "variants to", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "design/icons/round3")
