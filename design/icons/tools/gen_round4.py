#!/usr/bin/env python3
"""Round 4: colour options for variant B (brief of 2026-11-05).

B's construction stays fixed: a vertical tile gradient with a soft glow at
the top, the Wavebell glyph in tinted translucent lines (90%), and a soft
shadow under the glyph for lift. Only the colours change.

Each option writes background.svg, foreground.svg, foreground-small.svg
(16/32 px master), monochrome.svg and full.svg.

Usage: python3 gen_round4.py <output-dir>
"""
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from gen_concepts import SIZE, oklch, vgrad, svg  # noqa: E402
from gen_round2 import glyph_mask, glyph_solid, tile  # noqa: E402
from gen_round3 import small_mask  # noqa: E402

# (name, top, bottom, glow (colour, alpha), line from, line to, shadow (colour, alpha), note)
OPTIONS = [
    ("original", "Original B",
     (0.74, 0.125, 208), (0.33, 0.15, 264), ((0.84, 0.10, 205), 0.40),
     (0.95, 0.04, 205), (0.74, 0.125, 208), ((0.22, 0.09, 262), 0.18),
     "Cyan to deep blue; ice-to-cyan lines. The approved construction."),
    ("navy-tide", "Navy Tide",
     (0.66, 0.12, 212), (0.25, 0.10, 262), ((0.74, 0.125, 208), 0.35),
     (0.95, 0.04, 205), (0.84, 0.10, 205), ((0.16, 0.07, 262), 0.22),
     "Aqua into navy: darker and more serious, the lines glow more."),
    ("royal", "Royal",
     (0.70, 0.12, 222), (0.34, 0.17, 266), ((0.82, 0.09, 218), 0.40),
     (0.95, 0.04, 215), (0.82, 0.09, 226), ((0.22, 0.10, 266), 0.20),
     "Sky cyan into royal blue; the bluest of the set."),
    ("teal-deep", "Teal Deep",
     (0.70, 0.11, 192), (0.32, 0.09, 232), ((0.84, 0.08, 188), 0.40),
     (0.95, 0.04, 190), (0.80, 0.10, 190), ((0.20, 0.07, 232), 0.20),
     "Teal into deep sea blue; greener and calmer."),
    ("sea-glass", "Sea Glass",
     (0.80, 0.09, 178), (0.42, 0.10, 222), ((0.90, 0.06, 175), 0.45),
     (0.97, 0.03, 180), (0.86, 0.08, 180), ((0.26, 0.07, 222), 0.22),
     "Pale aqua into teal-blue; the softest, most Misty-like."),
    ("electric", "Electric",
     (0.80, 0.14, 205), (0.45, 0.21, 258), ((0.90, 0.09, 200), 0.45),
     (0.98, 0.02, 205), (0.86, 0.11, 205), ((0.26, 0.14, 258), 0.22),
     "Vivid cyan into electric blue; the most saturated, loud in a dock."),
    ("steel", "Steel",
     (0.66, 0.05, 228), (0.30, 0.05, 250), ((0.80, 0.05, 225), 0.35),
     (0.95, 0.02, 220), (0.80, 0.08, 210), ((0.18, 0.04, 250), 0.22),
     "Desaturated blue-grey with cyan lines; quiet and professional."),
    ("abyss", "Abyss (dark)",
     (0.30, 0.10, 252), (0.16, 0.04, 258), ((0.72, 0.12, 210), 0.55),
     (0.84, 0.10, 205), (0.74, 0.125, 208), ((0.05, 0.02, 258), 0.30),
     "Navy to near-black with a cyan glow: a dark appearance for macOS 26 and themed docks."),
    ("ice", "Ice (light)",
     (0.97, 0.02, 210), (0.86, 0.06, 215), ((1.0, 0.0, 0), 0.60),
     (0.46, 0.14, 252), (0.34, 0.15, 262), ((0.50, 0.05, 230), 0.15),
     "Ice white to pale cyan with deep blue lines: a light appearance."),
    ("lagoon", "Lagoon",
     (0.86, 0.09, 200), (0.55, 0.15, 245), ((0.95, 0.04, 200), 0.50),
     (1.0, 0.0, 0), (0.95, 0.04, 205), ((0.30, 0.10, 245), 0.28),
     "Bright lagoon into azure with white lines and a deeper shadow."),
]


def build(opt):
    oid, name, top, bottom, (glow, galpha), l1, l2, (shadow, salpha), note = opt
    pid = oid.replace("-", "")
    bg_defs, bg = tile(pid, "vert", [(0, oklch(*top)), (1, oklch(*bottom))],
                       [(512, 120, 520, oklch(*glow), galpha)])
    paint = vgrad(f"{pid}p", 200, 820, [(0, oklch(*l1), 1), (1, oklch(*l2), 1)])

    def layer(mask_body, mid, with_shadow):
        defs = (paint + f'<mask id="{mid}" maskUnits="userSpaceOnUse" x="0" y="0" width="{SIZE}" '
                f'height="{SIZE}"><rect width="{SIZE}" height="{SIZE}" fill="#000"/>{mask_body}</mask>')
        body = ""
        if with_shadow:
            defs += (f'<filter id="{mid}s" x="-20%" y="-20%" width="140%" height="150%">'
                     f'<feGaussianBlur stdDeviation="14"/></filter>')
            body += (f'<g transform="translate(0 16)" filter="url(#{mid}s)" opacity="{salpha}">'
                     f'{glyph_solid(oklch(*shadow), "solid")}</g>')
        body += f'<rect width="{SIZE}" height="{SIZE}" fill="url(#{pid}p)" mask="url(#{mid})"/>'
        return defs, body

    fg_defs, fg = layer(glyph_mask("translucent", opacity=0.9), f"{pid}m", True)
    sm_defs, sm = layer(small_mask(), f"{pid}sm", False)
    return dict(id=oid, name=name, note=note, bg=(bg_defs, bg), fg=(fg_defs, fg), small=(sm_defs, sm),
                colours={"tile_top": oklch(*top), "tile_bottom": oklch(*bottom),
                         "line_top": oklch(*l1), "line_bottom": oklch(*l2)})


def main(out):
    out = pathlib.Path(out)
    meta = []
    mono = glyph_solid("#000", "solid", width_boost=6)
    for opt in OPTIONS:
        v = build(opt)
        d = out / v["id"]
        d.mkdir(parents=True, exist_ok=True)
        (d / "background.svg").write_text(svg(v["bg"][1], v["bg"][0], f"{v['name']} background"))
        (d / "foreground.svg").write_text(svg(v["fg"][1], v["fg"][0], f"{v['name']} foreground"))
        (d / "foreground-small.svg").write_text(svg(v["small"][1], v["small"][0], f"{v['name']} foreground, 16-32 px"))
        (d / "monochrome.svg").write_text(svg(mono, "", f"{v['name']} monochrome"))
        (d / "full.svg").write_text(svg(v["bg"][1] + v["fg"][1], v["bg"][0] + v["fg"][0], f"{v['name']} (unmasked)"))
        meta.append({"id": v["id"], "name": v["name"], "idea": v["note"], "palette": v["colours"], "notes": ""})
    (out / "concepts.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False) + "\n")
    print("wrote", len(meta), "colour options to", out)


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "design/icons/round4")
