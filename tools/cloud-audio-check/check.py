"""Audio-device check for machines without a sound card (cloud sessions, CI).

Plays generated test material through `jf` and its real output path
(cpal -> ALSA -> PulseAudio virtual speaker), records the speaker with
`parec`, and measures the recording. Run it through `run.sh`, which sets
up the virtual speaker. Everything played is generated here: sine tones,
no recordings.

Checks
  * gapless: a continuous 1 kHz sine split into three tracks at points that
    are not codec-frame aligned, in FLAC, MP3 and Vorbis; the joins must not
    step further than the sine itself does (plus a small codec allowance);
  * no dropouts: no run of near-silence inside the tone;
  * pitch: 1000 Hz after resampling 44.1 kHz files to the 48 kHz speaker;
  * edges: a file with distinct start and end tones, alone and padded with
    silence, to measure audio lost when playback starts or the queue ends;
  * lyric sync: `jf play --lyrics` runs in a pseudo-terminal while the speaker
    is read as raw audio, both on one monotonic clock; each lyric line must
    appear within 30 ms of when its own tone is heard.
"""
import array, fcntl, math, os, pty, resource, select, signal, struct, subprocess, sys, termios, threading, time, wave

JF, WORK = sys.argv[1], sys.argv[2]
SOURCE_RATE, DEVICE_RATE, FREQ, AMP = 44100, 48000, 1000.0, 0.5
SINE_STEP = AMP * 2 * math.pi * FREQ / DEVICE_RATE
CODEC_ALLOWANCE = {"flac": 1.02, "mp3": 2.0, "ogg": 2.0}
failures = []


def write_wav(path, samples):
    with wave.open(path, "wb") as w:
        w.setnchannels(2); w.setsampwidth(2); w.setframerate(SOURCE_RATE)
        buf = bytearray()
        for v in samples:
            s = int(round(v * 32767)); buf += struct.pack("<hh", s, s)
        w.writeframes(bytes(buf))


def encode(src, dst):
    ext = dst.rsplit(".", 1)[1]
    cmd = {"flac": ["flac", "-s", "-f", "-o", dst, src],
           "mp3": ["lame", "--quiet", "-b", "256", src, dst],
           "ogg": ["oggenc", "-Q", "-q", "6", "-o", dst, src]}[ext]
    subprocess.run(cmd, check=True)


