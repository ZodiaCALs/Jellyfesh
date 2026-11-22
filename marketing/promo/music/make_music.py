#!/usr/bin/env python3
"""The promo's music: an original bed synthesised here, with numpy only.

    python3 marketing/promo/music/make_music.py OUT.wav

48 kHz, 16-bit stereo, 70 s. 96 BPM, so a bar is 2.5 s and a beat 0.625 s: the
same grid as scene/scene.js, whose cuts land on these bars and whose hits land
on these beats. D major: a warm pad, glassy bells, a plucked arpeggio, a round
bass and light drums, an impact on every new chapter, and a quiet resolve
under the end card. No samples and no recordings: every sound is made below
from sine waves and noise with a fixed seed, so the file is the same each time.
"""
import sys
import wave

import numpy as np

SR = 48000
BPM = 96
BEAT = 60 / BPM            # 0.625 s
BAR = 4 * BEAT             # 2.5 s
LENGTH = 70.0
N = int(LENGTH * SR)
rng = np.random.default_rng(20261122)


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


def env_adsr(n, a, d, s, r, total):
    """An envelope n samples long: attack a, decay d to level s, release r (seconds)."""
    t = np.arange(n) / SR
    e = np.ones(n) * s
    e = np.where(t < a, t / max(a, 1e-4), e)
    dmask = (t >= a) & (t < a + d)
    e = np.where(dmask, 1 - (1 - s) * (t - a) / max(d, 1e-4), e)
    rel_start = total - r
    rmask = t >= rel_start
    e = np.where(rmask, e * np.clip(1 - (t - rel_start) / max(r, 1e-4), 0, 1), e)
    # Smooth the corners a little (no clicks).
    return e


def tail(sig, ms=20):
    """Fades the last `ms` of a voice to silence, so a cut-off tail never clicks."""
    n = min(len(sig), int(ms / 1000 * SR))
    ramp = 0.5 + 0.5 * np.cos(np.linspace(0, np.pi, n))
    sig = sig.copy()
    if sig.ndim == 1:
        sig[-n:] *= ramp
    else:
        sig[-n:] *= ramp[:, None]
    return sig


def place(buf, start, sig, pan=0.0):
    """Adds a mono or stereo signal into the stereo buffer at `start` seconds, panned."""
    i = int(round(start * SR))
    if i >= N:
        return
    if sig.ndim == 1:
        left = sig * np.sqrt(0.5 * (1 - pan))
        right = sig * np.sqrt(0.5 * (1 + pan))
        sig = np.stack([left, right], axis=1)
    j = min(N, i + len(sig))
    if i < 0:
        sig = sig[-i:]
        i = 0
    buf[i:j] += sig[: j - i]


def fft_filter(x, lo=None, hi=None, slope=0.25):
    """A zero-phase band filter by FFT, with raised-cosine edges (`slope` octaves wide)."""
    n = len(x)
    spec = np.fft.rfft(x, axis=0)
    f = np.fft.rfftfreq(n, 1 / SR)
    g = np.ones_like(f)
    with np.errstate(divide="ignore"):
        lf = np.log2(np.maximum(f, 1e-3))
    if lo:
        g *= np.clip((lf - np.log2(lo)) / slope + 0.5, 0, 1)
    if hi:
        g *= np.clip((np.log2(hi) - lf) / slope + 0.5, 0, 1)
    g = 0.5 - 0.5 * np.cos(np.pi * g)
    if x.ndim == 2:
        g = g[:, None]
    return np.fft.irfft(spec * g, n=n, axis=0)


# ---------- Voices ----------

