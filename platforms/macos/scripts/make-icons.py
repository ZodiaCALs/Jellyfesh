#!/usr/bin/env python3
"""Render the macOS icon assets from design/icons/round4/ (ADR-0017).

Writes PNGs into platforms/macos/Jellyfesh/Resources/Assets.xcassets:

* AppIcon.appiconset: Ice, the bundle icon, in the ten macOS sizes. 16 and
  32 px use the simplified master (foreground-small.svg), larger sizes the
  full glyph. The art sits in the macOS icon shape (824 of 1024 px, rounded,
  with a soft shadow), as macOS 11-15 icons do.
* DockIcon-<id>.imageset: the same shape for every option except Ice (the
  bundle icon already is Ice), for NSApplication.applicationIconImage. Steel,
  the default dark icon, gets 1024 px; the other eight 512 px, which is the
  largest a Dock tile shows on a Retina screen, to keep the app small.
* IconThumb-<id>.imageset: the full-bleed square art at 64 pt (64 and 128
  px) for the picker in Settings, which rounds the corners itself.

ADR-0017 plans vectors rasterised on first selection for the eight other
icons; macOS has no public SVG renderer that handles these files' filters and
masks, so they ship as small PNGs instead.

Needs resvg-py and Pillow (pip install resvg-py pillow): resvg renders the
luminance masks, the blur and the grain of these files correctly, which
cairosvg does not (it fills the whole tile with the line gradient). Run from
anywhere:

    python3 platforms/macos/scripts/make-icons.py
"""
import io
import json
import math
import pathlib
import sys

try:
    import resvg_py
    from PIL import Image, ImageDraw, ImageFilter
except ImportError:  # pragma: no cover - a hint for people running it by hand
    sys.exit("make-icons.py needs resvg-py and Pillow: pip install resvg-py pillow")

ROOT = pathlib.Path(__file__).resolve().parents[3]
ICONS = ROOT / "design" / "icons" / "round4"
ASSETS = ROOT / "platforms" / "macos" / "Jellyfesh" / "Resources" / "Assets.xcassets"

# The ten options in the order Settings shows them (design/tokens.json brand.icon-options).
OPTIONS = ["ice", "steel", "original", "navy-tide", "royal", "teal-deep", "sea-glass", "electric", "abyss", "lagoon"]
BUNDLE_ICON = "ice"
DARK_DEFAULT = "steel"

# macOS 11-15 icon grid on a 1024 px canvas: an 824 px body, centred.
BODY = 824 / 1024
# Superellipse exponent that matches the macOS icon outline closely.
SHAPE_EXPONENT = 5.0
# Drop shadow on the 1024 px canvas: offset, blur radius, opacity.
SHADOW_Y = 12 / 1024
SHADOW_BLUR = 14 / 1024
SHADOW_ALPHA = 0.28
# Pixel sizes that use the simplified master (ADR-0017: 16 and 32 px).
SMALL_MASTER_MAX = 32
SUPERSAMPLE = 4


def render_svg(path: pathlib.Path, size: int) -> Image.Image:
    png = resvg_py.svg_to_bytes(svg_path=str(path), width=size, height=size)
    return Image.open(io.BytesIO(bytes(png))).convert("RGBA")


def square_art(option: str, size: int, small: bool = False) -> Image.Image:
    """The unmasked square art rendered at `size` px; `small` picks the 16 and 32 px master."""
    folder = ICONS / option
    if small:
        art = render_svg(folder / "background.svg", size)
        art.alpha_composite(render_svg(folder / "foreground-small.svg", size))
        return art
    return render_svg(folder / "full.svg", size)


def shape_mask(size: int) -> Image.Image:
    """An anti-aliased superellipse filling a `size` px square."""
    big = size * SUPERSAMPLE
    half = big / 2
    points = []
    steps = 2048
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        x = half + half * math.copysign(abs(c) ** (2 / SHAPE_EXPONENT), c)
        y = half + half * math.copysign(abs(s) ** (2 / SHAPE_EXPONENT), s)
        points.append((x, y))
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).polygon(points, fill=255)
    return mask.resize((size, size), Image.LANCZOS)


def mac_icon(option: str, size: int) -> Image.Image:
    """The option in the macOS icon shape with its shadow, on a transparent `size` px canvas."""
    body = max(1, round(size * BODY))
    offset = (size - body) // 2
    mask = shape_mask(body)

    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    if size >= 64:
        shadow_mask = Image.new("L", (size, size), 0)
        shadow_mask.paste(mask, (offset, offset + max(1, round(size * SHADOW_Y))))
        shadow_mask = shadow_mask.filter(ImageFilter.GaussianBlur(size * SHADOW_BLUR))
        shadow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        shadow.putalpha(shadow_mask.point(lambda v: round(v * SHADOW_ALPHA)))
        canvas.alpha_composite(shadow)

    # Render the art larger and scale down for clean edges at small sizes.
    render = body * (SUPERSAMPLE if body < 256 else 1)
    art = square_art(option, render, small=size <= SMALL_MASTER_MAX)
    if art.size != (body, body):
        art = art.resize((body, body), Image.LANCZOS)
    art.putalpha(Image.composite(art.getchannel("A"), Image.new("L", art.size, 0), mask))
    canvas.alpha_composite(art, (offset, offset))
    return canvas


def save(image: Image.Image, path: pathlib.Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, optimize=True)


def write_json(path: pathlib.Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def app_icon_set() -> None:
    folder = ASSETS / "AppIcon.appiconset"
    images = []
    for points in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            pixels = points * scale
            name = f"icon_{points}x{points}{'@2x' if scale == 2 else ''}.png"
            save(mac_icon(BUNDLE_ICON, pixels), folder / name)
            images.append({"filename": name, "idiom": "mac", "scale": f"{scale}x", "size": f"{points}x{points}"})
    write_json(folder / "Contents.json", {"images": images, "info": {"author": "xcode", "version": 1}})


def image_set(name: str, files: list) -> None:
    """An image set from (filename, scale, image) triples."""
    folder = ASSETS / f"{name}.imageset"
    images = []
    for filename, scale, image in files:
        save(image, folder / filename)
        images.append({"filename": filename, "idiom": "universal", "scale": scale})
    write_json(folder / "Contents.json", {"images": images, "info": {"author": "xcode", "version": 1}})


def main() -> None:
    write_json(ASSETS / "Contents.json", {"info": {"author": "xcode", "version": 1}})
    app_icon_set()
    for option in OPTIONS:
        if option != BUNDLE_ICON:
            largest = 1024 if option == DARK_DEFAULT else 512
            image_set(
                f"DockIcon-{option}",
                [
                    (f"dock-{option}-{largest // 2}.png", "1x", mac_icon(option, largest // 2)),
                    (f"dock-{option}-{largest}.png", "2x", mac_icon(option, largest)),
                ],
            )
        image_set(
            f"IconThumb-{option}",
            [
                (f"thumb-{option}-64.png", "1x", square_art(option, 64)),
                (f"thumb-{option}-128.png", "2x", square_art(option, 128)),
            ],
        )
    total = sum(p.stat().st_size for p in ASSETS.rglob("*.png"))
    print(f"wrote {ASSETS.relative_to(ROOT)} ({total / 1024:.0f} KiB of PNG)")


if __name__ == "__main__":
    main()
