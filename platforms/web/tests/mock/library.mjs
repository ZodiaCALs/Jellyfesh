// The invented library both mock servers serve. Every name, title and lyric line here was made up
// for the tests; the audio is generated sine tones and the covers generated gradients.

/** Seconds per track: short, so the end-to-end tests reach every transition quickly. */
export const TRACK_SECONDS = 8;

export const ARTISTS = [
  { id: '1', name: 'Glass Harbour' },
  { id: '2', name: 'Tidal Choir' },
];

export const ALBUMS = [
  {
    id: '1',
    title: 'Slow Tide',
    artist: '1',
    year: 2025,
    cover: { from: [38, 132, 160], to: [22, 40, 92] },
    tracks: [
      { id: '1', title: 'Morning Harbour', frequency: 440 },
      { id: '2', title: 'Rope and Mast', frequency: 554.37 },
      { id: '3', title: 'Lanterns', frequency: 659.25 },
      { id: '4', title: 'Still Water', frequency: 880 },
    ],
  },
  {
    id: '2',
    title: 'Paper Lanterns',
    artist: '2',
    year: 2024,
    cover: { from: [236, 132, 104], to: [104, 60, 152] },
    tracks: [
      { id: '5', title: 'Night Ferry', frequency: 392 },
      { id: '6', title: '海の灯', frequency: 523.25 },
    ],
  },
];

export const PLAYLISTS = [{ id: '1', title: 'Evening Walk', tracks: ['3', '5', '4'] }];

/** Another account's public playlist, which the person follows (Subsonic lists it too). */
export const FOLLOWED_PLAYLISTS = [{ id: '2', title: 'Harbour Mornings', owner: 'mira', tracks: ['1', '2'] }];

/**
 * What the account marked on the server (plugin API 1.3 `library.likes`): songs, albums and
 * artists by id, with when (Subsonic's `starred`; Jellyfin keeps no date and lists them by name).
 */
export const FAVOURITES = {
  songs: [
    { id: '4', at: '2026-11-20T09:00:00Z' },
    { id: '2', at: '2026-11-22T09:00:00Z' },
  ],
  albums: [{ id: '2', at: '2026-11-21T09:00:00Z' }],
  artists: [{ id: '2', at: '2026-11-21T09:00:00Z' }],
};

/** Songs the server will not let the account download (Subsonic: error 50 instead of the file). */
export const NO_DOWNLOAD = ['6'];

/** Every track with its album. */
export function tracks() {
  return ALBUMS.flatMap((album) => album.tracks.map((track, index) => ({ ...track, album, number: index + 1 })));
}

export function track(id) {
  return tracks().find((t) => t.id === id) ?? null;
}

export function artistOf(album) {
  return ARTISTS.find((a) => a.id === album.artist);
}

/**
 * Lyrics by track id: `[ms, text]` lines. Enhanced LRC word stamps (`<mm:ss.xx>`) in a line make
 * it word-timed; a second line at the same time in another script is its translation (the
 * bilingual LRC convention jf-lyrics reads).
 */
export const LYRICS = {
  1: [
    [500, '<00:00.50>Morning <00:01.10>folds <00:01.50>the <00:01.80>harbour <00:02.40>into <00:02.80>light <00:03.30>'],
    [500, '清晨把港湾折进光里'],
    [3400, '<00:03.40>Every <00:03.90>rope <00:04.40>and <00:04.70>mast <00:05.20>remembers <00:06.00>night <00:06.60>'],
    [3400, '每根绳索与桅杆都还记得夜'],
    [6700, '<00:06.70>Slow <00:07.10>tide, <00:07.50>carry <00:07.90>me <00:08.00>'],
    [6700, '慢潮啊，带我走'],
  ],
  6: [
    [400, '灯台のあかり'],
    [400, 'The lighthouse light'],
    [3200, '波にゆれて'],
    [3200, 'Swaying on the waves'],
    [6000, '海の灯'],
    [6000, 'Lights of the sea'],
  ],
};
