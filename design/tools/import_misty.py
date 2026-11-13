#!/usr/bin/env python3
"""Build design/tokens.json: Misty 1.1 base tokens plus Jellyfesh extensions.

Reads the Misty token file as data (Design Tokens Format Module 2025.10) and
writes one self-contained token source for Jellyfesh. Re-run it when
upgrading Misty; Jellyfesh-specific tokens live in JELLYFESH below.

Misty's tokens are Apache-2.0 (c) 2026 ZodiaCALs; see NOTICE. This file
reads them; it does not run any Misty code.

Usage: python3 design/tools/import_misty.py <misty.tokens.json> design/tokens.json
"""
import copy
import json
import sys

PROPOSED = {"status": "proposed", "flag": "Default proposed in Step 0; replace with your value or approve."}


def convert(node):
    """Copy a Misty token tree, renaming the org.misty extension to org.jellyfesh."""
    if isinstance(node, dict):
        out = {}
        for k, v in node.items():
            if k == "$extensions" and isinstance(v, dict) and "org.misty" in v:
                ext = copy.deepcopy(v["org.misty"])
                ext.pop("css", None)  # CSS custom property names are Misty's, not ours
                out["$extensions"] = {"org.jellyfesh": {"from": "misty-1.1", **ext}}
            else:
                out[k] = convert(v)
        return out
    if isinstance(node, list):
        return [convert(x) for x in node]
    return node


def dim(value, unit="px"):
    return {"value": value, "unit": unit}


def ms(value):
    return {"$type": "duration", "$value": dim(value, "ms")}


def tok(type_, value, description, flagged=True, **ext):
    t = {"$type": type_, "$value": value, "$description": description}
    e = dict(PROPOSED) if flagged else {}
    e.update(ext)
    if e:
        t["$extensions"] = {"org.jellyfesh": e}
    return t