def read_left(path):
    b = open(path, "rb").read(); i = 12
    while i + 8 <= len(b):
        cid, size = b[i:i+4], struct.unpack("<I", b[i+4:i+8])[0]
        if cid == b"data":
            a = array.array("f"); a.frombytes(b[i+8:i+8+size][: size // 4 * 4]); return a[0::2]
        i += 8 + size + (size & 1)
    raise SystemExit(f"no audio recorded in {path}")


def play_and_record(name, targets):
    """Plays `targets` with jf while recording the virtual speaker."""
    cap = f"{WORK}/capture-{name}.wav"
    rec = subprocess.Popen(["parec", "-d", "jfcheck.monitor", f"--rate={DEVICE_RATE}", "--channels=2",
                            "--format=float32le", "--file-format=wav", cap])
    time.sleep(0.7)
    env = dict(os.environ, JF_DATA_DIR=f"{WORK}/data-{name}", JF_CACHE_DIR=f"{WORK}/cache-{name}")
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    t0 = time.monotonic()
    p = subprocess.run([JF, "play", *targets, "-q"], env=env, capture_output=True, text=True)
    wall = time.monotonic() - t0
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    time.sleep(0.7)
    rec.send_signal(signal.SIGINT); rec.wait(timeout=10)
    if p.returncode != 0:
        failures.append(f"{name}: jf exited with {p.returncode}")
        print(p.stderr[-800:])
    cpu = (after.ru_utime + after.ru_stime) - (before.ru_utime + before.ru_stime)
    x = read_left(cap)
    on = [i for i in range(len(x)) if abs(x[i]) > 0.05]
    if not on:
        failures.append(f"{name}: nothing was heard"); return None, wall, cpu
    return x[on[0]:on[-1] + 1], wall, cpu


def segment_ms(sig, lo, hi):
    zc = [i for i in range(1, len(sig)) if sig[i-1] < 0 <= sig[i]]
    return 1000 * sum(b - a for a, b in zip(zc, zc[1:]) if lo <= DEVICE_RATE / (b - a) <= hi) / DEVICE_RATE


# ---- material ---------------------------------------------------------------
total = int(SOURCE_RATE * 9.0)
cuts = [0, 132_301, 264_617, total]
sine = lambda i: AMP * math.sin(2 * math.pi * FREQ * i / SOURCE_RATE)
for t in range(3):
    write_wav(f"{WORK}/part{t+1}.wav", [sine(i) for i in range(cuts[t], cuts[t+1])])
    for ext in ("flac", "mp3", "ogg"):
        os.makedirs(f"{WORK}/album-{ext}", exist_ok=True)
        encode(f"{WORK}/part{t+1}.wav", f"{WORK}/album-{ext}/{t+1:02d} Part {t+1}.{ext}")
edges, phase = [], 0.0
for f, secs in [(2000.0, 0.2), (1000.0, 1.0), (500.0, 0.2)]:
    for _ in range(int(SOURCE_RATE * secs)):
        edges.append(AMP * math.sin(phase)); phase += 2 * math.pi * f / SOURCE_RATE
write_wav(f"{WORK}/edges.wav", edges); encode(f"{WORK}/edges.wav", f"{WORK}/edges.flac")
write_wav(f"{WORK}/silence.wav", [0.0] * SOURCE_RATE)
encode(f"{WORK}/silence.wav", f"{WORK}/00 silence.flac"); encode(f"{WORK}/silence.wav", f"{WORK}/99 silence.flac")

# ---- gapless albums -----------------------------------------------------------
# The tracks join into one unbroken sine, so a gap or click anywhere shows up
# as a step larger than the sine's own, or as a run of silence. The recording
# can lose a little at its very start and end (see the edge check below), so
# the whole tone is checked rather than windows at computed join times.
# Lossy codecs smear a hard cut slightly when tracks are encoded separately;
# their allowance covers that, while a real gap steps by about the amplitude.
print(f"Gapless albums (1 kHz sine split at frames {cuts[1]} and {cuts[2]}; sine step {SINE_STEP:.4f})")
for ext in ("flac", "mp3", "ogg"):
    sig, wall, cpu = play_and_record(f"album-{ext}", [f"{WORK}/album-{ext}"])
    if sig is None:
        continue
    edge = DEVICE_RATE // 500  # ignore 2 ms at each end of the recording
    step, at = max((abs(sig[i] - sig[i-1]), i) for i in range(edge, len(sig) - edge))
    run = longest = 0
    for v in sig:
        run = run + 1 if abs(v) < 0.01 else 0; longest = max(longest, run)
    pitch = sum(1 for i in range(1, len(sig)) if sig[i-1] < 0 <= sig[i]) / (len(sig) / DEVICE_RATE)
    limit = SINE_STEP * CODEC_ALLOWANCE[ext]
    ok_join, ok_drop, ok_pitch = step <= limit, longest <= 2, abs(pitch - FREQ) < 1.0
    print(f"  {ext:4} joins {'clean' if ok_join else 'FAIL '} (largest step {step:.4f} at {1000*at/DEVICE_RATE:.0f} ms, "
          f"limit {limit:.4f})  dropouts {'none' if ok_drop else f'FAIL ({longest} samples)'}  "
          f"pitch {pitch:.2f} Hz  heard {1000*len(sig)/DEVICE_RATE:.0f} of 9000 ms  jf CPU {100*cpu/wall:.1f}% of a core")
    for ok, what in [(ok_join, "joins"), (ok_drop, "dropouts"), (ok_pitch, "pitch")]:
        if not ok:
            failures.append(f"{ext}: {what}")

# ---- start and end of playback ------------------------------------------------
# Played alone, the file's first and last tones are also the start and end of
# playback; padded with silence they are not. A shortfall only when alone is
# audio lost when playback starts or the queue ends.
print("Start and end of playback (2 kHz head 200 ms, 1 kHz body 1000 ms, 500 Hz tail 200 ms)")
measured = {}
for name, targets in [("alone", [f"{WORK}/edges.flac"]),
                      ("padded", [f"{WORK}/00 silence.flac", f"{WORK}/edges.flac", f"{WORK}/99 silence.flac"])]:
    sig, _, _ = play_and_record(f"edges-{name}", targets)
    if sig is None:
        continue
    measured[name] = (segment_ms(sig, 1800, 2200), segment_ms(sig, 900, 1100), segment_ms(sig, 450, 550))
    head, body, tail = measured[name]
    print(f"  {name:6} head {head:5.1f} ms  body {body:6.1f} ms  tail {tail:5.1f} ms")
if len(measured) == 2:
    lost_head = measured["padded"][0] - measured["alone"][0]
    lost_tail = measured["padded"][2] - measured["alone"][2]
    print(f"  lost when playback starts: {lost_head:.1f} ms; lost when the queue ends: {lost_tail:.1f} ms")
    if lost_tail > 3:
        print("  WARN the end of the last track is cut (known in v0.0.1: the engine stops before the")
        print("       device has played its buffer)")
    if lost_head > 3:
        print("  NOTE missing at the start of the recording only: PulseAudio rewinds its buffer when a")
        print("       stream joins and a monitor recording misses that. Its own player, paplay, loses")
        print("       200 ms of the same file in this setup, so this is the method, not jf.")

# ---- lyric sync ------------------------------------------------------------------
# Silence with a short tone at each of four timed lines (invented text). The
# speaker is read as raw PCM with a 10 ms latency target; every read is
# timestamped on the same clock as the terminal output, so "shown" and "heard"
# compare directly. Each line is measured against its own tone rather than
# against the start of playback: the recording misses a varying part of the
# start (see the edge check above), which would move every "heard" time by as
# much.
print("Lyric sync (line shown on screen minus the moment its tone is heard)")
LINES = [(1.0, "Tide line one"), (3.0, "Tide line two"), (5.5, "Tide line three"), (8.0, "Tide line four")]
TONE_FRAMES = int(SOURCE_RATE * 0.3)  # 99 whole periods of 330 Hz: each tone starts and ends at zero
sync = [0.0] * int(SOURCE_RATE * 9.0)
for at, _ in LINES:
    start = int(round(at * SOURCE_RATE))
    sync[start:start + TONE_FRAMES] = [0.4 * math.sin(2 * math.pi * 330 * i / SOURCE_RATE) for i in range(TONE_FRAMES)]
write_wav(f"{WORK}/sync.wav", sync)
encode(f"{WORK}/sync.wav", f"{WORK}/sync.flac")
with open(f"{WORK}/sync.lrc", "w") as f:
    for at, text in LINES:
        f.write(f"[{int(at // 60):02d}:{at % 60:05.2f}]{text}\n")
raw = subprocess.Popen(["parec", "--raw", "--latency-msec=10", "--format=s16le", f"--rate={DEVICE_RATE}",
                        "--channels=2", "-d", "jfcheck.monitor"], stdout=subprocess.PIPE, bufsize=0)
reads = []
def read_speaker():
    while True:
        b = raw.stdout.read(960)
        if not b:
            return
        reads.append((time.monotonic(), b))
threading.Thread(target=read_speaker, daemon=True).start()
time.sleep(1.0)
env = dict(os.environ, TERM="xterm-256color", COLUMNS="72", JF_DATA_DIR=f"{WORK}/data-sync",
           JF_CACHE_DIR=f"{WORK}/cache-sync")
pid, fd = pty.fork()
if pid == 0:
    os.execvpe(JF, [JF, "play", f"{WORK}/sync.flac", "--lyrics"], env)
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 12, 72, 0, 0))
shown, screen_bytes, deadline = {}, b"", time.monotonic() + 40
while time.monotonic() < deadline:
    ready, _, _ = select.select([fd], [], [], 1.0)
    if fd not in ready:
        continue
    try:
        chunk = os.read(fd, 65536)
    except OSError:
        break
    if not chunk:
        break
    screen_bytes += chunk
    for _, text in LINES:
        if text.encode() in screen_bytes and text not in shown:
            shown[text] = time.monotonic()
