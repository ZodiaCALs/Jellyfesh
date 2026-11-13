// Port of DisplayClock, next_second_us and next_step_us (core/jf-core/src/timing.rs) and
// PlaybackAnchor::position_at (core/jf-engine/src/anchor.rs). Tested against
// tests/contract/display-clock.json; keep it in step with the Rust original (docs/ffi.md,
// "Drawing-time ports").
//
// Times are integers with the Rust types' ranges and saturation: positions in microseconds (i64),
// monotonic times in nanoseconds (u64), both as bigint. The page's monotonic clock is
// performance.now(); see monotonicNowNs().

/** The playback anchor (architecture §5): where playback was, when, and how fast it moves. */
export interface PlaybackAnchor {
  /** Changes on every track change and seek. */
  trackGeneration: bigint;
  /** Audible position at the anchor time, µs. */
  positionUs: bigint;
  /** The monotonic time of the anchor, ns. */
  anchorMonoNs: bigint;
  /** 0 while paused or buffering, 1 while playing (an f32 in the core). */
  rate: number;
  /** The track's length, µs, if known. */
  durationUs: bigint | null;
}

const I64_MAX = (1n << 63n) - 1n;
const I64_MIN = -(1n << 63n);
const U64_MAX = (1n << 64n) - 1n;

/** Corrections up to this size are slewed; larger ones jump. */
export const MAX_SLEW_US = 250_000n;
/** Shortest slew; longer corrections take four times their size. */
export const MIN_SLEW_US = 100_000n;

function saturateI64(value: bigint): bigint {
  return value > I64_MAX ? I64_MAX : value < I64_MIN ? I64_MIN : value;
}

/** Rust's `f64 as u64`: truncates, saturates, NaN and negatives are 0. */
function f64ToU64(value: number): bigint {
  if (Number.isNaN(value) || value <= 0) return 0n;
  if (value >= 18446744073709551615) return U64_MAX;
  return BigInt(Math.trunc(value));
}

/** Rust's `f64 as i64`: truncates, saturates, NaN is 0. */
function f64ToI64(value: number): bigint {
  if (Number.isNaN(value)) return 0n;
  if (value >= 9223372036854775807) return I64_MAX;
  if (value <= -9223372036854775808) return I64_MIN;
  return BigInt(Math.trunc(value));
}

/** Rust's `f64::round`: halves away from zero. */
function roundHalfAwayFromZero(value: number): number {
  return Math.sign(value) * Math.round(Math.abs(value));
}

function abs(value: bigint): bigint {
  return value < 0n ? -value : value;
}

function max(a: bigint, b: bigint): bigint {
  return a > b ? a : b;
}

function min(a: bigint, b: bigint): bigint {
  return a < b ? a : b;
}

/** Rust's `div_euclid` for a positive divisor: rounds towards negative infinity. */
function divFloor(value: bigint, divisor: bigint): bigint {
  const q = value / divisor;
  return value % divisor !== 0n && value < 0n ? q - 1n : q;
}

/**
 * The extrapolated audible position at `nowNs`, in µs: `positionUs + (nowNs - anchorMonoNs) * rate
 * / 1000`, rounded half away from zero. A paused anchor (rate 0) always returns `positionUs`.
 */
export function positionAt(anchor: PlaybackAnchor, nowNs: bigint): bigint {
  if (anchor.rate === 0) return anchor.positionUs;
  // i128 → f64 rounds to nearest, as Number(bigint) does.
  const elapsedNs = Number(nowNs - anchor.anchorMonoNs);
  const advanceUs = roundHalfAwayFromZero((elapsedNs * Math.fround(anchor.rate)) / 1000);
  return saturateI64(anchor.positionUs + f64ToI64(advanceUs));
}

/**
 * A smoothed, monotonic playback position for drawing progress bars and lyrics.
 *
 * Feed it every anchor and draw from positionUs(monotonicNowNs()). It never steps backwards while
 * playing and spreads the small corrections of a re-anchoring (up to 250 ms) over a short slew; a
 * new track, a seek, a pause or a resume shows at once. waitUntil turns "the next visible change is
 * at position X" into a delay, so a view sleeps until something on screen changes.
 */
