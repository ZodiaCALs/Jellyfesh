"""Draws the Subsonic and Jellyfin plugin icons (256x256 RGBA PNG) with
signed distance functions, so edges are anti-aliased without any image
library. Usage: python3 plugins/icons/draw.py plugins (writes <name>/icon.png for both)."""

import math
import struct
import sys
import zlib

SIZE = 256


def png(path, pixels):
    raw = bytearray()
    for y in range(SIZE):
        raw.append(0)
        for x in range(SIZE):
            raw.extend(pixels[y * SIZE + x])

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(data)


def coverage(d):
    """Pixel coverage for a signed distance (negative inside)."""
    return max(0.0, min(1.0, 0.5 - d))


def rounded_rect(px, py, x0, y0, x1, y1, r):
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
    hx, hy = (x1 - x0) / 2 - r, (y1 - y0) / 2 - r
    qx, qy = abs(px - cx) - hx, abs(py - cy) - hy
    outside = math.hypot(max(qx, 0), max(qy, 0))
    inside = min(max(qx, qy), 0)
    return outside + inside - r


def segment(px, py, ax, ay, bx, by):
    vx, vy = bx - ax, by - ay
    wx, wy = px - ax, py - ay
    t = max(0.0, min(1.0, (wx * vx + wy * vy) / (vx * vx + vy * vy)))
    return math.hypot(wx - t * vx, wy - t * vy)


def arc(px, py, cx, cy, r, a0, a1):
    """Distance to a circular arc from angle a0 to a1 (radians, a0 < a1,
    y pointing down)."""
    angle = math.atan2(py - cy, px - cx)
    while angle < a0:
        angle += 2 * math.pi
    if angle <= a1:
        return abs(math.hypot(px - cx, py - cy) - r)
    ends = [(cx + r * math.cos(a), cy + r * math.sin(a)) for a in (a0, a1)]
    return min(math.hypot(px - ex, py - ey) for ex, ey in ends)


def polygon_inside(px, py, points):
    inside = False
    n = len(points)
    for i in range(n):
        x0, y0 = points[i]
        x1, y1 = points[(i + 1) % n]
        if (y0 > py) != (y1 > py) and px < (x1 - x0) * (py - y0) / (y1 - y0) + x0:
            inside = not inside
    return inside


def polygon(px, py, points):
    """Signed distance to a closed polygon."""
    n = len(points)
    d = min(segment(px, py, *points[i], *points[(i + 1) % n]) for i in range(n))
    return -d if polygon_inside(px, py, points) else d


def lerp(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def hex_rgb(text):
    return tuple(int(text[i : i + 2], 16) for i in (1, 3, 5))


def render(path, top, bottom, glyph):
    top, bottom = hex_rgb(top), hex_rgb(bottom)
    pixels = []
    for y in range(SIZE):
        for x in range(SIZE):
            px, py = x + 0.5, y + 0.5
            shape = coverage(rounded_rect(px, py, 8, 8, 248, 248, 60))
            if shape <= 0:
                pixels.append((0, 0, 0, 0))
                continue
            t = max(0.0, min(1.0, (px + py - 16) / 464))
            colour = lerp(top, bottom, t)
            mark = glyph(px, py)
            colour = lerp(colour, (255, 255, 255), mark)
            pixels.append(tuple(int(round(c)) for c in colour) + (int(round(255 * shape)),))
    png(path, pixels)


def subsonic(px, py):
    """A point with three arcs: sound reaching out from a server."""
    alpha = 0.0
    cx, cy = 92, 180
    for radius, opacity in ((44, 0.95), (84, 0.8), (124, 0.62)):
        d = arc(px, py, cx, cy, radius, -math.pi / 2, 0) - 9
        alpha = max(alpha, coverage(d) * opacity)
    alpha = max(alpha, coverage(math.hypot(px - cx, py - cy) - 20))
    return alpha


def jellyfin(px, py):
    """A house outline with a play triangle: your media at home."""
    house = [(128, 54), (202, 116), (202, 202), (54, 202), (54, 116)]
    n = len(house)
    outline = min(segment(px, py, *house[i], *house[(i + 1) % n]) for i in range(n)) - 8
    play = polygon(px, py, [(110, 128), (110, 178), (154, 153)]) - 4
    return max(coverage(outline), coverage(play))


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "plugins"
    render(f"{out}/subsonic/icon.png", "#2a9bb5", "#23367f", subsonic)
    render(f"{out}/jellyfin/icon.png", "#8a5bd6", "#2b4aa8", jellyfin)