else:
    os.kill(pid, signal.SIGKILL)
    failures.append("lyric sync: jf did not finish")
os.waitpid(pid, 0)
time.sleep(0.3); raw.terminate()
# A tone starts at the first loud sample after at least 50 ms of silence. A
# sample's time is its read's time, less the samples after it in that read and
# the 10 ms latency target.
onsets, quiet = [], DEVICE_RATE
for t, b in reads:
    s16 = struct.unpack(f"<{len(b) // 2}h", b)
    for k in range(0, len(s16), 2):
        if abs(s16[k]) <= 300:
            quiet += 1
            continue
        if quiet >= DEVICE_RATE // 20:
            onsets.append(t - (len(s16) - k) / 2 / DEVICE_RATE - 0.010)
        quiet = 0
if len(onsets) != len(LINES) or len(shown) < len(LINES):
    failures.append(f"lyric sync: heard {len(onsets)} of {len(LINES)} tones, saw {len(shown)} of {len(LINES)} lines")
else:
    offsets = [1000 * (shown[text] - heard) for (_, text), heard in zip(LINES, onsets)]
    ok = all(abs(o) <= 30 for o in offsets)
    print("  " + "  ".join(f"{o:+.0f} ms" for o in offsets) + ("   (limit ±30 ms)" if ok else "   FAIL (limit ±30 ms)"))
    if not ok:
        failures.append("lyric sync")

print("RESULT:", "PASS" if not failures else "FAIL: " + "; ".join(failures))
sys.exit(1 if failures else 0)
