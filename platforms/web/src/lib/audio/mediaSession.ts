// The Media Session API: what is playing on lock screens and in system media controls, and the
// media keys (the browser's MPRIS / SMTC / MPNowPlayingInfoCenter).

export interface MediaSessionActions {
  play(): void;
  pause(): void;
  next(): void;
  previous(): void;
  seekTo(ms: number): void;
  seekBy(ms: number): void;
  stop(): void;
}

export interface NowPlayingMeta {
  title: string;
  artist: string | null;
  album: string | null;
  artwork: string | null;
}

const supported = typeof navigator !== 'undefined' && 'mediaSession' in navigator;

export function bindMediaSession(actions: MediaSessionActions) {
  if (!supported) return;
  const set = (action: MediaSessionAction, handler: MediaSessionActionHandler | null) => {
    try {
      navigator.mediaSession.setActionHandler(action, handler);
    } catch {
      // Not every browser knows every action.
    }
  };
  set('play', () => actions.play());
  set('pause', () => actions.pause());
  set('nexttrack', () => actions.next());
  set('previoustrack', () => actions.previous());
  set('stop', () => actions.stop());
  set('seekto', (details) => details.seekTime !== undefined && actions.seekTo(details.seekTime * 1000));
  set('seekforward', (details) => actions.seekBy((details.seekOffset ?? 10) * 1000));
  set('seekbackward', (details) => actions.seekBy(-(details.seekOffset ?? 10) * 1000));
}

export function setMetadata(meta: NowPlayingMeta | null) {
  if (!supported) return;
  if (!meta) {
    navigator.mediaSession.metadata = null;
    return;
  }
  navigator.mediaSession.metadata = new MediaMetadata({
    title: meta.title,
    artist: meta.artist ?? '',
    album: meta.album ?? '',
    artwork: meta.artwork ? [{ src: meta.artwork, sizes: '512x512' }] : [],
  });
}

export function setPlaybackState(state: 'playing' | 'paused' | 'none') {
  if (supported) navigator.mediaSession.playbackState = state;
}

export function setPosition(positionMs: number, durationMs: number | null, rate: number) {
  if (!supported || !navigator.mediaSession.setPositionState) return;
  try {
    if (durationMs === null || !(durationMs > 0)) {
      navigator.mediaSession.setPositionState();
      return;
    }
    navigator.mediaSession.setPositionState({
      duration: durationMs / 1000,
      position: Math.min(durationMs, Math.max(0, positionMs)) / 1000,
      playbackRate: rate > 0 ? rate : 1,
    });
  } catch {
    // Inconsistent values (a race with a track change) are dropped.
  }
}