def pad_note(midi, dur, amp=1.0, bright=1.0):
    """A soft pad: three slightly detuned additive voices, slow attack and release."""
    n = int((dur + 1.4) * SR)
    t = np.arange(n) / SR
    out = np.zeros((n, 2))
    f0 = hz(midi)
    for k, (cents, pan) in enumerate([(-7, -0.6), (0, 0.0), (6, 0.6)]):
        f = f0 * 2 ** (cents / 1200)
        vib = 1 + 0.0015 * np.sin(2 * np.pi * (0.21 + 0.07 * k) * t + k)
        phase = 2 * np.pi * f * np.cumsum(vib) / SR
        v = np.zeros(n)
        for h in range(1, 9):
            if f * h > 9000:
                break
            v += np.sin(h * phase + 0.3 * h * k) * (1 / h ** (1.9 - 0.5 * bright))
        out[:, 0] += v * np.sqrt(0.5 * (1 - pan))
        out[:, 1] += v * np.sqrt(0.5 * (1 + pan))
    e = env_adsr(n, 0.9, 0.6, 0.85, 1.3, n / SR)
    return out * e[:, None] * amp * 0.06


def air_note(midi, dur):
    """A soft sine an octave or two above the pad, with a slow tremolo: the mix's air."""
    n = int((dur + 1.4) * SR)
    t = np.arange(n) / SR
    f = hz(midi)
    v = np.sin(2 * np.pi * f * t) * (0.75 + 0.25 * np.sin(2 * np.pi * 0.4 * t + midi))
    e = env_adsr(n, 1.0, 0.5, 0.9, 1.3, n / SR)
    return np.stack([v * e, np.roll(v, 240) * e], axis=1) * 0.012


def bell(midi, amp=1.0, decay=1.6):
    """A glassy bell: FM with an inharmonic modulator and a quick shimmer."""
    dur = decay * 4
    n = int(dur * SR)
    t = np.arange(n) / SR
    f = hz(midi)
    index = 1.4 * np.exp(-t / 0.35)
    mod = np.sin(2 * np.pi * f * 3.5 * t) * index
    car = np.sin(2 * np.pi * f * t + mod)
    car += 0.35 * np.sin(2 * np.pi * f * 2.0 * t) * np.exp(-t / (decay * 0.5))
    a = np.minimum(t / 0.004, 1) * np.exp(-t / decay)
    return tail(car * a * amp * 0.09, 60)


def pluck(midi, amp=1.0, decay=0.42):
    """A soft plucked note: harmonics that fade faster the higher they are."""
    n = int(decay * 5 * SR)
    t = np.arange(n) / SR
    f = hz(midi)
    v = np.zeros(n)
    for h in range(1, 10):
        if f * h > 12000:
            break
        v += np.sin(2 * np.pi * f * h * t + h) * np.exp(-t * (1 / decay) * (1 + 0.55 * (h - 1))) / h
    a = np.minimum(t / 0.003, 1)
    return tail(v * a * amp * 0.11, 40)


def bass_note(midi, dur, amp=1.0):
    n = int((dur + 0.12) * SR)
    t = np.arange(n) / SR
    f = hz(midi)
    v = np.sin(2 * np.pi * f * t) + 0.32 * np.sin(2 * np.pi * 2 * f * t) + 0.08 * np.sin(2 * np.pi * 3 * f * t)
    v = np.tanh(1.4 * v) / np.tanh(1.4)
    e = env_adsr(n, 0.008, 0.25, 0.7, 0.1, n / SR)
    return v * e * amp * 0.22


def kick(amp=1.0):
    n = int(0.6 * SR)
    t = np.arange(n) / SR
    f = 46 + 110 * np.exp(-t / 0.035)
    phase = 2 * np.pi * np.cumsum(f) / SR
    v = np.sin(phase) * np.exp(-t / 0.2)
    click = fft_filter(rng.standard_normal(n), lo=1500, hi=6000) * np.exp(-t / 0.004) * 0.1
    return tail((v + click) * np.minimum(t / 0.001, 1) * amp * 0.5, 60)


NOISE = rng.standard_normal(SR * 2)
CLAP_NOISE = fft_filter(NOISE, lo=1100, hi=6000)
HAT_NOISE = fft_filter(NOISE, lo=6500)
AIR_NOISE = fft_filter(NOISE, lo=300, hi=9000)


