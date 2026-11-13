#!/usr/bin/env bash
# Tests jf's audio-device path on a Linux machine without a sound card, such as
# a cloud session or a CI runner. It starts a private PulseAudio server with a
# virtual 48 kHz speaker, points ALSA at it for jf only (a private HOME holds
# the .asoundrc), plays generated test albums, records the speaker and checks
# the recording. Nothing outside a temporary folder is changed.
#
#   cargo build --release -p jf-cli
#   tools/cloud-audio-check/run.sh [path/to/jf]
#
# Needs: pulseaudio pulseaudio-utils libasound2-plugins flac lame vorbis-tools python3
# (Debian/Ubuntu: sudo apt-get install -y pulseaudio pulseaudio-utils libasound2-plugins flac lame vorbis-tools)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
jf="$(realpath "${1:-$here/../../target/release/jf}")"
[[ -x "$jf" ]] || { echo "jf not found at $jf: run cargo build --release -p jf-cli" >&2; exit 2; }
for tool in pulseaudio pactl parec flac lame oggenc python3; do
    command -v "$tool" >/dev/null || { echo "missing $tool (see the header of this script)" >&2; exit 2; }
done

work="$(mktemp -d)"
export XDG_RUNTIME_DIR="$work/runtime" HOME="$work/home"
mkdir -p "$XDG_RUNTIME_DIR" "$HOME" && chmod 700 "$XDG_RUNTIME_DIR"
printf 'pcm.!default { type pulse }\nctl.!default { type pulse }\n' > "$HOME/.asoundrc"

cleanup() { pulseaudio --kill 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT

pulseaudio --daemonize=yes --exit-idle-time=-1 -n \
    --load="module-native-protocol-unix" \
    --load="module-null-sink sink_name=jfcheck rate=48000 channels=2" \
    --log-target=file:"$work/pulseaudio.log"
for _ in $(seq 50); do pactl info >/dev/null 2>&1 && break; sleep 0.1; done
pactl set-default-sink jfcheck

echo "jf: $("$jf" --version); virtual speaker: $(pactl list short sinks | awk '{print $2, $4, $5, $6}')"
python3 -I "$here/check.py" "$jf" "$work" 2> >(grep -v -E '^(ALSA lib|Cannot connect|jack|Jack)' >&2)
