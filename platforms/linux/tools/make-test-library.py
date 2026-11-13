#!/usr/bin/env python3
"""Generate a small music library for testing and screenshots.

Every file is invented: sine tones tagged with made-up titles, artists and
albums, generated cover images, and a word-timed bilingual LRC file with
made-up lines (the placeholder lines of the design canvas). Nothing here is a
real release.

Usage: python3 make-test-library.py OUT_DIR [--tracks-per-album N]
Needs: ffmpeg, metaflac (flac). Runs in a few seconds.
"""

import argparse
import math
import os
import pathlib
import subprocess
import sys

ALBUMS = [
    # (album, album artist, year, hue (degrees), format, [(title, seconds)])
    ("Lanterns of the Bay", "The Quiet Fleet", 2024, 18, "flac", [
        ("Every Rope and Mast", 64), ("Harbour Lights", 32), ("Paper Boats", 28),
        ("Night Ferry", 36), ("Low Water", 30),
    ]),
    ("Tidal Atlas", "Mira Okafor", 2023, 205, "mp3", [
        ("North Current", 34), ("Salt Maps", 30), ("Kelp Forest", 33), ("Atlas Light", 29),
    ]),
    ("Glass Coral", "Nami Arakawa", 2022, 300, "flac", [
        ("硝子の珊瑚", 31), ("Tide Pool", 27), ("Moon Jelly", 35), ("Blue Hour", 30),
    ]),
    ("Slow Current", "Lin Yue", 2021, 145, "mp3", [
        ("慢潮", 33), ("River Lamps", 29), ("Willow Rain", 31),
    ]),
    ("Northern Lumen", "Aurora Vale", 2020, 265, "flac", [
        ("First Light", 28), ("Polar Drift", 34), ("Lumen", 30), ("Snow Glass", 32),
    ]),
    ("Deep Blue Hours", "The Quiet Fleet", 2019, 225, "mp3", [
        ("Sounding", 30), ("Abyssal Plain", 33), ("Lighthouse Keeper", 29),
    ]),
    ("Saltwater Letters", "Mira Okafor", 2018, 40, "flac", [
        ("Dear Shoreline", 31), ("Postmark", 28), ("Ink and Spray", 33),
    ]),
    ("Coral Static", "Nami Arakawa", 2025, 340, "mp3", [
        ("Signal Reef", 30), ("Static Bloom", 29), ("Neon Shoal", 32),
    ]),
]

# Word-timed lines with a Chinese translation on the same stamp (bilingual
# LRC), for "Every Rope and Mast". Times are seconds.
LYRICS = [
    (2.0, "Morning folds the harbour into light", "清晨把港湾折进光里"),
    (9.0, "Every rope and mast remembers night", "每根绳索与桅杆都还记得夜"),
    (16.0, "Slow tide, carry what I couldn't say", "慢潮啊，带走我说不出口的话"),
    (23.0, "Lanterns drifting out across the bay", "灯笼漂向海湾的另一边"),
    (30.0, "Hold still, the water knows the way", "别动，海水认得路"),
    (37.0, "Paper boats beneath a silver sky", "纸船在银色的天空下"),
    (44.0, "Every wave a whisper, every whisper why", "每道浪都是低语，每句低语都在问为什么"),
    (51.0, "Morning folds the harbour into light", "清晨把港湾折进光里"),
]


def oklch_to_srgb(l, c, h):
    """OKLCH to 8-bit sRGB (CSS Color 4 matrices, clipped)."""
    a, b = c * math.cos(math.radians(h)), c * math.sin(math.radians(h))
    l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3
    m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3
    s_ = (l - 0.0894841775 * a - 1.2914855480 * b) ** 3
    rgb = (
        4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
        -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
        -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_,
    )

    def gamma(x):
        x = min(max(x, 0.0), 1.0)
        return 12.92 * x if x <= 0.0031308 else 1.055 * x ** (1 / 2.4) - 0.055

    return tuple(int(round(gamma(x) * 255)) for x in rgb)


