#!/usr/bin/env python3
"""Write Jellyfesh/Sources/Design/FallbackTheme.swift: the theme at the default hue.

The core resolves every theme colour from one hue and sends sRGB (Misty
PLT-03). Before the first track plays there is no theme snapshot, so the app
needs the theme at the default hue (250) for its first frame
(docs/design-tokens.md, "generated hue-250 snapshot as the first-frame
fallback"). This script reads the colour definitions the core itself uses
(core/jf-tokens/src/generated.rs, THEME_COLORS) and resolves them exactly as
jf-palette does: CSS Color 4 OKLab matrices in double precision, each channel
clipped to 0..1, 8-bit rounding as floor(v * 255 + 0.5).

It checks itself against the Misty hex values that jf-palette's tests pin,
then writes the Swift file. Run it after design/tokens.json changes:

    python3 platforms/macos/scripts/make-fallback-theme.py
"""
import math
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[3]
GENERATED = ROOT / "core" / "jf-tokens" / "src" / "generated.rs"
OUT = ROOT / "platforms" / "macos" / "Jellyfesh" / "Sources" / "Design" / "FallbackTheme.swift"
HUE = 250.0

OKLAB_TO_LMS = [
    [1.0, 0.3963377773761749, 0.2158037573099136],
    [1.0, -0.1055613458156586, -0.0638541728258133],
    [1.0, -0.0894841775298119, -1.2914855480194092],
]
LMS_TO_XYZ = [
    [1.2268798758459243, -0.5578149944602171, 0.2813910456659647],
    [-0.0405757452148008, 1.1122868032803170, -0.0717110580655164],
    [-0.0763729366746601, -0.4214933324022432, 1.5869240198367816],
]
XYZ_TO_LINEAR_SRGB = [
    [12831.0 / 3959.0, -329.0 / 214.0, -1974.0 / 3959.0],
    [-851781.0 / 878810.0, 1648619.0 / 878810.0, 36519.0 / 878810.0],
    [705.0 / 12673.0, -2585.0 / 12673.0, 705.0 / 667.0],
]

# Hex values jf-palette's tests pin (Misty 1.1 at hue 250; alpha not included).
EXPECT_LIGHT = {
    "field.sky": "#eef3f9", "field.sky-high": "#f7fafe", "field.aurora-1": "#97d2ff",
    "field.aurora-2": "#e5ccff", "field.aurora-3": "#beefe4", "ink.ink-90": "#101820",
    "accent.accent": "#0059a7", "accent.accent-hover": "#00509d", "accent.accent-press": "#004794",
    "accent.accent-text": "#0059a2", "accent.accent-soft": "#d2ebff", "accent.on-accent": "#f9fcff",
    "glass.glass-edge": "#ffffff", "glass.cloud": "#f9fbfe", "glass.cloud-soft": "#f2f6fb",
    "elevation.scrim": "#0b1723",
}
EXPECT_DARK = {
    "field.sky": "#0a1016", "field.sky-high": "#12181e", "glass.cloud": "#191f25",
    "glass.cloud-soft": "#1f262e", "ink.ink-90": "#f6f9fc", "accent.accent": "#7abdff",
}


def mul(m, v):
    return [m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2] for i in range(3)]


def encode_srgb(v):
    a = abs(v)
    encoded = 1.055 * a ** (1 / 2.4) - 0.055 if a > 0.0031308 else 12.92 * a
    return math.copysign(encoded, v)


def oklch_to_srgb(l, c, h):
    h = math.radians(h)
    lms = [x * x * x for x in mul(OKLAB_TO_LMS, [l, c * math.cos(h), c * math.sin(h)])]
    linear = mul(XYZ_TO_LINEAR_SRGB, mul(LMS_TO_XYZ, lms))
    return [min(max(encode_srgb(x), 0.0), 1.0) for x in linear]


def quantize(v):
    return int(math.floor(min(max(v, 0.0), 1.0) * 255 + 0.5))


def f32(x):
    """Round through IEEE single precision, as the Rust tables store f32."""
    import struct

    return struct.unpack("f", struct.pack("f", x))[0]


def resolve(lcha, hue):
    l, c, h, alpha, relative = lcha
    h = f32(hue + h) if relative else h
    r, g, b = (quantize(x) for x in oklch_to_srgb(l, c, h))
    return r, g, b, quantize(alpha)


LCHA = r"Lcha::(relative|fixed)\(\s*([-\d.e]+),\s*([-\d.e]+),\s*([-\d.e]+),\s*([-\d.e]+)\s*\)"
DEF = re.compile(
    r'pub const \w+: ColorDef = ColorDef \{\s*name: "([^"]+)",\s*light: ' + LCHA + r",\s*dark: " + LCHA,
    re.S,
)


def parse():
    text = GENERATED.read_text()
    colors = {}
    for m in DEF.finditer(text):
        name = m.group(1)
        light = (f32(float(m.group(3))), f32(float(m.group(4))), f32(float(m.group(5))), f32(float(m.group(6))), m.group(2) == "relative")
        dark = (f32(float(m.group(8))), f32(float(m.group(9))), f32(float(m.group(10))), f32(float(m.group(11))), m.group(7) == "relative")
        colors[name] = (light, dark)
    if not colors:
        sys.exit(f"no colour definitions found in {GENERATED}")
    return colors


def hexcode(rgba):
    return "#{:02x}{:02x}{:02x}".format(*rgba[:3])


def main():
    colors = parse()
    light = {name: resolve(defs[0], HUE) for name, defs in sorted(colors.items())}
    dark = {name: resolve(defs[1], HUE) for name, defs in sorted(colors.items())}
    for expected, resolved, label in ((EXPECT_LIGHT, light, "light"), (EXPECT_DARK, dark, "dark")):
        for name, want in expected.items():
            got = hexcode(resolved[name])
            if got != want:
                sys.exit(f"{label} {name}: resolved {got}, jf-palette pins {want}")

    def table(theme):
        rows = []
        for name, (r, g, b, a) in theme.items():
            rows.append(f'        "{name}": 0x{r:02x}{g:02x}{b:02x}{a:02x},')
        return "\n".join(rows)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(
        "// @generated by platforms/macos/scripts/make-fallback-theme.py from core/jf-tokens/src/generated.rs.\n"
        "// Do not edit by hand.\n"
        "//\n"
        f"// The theme at the default hue ({HUE:g}), resolved exactly as jf-palette does, for the frames\n"
        "// before the core sends its first theme snapshot (docs/design-tokens.md). Every later colour\n"
        "// comes from the core's Theme slice.\n"
        "\n"
        "enum FallbackTheme {\n"
        f"    static let hue: Float = {HUE:g}\n"
        "\n"
        "    /// RGBA (0xRRGGBBAA) per colour name, light appearance.\n"
        "    static let light: [String: UInt32] = [\n"
        f"{table(light)}\n"
        "    ]\n"
        "\n"
        "    /// RGBA (0xRRGGBBAA) per colour name, dark appearance.\n"
        "    static let dark: [String: UInt32] = [\n"
        f"{table(dark)}\n"
        "    ]\n"
        "}\n"
    )
    print(f"wrote {OUT.relative_to(ROOT)} ({len(light)} colours, checked against jf-palette's pins)")


if __name__ == "__main__":
    main()