def clap(amp=1.0):
    n = int(0.35 * SR)
    t = np.arange(n) / SR
    e = np.zeros(n)
    for k, d in enumerate([0.0, 0.009, 0.019]):
        e += np.where(t >= d, np.exp(-(t - d) / (0.006 if k < 2 else 0.07)), 0) * (0.7 if k < 2 else 1.0)
    return tail(CLAP_NOISE[:n] * e * amp * 0.16)


def hat(amp=1.0, decay=0.028):
    n = int(0.2 * SR)
    t = np.arange(n) / SR
    return tail(HAT_NOISE[1000 : 1000 + n] * np.exp(-t / decay) * np.minimum(t / 0.0015, 1) * amp * 0.07)


def impact(amp=1.0):
    """A soft boom for a new chapter: a falling sub, a breath of air, and a bright edge."""
    n = int(3.0 * SR)
    t = np.arange(n) / SR
    f = 34 + 40 * np.exp(-t / 0.18)
    sub = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.7)
    air = AIR_NOISE[:n] if n <= len(AIR_NOISE) else np.resize(AIR_NOISE, n)
    air = fft_filter(air, lo=200, hi=3500) * np.exp(-t / 0.45) * 0.35
    a = np.minimum(t / 0.004, 1)
    return tail((sub * 0.9 + air) * a * amp * 0.55, 200)


def riser(dur, amp=1.0):
    """Air that brightens and swells towards the next chapter."""
    n = int(dur * SR)
    t = np.arange(n) / SR
    x = t / dur
    bands = [(400, 900), (900, 2000), (2000, 4500), (4500, 10000)]
    noise = rng.standard_normal(n)
    out = np.zeros(n)
    for k, (lo, hi) in enumerate(bands):
        centre = (k + 0.5) / len(bands)
        w = np.exp(-((x - centre * 0.9 - 0.05) / 0.28) ** 2)
        out += fft_filter(noise, lo=lo, hi=hi) * w
    swell = x ** 2.2
    return out * swell * np.minimum((1 - x) / 0.02, 1) * amp * 0.12


# ---------- The arrangement ----------
CHORDS = {
    "Dmaj9": [50, 57, 61, 64, 66],
    "Bm9": [47, 54, 57, 61, 62],
    "Gmaj9": [43, 50, 54, 57, 59],
    "Aadd9": [45, 52, 59, 61, 64],
    "F#m7": [42, 49, 52, 57, 61],
    "Em9": [40, 47, 50, 54, 55],
}
ROOTS = {"Dmaj9": 38, "Bm9": 35, "Gmaj9": 43, "Aadd9": 45, "F#m7": 42, "Em9": 40}
# One chord a bar, 28 bars.
PROGRESSION = [
    "Dmaj9", "Bm9", "Gmaj9",                      # 0 – 7.5    cold open
    "Dmaj9", "Bm9", "Gmaj9", "Aadd9", "Dmaj9",    # 7.5 – 20   AI Lyrics
    "F#m7", "Gmaj9", "Aadd9",                     # 20 – 27.5  lyrics bar, Desktop Lyrics
    "Bm9", "Gmaj9", "Dmaj9", "Aadd9", "Bm9",      # 27.5 – 40  Customize
    "Gmaj9", "Dmaj9", "Aadd9", "Bm9",             # 40 – 50    Discover
    "Em9", "Gmaj9", "Aadd9",                      # 50 – 57.5  platforms
    "Gmaj9", "Aadd9",                             # 57.5 – 62.5 Pro
    "Dmaj9", "Dmaj9", "Dmaj9",                    # 62.5 – 70  end card
]
CHAPTERS = [7.5, 20.0, 27.5, 40.0, 50.0, 62.5]


