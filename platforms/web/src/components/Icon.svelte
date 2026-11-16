<script lang="ts" module>
  // Misty glyphs (ICO-01, ICO-03): 24-unit grid, 1.75-unit round strokes, outlined at rest; the
  // few filled ones (play, pause) are media states. Decorative: the button around carries the label.
  const STROKE: Record<string, string> = {
    home: 'M4 11.5 12 5l8 6.5V19a1 1 0 0 1-1 1h-4.5v-5h-5v5H5a1 1 0 0 1-1-1z',
    search: 'M10.5 4a6.5 6.5 0 1 1 0 13 6.5 6.5 0 0 1 0-13zM15.5 15.5 20 20',
    sources: 'M5 6.5h14v4H5zM5 13.5h14v4H5zM8 8.5h.01M8 15.5h.01',
    settings: 'M5 7h9M18 7h1M5 17h3M12 17h7M14 5v4M8 15v4M5 12h5M14 12h5M10 10v4',
    previous: 'M6 5v14M19 6l-10 6 10 6z',
    next: 'M18 5v14M5 6l10 6-10 6z',
    shuffle:
      'M3 7h3c1.7 0 3.2.9 4 2.3l3.9 5.4c.8 1.4 2.3 2.3 4 2.3H21M18 14l3 3-3 3M3 17h3c1 0 1.9-.3 2.7-.8M15.3 7.8c.8-.5 1.7-.8 2.7-.8H21M18 4l3 3-3 3',
    repeat: 'M4 12V9a3 3 0 0 1 3-3h12M16 3l3 3-3 3M20 12v3a3 3 0 0 1-3 3H5M8 21l-3-3 3-3',
    'repeat-one': 'M4 12V9a3 3 0 0 1 3-3h12M16 3l3 3-3 3M20 12v3a3 3 0 0 1-3 3H5M8 21l-3-3 3-3M11.5 10.5l1.5-1v5',
    'volume-low': 'M11 5 6 9H3v6h3l5 4zM16 9.5a3.5 3.5 0 0 1 0 5',
    'volume-high': 'M11 5 6 9H3v6h3l5 4zM16 9a4 4 0 0 1 0 6M19 6a8 8 0 0 1 0 12',
    mute: 'M11 5 6 9H3v6h3l5 4zM16 9.5l5 5M21 9.5l-5 5',
    lyrics: 'M5 5h14v10H10l-5 4zM9 9h6',
    queue: 'M4 6h16M4 12h16M4 18h10',
    info: 'M12 3.5a8.5 8.5 0 1 1 0 17 8.5 8.5 0 0 1 0-17zM12 11v5M12 8h.01',
    back: 'M15 5l-7 7 7 7',
    forward: 'M9 5l7 7-7 7',
    down: 'M5 9l7 7 7-7',
    more: 'M5.5 12h.01M12 12h.01M18.5 12h.01',
    close: 'M6 6l12 12M18 6 6 18',
    plus: 'M12 5v14M5 12h14',
    file: 'M7 3.5h7l4 4V20a.5.5 0 0 1-.5.5h-10A.5.5 0 0 1 6 20V4a.5.5 0 0 1 .5-.5zM14 3.5V8h4M9.5 13.5v3.5M9.5 17a1.5 1.5 0 1 1-1.5-1.5h1.5M9.5 13.5l4-1',
    album: 'M12 3.5a8.5 8.5 0 1 1 0 17 8.5 8.5 0 0 1 0-17zM12 10a2 2 0 1 1 0 4 2 2 0 0 1 0-4z',
    artist: 'M12 4a4 4 0 1 1 0 8 4 4 0 0 1 0-8zM4.5 20a7.5 7.5 0 0 1 15 0',
    playlist: 'M4 6h12M4 11h12M4 16h7M17 13v6.5M17 19.5a2 2 0 1 1-2-2h2M17 13l3.5-1',
    folder: 'M3.5 6.5a1 1 0 0 1 1-1h5l2 2h8a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1z',
    note: 'M9 18V6l10-2v12M9 18a2.5 2.5 0 1 1-2.5-2.5H9M19 16a2.5 2.5 0 1 1-2.5-2.5H19',
    check: 'M5 12.5l4.5 4.5L19 7.5',
    alert: 'M12 4 21 19.5H3zM12 10v4.5M12 17h.01',
    signout: 'M14 5h4a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1h-4M10 8l-4 4 4 4M6 12h9',
    install: 'M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14',
    qr: 'M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h2v2h-2zM18 18h2v2h-2zM14 18h2M18 14h2',
    globe: 'M12 3.5a8.5 8.5 0 1 1 0 17 8.5 8.5 0 0 1 0-17zM3.5 12h17M12 3.5c2.5 2.3 3.5 5.3 3.5 8.5s-1 6.2-3.5 8.5c-2.5-2.3-3.5-5.3-3.5-8.5s1-6.2 3.5-8.5z',
    key: 'M8 11a3.5 3.5 0 1 1 0 .01zM11.5 11H20M17 11v3M20 11v2.5',
    sort: 'M8 5v14M5 16l3 3 3-3M16 19V5M13 8l3-3 3 3',
    library: 'M5 4.5v15M9.5 4.5v15M14 5.5l4.5 13.5',
    'folder-plus': 'M3.5 6.5a1 1 0 0 1 1-1h5l2 2h8a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1zM12 10.5v5M9.5 13h5',
    // Open in {Service}: out of the box, to another place.
    external: 'M13.5 4.5h6v6M19.5 4.5 11 13M17.5 13.5v5a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-11a1 1 0 0 1 1-1h5',
  };
  const FILLED: Record<string, string> = {
    play: 'M8 5.6v12.8a.9.9 0 0 0 1.4.8l10-6.4a.9.9 0 0 0 0-1.6l-10-6.4A.9.9 0 0 0 8 5.6z',
    pause: 'M7.7 5h2.4c.66 0 1.2.54 1.2 1.2v11.6c0 .66-.54 1.2-1.2 1.2H7.7c-.66 0-1.2-.54-1.2-1.2V6.2c0-.66.54-1.2 1.2-1.2zM14.1 5h2.4c.66 0 1.2.54 1.2 1.2v11.6c0 .66-.54 1.2-1.2 1.2h-2.4c-.66 0-1.2-.54-1.2-1.2V6.2c0-.66.54-1.2 1.2-1.2z',
  };
</script>

<script lang="ts">
  let { name, size = 20 }: { name: string; size?: number } = $props();
  const filled = $derived(name in FILLED);
</script>

<svg
  aria-hidden="true"
  focusable="false"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={filled ? 'currentColor' : 'none'}
  stroke={filled ? 'none' : 'currentColor'}
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d={filled ? FILLED[name] : (STROKE[name] ?? '')} />
</svg>
