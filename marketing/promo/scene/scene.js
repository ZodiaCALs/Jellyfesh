// Jellyfesh v0.0.7 promo: a deterministic timeline. render.mjs calls window.renderAt(t) for
// each frame (t in seconds) and takes a screenshot; nothing here reads a clock, and the
// stylesheet turns every CSS transition and animation off. Beats, in seconds (96 BPM, a bar is
// 2.5 s; music/make_music.py uses the same grid):
//
//   0.0  Cold open: the aurora, "Your music. Beautifully.", the Jellyfesh lettering
//   7.5  AI Lyrics: Create Synced Lyrics, Recognising… 42%, Timing Words…, the lyrics
//  20.0  The lyrics bar in the player bar, then Desktop Lyrics
//  27.5  Customize: corners, glass, aurora, typeface, lyrics glow and colour, layouts
//  40.0  Discover, Get Info, Comments
//  50.0  One app, five platforms
//  57.5  Jellyfesh Pro
//  62.5  End card
(() => {
  'use strict';
  const C = window.CONTENT;
  const DURATION = 70;
  const BEAT = 0.625;

  // ---------- Motion ----------
  const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
  const lerp = (a, b, p) => a + (b - a) * p;
  const seg = (t, a, b) => clamp((t - a) / (b - a));
  function bezier(x1, y1, x2, y2) {
    const bx = (s) => 3 * x1 * s * (1 - s) * (1 - s) + 3 * x2 * s * s * (1 - s) + s * s * s;
    const by = (s) => 3 * y1 * s * (1 - s) * (1 - s) + 3 * y2 * s * s * (1 - s) + s * s * s;
    return (x) => {
      if (x <= 0) return 0;
      if (x >= 1) return 1;
      let lo = 0;
      let hi = 1;
      let s = x;
      for (let i = 0; i < 28; i++) {
        s = (lo + hi) / 2;
        if (bx(s) < x) lo = s;
        else hi = s;
      }
      return by(s);
    };
  }
  // The tokens' curves (motion.ease-fluid, ease-spring) and two for the camera and the type.
  const E = {
    fluid: bezier(0.22, 1, 0.36, 1),
    spring: bezier(0.34, 1.36, 0.64, 1),
    inout: bezier(0.65, 0, 0.35, 1),
    out: bezier(0.16, 1, 0.3, 1),
    soft: bezier(0.45, 0, 0.2, 1),
    in: bezier(0.55, 0, 1, 0.45),
    linear: (x) => x,
  };
  const mix = (a, b, p) => (Array.isArray(a) ? a.map((x, i) => lerp(x, b[i], p)) : lerp(a, b, p));
  /** A value moving through [time, value, ease-to-the-next] keys. */
  function track(keys, ease = E.inout) {
    return (t) => {
      if (t <= keys[0][0]) return keys[0][1];
      for (let i = 0; i < keys.length - 1; i++) {
        const [t0, v0, e0] = keys[i];
        const [t1, v1] = keys[i + 1];
        if (t < t1) return mix(v0, v1, (e0 || ease)((t - t0) / (t1 - t0)));
      }
      return keys[keys.length - 1][1];
    };
  }
  const fadeIn = (t, a, d = 0.5, e = E.fluid) => e(seg(t, a, a + d));
  const fadeOut = (t, a, d = 0.4, e = E.inout) => 1 - e(seg(t, a, a + d));

  // ---------- DOM helpers ----------
  const stage = document.getElementById('stage');
  function el(html) {
    const t = document.createElement('template');
    t.innerHTML = html.trim();
    return t.content.firstElementChild;
  }
  const $ = (root, sel) => root.querySelector(sel);
  const $$ = (root, sel) => [...root.querySelectorAll(sel)];
  function show(node, on) {
    const v = on ? '' : 'none';
    if (node.style.display !== v) node.style.display = v;
  }
  function setStyle(node, prop, value) {
    if (node.style[prop] !== value) node.style[prop] = value;
  }
  const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const cover = (id) => `../assets/covers/${id}.jpg`;
  const album = (id) => C.albums[id];

  // ---------- Icons (platforms/web/src/components/Icon.svelte; Misty glyphs) ----------
  const STROKE = {
    home: 'M4 11.5 12 5l8 6.5V19a1 1 0 0 1-1 1h-4.5v-5h-5v5H5a1 1 0 0 1-1-1z',
    search: 'M10.5 4a6.5 6.5 0 1 1 0 13 6.5 6.5 0 0 1 0-13zM15.5 15.5 20 20',
    sources: 'M5 6.5h14v4H5zM5 13.5h14v4H5zM8 8.5h.01M8 15.5h.01',
    settings: 'M5 7h9M18 7h1M5 17h3M12 17h7M14 5v4M8 15v4M5 12h5M14 12h5M10 10v4',
    previous: 'M6 5v14M19 6l-10 6 10 6z',
    next: 'M18 5v14M5 6l10 6-10 6z',
    shuffle:
      'M3 7h3c1.7 0 3.2.9 4 2.3l3.9 5.4c.8 1.4 2.3 2.3 4 2.3H21M18 14l3 3-3 3M3 17h3c1 0 1.9-.3 2.7-.8M15.3 7.8c.8-.5 1.7-.8 2.7-.8H21M18 4l3 3-3 3',
    repeat: 'M4 12V9a3 3 0 0 1 3-3h12M16 3l3 3-3 3M20 12v3a3 3 0 0 1-3 3H5M8 21l-3-3 3-3',
    'volume-low': 'M11 5 6 9H3v6h3l5 4zM16 9.5a3.5 3.5 0 0 1 0 5',
    'volume-high': 'M11 5 6 9H3v6h3l5 4zM16 9a4 4 0 0 1 0 6M19 6a8 8 0 0 1 0 12',
    lyrics: 'M5 5h14v10H10l-5 4zM9 9h6',
    queue: 'M4 6h16M4 12h16M4 18h10',
    info: 'M12 3.5a8.5 8.5 0 1 1 0 17 8.5 8.5 0 0 1 0-17zM12 11v5M12 8h.01',
    comments: 'M4 6.5a2 2 0 0 1 2-2h12a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-7l-4.5 3.5V16.5H6a2 2 0 0 1-2-2zM8 9h8M8 12.5h5',
    heart: 'M12 19.5s-7.5-4.6-7.5-9.7A4.3 4.3 0 0 1 12 7a4.3 4.3 0 0 1 7.5 2.8c0 5.1-7.5 9.7-7.5 9.7z',
    back: 'M15 5l-7 7 7 7',
    forward: 'M9 5l7 7-7 7',
    down: 'M5 9l7 7 7-7',
    more: 'M5.5 12h.01M12 12h.01M18.5 12h.01',
    close: 'M6 6l12 12M18 6 6 18',
    album: 'M12 3.5a8.5 8.5 0 1 1 0 17 8.5 8.5 0 0 1 0-17zM12 10a2 2 0 1 1 0 4 2 2 0 0 1 0-4z',
    artist: 'M12 4a4 4 0 1 1 0 8 4 4 0 0 1 0-8zM4.5 20a7.5 7.5 0 0 1 15 0',
    playlist: 'M4 6h12M4 11h12M4 16h7M17 13v6.5M17 19.5a2 2 0 1 1-2-2h2M17 13l3.5-1',
    folder: 'M3.5 6.5a1 1 0 0 1 1-1h5l2 2h8a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-15a1 1 0 0 1-1-1z',
    note: 'M9 18V6l10-2v12M9 18a2.5 2.5 0 1 1-2.5-2.5H9M19 16a2.5 2.5 0 1 1-2.5-2.5H19',
    // Desktop Lyrics' Lock, drawn in the same 24-unit, round-stroke style.
    lock: 'M7 11h10a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1zM8.5 11V8a3.5 3.5 0 0 1 7 0v3',
  };
  const FILLED = {
    play: 'M8 5.6v12.8a.9.9 0 0 0 1.4.8l10-6.4a.9.9 0 0 0 0-1.6l-10-6.4A.9.9 0 0 0 8 5.6z',
    pause:
      'M7.7 5h2.4c.66 0 1.2.54 1.2 1.2v11.6c0 .66-.54 1.2-1.2 1.2H7.7c-.66 0-1.2-.54-1.2-1.2V6.2c0-.66.54-1.2 1.2-1.2zM14.1 5h2.4c.66 0 1.2.54 1.2 1.2v11.6c0 .66-.54 1.2-1.2 1.2h-2.4c-.66 0-1.2-.54-1.2-1.2V6.2c0-.66.54-1.2 1.2-1.2z',
  };
  function icon(name, size = 20) {
    const filled = name in FILLED;
    return `<svg class="icon" width="${size}" height="${size}" viewBox="0 0 24 24" fill="${filled ? 'currentColor' : 'none'}" stroke="${filled ? 'none' : 'currentColor'}" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="${filled ? FILLED[name] : STROKE[name]}"/></svg>`;
  }

  // ---------- Themes (assets/themes: the core's colours for each cover) ----------
  const THEMES = {};
  function cssName(token) {
    const dot = token.indexOf('.');
    return `--jf-${token.slice(dot + 1).replaceAll('.', '-')}`;
  }
  function alphaOf(hex) {
    return hex.length === 9 ? parseInt(hex.slice(7, 9), 16) / 255 : 1;
  }
  /** The custom properties of a cover's theme in one appearance, as theme.svelte.ts sets them. */
  function themeVars(id, appearance) {
    const theme = THEMES[id][appearance];
    let css = '';
    for (const [k, v] of Object.entries(theme.colors)) css += `${cssName(k)}:${v};`;
    const c = theme.colors;
    css += `--jf-lyrics-dim:${(alphaOf(c['ink.ink-50']) / alphaOf(c['ink.ink-90'])).toFixed(3)};`;
    css += `--jf-lyrics-dim-sung:${(alphaOf(c['ink.ink-30']) / alphaOf(c['ink.ink-90'])).toFixed(3)};`;
    if (appearance === 'dark') {
      css += '--jf-elevation-shadow-1:0 1px 2px 0 var(--jf-shadow-1-0),0 12px 30px -16px var(--jf-shadow-1-1);';
      css += '--jf-elevation-shadow-2:0 2px 6px 0 var(--jf-shadow-2-0),0 24px 52px -22px var(--jf-shadow-2-1);';
      css += '--jf-elevation-shadow-3:0 8px 24px 0 var(--jf-shadow-3-0),0 44px 84px -28px var(--jf-shadow-3-1);';
    } else {
      css += '--jf-elevation-shadow-1:0 1px 2px 0 var(--jf-shadow-1-0),0 10px 28px -18px var(--jf-shadow-1-1);';
      css += '--jf-elevation-shadow-2:0 2px 6px 0 var(--jf-shadow-2-0),0 22px 48px -24px var(--jf-shadow-2-1);';
      css += '--jf-elevation-shadow-3:0 8px 24px 0 var(--jf-shadow-3-0),0 40px 80px -30px var(--jf-shadow-3-1);';
    }
    return css;
  }

  // ---------- Words ----------
  /** Big type: each word in a mask it rises into. */
  function words(text, cls = '') {
    return text
      .split(' ')
      .map((w) => `<span class="reveal"><span class="${cls}">${esc(w)}</span></span>`)
      .join(' ');
  }
  function riseWords(node, t, start, stagger = 0.06, dur = 0.75) {
    $$(node, '.reveal > span').forEach((s, i) => {
      const p = E.out(seg(t, start + i * stagger, start + i * stagger + dur));
      setStyle(s, 'transform', `translate3d(0, ${((1 - p) * 108).toFixed(2)}%, 0)`);
    });
  }
  /** A lyric line's words, each a span the sweep fills. */
  function lyricWords(text) {
    return text
      .split(' ')
      .map((w) => `<span class="word">${esc(w)} </span>`)
      .join('');
  }
  /** Word timings for a line sung from `start`, one per eighth note, the last word held longer. */
  function wordTimes(text, start, step = BEAT / 2, last = BEAT) {
    const ws = text.split(' ');
    return ws.map((_, i) => [start + i * step, start + i * step + (i === ws.length - 1 ? last : step)]);
  }
  /** Fills a line's words for time t (LyricsPane: the sung layer feathered over 0.3 em). */
  function sweep(lineNode, times, t) {
    $$(lineNode, '.word').forEach((w, i) => {
      const [a, b] = times[i];
      const p = clamp((t - a) / (b - a));
      setStyle(w, 'backgroundSize', `calc(${(p * 100).toFixed(2)}% + 0.3em) 100%, 100% 100%`);
    });
  }

  // ---------- The app's pieces ----------
  const PLACES = [
    ['home', 'Home', 'home'],
    ['albums', 'Albums', 'album'],
    ['artists', 'Artists', 'artist'],
    ['songs', 'Songs', 'note'],
    ['playlists', 'Playlists', 'playlist'],
    ['favourites', 'Favourites', 'heart'],
    null,
    ['search', 'Search', 'search'],
    ['sources', 'Sources', 'sources'],
  ];
  function sidebar(active, dark = false) {
    const items = PLACES.map((p) =>
      p ? `<div class="nav-item${p[0] === active ? ' on' : ''}">${icon(p[2])}${p[1]}</div>` : '<div class="nav-gap"></div>',
    ).join('');
    return `<nav class="sidebar glass-clear"><div class="brand"><img src="/website/assets/icon-${dark ? 'steel' : 'ice'}.svg" alt=""><span>Jellyfesh</span></div>${items}<div class="nav-end"><div class="nav-item">${icon('settings')}Settings</div></div></nav>`;
  }
  function transport(dock = false) {
    return `<div class="transport"><span class="toggle">${icon('shuffle', dock ? 20 : 18)}</span><span class="skip">${icon('previous', dock ? 24 : 20)}</span><span class="play">${icon('pause', dock ? 26 : 22)}</span><span class="skip">${icon('next', dock ? 24 : 20)}</span><span class="toggle">${icon('repeat', dock ? 20 : 18)}</span></div>`;
  }
  function seekBar(start, end, fraction) {
    return `<div class="seek"><span class="time start">${start}</span><span class="line"><span class="played" style="transform:scaleX(${fraction})"></span></span><span class="time">${end}</span></div>`;
  }
  function playerBar({ id, title, lyric = null }) {
    const a = album(id);
    const second = lyric
      ? `<span class="pb-lyric">${lyricWords(lyric)}</span>`
      : `<span class="pb-second">${esc(a.artist)} · ${esc(a.title)}</span>`;
    return `<div class="player-bar glass-veil"><div class="pb-open"><span class="pb-art"><img class="cover-img" src="${cover(id)}" alt=""></span><span class="pb-text"><span class="pb-title">${esc(title)}</span>${second}</span></div><div class="pb-middle">${transport()}${seekBar('1:12', '−2:36', 0.32)}</div><div class="pb-end"><span class="tool">${icon('lyrics')}</span><span class="tool">${icon('queue')}</span><span class="volume" style="width:150px"><span class="tool">${icon('volume-low')}</span><span class="vline"><span></span></span></span></div></div>`;
  }
  function card(id) {
    const a = album(id);
    return `<div class="card"><span class="cover"><img src="${cover(id)}" alt=""></span><div class="title">${esc(a.title)}</div><div class="second">${esc(a.artist)}</div></div>`;
  }
  function itemRow({ n = null, id, title, sub, len, playing = false }) {
    const lead = n !== null ? `<span class="number">${n}</span>` : `<span class="art"><img class="cover-img" src="${cover(id)}" alt=""></span>`;
    return `<div class="item-row${playing ? ' playing' : ''}">${lead}${n !== null ? `<span class="art"><img class="cover-img" src="${cover(id)}" alt=""></span>` : ''}<span class="text"><span class="t1">${esc(title)}</span><span class="t2">${esc(sub)}</span></span><span class="len">${len}</span><span class="more">${icon('more', 20)}</span></div>`;
  }

  /** A 1600 × 900 window with the library: sidebar, a page, the player bar. */
  function libraryWindow({ theme, appearance = 'light', active, page, bar }) {
    const dark = appearance === 'dark';
    const node = el(`<div class="win ${dark ? 'dark' : 'light'}" style="${themeVars(theme, appearance)}"><div class="field"></div>${sidebar(active, dark)}<main class="content masked"><div class="scroll">${page}</div></main>${bar ? playerBar(bar) : ''}</div>`);
    return node;
  }

  /** Now Playing in a 1600 × 900 window (NowPlaying.svelte at that size: the stage, the dock). */
  const NP = { art: [140, 100, 380], titles: [140, 512], pane: [584, 78, 876, 694] };
  function nowPlayingWindow({ theme, appearance = 'dark', id, title, panes = ['Lyrics', 'Queue', 'Info'], on = 'Lyrics', backdrop = 'artwork', pane = '' }) {
    const a = album(id);
    const segs = panes.map((p) => `<span class="${p === on ? 'on' : ''}" data-pane="${p}">${p}</span>`).join('');
    const back =
      backdrop === 'artwork'
        ? `<img class="backdrop" src="../assets/backdrops/${id}-${appearance}.png" alt="">`
        : `<div class="field" style="inset:0;--glow:1.25"></div>`;
    return el(`<div class="win ${appearance}" style="${themeVars(theme, appearance)}"><div class="np">${back}
      <div class="np-top"><span class="np-back glass-veil-flat">${icon('down')}Library</span><div class="segmented glass-veil-flat">${segs}</div></div>
      <div class="np-art" style="left:${NP.art[0]}px;top:${NP.art[1]}px;width:${NP.art[2]}px;height:${NP.art[2]}px"><img class="cover-img" src="${cover(id)}" alt=""></div>
      <div class="np-titles" style="left:${NP.titles[0]}px;top:${NP.titles[1]}px"><h1 class="np-title">${esc(title)}</h1><p class="np-artist">${esc(a.artist)} · ${esc(a.title)}</p></div>
      <div class="np-pane" style="left:${NP.pane[0]}px;top:${NP.pane[1]}px;width:${NP.pane[2]}px;height:${NP.pane[3]}px">${pane}</div>
      <div class="dock glass-veil">${transport(true)}${seekBar('1:24', '−2:24', 0.37)}<div class="volume"><span class="tool">${icon('volume-low')}</span><span class="vline"><span></span></span><span>${icon('volume-high')}</span></div><span class="tool">${icon('more')}</span></div>
    </div></div>`);
  }

  /** Lyric lines placed as LyricsPane places them: 40 px bold, 75.2 px apart, the active line's
   *  middle at 38 % of the pane, the others at 92 % and dimmed. */
  const LINE_PITCH = 75.2;
  function lyricLines(lines, { credit = null } = {}) {
    const html = lines
      .map((l, i) => `<div class="lyric-line" data-i="${i}" style="top:${208 + i * LINE_PITCH}px"><span class="text">${lyricWords(l)}</span></div>`)
      .join('');
    const c = credit ? `<p class="credit" style="top:${208 + lines.length * LINE_PITCH - 6}px">${esc(credit)}</p>` : '';
    return `<div class="lyrics-mask"><div class="lyric-box abs" style="left:0;top:0;right:0;height:1400px">${html}${c}</div></div>`;
  }
  /** Lays the lines out for time t: the active line, the cascade (40 ms apart) when it moves. */
  function layLines(paneNode, t, { active, moves, entry = null, dim = true }) {
    const box = $(paneNode, '.lyric-box');
    const lines = $$(box, '.lyric-line');
    // The scroll: 0 for the first line, then the active line's middle at 38 % (263.7 px).
    const scrollFor = (k) => Math.max(0, 208 + k * LINE_PITCH + 27.6 - 0.38 * 694);
    lines.forEach((line, i) => {
      let y = 0;
      for (const [time, from, to] of moves) {
        const k = Math.min(8, Math.max(0, i - to + 1));
        const p = E.fluid(seg(t, time + k * 0.04, time + k * 0.04 + 0.38));
        y += (scrollFor(to) - scrollFor(from)) * p;
      }
      const isActive = i === active(t);
      const a = active(t);
      const sungLine = a !== null && i < a;
      const target = isActive || !dim ? 1 : sungLine ? 'sung' : 'dim';
      let op = target === 1 ? 1 : target === 'sung' ? 0.38 : 0.62;
      let sc = isActive || !dim ? 1 : 0.92;
      let dy = 0;
      if (entry !== null) {
        const p = E.fluid(seg(t, entry + i * 0.08, entry + i * 0.08 + 0.55));
        op *= p;
        dy = (1 - p) * 26;
      }
      line.classList.toggle('active', isActive);
      setStyle(line, 'transform', `translate3d(0, ${(dy - y).toFixed(2)}px, 0) scale(${sc})`);
      setStyle(line, 'opacity', op.toFixed(3));
    });
    let total = 0;
    for (const [time, from, to] of moves) total += (scrollFor(to) - scrollFor(from)) * E.fluid(seg(t, time, time + 0.38));
    const credit = $(box, '.credit');
    if (credit) {
      let op = 1;
      if (entry !== null) op = E.fluid(seg(t, entry + 0.4, entry + 0.95));
      setStyle(credit, 'transform', `translate3d(0, ${(-total).toFixed(2)}px, 0)`);
      setStyle(credit, 'opacity', op.toFixed(3));
    }
  }

  /** A settings group (Mist card) with rows: sliders, segmented choices, a colour well. */
  function settingsGroup(title, rows) {
    const body = rows
      .map((r) => {
        if (r.type === 'heading') return `<p class="gh" style="margin-top:${r.first ? 0 : 14}px">${esc(r.label)}</p>`;
        let control = '';
        if (r.type === 'slider')
          control = `<div style="display:flex;align-items:center;gap:12px"><div class="slider" data-id="${r.id}"><span class="rest"><span class="fill"></span></span><span class="thumb"></span></div><span class="slider-out" data-out="${r.id}"></span></div>`;
        else if (r.type === 'seg')
          control = `<div class="seg" data-id="${r.id}">${r.options.map((o) => `<span data-v="${esc(o)}">${esc(o)}</span>`).join('')}</div>`;
        else if (r.type === 'well') control = `<span class="well" data-id="${r.id}"></span>`;
        return `<div class="set-row"><span class="label">${esc(r.label)}</span>${control}</div>`;
      })
      .join('');
    return `<section class="group surface-mist"><h2 class="gtitle">${esc(title)}</h2>${body}</section>`;
  }
  function setSlider(root, id, value, min, max, text) {
    const s = $(root, `.slider[data-id="${id}"]`);
    if (!s) return;
    const f = (value - min) / (max - min);
    setStyle($(s, '.fill'), 'width', `${(f * 100).toFixed(2)}%`);
    setStyle($(s, '.thumb'), 'left', `${(f * 100).toFixed(2)}%`);
    const out = $(root, `[data-out="${id}"]`);
    if (out && out.textContent !== text) out.textContent = text;
  }
  function setSeg(root, id, value) {
    $$(root, `.seg[data-id="${id}"] span`).forEach((s) => s.classList.toggle('on', s.dataset.v === value));
  }

  // ---------- Camera ----------
  /** Frames a point of the window (in window px) at scale s (1 = the window at 1600 × 900). The
   *  window sits at (160, 90) in the 1920 × 1080 frame when the camera is at rest. */
  function camera(node, [cx, cy, s], winX = 160, winY = 90) {
    // Window point (cx, cy) goes to the frame's centre; then scale around the centre.
    const x = 960 - (winX + cx) * s;
    const y = 540 - (winY + cy) * s;
    setStyle(node, 'transform', `translate3d(${x.toFixed(2)}px, ${y.toFixed(2)}px, 0) scale(${s.toFixed(4)})`);
  }
  const REST = [800, 450, 1];

  // ---------- Scenes ----------
  const scenes = [];
  function scene(from, to, build) {
    const node = el('<div class="scene"></div>');
    stage.appendChild(node);
    const s = { from, to, node, render: build(node) };
    scenes.push(s);
    return s;
  }

  /** A caption on a soft acrylic pill, for words over a screen. */
  function captionPill(text, dark) {
    return el(`<div class="abs caption-pill" style="left:0;right:0;text-align:center"><span style="display:inline-flex;align-items:center;height:76px;padding:0 34px;border-radius:999px;font-size:38px;font-weight:650;letter-spacing:-0.015em;${
      dark
        ? 'color:rgba(255,255,255,0.95);background:rgba(14,18,28,0.82);box-shadow:inset 0 0 0 1px rgba(255,255,255,0.14),0 18px 50px -18px rgba(0,0,0,0.6)'
        : 'color:rgba(18,23,32,0.92);background:rgba(255,255,255,0.9);box-shadow:inset 0 0 0 1px rgba(255,255,255,0.7),0 18px 50px -20px rgba(30,40,90,0.35)'
    };backdrop-filter:blur(26px) saturate(1.6)">${esc(text)}</span></div>`);
  }
  function pill(node, t, a, b, y) {
    const p = fadeIn(t, a, 0.45) * fadeOut(t, b - 0.35, 0.35);
    show(node, p > 0.001);
    setStyle(node, 'opacity', p.toFixed(3));
    setStyle(node, 'transform', `translate3d(0, ${(y + (1 - E.fluid(seg(t, a, a + 0.6))) * 18).toFixed(1)}px, 0)`);
  }

  /** A full-frame chapter title: eyebrow, headline, sub. */
  function titleCard(node, { eyebrow = null, pro = false, headline, sub = null, dark, grad = false, y = 380 }) {
    const box = el(`<div class="abs ${dark ? 'on-dark' : 'on-light'}" style="inset:0"></div>`);
    if (eyebrow)
      box.appendChild(
        el(`<div class="eyebrow" style="top:${y - 86}px"><span class="reveal"><span class="${dark ? 'grad-dark' : 'grad-light'}">${esc(eyebrow)}</span></span>${pro ? '<span class="reveal"><span class="pro-badge">Pro</span></span>' : ''}</div>`),
      );
    box.appendChild(el(`<div class="headline" style="top:${y}px;font-size:150px">${words(headline, grad ? (dark ? 'grad-dark' : 'grad-light') : '')}</div>`));
    if (sub) box.appendChild(el(`<div class="sub" style="top:${y + 196}px;font-size:54px">${words(sub)}</div>`));
    node.appendChild(box);
    return box;
  }
  function animateTitle(box, t, start, end) {
    const parts = $$(box, '.eyebrow, .headline, .sub');
    parts.forEach((p, i) => riseWords(p, t, start + i * 0.16, 0.06, 0.8));
    const out = E.inout(seg(t, end - 0.45, end));
    setStyle(box, 'opacity', (1 - out).toFixed(3));
    setStyle(box, 'transform', `translate3d(0, ${(-40 * out).toFixed(1)}px, 0) scale(${(1 - 0.03 * out).toFixed(4)})`);
  }

  // Backgrounds shared by several beats.
  const bgDark = el('<div class="scene" style="display:block"><div class="aurora-dark"></div></div>');
  const bgLight = el('<div class="scene"><div class="aurora-light"></div></div>');
  stage.appendChild(bgDark);
  stage.appendChild(bgLight);
  function drift(node, t, amount = 1) {
    const a = $(node, '.aurora-dark, .aurora-light');
    const x = Math.sin(t * 0.21) * 34 * amount;
    const y = Math.cos(t * 0.17) * 22 * amount;
    setStyle(a, 'transform', `translate3d(${x.toFixed(1)}px, ${y.toFixed(1)}px, 0)`);
  }

  let lettering = null;
  function buildScenes() {
  // ===== 1. Cold open (0 – 7.5) =====
  scene(0, 7.6, (node) => {
    const line = el(`<div class="headline on-dark" style="top:452px;font-size:128px">${words('Your music.')} <span class="grad-dark-wrap">${words('Beautifully.', 'grad-dark')}</span></div>`);
    node.appendChild(line);
    const markBox = el('<div class="abs" style="left:440px;top:330px;width:1040px;height:339px"></div>');
    node.appendChild(markBox);
    lettering = markBox;
    const beta = el('<div class="sub on-dark" style="top:720px;font-size:36px;font-weight:600;letter-spacing:0.01em">Public Beta v0.0.7</div>');
    node.appendChild(beta);
    return (t) => {
      // "Your music." on beats 1 and 1½, "Beautifully." on beat 3.
      const ws = $$(line, '.reveal > span');
      const starts = [0.625, 0.86, 1.875];
      ws.forEach((s, i) => {
        const p = E.out(seg(t, starts[i], starts[i] + 0.85));
        setStyle(s, 'transform', `translate3d(0, ${((1 - p) * 108).toFixed(2)}%, 0)`);
      });
      const out = E.inout(seg(t, 3.75, 4.4));
      setStyle(line, 'opacity', (1 - out).toFixed(3));
      setStyle(line, 'transform', `translate3d(0, ${(-30 * out).toFixed(1)}px, 0)`);
      show(line, t < 4.45);
      // The lettering draws itself, stroke by stroke, then glows; then gives way.
      drawLettering(t);
      const exit = E.in(seg(t, 6.85, 7.5));
      setStyle(markBox, 'opacity', (1 - exit).toFixed(3));
      setStyle(markBox, 'transform', `scale(${(1 + 0.06 * exit).toFixed(4)})`);
      const b = fadeIn(t, 5.75, 0.7) * (1 - exit);
      setStyle(beta, 'opacity', b.toFixed(3));
      setStyle(beta, 'transform', `translate3d(0, ${((1 - E.out(seg(t, 5.75, 6.5))) * 16).toFixed(1)}px, 0)`);
    };
  });

  /* The rest of the scenes follow in buildScenes. */
  buildRest();
  }

  /** The website's lettering (website/assets/lettering.svg) rebuilt stroke by stroke. */
  let strokes = [];
  async function buildLettering() {
    const text = await (await fetch('/website/assets/lettering.svg')).text();
    const d = text.match(/<path d="([^"]+)" fill="none" stroke="#fff" stroke-width="18"/)[1];
    const parts = d.split(/(?=M)/).map((p) => p.trim());
    const dot = text.match(/<path d="([^"]+)" fill="#fff"\/>/)[1];
    const paths = (stroke, width, extra = '') =>
      parts.map((p, i) => `<path data-s="${i}" d="${p}" pathLength="1" fill="none" stroke="${stroke}" stroke-width="${width}" stroke-linecap="round" stroke-linejoin="round" stroke-dasharray="1 1" stroke-dashoffset="1" ${extra}/>`).join('');
    lettering.innerHTML = `<svg viewBox="-40 -200 890 290" width="1040" height="339" style="overflow:visible">
      <defs>
        <linearGradient id="pl-paint" x1="0" y1="-160" x2="0" y2="60" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#eef4ff"/><stop offset="1" stop-color="#8db7ff"/></linearGradient>
        <mask id="pl-tube" maskUnits="userSpaceOnUse" x="-40" y="-200" width="890" height="290">${paths('#fff', 18)}${paths('#666', 10)}<path class="pl-dot" d="${dot}" fill="#fff" opacity="0"/></mask>
        <filter id="pl-glow" x="-10%" y="-40%" width="120%" height="180%"><feGaussianBlur stdDeviation="14"/></filter>
      </defs>
      <g class="pl-glow" filter="url(#pl-glow)" opacity="0">${paths('#5b8cff', 24)}</g>
      <rect x="-40" y="-200" width="890" height="290" fill="url(#pl-paint)" mask="url(#pl-tube)"/>
    </svg>`;
    strokes = parts.map((_, i) => $$(lettering, `path[data-s="${i}"]`));
  }
  function drawLettering(t) {
    if (!strokes.length) return;
    const n = strokes.length;
    // Letter by letter across 1.7 s from beat 7 (4.375 s), each stroke 0.42 s.
    strokes.forEach((group, i) => {
      const a = 4.375 + (i / n) * 1.3;
      const p = E.soft(seg(t, a, a + 0.42));
      const off = (1 - p).toFixed(4);
      const vis = p > 0.002 ? 'visible' : 'hidden';
      for (const path of group) {
        if (path.getAttribute('stroke-dashoffset') !== off) path.setAttribute('stroke-dashoffset', off);
        if (path.getAttribute('visibility') !== vis) path.setAttribute('visibility', vis);
      }
    });
    const dotP = E.out(seg(t, 5.55, 5.95));
    $(lettering, '.pl-dot').setAttribute('opacity', dotP.toFixed(3));
    const glow = 0.55 * E.soft(seg(t, 4.6, 6.2));
    $(lettering, '.pl-glow').setAttribute('opacity', glow.toFixed(3));
  }

  function buildRest() {
  // ===== 2. AI Lyrics (7.5 – 20) =====
  scene(7.3, 20.35, (node) => {
    const title = titleCard(node, { eyebrow: 'New', pro: true, headline: 'AI Lyrics', sub: 'Synced lyrics for any song.', dark: true, grad: true });
    const cam = el('<div class="cam"></div>');
    node.appendChild(cam);
    const song = C.aiSong;
    const lines = song.lines;
    const pane = `<div class="no-lyrics"><p class="title-2">No Lyrics</p><p>Jellyfesh shows lyrics from a .lrc, .ttml or .vtt file next to the song, or from its tags.</p><div class="make">
        <span class="btn-primary create">Create Synced Lyrics</span>
        <div class="job" style="display:none;flex-direction:column;align-items:flex-start;gap:12px"><p class="stage-text">Reading Audio…</p><div class="job-bar"><span></span></div><span class="btn-secondary">Cancel</span></div>
      </div></div>${lyricLines(lines, { credit: `Made by AI Lyrics · ${song.engine}` })}`;
    const win = nowPlayingWindow({ theme: 'low-tide', id: song.album, title: song.title, pane });
    win.style.left = '160px';
    win.style.top = '90px';
    cam.appendChild(win);
    const paneNode = $(win, '.np-pane');
    const noLyrics = $(win, '.no-lyrics');
    const create = $(win, '.create');
    const job = $(win, '.job');
    const stageText = $(win, '.stage-text');
    const barFill = $(win, '.job-bar span');
    const box = $(win, '.lyric-box');
    const scrim = el('<div class="abs" style="inset:0;background:linear-gradient(to top, rgba(4,8,14,0.86) 0%, rgba(4,8,14,0.55) 36%, rgba(4,8,14,0) 70%)"></div>');
    node.appendChild(scrim);
    const modes = el(`<div class="abs on-dark" style="left:0;right:0;top:730px;text-align:center"><div style="font-size:64px;font-weight:700;letter-spacing:-0.03em">${words('On your device, or online with your own key.')}</div><div class="footnote" style="position:relative;margin-top:22px;color:rgba(247,248,252,0.7)">On-device mode isn’t in Jellyfesh Web or on Windows on Arm.</div></div>`);
    node.appendChild(modes);

    // Line 1 from 15.0 s, line 2 from 18.125 s (one bar and a quarter later).
    const t1 = wordTimes(lines[0], 15.0);
    const t2 = wordTimes(lines[1], 18.125);
    const active = (t) => (t < 14.9 ? null : t < 18.0 ? 0 : 1);
    const moves = [[18.0, 0, 1]];
    const cameraTrack = track([
      [9.7, [800, 470, 0.94]],
      [10.0, [800, 450, 1.0], E.inout],
      [10.5, [800, 450, 1.0], E.inout],
      [11.4, [975, 300, 1.82]],
      [14.1, [978, 305, 1.9], E.inout],
      [15.0, [800, 450, 1.1]],
      [16.25, [800, 452, 1.13], E.inout],
      [17.0, [900, 500, 2.1]],
      [17.9, [905, 505, 2.16], E.inout],
      [18.6, [800, 450, 1.04]],
      [20.4, [800, 450, 1.07]],
    ]);
    return (t) => {
      show(title, t < 10.1);
      animateTitle(title, t, 7.5, 10.0);
      // The window rises in as the title leaves.
      const inP = E.fluid(seg(t, 9.6, 10.4));
      show(cam, t > 9.55);
      setStyle(cam, 'opacity', inP.toFixed(3));
      camera(cam, cameraTrack(t));
      setStyle(win, 'transform', `translate3d(0, ${((1 - inP) * 70).toFixed(1)}px, 0)`);

      // Create Synced Lyrics, pressed on beat 17 (10.625 s), then the job's stages.
      const press = seg(t, 10.55, 10.75);
      const sc = 1 - 0.03 * Math.sin(Math.PI * press);
      setStyle(create, 'transform', `scale(${sc.toFixed(4)})`);
      setStyle(create, 'filter', press > 0 && press < 1 ? 'brightness(1.12)' : 'none');
      const running = t >= 10.75 && t < 14.85;
      show(create, t < 10.75);
      setStyle(job, 'display', running ? 'flex' : 'none');
      let text = 'Reading Audio…';
      let fill = 0;
      let left = 0;
      if (t < 11.25) {
        // Reading Audio: an indeterminate bar.
        const u = ((t - 10.75) / 0.9) % 1;
        left = -40 + u * 140;
        fill = 40;
      } else if (t < 14.1) {
        const pct = t < 12.5 ? 42 * E.out(seg(t, 11.25, 12.5)) : t < 13.35 ? 42 : 42 + 58 * E.inout(seg(t, 13.35, 14.1));
        text = `Recognising… ${Math.round(pct)}%`;
        fill = pct;
      } else {
        text = 'Timing Words…';
        const u = ((t - 14.1) / 0.9) % 1;
        left = -40 + u * 140;
        fill = 40;
      }
      if (stageText.textContent !== text) stageText.textContent = text;
      setStyle(barFill, 'left', `${left}%`);
      setStyle(barFill, 'width', `${fill}%`);
      const noOp = 1 - E.inout(seg(t, 14.8, 15.1));
      setStyle(noLyrics, 'opacity', noOp.toFixed(3));
      show(noLyrics, noOp > 0.001);
      show(box.parentElement, t > 14.8);
      if (t > 14.8) {
        layLines(paneNode, t, { active, moves, entry: 14.85 });
        const ls = $$(box, '.lyric-line');
        sweep(ls[0], t1, t);
        sweep(ls[1], t2, t);
      }
      // The two modes, over the screen.
      const m = fadeIn(t, 18.35, 0.5) * fadeOut(t, 20.0, 0.3);
      setStyle(scrim, 'opacity', (m * 0.95).toFixed(3));
      show(scrim, m > 0.001);
      show(modes, m > 0.001);
      setStyle(modes, 'opacity', m.toFixed(3));
      riseWords(modes, t, 18.4, 0.05, 0.7);
      const exit = E.inout(seg(t, 19.95, 20.35));
      setStyle(node, 'opacity', (1 - exit).toFixed(3));
    };
  });

  // ===== 3. The lyrics bar and Desktop Lyrics (20 – 27.5) =====
  scene(19.85, 27.85, (node) => {
    const title = titleCard(node, { headline: 'Lyrics, word by word.', sub: 'In the player bar, and on your desktop.', dark: false, y: 400 });
    // The desktop: a wallpaper of soft dusk light (no system's own), the window, Desktop Lyrics.
    const desk = el(`<div class="abs" style="inset:0;background:
        radial-gradient(60% 70% at 20% 10%, #3b3a8f 0%, transparent 70%),
        radial-gradient(60% 70% at 85% 25%, #7a3f8c 0%, transparent 70%),
        radial-gradient(70% 70% at 60% 100%, #c2668a 0%, transparent 70%),
        linear-gradient(180deg, #1c1d4a 0%, #3a2456 60%, #5a2f5c 100%)"></div>`);
    node.appendChild(desk);
    const cam = el('<div class="cam"></div>');
    node.appendChild(cam);
    const s = C.barSong;
    const recent = ['night-ferry', 'saltglass', 'copper', 'afterglow', 'weightless'];
    const page = `<h1 class="page-title">Home</h1><p class="summary">1,204 songs · 96 albums · 41 artists</p><h2 class="section">Recently Added</h2><div class="grid">${recent.map(card).join('')}</div>
      <h2 class="section" style="margin-top:20px">Recently Played</h2><div class="rows surface-mist">${itemRow({ id: 'night-ferry', title: s.title, sub: 'Elm & Ember · Night Ferry Postcards', len: '4:02', playing: true })}${itemRow({ id: 'saltglass', title: 'Lilac Wire', sub: 'Violet Harbour · Saltglass', len: '3:05' })}</div>`;
    const win = libraryWindow({ theme: 'night-ferry', active: 'home', page, bar: { id: s.album, title: s.title, lyric: s.line } });
    win.style.left = '160px';
    win.style.top = '90px';
    cam.appendChild(win);
    const barLine = $(win, '.pb-lyric');
    const barTimes = wordTimes(s.line, 22.5, 0.3125, 0.625);
    // Desktop Lyrics: the line and the next, in light ink with a soft shadow.
    const dl = el(`<div class="desk-lyrics" style="left:0;top:0"><div class="desk-bg abs" style="inset:0;border-radius:20px;background:rgba(10,12,22,0.55);box-shadow:inset 0 0 0 1px rgba(255,255,255,0.1)"></div><div class="now" style="position:relative;--hl:rgba(247,248,252,0.96);--jf-ink-70:rgba(247,248,252,0.6)">${lyricWords(s.desktop[0])}</div><div class="next" style="position:relative">${esc(s.desktop[1])}</div><div class="desk-controls"><span>${icon('lock', 20)}</span><span>${icon('close', 20)}</span></div></div>`);
    node.appendChild(dl);
    const dlNow = $(dl, '.now');
    const dlTimes = wordTimes(s.desktop[0], 24.6875);
    const dlBg = $(dl, '.desk-bg');
    const dlCtl = $(dl, '.desk-controls');
    const cap = el(`<div class="abs on-dark" style="left:1060px;width:760px;top:250px;text-align:left"><div style="font-size:84px;font-weight:700;letter-spacing:-0.03em;line-height:1.05">${words('Desktop Lyrics')}</div><div class="sub" style="position:relative;margin-top:20px;font-size:40px;text-align:left;line-height:1.25">${words('The line being sung, in a window of its own.')}</div></div>`);
    node.appendChild(cap);
    const camTrack = track([
      [21.6, [800, 470, 0.95]],
      [22.0, [800, 450, 1.0], E.inout],
      [22.2, [800, 450, 1.0], E.inout],
      [23.0, [520, 680, 2.55]],
      [24.2, [526, 682, 2.62], E.inout],
      [25.0, [1555, 722, 0.54]],
      [27.9, [1540, 716, 0.555]],
    ]);
    return (t) => {
      show(title, t < 22.2);
      animateTitle(title, t, 20.0, 22.1);
      const inP = E.fluid(seg(t, 21.6, 22.3));
      show(cam, t > 21.55);
      setStyle(cam, 'opacity', inP.toFixed(3));
      // Pulling back, the window settles at the upper left of the desktop.
      camera(cam, camTrack(t));
      setStyle(win, 'transform', `translate3d(0, ${((1 - inP) * 60).toFixed(1)}px, 0)`);
      sweep(barLine, barTimes, t);
      // The desktop appears around the window as the camera pulls back.
      const d = E.inout(seg(t, 24.15, 24.9));
      show(desk, d > 0.001);
      setStyle(desk, 'opacity', d.toFixed(3));
      const dIn = E.fluid(seg(t, 24.45, 25.1));
      show(dl, dIn > 0.001);
      setStyle(dl, 'opacity', dIn.toFixed(3));
      setStyle(dl, 'transform', `translate3d(${612}px, ${(770 + (1 - dIn) * 24).toFixed(1)}px, 0)`);
      sweep(dlNow, dlTimes, t);
      // Pointing at it shows its backdrop with Lock and Close.
      const hover = fadeIn(t, 26.1, 0.3) * fadeOut(t, 27.3, 0.3);
      setStyle(dlBg, 'opacity', hover.toFixed(3));
      setStyle(dlCtl, 'opacity', hover.toFixed(3));
      const cp = fadeIn(t, 25.0, 0.5) * fadeOut(t, 27.35, 0.35);
      show(cap, cp > 0.001);
      setStyle(cap, 'opacity', cp.toFixed(3));
      riseWords(cap, t, 25.0, 0.05, 0.75);
      const exit = E.inout(seg(t, 27.4, 27.85));
      setStyle(node, 'opacity', (1 - exit).toFixed(3));
    };
  });

  // ===== 4. Customize (27.5 – 40) =====
  scene(27.25, 40.35, (node) => {
    const title = titleCard(node, { eyebrow: 'New', pro: true, headline: 'Customize', sub: 'Make Jellyfesh yours.', dark: true, grad: true, y: 390 });
    const cam = el('<div class="cam"></div>');
    node.appendChild(cam);
    const song = C.lookSong;
    const THEME = song.album;
    const grid = ['velvet', 'afterglow', 'glasshouse', 'saltglass', 'signal-fires', 'copper', 'moth-light', 'northern-arcade', 'greenhouse', 'weightless'];
    const page = `<h1 class="page-title">Albums</h1><div class="grid">${grid.map(card).join('')}</div>`;
    const lib = libraryWindow({ theme: THEME, appearance: 'dark', active: 'albums', page, bar: { id: song.album, title: song.title } });
    cam.appendChild(lib);
    const np = nowPlayingWindow({ theme: THEME, appearance: 'dark', id: song.album, title: song.title, pane: lyricLines(song.lines), backdrop: 'misty' });
    cam.appendChild(np);
    for (const w of [lib, np]) {
      w.style.left = '0px';
      w.style.top = '0px';
    }
    const npPane = $(np, '.np-pane');
    const sung = el(`<p class="abs sung-line" style="margin:0;left:0;width:1600px;text-align:center;font-size:24px;font-weight:var(--lw);letter-spacing:-0.01em">${lyricWords(song.lines[1])}</p>`);
    $(np, '.np').appendChild(sung);
    // The Customize extension's settings (plugins/customize/settings.schema.json), each a label
    // over its control as the apps' plugin settings show them.
    const row = (id, label, control) => `<div class="set-field"><div class="field-head"><span>${esc(label)}</span>${control === 'slider' ? `<span class="slider-out" data-out="${id}"></span>` : ''}</div>${
      control === 'slider'
        ? `<div class="slider wide" data-id="${id}"><span class="rest"><span class="fill"></span></span><span class="thumb"></span></div>`
        : control === 'well'
          ? `<span class="well" data-id="${id}"></span>`
          : `<div class="seg wide" data-id="${id}">${control.map((o) => `<span data-v="${esc(o)}">${esc(o)}</span>`).join('')}</div>`
    }</div>`;
    const panel = el(`<div class="abs look-panel" style="left:0;top:0;width:430px;${themeVars(THEME, 'dark')}"></div>`);
    panel.innerHTML = `<p class="crumbs">Settings › Extensions › <b>Customize</b></p>
      <section class="group look-card"><h2 class="gtitle">Customize</h2>
        <div class="look-group"><p class="gh">Look</p>${row('corners', 'Corners', 'slider')}${row('glass', 'Glass', 'slider')}${row('aurora', 'Aurora', 'slider')}${row('typeface', 'Typeface', ['System', 'Rounded', 'Serif', 'Monospaced'])}</div>
        <div class="lyrics-group"><p class="gh">Lyrics</p>${row('glow', 'Glow', 'slider')}${row('highlight', 'Highlight', ['Artwork', 'Accent Colour', 'Custom Colour'])}<div class="set-field" style="flex-direction:row;align-items:center;justify-content:space-between"><span class="field-head" style="margin:0"><span>Highlight Colour</span></span><span class="well" data-id="colour"></span></div><p class="gh" style="margin-top:18px">Layout</p>${row('layout', 'Now Playing', ['Stage', 'Artwork', 'Lyrics'])}</div>
      </section>`;
    node.appendChild(panel);
    const lookGroup = $(panel, '.look-group');
    const lyricsGroup = $(panel, '.lyrics-group');

    const FACES = { System: "'Inter', sans-serif", Rounded: "'Nunito', sans-serif", Serif: "'Source Serif 4', serif", Monospaced: "'JetBrains Mono', monospace" };
    const corners = track([[30.0, 1], [30.45, 0.0], [30.625, 0.0], [31.1, 2.0], [31.25, 2.0], [31.6, 1.25]]);
    const glass = track([[31.6, 1], [32.05, 0.0], [32.2, 0.0], [32.55, 1.0]]);
    const aurora = track([[32.5, 0.75], [32.85, 1.0], [32.95, 1.0], [33.35, 0.0], [33.5, 0.0], [33.8, 0.75]]);
    const faceAt = (t) => (t < 33.75 ? 'System' : t < 34.0625 ? 'Rounded' : t < 34.375 ? 'Serif' : t < 34.6875 ? 'Monospaced' : 'System');
    const glow = track([[35.6, 0], [36.2, 1.0]]);
    const highlightAt = (t) => (t < 36.25 ? 'Artwork' : t < 36.875 ? 'Accent Colour' : 'Custom Colour');
    const layoutAt = (t) => (t < 37.5 ? 'Stage' : t < 38.4375 ? 'Artwork' : t < 39.375 ? 'Lyrics' : 'Stage');
    const lineTimes = song.lines.map((l, i) => wordTimes(l, 35.0 + i * 1.25, 0.15625, 0.3125));
    const active = (t) => (t < 36.25 ? 0 : t < 37.5 ? 1 : t < 38.75 ? 2 : 3);
    const moves = [[36.2, 0, 1], [37.45, 1, 2], [38.7, 2, 3]];
    // Now Playing's three layouts (look.ts `arrangement`): where the cover, the titles, the pane
    // and the sung line go, in window px.
    const LAYOUT = {
      Stage: { art: [140, 100, 380], titles: [140, 512, 0], pane: [584, 1], sung: 0 },
      Artwork: { art: [560, 96, 480], titles: [800, 600, 1], pane: [584, 0], sung: 1 },
      Lyrics: { art: [140, 96, 72], titles: [228, 105, 0], pane: [140, 1], sung: 0 },
    };
    function layoutValues(t) {
      const keys = [[37.5, 'Stage', 'Artwork'], [38.4375, 'Artwork', 'Lyrics'], [39.375, 'Lyrics', 'Stage']];
      let from = 'Stage';
      let to = 'Stage';
      let p = 0;
      for (const [time, a, b] of keys) {
        if (t >= time) {
          from = a;
          to = b;
          p = E.fluid(seg(t, time, time + 0.55));
        }
      }
      const A = LAYOUT[from];
      const B = LAYOUT[to];
      return { art: mix(A.art, B.art, p), titles: mix(A.titles, B.titles, p), pane: mix(A.pane, B.pane, p), sung: lerp(A.sung, B.sung, p), from, to, p };
    }
    // The window at 80 % on the left of the frame, the settings on the right.
    const camTrack = track([
      [29.2, [1140, 480, 0.76]],
      [29.75, [1140, 450, 0.8], E.inout],
      [34.6, [1130, 450, 0.815], E.inout],
      [35.1, [1140, 450, 0.8], E.inout],
      [40.4, [1130, 450, 0.82]],
    ]);
    return (t) => {
      show(title, t < 29.7);
      animateTitle(title, t, 27.5, 29.6);
      const inP = E.fluid(seg(t, 29.2, 29.9));
      show(cam, t > 29.15);
      setStyle(cam, 'opacity', inP.toFixed(3));
      camera(cam, camTrack(t), 0, 0);
      // The look, applied live.
      const cs = corners(t);
      const gl = glass(t);
      const au = aurora(t);
      const face = faceAt(t);
      for (const w of [lib, np]) {
        w.style.setProperty('--cs', cs.toFixed(3));
        w.style.setProperty('--gl', gl.toFixed(3));
        w.style.setProperty('--glow', au.toFixed(3));
        w.style.setProperty('--face', FACES[face]);
      }
      setSlider(panel, 'corners', cs, 0, 2, cs.toFixed(2));
      setSlider(panel, 'glass', gl, 0, 1, gl.toFixed(2));
      setSlider(panel, 'aurora', au, 0, 1, au.toFixed(2));
      setSeg(panel, 'typeface', face);
      // The library first, then Now Playing (from beat 56, 35 s).
      const toNp = E.inout(seg(t, 34.85, 35.25));
      show(lib, toNp < 1);
      show(np, toNp > 0);
      setStyle(np, 'opacity', toNp.toFixed(3));
      setStyle(lib, 'transform', `translate3d(0, ${((1 - inP) * 60).toFixed(1)}px, 0)`);
      const pIn = E.fluid(seg(t, 29.55, 30.15));
      show(panel, pIn > 0.001);
      setStyle(panel, 'opacity', pIn.toFixed(3));
      setStyle(panel, 'transform', `translate3d(${(1410 + (1 - pIn) * 40).toFixed(1)}px, 150px, 0) scale(1.1)`);
      const swap = E.inout(seg(t, 34.85, 35.25));
      show(lookGroup, swap < 0.5);
      show(lyricsGroup, swap >= 0.5);
      const gOp = swap < 0.5 ? 1 - swap * 2 : (swap - 0.5) * 2;
      setStyle(lookGroup, 'opacity', gOp.toFixed(3));
      setStyle(lyricsGroup, 'opacity', gOp.toFixed(3));
      // Lyrics: glow, highlight colour; then the layouts.
      const gw = glow(t);
      const hl = highlightAt(t);
      const lay = layoutAt(t);
      setSlider(panel, 'glow', gw, 0, 1, gw.toFixed(2));
      setSeg(panel, 'highlight', hl);
      setSeg(panel, 'layout', lay);
      const custom = '#7cf0d0';
      $(panel, '.well[data-id="colour"]').style.background = custom;
      const hlColour = hl === 'Artwork' ? 'var(--jf-ink-90)' : hl === 'Accent Colour' ? 'var(--jf-accent-text)' : custom;
      np.style.setProperty('--hl', hlColour);
      np.style.setProperty('--lglow', gw.toFixed(3));
      if (toNp > 0) {
        const L = layoutValues(t);
        const art = $(np, '.np-art');
        setStyle(art, 'left', `${L.art[0].toFixed(1)}px`);
        setStyle(art, 'top', `${L.art[1].toFixed(1)}px`);
        setStyle(art, 'width', `${L.art[2].toFixed(1)}px`);
        setStyle(art, 'height', `${L.art[2].toFixed(1)}px`);
        const titles = $(np, '.np-titles');
        // Centred under the cover in Artwork; beside the small cover in Lyrics.
        const centred = L.titles[2];
        setStyle(titles, 'left', `${L.titles[0].toFixed(1)}px`);
        setStyle(titles, 'top', `${L.titles[1].toFixed(1)}px`);
        setStyle(titles, 'transform', `translateX(${(-50 * centred).toFixed(2)}%)`);
        setStyle(titles, 'alignItems', centred > 0.5 ? 'center' : 'flex-start');
        setStyle(titles, 'textAlign', centred > 0.5 ? 'center' : 'start');
        setStyle(npPane, 'left', `${L.pane[0].toFixed(1)}px`);
        setStyle(npPane, 'width', `${(1460 - L.pane[0]).toFixed(1)}px`);
        setStyle(npPane, 'opacity', L.pane[1].toFixed(3));
        const wide = L.to === 'Lyrics' ? L.p : L.from === 'Lyrics' ? 1 - L.p : 0;
        setStyle(npPane, 'top', `${(78 + 110 * wide).toFixed(1)}px`);
        setStyle(npPane, 'height', `${(694 - 110 * wide).toFixed(1)}px`);
        setStyle(sung, 'opacity', L.sung.toFixed(3));
        setStyle(sung, 'top', `${(L.titles[1] + 92).toFixed(1)}px`);
        show(sung, L.sung > 0.01);
        layLines(npPane, t, { active, moves });
        const ls = $$(npPane, '.lyric-line');
        ls.forEach((l, i) => sweep(l, lineTimes[i], t));
        // The sung line under the cover (Artwork layout): the line being sung now.
        const k = active(t);
        if (sung.dataset.k !== String(k)) {
          sung.innerHTML = lyricWords(song.lines[k]);
          sung.dataset.k = String(k);
        }
        sweep(sung, lineTimes[k], t);
        setStyle(sung, 'textShadow', `0 0 ${(18 * gw).toFixed(1)}px color-mix(in srgb, ${hlColour} ${(gw * 60).toFixed(0)}%, transparent)`);
      }
      const exit = E.inout(seg(t, 39.9, 40.35));
      setStyle(node, 'opacity', (1 - exit).toFixed(3));
    };
  });

  // ===== 5. Discover, Get Info, Comments (40 – 50) =====
  scene(39.75, 50.35, (node) => {
    const title = titleCard(node, { headline: 'Discover', sub: 'Every source opens on its own Discover page.', dark: false, y: 390 });
    const cam = el('<div class="cam"></div>');
    node.appendChild(cam);
    const src = C.source;
    const page = `<header style="display:flex;align-items:flex-end;justify-content:space-between;margin-bottom:20px"><div><h1 class="page-title" style="margin:0">${esc(src.name)}</h1><p style="margin:4px 0 0;color:var(--jf-ink-70)">Signed in as ${esc(src.account)}</p></div><span class="btn-secondary" style="gap:8px">${icon('folder', 18)}Browse</span></header>
      <section class="shelf"><div style="display:flex;align-items:baseline;justify-content:space-between;margin-bottom:12px"><h2 class="section" style="margin:0">For You</h2></div><div class="row-shelf">${C.forYou.map(card).join('')}</div></section>
      <section class="shelf" style="margin-top:12px"><div style="display:flex;align-items:baseline;justify-content:space-between;margin-bottom:12px"><h2 class="section" style="margin:0">Charts</h2><span style="display:inline-flex;align-items:center;gap:2px;color:var(--jf-accent-text);font-size:14px;font-weight:600">See All${icon('forward', 16)}</span></div><div class="rows surface-mist">${C.charts.map((c, i) => itemRow({ n: i + 1, id: c[2], title: c[0], sub: c[1], len: c[3], playing: i === 1 })).join('')}</div></section>
      <section class="shelf" style="margin-top:24px"><div style="display:flex;align-items:baseline;justify-content:space-between;margin-bottom:12px"><h2 class="section" style="margin:0">Recently Added</h2><span style="display:inline-flex;align-items:center;gap:2px;color:var(--jf-accent-text);font-size:14px;font-weight:600">See All${icon('forward', 16)}</span></div><div class="row-shelf">${C.recentlyAdded.map(card).join('')}</div></section>`;
    const win = libraryWindow({ theme: 'northern-arcade', active: 'sources', page, bar: { id: 'northern-arcade', title: 'Slow Satellites' } });
    win.style.left = '160px';
    win.style.top = '90px';
    cam.appendChild(win);
    const scroll = $(win, '.scroll');
    const cards = $$(win, '.row-shelf .card');
    const rows = $$(win, '.rows .item-row');
    // Get Info (GetInfoSheet): About This Song, then Audio.
    const dl = (pairs) => `<dl>${pairs.map(([k, v]) => `<dt>${esc(k)}</dt><dd>${esc(v)}</dd>`).join('')}</dl>`;
    const sheetLayer = el(`<div class="abs" style="inset:0"><div class="scrim"></div><div class="sheet" style="left:540px;top:130px"><header><h2>About This Song</h2><span class="close">${icon('close')}</span></header><div class="body">${dl(C.info.about)}<h3>Audio</h3>${dl(C.info.audio)}</div></div></div>`);
    win.appendChild(sheetLayer);
    const sheet = $(sheetLayer, '.sheet');
    const scrim = $(sheetLayer, '.scrim');
    // Comments (CommentsPane) in Now Playing's fourth pane.
    const avatars = ['linear-gradient(135deg,#ffb38a,#ff6f91)', 'linear-gradient(135deg,#7ad7f0,#4f7cff)', 'linear-gradient(135deg,#b8f28a,#2fb67a)', 'linear-gradient(135deg,#d6b8ff,#8a6cff)'];
    const comments = `<div class="comments" style="padding:22px 8px 0"><p class="from">Comments from ${esc(src.name)}</p><h3>Popular</h3><ul>${C.comments
      .map(
        (c, i) =>
          `<li><span class="avatar" style="background:${avatars[i]}"></span><div><p class="meta"><span class="author">${esc(c[0])}</span><span>${esc(c[1])}</span><span class="likes">${icon('heart', 14)}${esc(c[2])}</span></p><p class="ctext">${esc(c[3])}</p></div></li>`,
      )
      .join('')}</ul></div>`;
    const np = nowPlayingWindow({ theme: 'northern-arcade', appearance: 'light', id: 'northern-arcade', title: 'Slow Satellites', panes: ['Lyrics', 'Queue', 'Info', 'Comments'], on: 'Comments', pane: comments });
    np.style.left = '160px';
    np.style.top = '90px';
    cam.appendChild(np);
    const items = $$(np, '.comments li');
    const capInfo = captionPill('Get Info, with the sound’s real quality.', true);
    const capComments = captionPill('Comments, where a source has them.', false);
    node.appendChild(capInfo);
    node.appendChild(capComments);
    const camTrack = track([
      [41.6, [800, 470, 0.95]],
      [42.0, [800, 450, 1.0], E.inout],
      [42.2, [800, 450, 1.0], E.inout],
      [43.0, [800, 450, 1.2], E.inout],
      [45.55, [800, 450, 1.2], E.inout],
      [46.3, [800, 405, 1.5]],
      [47.45, [800, 420, 1.58], E.inout],
      [48.0, [835, 420, 1.32]],
      [50.4, [828, 414, 1.36]],
    ]);
    const scrollTrack = track([[43.3, 0], [44.4, -296], [45.0, -296]]);
    return (t) => {
      show(title, t < 42.1);
      animateTitle(title, t, 40.0, 42.0);
      const inP = E.fluid(seg(t, 41.6, 42.3));
      show(cam, t > 41.55);
      setStyle(cam, 'opacity', inP.toFixed(3));
      camera(cam, camTrack(t));
      setStyle(win, 'transform', `translate3d(0, ${((1 - inP) * 60).toFixed(1)}px, 0)`);
      // Cards and rows rise in one after another (ItemCard, ItemRow).
      cards.forEach((c, i) => {
        const p = E.fluid(seg(t, 41.85 + (i % 7) * 0.06 + (i >= 7 ? 0.9 : 0), 42.35 + (i % 7) * 0.06 + (i >= 7 ? 0.9 : 0)));
        setStyle(c, 'opacity', p.toFixed(3));
        setStyle(c, 'transform', `translate3d(0, ${((1 - p) * 14).toFixed(1)}px, 0)`);
      });
      rows.forEach((r, i) => {
        const p = E.fluid(seg(t, 42.3 + i * 0.05, 42.8 + i * 0.05));
        setStyle(r, 'opacity', p.toFixed(3));
        setStyle(r, 'transform', `translate3d(0, ${((1 - p) * 12).toFixed(1)}px, 0)`);
      });
      setStyle(scroll, 'transform', `translate3d(0, ${scrollTrack(t).toFixed(1)}px, 0)`);
      // Get Info: the sheet scales from 96 % and fades in (dur-2 Fluid), over the scrim.
      const sIn = E.fluid(seg(t, 45.625, 46.0));
      show(sheetLayer, sIn > 0.001 && t < 47.7);
      setStyle(scrim, 'opacity', sIn.toFixed(3));
      setStyle(sheet, 'opacity', sIn.toFixed(3));
      setStyle(sheet, 'transform', `scale(${(0.96 + 0.04 * sIn).toFixed(4)})`);
      pill(capInfo, t, 46.2, 47.45, 962);
      // Comments.
      const cIn = E.inout(seg(t, 47.45, 47.85));
      show(np, cIn > 0);
      show(win, cIn < 1);
      setStyle(np, 'opacity', cIn.toFixed(3));
      items.forEach((it, i) => {
        const p = E.fluid(seg(t, 47.75 + i * 0.09, 48.3 + i * 0.09));
        setStyle(it, 'opacity', p.toFixed(3));
        setStyle(it, 'transform', `translate3d(0, ${((1 - p) * 14).toFixed(1)}px, 0)`);
      });
      pill(capComments, t, 48.4, 50.0, 962);
      const exit = E.inout(seg(t, 49.95, 50.35));
      setStyle(node, 'opacity', (1 - exit).toFixed(3));
    };
  });

  // ===== 6. One app, five platforms (50 – 57.5) =====
  scene(49.85, 57.85, (node) => {
    const head = el(`<div class="headline on-light" style="top:150px;font-size:110px">${words('One app. Five platforms.')}</div>`);
    node.appendChild(head);
    // Generic windows: plain rounded shapes holding the same app, the phone a taller one.
    const tiles = [
      { name: 'macOS', w: 352, h: 198, kind: 'library', theme: 'saltglass', appearance: 'light' },
      { name: 'Windows', w: 352, h: 198, kind: 'np', theme: 'afterglow', appearance: 'dark' },
      { name: 'Linux', w: 352, h: 198, kind: 'albums', theme: 'greenhouse', appearance: 'light' },
      { name: 'Android', w: 138, h: 284, kind: 'phone', theme: 'night-ferry', appearance: 'dark' },
      { name: 'Web', w: 352, h: 198, kind: 'discover', theme: 'northern-arcade', appearance: 'light' },
    ];
    const gap = 36;
    const total = tiles.reduce((s, x) => s + x.w, 0) + gap * (tiles.length - 1);
    let x = (1920 - total) / 2;
    const built = tiles.map((tile, i) => {
      const box = el(`<div class="abs" style="left:${x}px;top:${545 - tile.h / 2}px;width:${tile.w}px;height:${tile.h}px"></div>`);
      const frame = el(`<div class="device" style="inset:0;border-radius:${tile.kind === 'phone' ? 22 : 12}px;box-shadow:0 0 0 1px rgba(20,30,60,0.10),0 26px 60px -22px rgba(30,40,90,0.45),0 8px 20px rgba(30,40,90,0.12)"></div>`);
      box.appendChild(frame);
      let inner;
      if (tile.kind === 'phone') inner = phoneScreen(tile.theme);
      else if (tile.kind === 'np') inner = nowPlayingWindow({ theme: tile.theme, id: tile.theme, title: 'Ember Avenue', pane: lyricLines(['Every light along the avenue', 'Burns a little brighter now', 'Follow me where the night is new']) });
      else if (tile.kind === 'library')
        inner = libraryWindow({ theme: tile.theme, active: 'home', page: `<h1 class="page-title">Home</h1><p class="summary">1,204 songs · 96 albums · 41 artists</p><h2 class="section">Recently Added</h2><div class="grid">${['saltglass', 'velvet', 'copper', 'weightless', 'moth-light'].map(card).join('')}</div>`, bar: { id: 'saltglass', title: 'Lilac Wire' } });
      else if (tile.kind === 'albums')
        inner = libraryWindow({ theme: tile.theme, active: 'albums', page: `<h1 class="page-title">Albums</h1><div class="grid">${['greenhouse', 'low-tide', 'northern-arcade', 'signal-fires', 'night-ferry', 'glasshouse', 'afterglow', 'velvet', 'copper', 'saltglass'].map(card).join('')}</div>`, bar: { id: 'greenhouse', title: 'Night Garden' } });
      else
        inner = libraryWindow({ theme: tile.theme, active: 'sources', page: `<h1 class="page-title">${esc(C.source.name)}</h1><h2 class="section">For You</h2><div class="row-shelf">${C.forYou.map(card).join('')}</div><h2 class="section" style="margin-top:12px">Recently Added</h2><div class="row-shelf">${C.recentlyAdded.map(card).join('')}</div>`, bar: { id: 'northern-arcade', title: 'Slow Satellites' } });
      if (tile.kind !== 'phone') {
        inner.style.transformOrigin = '0 0';
        inner.style.transform = `scale(${tile.w / 1600})`;
        inner.style.borderRadius = '0';
        inner.style.boxShadow = 'none';
      }
      frame.appendChild(inner);
      const label = el(`<div class="device-label" style="left:${x - 40}px;width:${tile.w + 80}px;top:${545 + 284 / 2 + 30}px">${tile.name}</div>`);
      node.appendChild(box);
      node.appendChild(label);
      x += tile.w + gap;
      return { box, label, start: 51.875 + i * BEAT };
    });
    return (t) => {
      riseWords(head, t, 50.0, 0.08, 0.8);
      const up = E.inout(seg(t, 51.2, 51.85));
      setStyle(head, 'transform', `translate3d(0, ${(-40 * up).toFixed(1)}px, 0) scale(${(1 - 0.16 * up).toFixed(4)})`);
      for (const b of built) {
        const p = E.spring(seg(t, b.start, b.start + 0.55));
        const o = E.fluid(seg(t, b.start, b.start + 0.35));
        setStyle(b.box, 'opacity', o.toFixed(3));
        setStyle(b.box, 'transform', `translate3d(0, ${((1 - p) * 40).toFixed(1)}px, 0) scale(${(0.9 + 0.1 * p).toFixed(4)})`);
        const l = E.fluid(seg(t, b.start + 0.1, b.start + 0.5));
        setStyle(b.label, 'opacity', l.toFixed(3));
        setStyle(b.label, 'transform', `translate3d(0, ${((1 - l) * 12).toFixed(1)}px, 0)`);
      }
      const push = E.inout(seg(t, 55.0, 57.6));
      setStyle(node, 'transform', `scale(${(1 + 0.035 * push).toFixed(4)})`);
      const exit = E.inout(seg(t, 57.4, 57.85));
      setStyle(node, 'opacity', (1 - exit).toFixed(3));
    };
  });

  buildPro();
  }

  /** A phone's Now Playing (Android: a rounded dock card, one view at a time). */
  function phoneScreen(theme) {
    const a = album(theme);
    return el(`<div class="abs" style="inset:0;${themeVars(theme, 'dark')};background:var(--jf-sky);overflow:hidden">
      <img class="backdrop" src="../assets/backdrops/${theme}-dark.png" alt="" style="position:absolute;inset:0;width:100%;height:100%;object-fit:cover">
      <div class="abs" style="left:10px;right:10px;top:22px;aspect-ratio:1;border-radius:10px;overflow:hidden;box-shadow:0 10px 24px rgba(0,0,0,0.45)"><img class="cover-img" src="${cover(theme)}" alt=""></div>
      <div class="abs" style="left:11px;top:140px;color:var(--jf-ink-90);font-size:10px;font-weight:700;white-space:nowrap">Hold the Light</div>
      <div class="abs" style="left:11px;top:154px;color:var(--jf-ink-70);font-size:7px;font-weight:500;white-space:nowrap">${esc(a.artist)}</div>
      <div class="abs" style="left:8px;right:8px;bottom:12px;height:72px;border-radius:16px;background:var(--jf-tint-veil);box-shadow:inset 0 0 0 1px var(--jf-glass-edge-soft)">
        <div class="abs" style="left:12px;right:12px;top:14px;height:3px;border-radius:2px;background:var(--jf-ink-30)"><div style="width:38%;height:100%;border-radius:2px;background:var(--jf-ink-90)"></div></div>
        <div class="abs" style="left:0;right:0;top:30px;display:flex;justify-content:center;align-items:center;gap:14px;color:var(--jf-ink-90)">${icon('previous', 14)}<span style="width:30px;height:30px;border-radius:50%;background:var(--jf-accent);color:var(--jf-on-accent);display:grid;place-items:center">${icon('pause', 15)}</span>${icon('next', 14)}</div>
      </div>
    </div>`);
  }

  function buildPro() {
  // ===== 7. Jellyfesh Pro (57.5 – 62.5) =====
  scene(57.35, 62.85, (node) => {
    const glowLayer = el('<div class="abs" style="inset:0;background:radial-gradient(40% 46% at 50% 44%, rgba(141,183,255,0.28) 0%, transparent 70%), radial-gradient(30% 36% at 62% 58%, rgba(226,184,255,0.22) 0%, transparent 70%)"></div>');
    node.appendChild(glowLayer);
    const head = el(`<div class="headline on-dark" style="top:250px;font-size:150px">${words('Jellyfesh Pro', 'grad-dark')}</div>`);
    const sub = el(`<div class="sub on-dark" style="top:452px;font-size:56px">${words('AI Lyrics and Customize.')}</div>`);
    const free = el(`<div class="headline on-dark" style="top:548px;font-size:72px;letter-spacing:-0.025em">${words('Free during the public beta.')}</div>`);
    node.appendChild(head);
    node.appendChild(sub);
    node.appendChild(free);
    // Settings › Extensions › Jellyfesh Pro, as a Pro Preview build shows it.
    const cardBox = el(`<div class="abs" style="left:610px;top:722px;width:700px;${themeVars('weightless', 'dark')}"></div>`);
    cardBox.innerHTML = `<section class="group surface-mist" style="padding:22px 28px 22px;background:color-mix(in srgb, var(--jf-cloud) 80%, transparent);box-shadow:inset 0 0 0 1px var(--jf-glass-edge),0 30px 80px -24px rgba(0,0,0,0.6)"><h2 class="gtitle" style="margin:0 0 8px">Jellyfesh Pro</h2><p style="margin:0 0 12px;font-size:17px">Pro features are free during the public beta.</p><p style="margin:0;display:flex;gap:20px;font-size:14px;color:var(--jf-accent-text)"><span>Licence Agreement</span><span>Privacy Policy</span><span>Terms of Sale</span></p></section>`;
    node.appendChild(cardBox);
    return (t) => {
      const g = fadeIn(t, 57.5, 1.2, E.soft);
      setStyle(glowLayer, 'opacity', (g * (0.85 + 0.15 * Math.sin(t * 1.3))).toFixed(3));
      riseWords(head, t, 57.6, 0.08, 0.85);
      riseWords(sub, t, 58.75, 0.06, 0.8);
      riseWords(free, t, 60.0, 0.06, 0.8);
      const c = E.fluid(seg(t, 60.6, 61.2));
      setStyle(cardBox, 'opacity', c.toFixed(3));
      setStyle(cardBox, 'transform', `translate3d(0, ${((1 - c) * 24).toFixed(1)}px, 0)`);
      const exit = E.inout(seg(t, 62.3, 62.8));
      setStyle(node, 'opacity', (1 - exit).toFixed(3));
    };
  });

  // ===== 8. End card (62.5 – 70) =====
  scene(62.35, 70.01, (node) => {
    const iconBox = el('<div class="abs" style="left:870px;top:250px;width:180px;height:180px;border-radius:40px;overflow:hidden;box-shadow:0 0 0 1px rgba(255,255,255,0.12),0 30px 80px -20px rgba(0,0,0,0.7),0 0 120px rgba(141,183,255,0.25)"><img src="/website/assets/icon-ice.svg" alt="" style="width:100%;height:100%;display:block"></div>');
    node.appendChild(iconBox);
    const name = el(`<div class="headline on-dark" style="top:478px;font-size:66px;font-weight:650;letter-spacing:-0.02em">${words('Jellyfesh · Public Beta v0.0.7')}</div>`);
    node.appendChild(name);
    const url = el('<div class="sub" style="top:584px;font-size:40px;font-weight:600;color:#a3cbff;letter-spacing:-0.005em">https://zodiacals.github.io/Jellyfesh/</div>');
    node.appendChild(url);
    const legal = el('<div class="sub" style="top:956px;font-size:27px;font-weight:500;color:rgba(247,248,252,0.7)">Screens are simulated with sample songs and artwork. Features as of public beta v0.0.7; some vary by platform and device.</div>');
    node.appendChild(legal);
    const black = el('<div class="abs" style="inset:0;background:#000"></div>');
    node.appendChild(black);
    return (t) => {
      const i = E.spring(seg(t, 62.6, 63.3));
      setStyle(iconBox, 'opacity', E.fluid(seg(t, 62.6, 63.0)).toFixed(3));
      setStyle(iconBox, 'transform', `scale(${(0.86 + 0.14 * i).toFixed(4)})`);
      riseWords(name, t, 63.1, 0.06, 0.8);
      const u = E.fluid(seg(t, 63.75, 64.35));
      setStyle(url, 'opacity', u.toFixed(3));
      setStyle(url, 'transform', `translate3d(0, ${((1 - u) * 14).toFixed(1)}px, 0)`);
      const l = E.fluid(seg(t, 64.4, 65.0));
      setStyle(legal, 'opacity', l.toFixed(3));
      const b = E.inout(seg(t, 68.9, 70.0));
      setStyle(black, 'opacity', b.toFixed(3));
    };
  });

  }

  // ---------- The label and the grain ----------
  const label = el('<div id="label"><span class="dot"></span>Simulated screens · Sample content</div>');
  stage.appendChild(label);
  const grain = el('<canvas class="grain" width="960" height="540" style="width:1920px;height:1080px"></canvas>');
  stage.appendChild(grain);
  function paintGrain() {
    const ctx = grain.getContext('2d');
    const img = ctx.createImageData(960, 540);
    let s = 0x2545f491;
    for (let i = 0; i < img.data.length; i += 4) {
      s ^= s << 13;
      s ^= s >>> 17;
      s ^= s << 5;
      const v = 128 + ((s >>> 0) % 255) - 127;
      img.data[i] = v;
      img.data[i + 1] = v;
      img.data[i + 2] = v;
      img.data[i + 3] = 255;
    }
    ctx.putImageData(img, 0, 0);
  }

  // ---------- The frame ----------
  window.renderAt = (t) => {
    t = clamp(t, 0, DURATION);
    // The dark aurora under the dark beats (the cold open, AI Lyrics, Customize, Pro, the end),
    // the light one under the light beats (the lyrics bar, Discover, the platforms), crossfading.
    const light = Math.min(fadeIn(t, 19.85, 0.45, E.inout) * fadeOut(t, 27.25, 0.45), 1) + fadeIn(t, 39.75, 0.45, E.inout) * fadeOut(t, 57.35, 0.45);
    show(bgLight, light > 0.001);
    setStyle(bgLight, 'opacity', light.toFixed(3));
    show(bgDark, light < 0.999);
    setStyle(bgDark, 'opacity', E.soft(seg(t, 0, 1.6)).toFixed(3));
    drift(bgDark, t);
    drift(bgLight, t);
    for (const s of scenes) {
      const on = t >= s.from && t < s.to;
      show(s.node, on);
      if (on) {
        setStyle(s.node, 'opacity', '1');
        setStyle(s.node, 'transform', 'none');
        s.render(t);
      }
    }
    const l = fadeIn(t, 9.7, 0.5) * (1 - E.inout(seg(t, 62.3, 62.7)));
    label.style.display = l > 0.001 ? 'flex' : 'none';
    setStyle(label, 'opacity', l.toFixed(3));
  };

  window.sceneReady = (async () => {
    const ids = Object.keys(C.albums);
    await Promise.all(
      ids.map(async (id) => {
        THEMES[id] = await (await fetch(`../assets/themes/${id}.json`)).json();
      }),
    );
    return true;
  })();
  window.buildAll = async () => {
    await window.sceneReady;
    buildScenes();
    stage.appendChild(label);
    stage.appendChild(grain);
    await buildLettering();
    paintGrain();
    await document.fonts.load("700 100px 'Inter'");
    await document.fonts.load("600 20px 'Nunito'");
    await document.fonts.load("600 20px 'Source Serif 4'");
    await document.fonts.load("600 20px 'JetBrains Mono'");
    await document.fonts.ready;
    await Promise.all([...document.images].map((img) => (img.complete ? Promise.resolve() : img.decode().catch(() => {}))));
    window.renderAt(0);
    return true;
  };
})();
