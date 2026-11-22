#!/usr/bin/env python3
"""Makes the promo's sample artwork and the colours Jellyfesh gives it.

    python3 marketing/promo/tools/make_assets.py [--jf PATH]

* assets/covers/<id>.jpg: an abstract gradient cover for every invented
  album in ALBUMS (nothing photographic, no text, no real releases);
* assets/themes/<id>.json and assets/backdrops/<id>-{light,dark}.png: the
  theme colours and the baked Now Playing backdrop that Jellyfesh's own core
  makes from that cover (`jf theme`, the theme resolver every app uses), so
  the simulated screens carry the colours the apps would show.

The covers are deterministic (no randomness that depends on the machine).
`jf` is the command-line edition of Jellyfesh, built with
`cargo build -p jf-cli` (target/debug/jf); pass --jf to use another build.
Without it, only the covers are made and the existing themes stay.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
PROMO = os.path.dirname(HERE)
ROOT = os.path.dirname(os.path.dirname(PROMO))
SIZE = 800

# Invented albums. Colours are sRGB hex: the base gradient (from, to) and the
# glows laid over it. Styles: "blobs" (soft glows), "horizon" (a glowing disc
# over a horizon), "rings" (soft ripples), "beam" (a diagonal band of light).
ALBUMS = [
    ("low-tide", "Low Tide Lanterns", "Juniper Atlas", "horizon", ("#0b4652", "#041c28"), ["#3fc7c9", "#1b6f8a", "#eaf6ff"]),
    ("glasshouse", "Glasshouse Summer", "Marlowe Quay", "blobs", ("#ff6f61", "#7a1f5c"), ["#ffb347", "#ff3d7f", "#ffe0a3"]),
    ("night-ferry", "Night Ferry Postcards", "Elm & Ember", "beam", ("#1d1b4f", "#0b0a24"), ["#7c6cff", "#c38bff", "#5ec8ff"]),
    ("northern-arcade", "Northern Arcade", "Northbound Echo", "rings", ("#0a4f7a", "#071a33"), ["#3fd0e6", "#7ab8ff", "#b6f0ff"]),
    ("saltglass", "Saltglass", "Violet Harbour", "blobs", ("#cdb8f2", "#7d6bb8"), ["#9ef0d6", "#f3c4ff", "#ffffff"]),
    ("afterglow", "Afterglow Avenue", "The Lumen Choir", "horizon", ("#4a0f1f", "#1e0710"), ["#ff8a3d", "#ff3d5a", "#ffd27a"]),
    ("weightless", "Weightless Rooms", "Arlo Meridian", "beam", ("#a8d8ff", "#4f86c6"), ["#ffffff", "#d6ecff", "#7fb2ff"]),
    ("copper", "Copper Coastlines", "Odessa Bloom", "rings", ("#7a3b1d", "#1f3b3f"), ["#e3884a", "#3fa59a", "#ffc28a"]),
    ("greenhouse", "Midnight Greenhouse", "Saltwater Static", "blobs", ("#0f3d2e", "#061a24"), ["#3ddc97", "#1f6fd1", "#b8ff9e"]),
    ("signal-fires", "Signal Fires", "Fern & Forsythe", "blobs", ("#2a2f3a", "#111318"), ["#ff7a1a", "#ffb15c", "#ff4d2e"]),
    ("moth-light", "Moth Light", "Ivory Tides", "rings", ("#efe6cf", "#a99f8a"), ["#fff6d6", "#d9c08a", "#8c8fa8"]),
    ("velvet", "Velvet Static", "Cassia Drift", "beam", ("#3a0f3f", "#140616"), ["#ff6fb5", "#ff9a7a", "#b86bff"]),
]


def linear(hex_colour):
    c = np.array([int(hex_colour[i:i + 2], 16) / 255 for i in (1, 3, 5)])
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def to_srgb(x):
    x = np.clip(x, 0, 1)
    return np.where(x <= 0.0031308, x * 12.92, 1.055 * np.power(x, 1 / 2.4) - 0.055)


def glow(u, v, cx, cy, r, sharp=2.0):
    d = np.sqrt((u - cx) ** 2 + (v - cy) ** 2) / r
    return np.exp(-(d ** sharp))


def grain(seed, shape):
    """Fine, even grain from a fixed integer hash (the same on every machine)."""
    h, w = shape
    y, x = np.mgrid[0:h, 0:w].astype(np.uint64)
    n = (x * np.uint64(374761393) + y * np.uint64(668265263) + np.uint64(seed) * np.uint64(2246822519)) & np.uint64(0xFFFFFFFF)
    n = (n ^ (n >> np.uint64(13))) * np.uint64(1274126177) & np.uint64(0xFFFFFFFF)
    n = n ^ (n >> np.uint64(16))
    return (n.astype(np.float64) / 0xFFFFFFFF) - 0.5


def cover(index, style, base, glows):
    v, u = np.mgrid[0:SIZE, 0:SIZE] / (SIZE - 1)
    a, b = linear(base[0]), linear(base[1])
    t = np.clip((u * 0.45 + v * 0.75) / 1.2, 0, 1)[..., None]
    img = a * (1 - t) + b * t
    g = [linear(c) for c in glows]
    k = index * 0.37
    if style == "blobs":
        for j, (cx, cy, r, w) in enumerate([(0.28 + 0.1 * np.sin(k), 0.3, 0.42, 0.85), (0.78, 0.62 + 0.08 * np.cos(k), 0.38, 0.75), (0.45, 0.85, 0.3, 0.55)]):
            m = glow(u, v, cx, cy, r, 2.2)[..., None] * w
            img = img * (1 - m) + g[j] * m
    elif style == "horizon":
        sky = np.clip(1 - v * 1.4, 0, 1)[..., None] * 0.35
        img = img * (1 - sky) + g[1] * sky
        disc = glow(u, v, 0.5 + 0.12 * np.sin(k), 0.46, 0.17, 6.0)[..., None]
        halo = glow(u, v, 0.5 + 0.12 * np.sin(k), 0.46, 0.42, 1.6)[..., None] * 0.55
        img = img * (1 - halo) + g[0] * halo
        img = img * (1 - disc) + g[2] * disc
        sea = np.clip((v - 0.6) * 3.0, 0, 1)[..., None]
        shimmer = (0.5 + 0.5 * np.sin(v * 160 + np.sin(u * 9) * 2)) * glow(u, v, 0.5, 0.75, 0.3, 2)
        img = img * (1 - sea * 0.55) + (b * 0.6 + g[0] * 0.25 * shimmer[..., None]) * sea * 0.55
    elif style == "rings":
        cx, cy = 0.35 + 0.1 * np.cos(k), 0.4
        d = np.sqrt((u - cx) ** 2 + (v - cy) ** 2)
        ring = (0.5 + 0.5 * np.cos(d * 38)) * np.exp(-d * 2.2)
        img = img * (1 - ring[..., None] * 0.35) + g[0] * ring[..., None] * 0.35
        m = glow(u, v, cx, cy, 0.3, 2)[..., None] * 0.8
        img = img * (1 - m) + g[2] * m
        m2 = glow(u, v, 0.85, 0.85, 0.45, 2)[..., None] * 0.6
        img = img * (1 - m2) + g[1] * m2
    elif style == "beam":
        s = (u - v * 0.8 - 0.1)
        beam = np.exp(-((s - 0.15) / 0.16) ** 2)[..., None]
        img = img * (1 - beam * 0.75) + g[0] * beam * 0.75
        m = glow(u, v, 0.25, 0.2, 0.35, 2)[..., None] * 0.6
        img = img * (1 - m) + g[1] * m
        m2 = glow(u, v, 0.8, 0.9, 0.4, 2)[..., None] * 0.5
        img = img * (1 - m2) + g[2] * m2
    vignette = 1 - 0.18 * np.clip(np.sqrt((u - 0.5) ** 2 + (v - 0.5) ** 2) / 0.7, 0, 1)[..., None] ** 2
    img = to_srgb(img * vignette)
    img = img + grain(index + 1, (SIZE, SIZE))[..., None] * (2.2 / 255)
    return Image.fromarray(np.clip(img * 255 + 0.5, 0, 255).astype(np.uint8), "RGB")


def main():
    jf = None
    if "--jf" in sys.argv:
        jf = sys.argv[sys.argv.index("--jf") + 1]
    else:
        for candidate in (os.path.join(ROOT, "target", "release", "jf"), os.path.join(ROOT, "target", "debug", "jf")):
            if os.path.exists(candidate):
                jf = candidate
                break
    covers = os.path.join(PROMO, "assets", "covers")
    themes = os.path.join(PROMO, "assets", "themes")
    backdrops = os.path.join(PROMO, "assets", "backdrops")
    for d in (covers, themes, backdrops):
        os.makedirs(d, exist_ok=True)
    work = tempfile.mkdtemp(prefix="jf-promo-")
    env = dict(os.environ, JF_DATA_DIR=os.path.join(work, "data"), JF_CACHE_DIR=os.path.join(work, "cache"))
    try:
        for i, (cid, title, artist, style, base, glows) in enumerate(ALBUMS):
            path = os.path.join(covers, f"{cid}.jpg")
            cover(i, style, base, glows).save(path, quality=92, subsampling=0, optimize=True)
            if not jf:
                continue
            theme = {}
            for appearance in ("light", "dark"):
                args = [jf, "theme", path, "--json", "--out", os.path.join(backdrops, f"{cid}-{appearance}.png")]
                if appearance == "dark":
                    args.insert(3, "--dark")
                out = subprocess.run(args, env=env, check=True, capture_output=True, text=True).stdout
                theme[appearance] = json.loads(out)
            with open(os.path.join(themes, f"{cid}.json"), "w", encoding="utf-8") as fh:
                json.dump(theme, fh, indent=1, sort_keys=True)
            print(f"{cid}: hue {theme['light']['hue']:.0f}, accent {theme['dark']['colors']['accent.accent']}")
    finally:
        shutil.rmtree(work, ignore_errors=True)
    if not jf:
        print("jf not found: covers made, themes left as they were (build it with cargo build -p jf-cli)")


if __name__ == "__main__":
    main()