def section(b):
    t = b * BAR
    if t < 7.5:
        return "intro"
    if t < 20:
        return "ai"
    if t < 27.5:
        return "bar"
    if t < 40:
        return "look"
    if t < 50:
        return "discover"
    if t < 57.5:
        return "platforms"
    if t < 62.5:
        return "pro"
    return "end"


def build():
    pad = np.zeros((N, 2))
    air = np.zeros((N, 2))
    keys = np.zeros((N, 2))      # bells and plucks
    bass = np.zeros((N, 2))
    drums = np.zeros((N, 2))
    fx = np.zeros((N, 2))
    kick_env = np.zeros(N)

    for b, name in enumerate(PROGRESSION):
        t0 = b * BAR
        sec = section(b)
        chord = CHORDS[name]
        # The pad: every bar, held into the next; a longer last chord.
        dur = BAR + 0.15 if b < 25 else (LENGTH - t0 - 1.6 if b == 25 else 0)
        if dur > 0:
            bright = 0.6 if sec in ("intro", "pro", "end") else 1.0
            amp = 0.75 if sec == "intro" and b == 0 else 1.0
            place(pad, t0, pad_note(chord[0] + 12, dur, amp * 0.55, bright))
            for m in chord[1:]:
                place(pad, t0, pad_note(m + 12 if sec in ("look", "platforms") and m < 52 else m, dur, amp, bright))
            # Air: the two top notes two octaves up, very soft, slowly breathing.
            for m in chord[-2:]:
                place(air, t0, air_note(m + 24, dur))
        # The bass: on beat 1 and the "and" of beat 2, from AI Lyrics to Pro's first bar.
        if sec in ("ai", "bar", "look", "discover", "platforms"):
            root = ROOTS[name]
            root = root - 12 if root > 40 else root
            place(bass, t0, bass_note(root, BEAT * 1.4, 1.0))
            place(bass, t0 + BEAT * 1.5, bass_note(root, BEAT * 0.9, 0.75))
            place(bass, t0 + BEAT * 3, bass_note(root + 7 if sec == "look" else root, BEAT * 0.8, 0.7))
        # Drums.
        if sec in ("ai", "bar", "look", "discover", "platforms"):
            for beat in range(4):
                bt = t0 + beat * BEAT
                if beat in (0, 2) or (sec == "look" and beat == 3) or sec == "platforms":
                    k = kick(0.9 if beat == 0 else 0.75)
                    place(drums, bt, k)
                    i = int(bt * SR)
                    seg_len = min(N - i, int(0.35 * SR))
                    kick_env[i : i + seg_len] = np.maximum(kick_env[i : i + seg_len], np.exp(-np.arange(seg_len) / SR / 0.12))
                if beat in (1, 3) and sec in ("look", "discover", "platforms", "bar"):
                    place(drums, bt, clap(0.8 if sec == "look" else 0.6), pan=0.05)
                # Shaker: eighths, sixteenths in Customize and the platforms.
                steps = 4 if sec in ("look", "platforms") else 2
                for s in range(steps):
                    st = bt + s * BEAT / steps
                    accent = 1.0 if (s % 2 == 1) else 0.55
                    place(drums, st, hat(accent * (0.9 if sec == "look" else 0.7)), pan=0.35 if s % 2 else -0.2)
        # The plucked arpeggio: eighths in AI Lyrics and Discover, sixteenths in Customize.
        if sec in ("ai", "bar", "look", "discover", "platforms"):
            tones = sorted(chord)[1:] + [chord[2] + 12, chord[3] + 12]
            pattern = [0, 2, 3, 1, 4, 2, 5, 3]
            per = 8 if sec != "look" else 16
            for s in range(per):
                st = t0 + s * BAR / per
                m = tones[pattern[s % len(pattern)] % len(tones)] + 12
                vel = (0.75 if s % 2 == 0 else 0.5) * (0.8 if sec == "look" else 1.0)
                place(keys, st, pluck(m, vel, 0.35 if per == 16 else 0.45), pan=-0.35 if s % 2 else 0.35)

    # ---- The cold open: a note for each word, the lettering's glass arpeggio.
    for at, m in [(0.625, 74), (0.86, 78), (1.875, 81)]:
        place(keys, at, bell(m, 0.9, 1.8), pan=0.1)
    for k, m in enumerate([62, 66, 69, 74, 78, 81, 85, 86]):
        place(keys, 4.375 + k * 0.17, bell(m, 0.55 + 0.04 * k, 1.4), pan=-0.5 + k * 0.14)
    place(keys, 5.75, bell(90, 0.45, 2.2), pan=0.2)
    # ---- Little sparkles where the screens do something.
    for at, m in [(10.0, 86), (14.85, 81), (14.85, 85), (22.5, 78), (24.6875, 86), (45.625, 83), (47.5, 81)]:
        place(keys, at, bell(m, 0.5, 1.2), pan=0.25)
    # The typeface changes, the layouts, and the five platforms.
    for at, m in [(33.75, 85), (34.0625, 86), (34.375, 88), (34.6875, 90)]:
        place(keys, at, bell(m, 0.42, 0.6), pan=0.3)
    for at, m in [(37.5, 81), (38.4375, 83), (39.375, 86)]:
        place(keys, at, bell(m, 0.55, 1.0), pan=-0.2)
    for k, m in enumerate([74, 76, 78, 81, 83]):
        place(keys, 51.875 + k * BEAT, bell(m, 0.7, 1.3), pan=-0.6 + 0.3 * k)
    # ---- Pro: the drums rest; a bell line, then the hit on "Free during the public beta."
    for at, m in [(57.6, 78), (58.75, 81), (59.375, 83)]:
        place(keys, at, bell(m, 0.75, 1.6), pan=0.0)
    place(keys, 60.0, bell(86, 0.9, 2.0))
    place(keys, 60.0, bell(90, 0.5, 2.0), pan=0.3)
    # ---- The end: a falling arpeggio over the last chord.
    for k, m in enumerate([86, 81, 78, 74, 69, 66]):
        place(keys, 62.5 + 0.9 + k * 0.42, bell(m, 0.55 - 0.05 * k, 2.2), pan=0.4 - 0.16 * k)

    # ---- Chapters: an impact, and air rising into it.
    for at in CHAPTERS:
        place(fx, at, impact(1.0 if at not in (62.5,) else 0.85))
        place(fx, at - 1.875, np.stack([riser(1.875, 0.9), riser(1.875, 0.9)], axis=1))
    place(fx, 57.5, impact(0.55))
    place(fx, 60.0, impact(0.8))
    place(fx, 57.5 - 1.25, np.stack([riser(1.25, 0.8), riser(1.25, 0.8)], axis=1))

    # Pad and bass make room for the kick (a gentle pump).
    duck = 1 - 0.32 * kick_env
    pad *= duck[:, None]
    bass *= (1 - 0.45 * kick_env)[:, None]
    return pad + air, keys, bass, drums, fx


