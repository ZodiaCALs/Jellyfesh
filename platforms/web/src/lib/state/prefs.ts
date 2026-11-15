// Small per-browser conveniences (a list's sort order): localStorage, which can be missing or
// refuse (private windows), so every read and write is guarded and has a default.

export function remembered<T>(key: string, fallback: T): T {
  try {
    const text = localStorage.getItem(`jellyfesh:${key}`);
    return text === null ? fallback : (JSON.parse(text) as T);
  } catch {
    return fallback;
  }
}

export function remember<T>(key: string, value: T) {
  try {
    localStorage.setItem(`jellyfesh:${key}`, JSON.stringify(value));
  } catch {
    // Not kept: the default comes back next time.
  }
}
