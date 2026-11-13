#!/usr/bin/env bash
# Generates the codec fixtures in this folder: short sine tones, never recordings.
#
#   core/jf-engine/tests/fixtures/make-fixtures.sh
#
# Needs ffmpeg with its native `aac` encoder and `libopus`, and python3. The files are
# committed, so tests do not need ffmpeg; rerun this only to change them (encoders differ
# between ffmpeg versions, so a rerun changes the bytes, not what the tests check).
#
# Every pair is one continuous sine split at a frame that is not a multiple of the codec's
# frame size, so a join is only seamless when priming and padding are trimmed exactly:
#
#   aac-part1.m4a   1 kHz, 44.1 kHz mono, frames 0..26 461 (AAC LC, 64 kbit/s); gapless from
#                   ffmpeg's edit list (elst) and its trimmed last sample duration
#   aac-part2.m4a   frames 26 461..44 101
#   aac-smpb.m4a    aac-part1.m4a with the edit list removed and an iTunSMPB tag added instead
#   opus-part1.opus 1 kHz, 48 kHz mono, frames 0..33 601 (libopus, 64 kbit/s, 250 ms pages)
#   opus-part2.opus frames 33 601..53 601
#   opus-gain.opus  opus-part1.opus with an OpusHead output gain of -6 dB (Q7.8: -1536)
set -euo pipefail
cd "$(dirname "$0")"
ffmpeg=${FFMPEG:-/usr/bin/ffmpeg}

# A sine continuing from frame `start`: aevalsrc evaluates t = n / rate per sample.
tone() { # rate start frames codec args... output
    local rate=$1 start=$2 frames=$3
    shift 3
    "$ffmpeg" -hide_banner -loglevel error -y -f lavfi \
        -i "aevalsrc=exprs='0.5*sin(2*PI*1000*(t+${start}/${rate}))':s=${rate}:d=5" \
        -af "atrim=end_sample=${frames}" -ac 1 "$@"
}

tone 44100 0 26461 -c:a aac -b:a 64k -map_metadata -1 -fflags +bitexact aac-part1.m4a
tone 44100 26461 17640 -c:a aac -b:a 64k -map_metadata -1 -fflags +bitexact aac-part2.m4a
tone 48000 0 33601 -c:a libopus -b:a 64k -page_duration 250000 -map_metadata -1 -fflags +bitexact opus-part1.opus
tone 48000 33601 20000 -c:a libopus -b:a 64k -page_duration 250000 -map_metadata -1 -fflags +bitexact opus-part2.opus

python3 -I - <<'EOF'
import struct


def boxes(data, start, end):
    """(type, start, size, header length) of the boxes in data[start:end]."""
    out, pos = [], start
    while pos + 8 <= end:
        size, kind = struct.unpack(">I4s", data[pos:pos + 8])
        header = 8
        if size == 1:
            size, header = struct.unpack(">Q", data[pos + 8:pos + 16])[0], 16
        elif size == 0:
            size = end - pos
        out.append((kind, pos, size, header))
        pos += size
    return out


def find(data, start, end, path):
    """The first box along `path` (a list of types), as (start, size, header)."""
    for kind, pos, size, header in boxes(data, start, end):
        if kind == path[0]:
            if len(path) == 1:
                return pos, size, header
            inner = pos + header + (4 if kind == b"meta" else 0)
            return find(data, inner, pos + size, path[1:])
    raise SystemExit(f"no {path[0]!r}")


def full_box(kind, payload, version=0):
    return struct.pack(">I4sI", 12 + len(payload), kind, version) + payload


def box(kind, payload):
    return struct.pack(">I4s", 8 + len(payload), kind) + payload


# --- aac-smpb.m4a: iTunSMPB instead of the edit list ---------------------------------------
data = bytearray(open("aac-part1.m4a", "rb").read())
moov_pos, moov_size, moov_header = find(data, 0, len(data), [b"moov"])
assert moov_pos + moov_size == len(data), "moov must be last"
trak = [b"moov", b"trak"]
elst_pos, _, _ = find(data, 0, len(data), trak + [b"edts", b"elst"])
media_time = struct.unpack(">i", data[elst_pos + 12 + 4 + 4:elst_pos + 12 + 8 + 4])[0]
stts_pos, _, _ = find(data, 0, len(data), trak + [b"mdia", b"minf", b"stbl", b"stts"])
count = struct.unpack(">I", data[stts_pos + 12:stts_pos + 16])[0]
entries = [struct.unpack(">II", data[stts_pos + 16 + 8 * i:stts_pos + 24 + 8 * i]) for i in range(count)]
total = sum(n * d for n, d in entries)
packets = sum(n for n, _ in entries)
valid = total - media_time
padding = packets * 1024 - media_time - valid
# Turn the edit list into padding of the same size: offsets stay valid.
edts_pos, _, _ = find(data, 0, len(data), trak + [b"edts"])
data[edts_pos + 4:edts_pos + 8] = b"free"
smpb = " ".join(["", "00000000", f"{media_time:08X}", f"{padding:08X}", f"{valid:016X}"] + ["00000000"] * 8)
tag = full_box(b"mean", b"com.apple.iTunes") + full_box(b"name", b"iTunSMPB")
tag += box(b"data", struct.pack(">II", 1, 0) + smpb.encode())
freeform = box(b"----", tag)
# Append the tag to moov/udta/meta/ilst and grow every enclosing box (moov is last in the file).
ilst_pos, ilst_size, _ = find(data, 0, len(data), [b"moov", b"udta", b"meta", b"ilst"])
data[ilst_pos + ilst_size:ilst_pos + ilst_size] = freeform
for path in ([b"moov", b"udta", b"meta", b"ilst"], [b"moov", b"udta", b"meta"], [b"moov", b"udta"], [b"moov"]):
    pos, size, _ = find(data, 0, len(data), path)
    struct.pack_into(">I", data, pos, size + len(freeform))
open("aac-smpb.m4a", "wb").write(data)
print(f"aac-smpb.m4a: iTunSMPB delay {media_time}, padding {padding}, length {valid}")

# --- opus-gain.opus: OpusHead output gain -6 dB -------------------------------------------
def ogg_crc(page):
    crc = 0
    for byte in page:
        crc ^= byte << 24
        for _ in range(8):
            crc = ((crc << 1) ^ 0x04C11DB7) if crc & 0x80000000 else crc << 1
            crc &= 0xFFFFFFFF
    return crc


ogg = bytearray(open("opus-part1.opus", "rb").read())
segments = ogg[26]
body = 27 + segments
assert ogg[body:body + 8] == b"OpusHead"
struct.pack_into("<h", ogg, body + 16, -1536)
page_len = body + sum(ogg[27:27 + segments])
struct.pack_into("<I", ogg, 22, 0)
struct.pack_into("<I", ogg, 22, ogg_crc(ogg[:page_len]))
open("opus-gain.opus", "wb").write(ogg)
print("opus-gain.opus: output gain -1536 (Q7.8, -6 dB)")
EOF
ls -l ./*.m4a ./*.opus