def reverb(x, seconds=2.6, bright=0.5):
    """A stereo hall: exponentially decaying, slightly different noise in each ear."""
    n = int(seconds * SR)
    t = np.arange(n) / SR
    ir = np.zeros((n, 2))
    for ch in range(2):
        noise = rng.standard_normal(n)
        noise = fft_filter(noise, lo=180, hi=7000 * bright + 1500)
        ir[:, ch] = noise * np.exp(-t / (seconds / 6.5))
    ir[: int(0.012 * SR)] = 0
    ir /= np.sqrt((ir ** 2).sum(axis=0))
    size = 1 << int(np.ceil(np.log2(len(x) + n)))
    out = np.zeros_like(x)
    for ch in range(2):
        X = np.fft.rfft(x[:, ch], size)
        H = np.fft.rfft(ir[:, ch], size)
        out[:, ch] = np.fft.irfft(X * H, size)[: len(x)]
    return out


def eq(x):
    """The mix's tone: less low-mid mud, a little presence and air."""
    n = len(x)
    spec = np.fft.rfft(x, axis=0)
    f = np.maximum(np.fft.rfftfreq(n, 1 / SR), 1.0)
    lf = np.log2(f)
    db = -3.0 * np.exp(-((lf - np.log2(280)) / 0.9) ** 2)      # low mids
    db += 2.0 * np.exp(-((lf - np.log2(4200)) / 1.1) ** 2)     # presence
    db += 1.5 / (1 + np.exp(-(lf - np.log2(9500)) * 3))        # air
    return np.fft.irfft(spec * (10 ** (db / 20))[:, None], n=n, axis=0)