JELLYFESH = {
    "brand": {
        "$description": "Brand intent. Not rendered; guides review.",
        "mood": tok("string", "calm, precise, luminous, unhurried, tidal",
                    "Mood keywords for design review."),
        "hue-fallback": tok("number", 250, "Hue used when artwork is missing or nearly grey "
                            "(Misty preset “Mist”)."),
        "hue-min-chroma": tok("number", 0.04, "Minimum OKLCH chroma of the artwork's accent "
                              "cluster before its hue is used instead of the fallback."),
        "hue-source": tok("string", "artwork", "artwork | fixed. Appearance setting; "
                          "fixed uses brand.hue-fallback or the person's chosen hue."),
        "icon-default-light": tok("string", "ice", "App icon in light appearance (ADR-0017).", flagged=False,
                                  decided="2026-11-05"),
        "icon-default-dark": tok("string", "steel", "App icon in dark appearance (ADR-0017).", flagged=False,
                                 decided="2026-11-05"),
        "icon-options": tok("string", "ice, steel, original, navy-tide, royal, teal-deep, sea-glass, electric, "
                            "abyss, lagoon", "Icons offered in Settings → Appearance → App icon, after Automatic.",
                            flagged=False, decided="2026-11-05"),
        "wordmark-formal": tok("string", "type-caps", "Spaced capitals (design/brand) for About, installers, "
                               "documents and legal pages.", flagged=False, decided="2026-11-05"),
        "wordmark-hero": tok("string", "hero-biolume | hero-tentacle | hero-glass",
                             "Home-page hero wordmark; one of three options, awaiting your choice."),
        "style": tok("string", "artwork", "artwork | misty. Appearance setting built for comparison "
                     "(ADR-0013): artwork = blurred-artwork Now Playing backdrop and one liquid accent; "
                     "misty = Misty-conformant field at the artwork hue. Default chosen after the comparison."),
    },
    "type-roles": {
        "$description": "Interface roles mapped onto Misty's interface scale (TYP-03). "
                        "Sizes in pt/dp/epx at the default text size; platforms scale them.",
        "face": tok("string", "system", "Native interface face: SF Pro (macOS), Segoe UI "
                    "Variable (Windows), Adwaita Sans or Cantarell per GNOME version (Linux), "
                    "Roboto (Android). CJK falls back to PingFang/Hiragino/Apple SD Gothic Neo, "
                    "Microsoft YaHei UI/JhengHei UI/Yu Gothic UI/Malgun Gothic, Noto Sans CJK.",
                    flagged=False),
        "now-playing-title": tok("typography", {"fontSize": dim(28), "lineHeight": 1.1, "fontWeight": 700},
                                 "Track title on Now Playing (Misty Title)."),
        "now-playing-artist": tok("typography", {"fontSize": dim(16), "lineHeight": 1.4, "fontWeight": 600},
                                  "Artist and album line on Now Playing."),
        "list-title": tok("typography", {"fontSize": dim(14), "lineHeight": 1.3, "fontWeight": 500},
                          "Track title in a list row. Misty Small, tightened for 52 pt rows."),
        "list-secondary": tok("typography", {"fontSize": dim(12), "lineHeight": 1.3, "fontWeight": 400},
                              "Artist, album, duration in a list row."),
        "section": tok("typography", {"fontSize": dim(20), "lineHeight": 1.2, "fontWeight": 700},
                       "Section headings in Library and Search (Misty Heading)."),
    },
    "lyrics": {
        "$description": "Lyrics view. Restrained: no blur, no scale jumps on inactive lines.",
        "line": tok("typography", {"fontSize": dim(30), "lineHeight": 1.3, "fontWeight": 700},
                    "Every lyric line, active or not; size never changes, so nothing reflows.",
                    fluid={"min": 24, "max": 40, "basis": "window width"}),
        "line-cjk-line-height": tok("number", 1.6, "Line height for Han, Kana and Hangul lines "
                                    "(Misty TYP-08 uses 1.7 for body; display-size lyrics need less)."),
        "translation": tok("typography", {"fontSize": dim(18), "lineHeight": 1.45, "fontWeight": 500},
                           "Translation track under a line."),
        "romanization": tok("typography", {"fontSize": dim(15), "lineHeight": 1.45, "fontWeight": 500},
                            "Romanization track under a line."),
        "ink-active": tok("string", "ink-90", "Active line colour role."),
        "ink-inactive": tok("string", "ink-50", "Inactive lines (Misty tertiary ink)."),
        "ink-sung": tok("string", "ink-90", "Words already sung in the active line."),
        "ink-unsung": tok("string", "ink-70", "Words not yet sung in the active line: brighter than "
                          "inactive lines (ink-50), so the active line reads at a glance."),
        "line-gap": tok("dimension", dim(20), "Space between lyric lines (Misty s5)."),
        "anchor-position": tok("number", 0.38, "Active line sits at 38% of the viewport height."),
        "scroll": tok("string", "dur-3 fluid", "Line advance: 380 ms, Fluid. Reduced motion: jump."),
        "word-sweep": tok("string", "linear, word duration", "Karaoke fill is progress, so Linear "
                          "(Misty MOT-06). Reduced motion: whole line highlights at once."),
    },
    "artwork": {
        "radius-list": tok("dimension", dim(6), "List thumbnails (Misty r-sm, list icon tiles)."),
        "radius-grid": tok("dimension", dim(12), "Album grid covers (Misty r-lg, cards)."),
        "radius-hero": tok("dimension", dim(24), "Now Playing artwork (Misty r-2xl, hero media)."),
        "size-mini": tok("dimension", dim(40), "Mini-player artwork."),
    },
    "backdrop": {
        "$description": "Now Playing backdrop, baked once per artwork and appearance by jf-palette.",
        "downsample": tok("number", 0.25, "Scale before blurring; long edge capped at 256 px."),
        "blur-sigma-ratio": tok("number", 0.085, "Gaussian sigma as a fraction of the short edge "
                                "after downsampling (about 22 px at 256 px)."),
        "saturation": tok("number", 1.2, "Chroma multiplier after blur."),
        "l-band-dark": tok("number", [0.16, 0.34], "OKLab lightness band in dark appearance; keeps "
                           "ink-90 and ink-70 at 4.5:1 or more on every pixel."),
        "l-band-light": tok("number", [0.86, 0.95], "OKLab lightness band in light appearance."),
        "crossfade": tok("string", "dur-4 fluid", "Track change: 600 ms crossfade, Fluid."),
        "dither": tok("number", 1.0, "Dither amplitude in 8-bit steps, against banding."),
    },
    "edge-blur": {
        "$description": "Progressive blur where content scrolls under chrome.",
        "top-height": tok("dimension", dim(56), "Under the title bar or toolbar."),
        "bottom-height": tok("dimension", dim(48), "Above the mini-player."),
        "blur-max": tok("dimension", dim(32), "Blur at the chrome edge (Misty Veil blur)."),
        "tint-max": tok("number", 0.62, "Tint alpha at the chrome edge (Misty Veil, light; 0.68 dark)."),
        "ramp": tok("string", "smoothstep", "Blur and tint fall to zero along the height."),
    },
    "glass-budget": {
        "live-layers-desktop": tok("number", 3, "Most live backdrop-blur layers on one screen (desktop)."),
        "live-layers-mobile": tok("number", 2, "Most live backdrop-blur layers on one screen (Android)."),
        "liquid-accent": tok("string", "scrubber-thumb", "The one liquid-glass accent. Deviates from "
                             "Misty MAT-04; see ADR-0013."),
    },
    "player": {
        "mini-height": tok("dimension", dim(64), "Mini-player height (desktop; 72 on Android)."),
        "mini-radius": tok("dimension", dim(16), "Floating mini-player (Misty r-xl, large panels)."),
        "row-height": tok("dimension", dim(52), "Track row height."),
        "scrubber-track": tok("dimension", dim(4), "Scrubber track thickness (r-full)."),
        "scrubber-thumb": tok("dimension", dim(14), "Scrubber thumb diameter; 44 pt hit target."),
        "open-now-playing": tok("string", "Deeper 320 ms", "Mini-player to Now Playing (Misty transition)."),
        "close-now-playing": tok("string", "Back 300 ms", "Now Playing back to where you were."),
    },
}


def main(src, dst):
    misty = json.load(open(src, encoding="utf-8"))
    out = {
        "$description": "Jellyfesh design tokens: the single source for Swift, Kotlin, C#, Rust "
                        "and GTK CSS codegen. Base values come from the Misty Design Language 1.1 "
                        "token file (Apache-2.0, (c) 2026 ZodiaCALs); groups below “brand” are "
                        "Jellyfesh extensions. Everything marked status=proposed awaits approval. "
                        "Modified from Misty: the org.misty extension is renamed org.jellyfesh, "
                        "CSS custom-property names are removed, and Jellyfesh groups are added.",
        "$extensions": {"org.jellyfesh": {
            "version": "0.1.0-proposed",
            "base": "Misty 1.1 tokens/misty.tokens.json",
            "colour-conversion": "OKLCH to sRGB with the CSS Color 4 OKLab matrices, "
                                 "each channel clipped to 0..1 (matches Misty packages).",
            "hue": "Theme colours are resolved at run time from one hue, derived from artwork "
                   "by jf-palette (brand.hue-source).",
        }},
    }
    for k, v in misty.items():
        if k.startswith("$"):
            continue
        out[k] = convert(v)
    out.update(JELLYFESH)
    with open(dst, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print("wrote", dst)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