export class DisplayClock {
  private current: PlaybackAnchor | null = null;
  /** Correction added to the anchor's position, fading linearly to zero. */
  private offsetUs = 0n;
  private offsetSinceNs = 0n;
  private slewUs = 0n;
  /** Last position handed out, so it never decreases while playing. */
  private lastUs: bigint | null = null;

  /** The latest anchor. */
  get anchor(): PlaybackAnchor | null {
    return this.current;
  }

  /** Feeds an anchor. */
  update(anchor: PlaybackAnchor, nowNs: bigint): void {
    const old = this.current;
    const continuous =
      old !== null && old.trackGeneration === anchor.trackGeneration && old.rate > 0 && anchor.rate > 0;
    const shown = this.drawn(nowNs);
    this.current = anchor;
    const error = shown - positionAt(anchor, nowNs);
    if (continuous && error !== 0n && abs(error) <= MAX_SLEW_US) {
      this.offsetUs = error;
      this.offsetSinceNs = nowNs;
      this.slewUs = max(abs(error) * 4n, MIN_SLEW_US);
    } else {
      // Show the new anchor exactly, backwards too.
      this.offsetUs = 0n;
      this.lastUs = null;
    }
  }

  /** The position to draw at `nowNs`, in µs within the track. */
  positionUs(nowNs: bigint): bigint {
    const anchor = this.current;
    if (anchor === null) return 0n;
    let pos = this.drawn(nowNs);
    if (anchor.rate > 0 && this.lastUs !== null) pos = max(pos, this.lastUs);
    if (anchor.durationUs !== null) pos = min(pos, max(anchor.durationUs, 0n));
    pos = max(pos, 0n);
    this.lastUs = pos;
    return pos;
  }

  /** True while the position moves. */
  get isRunning(): boolean {
    return (this.current?.rate ?? 0) > 0;
  }

  /**
   * How long until the drawn position reaches `targetUs`, in µs: 0 when it already has, null while
   * stopped or paused. During a slew the result is off by up to a quarter, so re-ask on waking.
   */
  waitUntil(targetUs: bigint, nowNs: bigint): bigint | null {
    const anchor = this.current;
    if (anchor === null || !(anchor.rate > 0)) return null;
    const pos = max(this.drawn(nowNs), this.lastUs ?? I64_MIN);
    const ahead = saturateI64(targetUs - pos);
    if (ahead <= 0n) return 0n;
    return f64ToU64(Math.ceil(Number(ahead) / Math.fround(anchor.rate)));
  }

  private drawn(nowNs: bigint): bigint {
    const anchor = this.current;
    if (anchor === null) return 0n;
    return saturateI64(positionAt(anchor, nowNs) + this.fadingOffset(nowNs));
  }

  private fadingOffset(nowNs: bigint): bigint {
    if (this.offsetUs === 0n) return 0n;
    const elapsedNs = nowNs > this.offsetSinceNs ? nowNs - this.offsetSinceNs : 0n;
    const elapsedUs = min(elapsedNs / 1000n, I64_MAX);
    if (elapsedUs >= this.slewUs) return 0n;
    return (this.offsetUs * (this.slewUs - elapsedUs)) / this.slewUs;
  }
}

/** The next whole second after `positionUs`, when an `m:ss` display changes. */
export function nextSecond(positionUs: bigint): bigint {
  return saturateI64((divFloor(positionUs, 1_000_000n) + 1n) * 1_000_000n);
}

/**
 * The next multiple of `stepUs` after `positionUs`, when a progress bar drawn in steps of `stepUs`
 * changes; null for a step of zero or less.
 */
export function nextStep(positionUs: bigint, stepUs: bigint): bigint | null {
  return stepUs > 0n ? saturateI64((divFloor(positionUs, stepUs) + 1n) * stepUs) : null;
}

/** The page's monotonic clock in ns (performance.now(), which anchors are stamped with). */
export function monotonicNowNs(): bigint {
  return BigInt(Math.round(performance.now() * 1e6));
}