def compress(x, threshold_db=-16.0, ratio=1.6, attack=0.01, release=0.18):
    """Gentle bus compression from a smoothed level (10 ms blocks, then per sample)."""
    block = int(0.01 * SR)
    m = len(x) // block
    level = np.sqrt((x[: m * block] ** 2).mean(axis=1).reshape(m, block).mean(axis=1) + 1e-12)
    db = 20 * np.log10(level)
    over = np.maximum(db - threshold_db, 0)
    gain_db = -over * (1 - 1 / ratio)
    smooth = np.zeros(m)
    g = 0.0
    a_att = np.exp(-0.01 / attack)
    a_rel = np.exp(-0.01 / release)
    for i in range(m):
        target = gain_db[i]
        coef = a_att if target < g else a_rel
        g = coef * g + (1 - coef) * target
        smooth[i] = g
    gains = np.interp(np.arange(len(x)), np.arange(m) * block + block / 2, smooth)
    return x * (10 ** (gains / 20))[:, None]


def main():
    out_path = sys.argv[1] if len(sys.argv) > 1 else "music.wav"
    pad, keys, bass, drums, fx = build()
    dry = pad * 0.9 + keys * 0.85 + bass * 0.95 + drums * 0.9 + fx * 0.8
    send = pad * 0.35 + keys * 0.6 + drums * 0.08 + fx * 0.35
    wet = reverb(send, 2.8, 0.55)
    mix = dry + wet * 0.55
    mix = fft_filter(mix, lo=28, slope=0.5)          # no rumble below the bass
    mix = eq(mix)
    mix = compress(mix)
    # The shape: a quieter cold open that grows into the first chapter, a breath at Pro.
    t = np.arange(N) / SR
    shape_db = np.interp(t, [0, 4.3, 7.3, 7.5, 57.2, 57.5, 59.9, 60.0, 62.4, 62.5], [-5, -3.5, -1.5, 0, 0, -2.5, -2.0, 0, -0.5, 0])
    mix *= (10 ** (shape_db / 20))[:, None]
    # The fade at the very start and the long fade under the end card.
    fade = np.clip(t / 0.03, 0, 1) * np.clip((LENGTH - 0.05 - t) / 3.2, 0, 1) ** 1.6
    mix *= fade[:, None]
    # Loudness about -16 LUFS for the web, peaks kept under -1.5 dBFS with a soft knee.
    rms = np.sqrt((mix[int(7.5 * SR) : int(57.5 * SR)] ** 2).mean())
    mix *= 0.135 / max(rms, 1e-9)
    knee = 10 ** (-3.0 / 20)
    a = np.abs(mix)
    over = a > knee
    mix[over] = np.sign(mix[over]) * (knee + (1 - knee) * np.tanh((a[over] - knee) / (1 - knee)))
    peak = np.abs(mix).max()
    ceiling = 10 ** (-1.5 / 20)
    if peak > ceiling:
        mix *= ceiling / peak
    pcm = np.clip(np.round(mix * 32767), -32768, 32767).astype("<i2")
    with wave.open(out_path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())
    print(f"{out_path}: {LENGTH:.0f} s, peak {20 * np.log10(np.abs(mix).max()):.2f} dBFS")


if __name__ == "__main__":
    main()