def cover(path, hue, seed):
    """A 600 px cover: a diagonal gradient at the album's hue with soft discs."""
    size = 600
    top = oklch_to_srgb(0.78, 0.13, hue)
    bottom = oklch_to_srgb(0.38, 0.12, (hue + 40) % 360)
    glow = oklch_to_srgb(0.92, 0.08, (hue + 330) % 360)
    discs = [
        (0.30 + 0.1 * math.sin(seed), 0.32, 0.22),
        (0.70, 0.62 + 0.08 * math.cos(seed * 1.7), 0.30),
        (0.55, 0.20, 0.12),
    ]
    rows = bytearray()
    for y in range(size):
        for x in range(size):
            t = (x + y) / (2 * size)
            r, g, b = (top[i] * (1 - t) + bottom[i] * t for i in range(3))
            for cx, cy, rad in discs:
                d = math.hypot(x / size - cx, y / size - cy) / rad
                if d < 1.0:
                    k = 0.35 * (1 - d) ** 2
                    r, g, b = r + (glow[0] - r) * k, g + (glow[1] - g) * k, b + (glow[2] - b) * k
            rows += bytes((int(r), int(g), int(b)))
    ppm = path.with_suffix(".ppm")
    ppm.write_bytes(b"P6 %d %d 255\n" % (size, size) + bytes(rows))
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(ppm), "-q:v", "3", str(path)], check=True)
    ppm.unlink()


def tone(path, seconds, freq, fmt, tags, art):
    meta = []
    for key, value in tags.items():
        meta += ["-metadata", f"{key}={value}"]
    src = ["-f", "lavfi", "-i", f"sine=frequency={freq}:sample_rate=44100:duration={seconds}"]
    if fmt == "mp3":
        cmd = ["ffmpeg", "-v", "error", "-y", *src, "-i", str(art), "-map", "0:a", "-map", "1:v",
               "-c:a", "libmp3lame", "-b:a", "128k", "-ac", "2", "-af", "volume=0.25",
               "-c:v", "mjpeg", "-id3v2_version", "3", "-metadata:s:v", "title=Album cover",
               "-metadata:s:v", "comment=Cover (front)", "-disposition:v", "attached_pic", *meta, str(path)]
        subprocess.run(cmd, check=True)
    else:
        cmd = ["ffmpeg", "-v", "error", "-y", *src, "-ac", "2", "-af", "volume=0.25",
               "-c:a", "flac", *meta, str(path)]
        subprocess.run(cmd, check=True)
        subprocess.run(["metaflac", f"--import-picture-from=3||Cover (front)||{art}", str(path)], check=True)


def lrc(path, seconds):
    out = ["[ti:Every Rope and Mast]", "[ar:The Quiet Fleet]", "[al:Lanterns of the Bay]"]
    stamp = lambda t: f"{int(t // 60):02d}:{t % 60:05.2f}"
    for i, (start, text, translation) in enumerate(LYRICS):
        end = LYRICS[i + 1][0] - 0.6 if i + 1 < len(LYRICS) else min(start + 6.0, seconds - 1)
        words = text.split(" ")
        step = (end - start) / len(words)
        timed = "".join(f"<{stamp(start + k * step)}>{w} " for k, w in enumerate(words)).rstrip()
        out.append(f"[{stamp(start)}]{timed}<{stamp(end)}>")
        out.append(f"[{stamp(start)}]{translation}")
    path.write_text("\n".join(out) + "\n", encoding="utf-8")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--tracks-per-album", type=int, default=0, help="limit tracks per album (0: all)")
    args = ap.parse_args()
    root = pathlib.Path(args.out)
    root.mkdir(parents=True, exist_ok=True)
    for n, (album, artist, year, hue, fmt, tracks) in enumerate(ALBUMS):
        folder = root / artist / album
        folder.mkdir(parents=True, exist_ok=True)
        art = folder / "cover.jpg"
        cover(art, hue, n + 1)
        if args.tracks_per_album:
            tracks = tracks[: args.tracks_per_album]
        for i, (title, seconds) in enumerate(tracks, start=1):
            path = folder / f"{i:02d} {title}.{fmt}"
            tags = {
                "title": title, "artist": artist, "album_artist": artist, "album": album,
                "track": f"{i}/{len(tracks)}", "date": str(year), "genre": "Ambient",
            }
            tone(path, seconds, 220 + 40 * i + 15 * n, fmt, tags, art)
            if title == "Every Rope and Mast":
                lrc(path.with_suffix(".lrc"), seconds)
    print(f"library in {root}: {sum(len(a[5]) for a in ALBUMS)} tracks, {len(ALBUMS)} albums")


if __name__ == "__main__":
    sys.exit(main())
