#!/usr/bin/env python3
"""Render the Windows app's icons from design/icons/round4 (ADR-0017).

Writes into platforms/windows/src/Jellyfesh.App/Assets/:

* Icons/ice.ico, Icons/steel.ico: full .ico sets (16-256 px) for the two default icons; the
  executable carries ice.ico, and the app sets the window icon with WM_SETICON.
* Icons/<id>/<size>.png for all ten icons at 16, 24, 32, 48, 64 and 256 px: the picker's previews
  and the images the app packs into an .ico the first time an icon is chosen. 16 and 32 px use
  the simplified small master (foreground-small.svg on background.svg), as the ADR asks.
* Icons/automatic-64.png: Ice and Steel split on the diagonal, for the picker's Automatic choice.
* Taskbar/<glyph>-<theme>.ico: previous, play, pause and next for the taskbar thumbnail buttons,
  light glyphs for dark taskbars and dark glyphs for light ones.
* Package/*.png: the MSIX logos (only used when building an MSIX).

Windows icons get a rounded plate with a small margin rather than a full-bleed square.

Needs resvg-py and Pillow (pip install resvg-py pillow; cairosvg does not render the icons'
luminance masks correctly). Usage: python3 platforms/windows/scripts/make_icons.py
"""
import io
import pathlib

import resvg_py
from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parents[3]
ICONS = ROOT / "design/icons/round4"
ASSETS = ROOT / "platforms/windows/src/Jellyfesh.App/Assets"
IDS = ["ice", "steel", "original", "navy-tide", "royal", "teal-deep", "sea-glass", "electric", "abyss", "lagoon"]
PNG_SIZES = [16, 24, 32, 48, 64, 256]
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 256]
SMALL_MASTER_MAX = 32


def render(svg_path, size):
    data = resvg_py.svg_to_bytes(svg_path=str(svg_path), width=size, height=size, skip_system_fonts=True)
    return Image.open(io.BytesIO(bytes(data))).convert("RGBA")


def plate(icon_id, size):
    """The icon on a rounded plate: small master at 16-32 px, the full icon above."""
    # Render at 4x and downsample: crisper edges for the mask and the rounded plate.
    scale = 4
    big = size * scale
    margin = 0 if size <= 24 else round(big * 0.0625)
    inner = big - 2 * margin
    folder = ICONS / icon_id
    if size <= SMALL_MASTER_MAX:
        art = render(folder / "background.svg", inner)
        art.alpha_composite(render(folder / "foreground-small.svg", inner))
    else:
        art = render(folder / "full.svg", inner)
    radius = round(inner * (0.18 if size <= 24 else 0.22))
    mask = Image.new("L", (inner, inner), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, inner - 1, inner - 1), radius=radius, fill=255)
    art.putalpha(Image.composite(art.getchannel("A"), Image.new("L", art.size, 0), mask))
    canvas = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    canvas.alpha_composite(art, (margin, margin))
    return canvas.resize((size, size), Image.LANCZOS)


def png_bytes(image):
    out = io.BytesIO()
    image.save(out, format="PNG", optimize=True)
    return out.getvalue()


def write_ico(path, images):
    """An .ico whose entries are PNGs (what IcoFile.Write in Jellyfesh.Shared also writes)."""
    entries = sorted(images, key=lambda kv: kv[0])
    header = bytearray()
    header += (0).to_bytes(2, "little") + (1).to_bytes(2, "little") + len(entries).to_bytes(2, "little")
    offset = 6 + 16 * len(entries)
    blobs = []
    for size, image in entries:
        blob = png_bytes(image)
        dim = 0 if size >= 256 else size
        header += bytes([dim, dim, 0, 0]) + (1).to_bytes(2, "little") + (32).to_bytes(2, "little")
        header += len(blob).to_bytes(4, "little") + offset.to_bytes(4, "little")
        offset += len(blob)
        blobs.append(blob)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(bytes(header) + b"".join(blobs))


GLYPHS = {
    # The transport glyphs of the mock-ups (24-unit viewBox, 1.75 strokes; play and pause filled).
    "previous": '<path d="M6 5v14" fill="none" stroke="{c}" stroke-width="2" stroke-linecap="round"/>'
                '<path d="M19 6l-10 6 10 6z" fill="{c}" stroke="{c}" stroke-width="1.5" stroke-linejoin="round"/>',
    "next": '<path d="M18 5v14" fill="none" stroke="{c}" stroke-width="2" stroke-linecap="round"/>'
            '<path d="M5 6l10 6-10 6z" fill="{c}" stroke="{c}" stroke-width="1.5" stroke-linejoin="round"/>',
    "play": '<path d="M8 5.6v12.8a1 1 0 0 0 1.5.86l10-6.4a1 1 0 0 0 0-1.72l-10-6.4A1 1 0 0 0 8 5.6z" fill="{c}"/>',
    "pause": '<rect x="6.5" y="5" width="3.6" height="14" rx="1.2" fill="{c}"/>'
             '<rect x="13.9" y="5" width="3.6" height="14" rx="1.2" fill="{c}"/>',
}


def glyph(name, color, size):
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="{size}" height="{size}">{GLYPHS[name].format(c=color)}</svg>'
    data = resvg_py.svg_to_bytes(svg_string=svg, width=size, height=size, skip_system_fonts=True)
    return Image.open(io.BytesIO(bytes(data))).convert("RGBA")


def main():
    icons = ASSETS / "Icons"
    for icon_id in IDS:
        for size in PNG_SIZES:
            out = icons / icon_id / f"{size}.png"
            out.parent.mkdir(parents=True, exist_ok=True)
            plate(icon_id, size).save(out, optimize=True)
    for icon_id in ("ice", "steel"):
        write_ico(icons / f"{icon_id}.ico", [(s, plate(icon_id, s)) for s in ICO_SIZES])

    # Automatic: Ice above the diagonal, Steel below (as on the design canvas).
    ice, steel = plate("ice", 64), plate("steel", 64)
    split = Image.new("L", (64, 64), 0)
    ImageDraw.Draw(split).polygon([(64, 0), (64, 64), (0, 64)], fill=255)
    auto = ice.copy()
    auto.paste(steel, (0, 0), split)
    auto.save(icons / "automatic-64.png", optimize=True)

    taskbar = ASSETS / "Taskbar"
    for theme, color in (("light", "#f6f9fc"), ("dark", "#101820")):
        for name in GLYPHS:
            write_ico(taskbar / f"{name}-{theme}.ico", [(s, glyph(name, color, s)) for s in (16, 20, 24, 32)])

    package = ASSETS / "Package"
    package.mkdir(parents=True, exist_ok=True)
    for name, size in (("Square44x44Logo", 44), ("Square150x150Logo", 150), ("StoreLogo", 50)):
        plate("ice", size).save(package / f"{name}.png", optimize=True)
    for name, (w, h) in (("Wide310x150Logo", (310, 150)), ("SplashScreen", (620, 300))):
        canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        mark = plate("ice", h * 2 // 3)
        canvas.alpha_composite(mark, ((w - mark.width) // 2, (h - mark.height) // 2))
        canvas.save(package / f"{name}.png", optimize=True)

    total = sum(p.stat().st_size for p in ASSETS.rglob("*") if p.is_file())
    print(f"wrote {sum(1 for p in ASSETS.rglob('*') if p.is_file())} files, {total / 1024:.0f} KiB")


if __name__ == "__main__":
    main()
