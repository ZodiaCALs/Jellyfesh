// Sample content for the promo. Every artist, album, song, lyric line, comment and account
// here is invented for this video; the covers are abstract gradients made by
// tools/make_assets.py. Nothing is taken from real releases, services or people.

window.CONTENT = {
  albums: {
    'low-tide': { title: 'Low Tide Lanterns', artist: 'Juniper Atlas', year: 2026 },
    glasshouse: { title: 'Glasshouse Summer', artist: 'Marlowe Quay', year: 2026 },
    'night-ferry': { title: 'Night Ferry Postcards', artist: 'Elm & Ember', year: 2025 },
    'northern-arcade': { title: 'Northern Arcade', artist: 'Northbound Echo', year: 2026 },
    saltglass: { title: 'Saltglass', artist: 'Violet Harbour', year: 2024 },
    afterglow: { title: 'Afterglow Avenue', artist: 'The Lumen Choir', year: 2026 },
    weightless: { title: 'Weightless Rooms', artist: 'Arlo Meridian', year: 2025 },
    copper: { title: 'Copper Coastlines', artist: 'Odessa Bloom', year: 2026 },
    greenhouse: { title: 'Midnight Greenhouse', artist: 'Saltwater Static', year: 2025 },
    'signal-fires': { title: 'Signal Fires', artist: 'Fern & Forsythe', year: 2026 },
    'moth-light': { title: 'Moth Light', artist: 'Ivory Tides', year: 2024 },
    velvet: { title: 'Velvet Static', artist: 'Cassia Drift', year: 2026 },
  },

  // AI Lyrics: a song with no lyrics, for which AI Lyrics makes word-timed ones.
  aiSong: {
    title: 'Tidewater Hours',
    album: 'low-tide',
    lines: [
      'Salt on the window, gold on the floor',
      'You left the morning by the door',
      'I hum the tide back, line by line',
      'Until your voice catches up with mine',
    ],
    engine: 'Whisper Base on this device',
  },

  // The lyrics bar and Desktop Lyrics.
  barSong: {
    title: 'Hold the Light',
    album: 'night-ferry',
    line: 'Hold the light a little longer',
    desktop: ['Let the quiet city hum along', 'Every window keeps a little song'],
  },

  // Customize: Now Playing while the look changes.
  lookSong: {
    title: 'Colour Runs',
    album: 'velvet',
    lines: [
      'We were glass in the August heat',
      'Every colour running down the street',
      'Turn it up and let it shine',
      'On the summer we left behind',
    ],
  },

  // Discover on an invented source.
  source: { name: 'Sample Source', account: 'Sam' },
  forYou: ['northern-arcade', 'saltglass', 'copper', 'weightless', 'greenhouse', 'velvet', 'moth-light'],
  charts: [
    ['Colour Runs', 'Cassia Drift', 'velvet', '3:31'],
    ['Slow Satellites', 'Northbound Echo', 'northern-arcade', '4:12'],
    ['Tidewater Hours', 'Juniper Atlas', 'low-tide', '3:48'],
    ['Lilac Wire', 'Violet Harbour', 'saltglass', '3:05'],
    ['Ember Avenue', 'The Lumen Choir', 'afterglow', '3:57'],
  ],
  recentlyAdded: ['afterglow', 'signal-fires', 'night-ferry', 'low-tide', 'glasshouse', 'copper', 'saltglass'],

  // Get Info for a source's song.
  info: {
    about: [
      ['Title', 'Slow Satellites'],
      ['Artist', 'Northbound Echo'],
      ['Album', 'Northern Arcade'],
      ['Year', '2026'],
      ['Genre', 'Dream Pop'],
      ['Composer', 'R. Calloway'],
      ['Length', '4:12'],
    ],
    audio: [
      ['Quality', 'Hi-Res Lossless'],
      ['Format', 'FLAC · 24-bit · 96 kHz · Stereo'],
      ['Bitrate', '2,904 kbit/s'],
      ['Size', '91.5 MB'],
      ['Source', 'Sample Source'],
    ],
  },

  // Comments on a source's song (read-only in the app).
  comments: [
    ['Mira L.', 'Nov 12, 2026', '2,184', 'The bridge at 2:40 gets me every single time.'],
    ['tidepool', 'Nov 9, 2026', '1,032', 'Played this on the night bus home. Perfect.'],
    ['Jonas', 'Nov 2, 2026', '846', 'That bass line under the second chorus!'],
    ['ferrylights', 'Oct 28, 2026', '512', 'Saving this one for long drives.'],
  ],
};
