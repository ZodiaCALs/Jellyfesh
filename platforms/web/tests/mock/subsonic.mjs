// A mock OpenSubsonic server (Navidrome-like) for Jellyfesh Web's end-to-end tests. It answers the
// requests the official Subsonic plugin makes, in the shapes of the OpenSubsonic documentation,
// with token authentication (t = md5(password + salt)) and the invented library of ./library.mjs.
// CORS is on unless `cors: false` (to test the page's diagnosis).

import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import {
  ALBUMS,
  ARTISTS,
  FAVOURITES,
  FOLLOWED_PLAYLISTS,
  LYRICS,
  NO_DOWNLOAD,
  PLAYLISTS,
  TRACK_SECONDS,
  artistOf,
  track,
  tracks,
} from './library.mjs';
import { png, wav } from './media.mjs';

export const SUBSONIC_USER = { name: 'demo', password: 'invented-password' };

const COVERS = new Map();
const AUDIO = new Map();

function md5(text) {
  return createHash('md5').update(text).digest('hex');
}

function song(t) {
  const artist = artistOf(t.album);
  return {
    id: `s${t.id}`,
    parent: `al${t.album.id}`,
    isDir: false,
    title: t.title,
    album: t.album.title,
    albumId: `al${t.album.id}`,
    artist: artist.name,
    artistId: `ar${artist.id}`,
    track: t.number,
    discNumber: 1,
    year: t.album.year,
    coverArt: `al${t.album.id}`,
    size: TRACK_SECONDS * 88200 + 44,
    contentType: 'audio/wav',
    suffix: 'wav',
    duration: TRACK_SECONDS,
    bitRate: 706,
    type: 'music',
    mediaType: 'song',
  };
}

function album(a, withSongs) {
  const artist = artistOf(a);
  const out = {
    id: `al${a.id}`,
    name: a.title,
    artist: artist.name,
    artistId: `ar${artist.id}`,
    coverArt: `al${a.id}`,
    songCount: a.tracks.length,
    duration: a.tracks.length * TRACK_SECONDS,
    year: a.year,
    created: `${a.year}-01-01T00:00:00Z`,
  };
  if (withSongs) out.song = tracks().filter((t) => t.album.id === a.id).map(song);
  return out;
}

function artistJson(a) {
  return { id: `ar${a.id}`, name: a.name, albumCount: ALBUMS.filter((al) => al.artist === a.id).length };
}

function envelope(status, extra) {
  return {
    'subsonic-response': {
      status,
      version: '1.16.1',
      type: 'mocksonic',
      serverVersion: '0.1.0 (Jellyfesh Web tests)',
      openSubsonic: true,
      ...extra,
    },
  };
}

export function startSubsonic({ port = 0, cors = true, host = '127.0.0.1' } = {}) {
  const server = createServer((req, res) => {
    const url = new URL(req.url ?? '/', `http://${req.headers.host}`);
    const headers = {};
    if (cors) {
      headers['access-control-allow-origin'] = '*';
      headers['access-control-allow-methods'] = 'GET, POST, OPTIONS';
      headers['access-control-allow-headers'] = 'Content-Type, Range, Authorization';
      headers['access-control-expose-headers'] = 'Content-Range, Accept-Ranges, Content-Length';
    }
    const send = (status, body, type, extra = {}) => {
      res.writeHead(status, { ...headers, 'content-type': type, 'cache-control': 'no-store', ...extra });
      res.end(body);
    };
    const ok = (extra) => send(200, JSON.stringify(envelope('ok', extra)), 'application/json');
    const fail = (code, message) => send(200, JSON.stringify(envelope('failed', { error: { code, message } })), 'application/json');

    if (req.method === 'OPTIONS') return send(204, '', 'text/plain');
    let body = '';
    req.on('data', (chunk) => (body += chunk));
    req.on('end', () => {
      const params = new URLSearchParams(url.search);
      if (req.method === 'POST' && body) for (const [k, v] of new URLSearchParams(body)) params.append(k, v);
      const p = (key) => params.get(key);
      const match = /^\/rest\/(\w+?)(?:\.view)?$/.exec(url.pathname);
      if (!match) return send(404, '<html><body>Not found</body></html>', 'text/html');
      const method = match[1];
      if (p('c') !== 'Jellyfesh' || !p('v')) return fail(10, 'Required parameter is missing');
      if (method === 'getOpenSubsonicExtensions') {
        return ok({
          openSubsonicExtensions: [
            { name: 'formPost', versions: [1] },
            { name: 'songLyrics', versions: [1] },
            { name: 'transcodeOffset', versions: [1] },
          ],
        });
      }
      // Media requests are authenticated too (the token is in the URL).
      const user = p('u');
      const token = p('t');
      const salt = p('s');
      if (p('p')) return fail(42, 'Password authentication is not allowed');
      if (!user || !token || !salt) return fail(10, 'Required parameter is missing');
      if (user !== SUBSONIC_USER.name || salt.length < 6 || token !== md5(SUBSONIC_USER.password + salt)) {
        return fail(40, 'Wrong username or password');
      }
      if (p('f') !== 'json' && method !== 'stream' && method !== 'getCoverArt') {
        return send(200, '<subsonic-response status="ok"/>', 'text/xml');
      }
      const id = p('id') ?? '';
      switch (method) {
        case 'ping':
          return ok({});
        case 'getUser':
          return ok({
            user: { username: user, scrobblingEnabled: true, adminRole: false, streamRole: true, downloadRole: true, folder: [1] },
          });
        case 'getArtists':
          return ok({
            artists: {
              ignoredArticles: 'The An A',
              index: [...new Set(ARTISTS.map((a) => a.name[0].toUpperCase()))].sort().map((letter) => ({
                name: letter,
                artist: ARTISTS.filter((a) => a.name[0].toUpperCase() === letter).map(artistJson),
              })),
            },
          });
        case 'getArtist': {
          const a = ARTISTS.find((x) => `ar${x.id}` === id);
          if (!a) return fail(70, 'Artist not found');
          return ok({ artist: { ...artistJson(a), album: ALBUMS.filter((al) => al.artist === a.id).map((al) => album(al, false)) } });
        }
        case 'getAlbum': {
          const a = ALBUMS.find((x) => `al${x.id}` === id);
          return a ? ok({ album: album(a, true) }) : fail(70, 'Album not found');
        }
        case 'getSong': {
          const t = track(id.replace(/^s/, ''));
          return t && id.startsWith('s') ? ok({ song: song(t) }) : fail(70, 'Song not found');
        }
        case 'getAlbumList2': {
          const offset = Number(p('offset') ?? 0);
          const size = Math.min(500, Number(p('size') ?? 10));
          if (!p('type')) return fail(10, 'Required parameter is missing: type');
          return ok({ albumList2: { album: ALBUMS.map((a) => album(a, false)).slice(offset, offset + size) } });
        }
        case 'getPlaylists':
          return ok({
            playlists: {
              playlist: [
                ...PLAYLISTS.map((pl) => ({ ...pl, owner: user, public: false })),
                ...FOLLOWED_PLAYLISTS.map((pl) => ({ ...pl, public: true })),
              ].map((pl) => ({
                id: `pl${pl.id}`,
                name: pl.title,
                songCount: pl.tracks.length,
                duration: pl.tracks.length * TRACK_SECONDS,
                owner: pl.owner,
                public: pl.public,
              })),
            },
          });
        case 'getPlaylist': {
          const pl = [...PLAYLISTS, ...FOLLOWED_PLAYLISTS].find((x) => `pl${x.id}` === id);
          if (!pl) return fail(70, 'Playlist not found');
          return ok({
            playlist: {
              id: `pl${pl.id}`,
              name: pl.title,
              songCount: pl.tracks.length,
              duration: pl.tracks.length * TRACK_SECONDS,
              entry: pl.tracks.map((t) => song(track(t))),
            },
          });
        }
        case 'getStarred2': {
          // In the server's own order, not by date: the plugin sorts them newest first.
          const starred = (list, of) => list.map(({ id: key, at }) => ({ ...of(key), starred: at }));
          return ok({
            starred2: {
              artist: starred(FAVOURITES.artists, (key) => artistJson(ARTISTS.find((a) => a.id === key))),
              album: starred(FAVOURITES.albums, (key) => album(ALBUMS.find((a) => a.id === key), false)),
              song: starred(FAVOURITES.songs, (key) => song(track(key))),
            },
          });
        }
        case 'search3': {
          const query = (p('query') ?? '').toLowerCase().replace(/^"|"$/g, '');
          const has = (text) => text.toLowerCase().includes(query);
          const slice = (list, count, offset) => list.slice(Number(p(offset) ?? 0), Number(p(offset) ?? 0) + Number(p(count) ?? 20));
          return ok({
            searchResult3: {
              artist: slice(ARTISTS.filter((a) => has(a.name)).map(artistJson), 'artistCount', 'artistOffset'),
              album: slice(ALBUMS.filter((a) => has(a.title)).map((a) => album(a, false)), 'albumCount', 'albumOffset'),
              song: slice(tracks().filter((t) => has(t.title)).map(song), 'songCount', 'songOffset'),
            },
          });
        }
        case 'getLyricsBySongId': {
          const t = track(id.replace(/^s/, ''));
          if (!t) return fail(70, 'Song not found');
          const lines = LYRICS[t.id];
          if (!lines) return ok({ lyricsList: { structuredLyrics: [] } });
          return ok({
            lyricsList: {
              structuredLyrics: [
                {
                  displayArtist: artistOf(t.album).name,
                  displayTitle: t.title,
                  lang: t.id === '6' ? 'jpn' : 'eng',
                  synced: true,
                  line: lines.map(([start, value]) => ({ start, value })),
                },
              ],
            },
          });
        }
        case 'getLyrics':
          return ok({ lyrics: {} });
        case 'getSimilarSongs':
          return ok({ similarSongs: { song: tracks().slice(0, 3).map(song) } });
        case 'stream': {
          const t = track(id.replace(/^s/, ''));
          if (!t) return fail(70, 'Song not found');
          if (!AUDIO.has(t.id)) AUDIO.set(t.id, wav(TRACK_SECONDS, t.frequency));
          return sendRange(req, res, headers, AUDIO.get(t.id), 'audio/wav');
        }
        case 'download': {
          // The file as it is (`format=raw`, as Navidrome needs); a song the account may not have
          // is refused as Subsonic servers do: HTTP 200 with an error in JSON, not the file.
          const t = track(id.replace(/^s/, ''));
          if (!t) return fail(70, 'Song not found');
          if (NO_DOWNLOAD.includes(t.id)) return fail(50, 'User is not authorized for the given operation.');
          if (!AUDIO.has(t.id)) AUDIO.set(t.id, wav(TRACK_SECONDS, t.frequency));
          return send(200, AUDIO.get(t.id), 'audio/wav', {
            'content-disposition': `attachment; filename="${t.number} - ${t.title}.wav"`,
          });
        }
        case 'getCoverArt': {
          const a = ALBUMS.find((x) => `al${x.id}` === id);
          if (!a) return send(404, '', 'text/plain');
          const size = Math.min(600, Math.max(32, Number(p('size') ?? 300)));
          const key = `${a.id}:${size}`;
          if (!COVERS.has(key)) COVERS.set(key, png(size, a.cover.from, a.cover.to));
          return send(200, COVERS.get(key), 'image/png');
        }
        default:
          return fail(0, `Unknown method ${method}`);
      }
    });
  });
  return new Promise((resolve) => server.listen(port, host, () => resolve(server)));
}

/** Answers a Range request (206) or the whole body (200). */
export function sendRange(req, res, headers, body, type) {
  const range = /^bytes=(\d*)-(\d*)$/.exec(req.headers.range ?? '');
  const base = { ...headers, 'content-type': type, 'accept-ranges': 'bytes', 'cache-control': 'no-store' };
  if (!range || (range[1] === '' && range[2] === '')) {
    res.writeHead(200, { ...base, 'content-length': body.length });
    res.end(req.method === 'HEAD' ? undefined : body);
    return;
  }
  let start = range[1] === '' ? body.length - Number(range[2]) : Number(range[1]);
  let end = range[1] === '' || range[2] === '' ? body.length - 1 : Number(range[2]);
  start = Math.max(0, start);
  end = Math.min(body.length - 1, end);
  if (start > end) {
    res.writeHead(416, { ...base, 'content-range': `bytes */${body.length}` });
    res.end();
    return;
  }
  res.writeHead(206, { ...base, 'content-range': `bytes ${start}-${end}/${body.length}`, 'content-length': end - start + 1 });
  res.end(req.method === 'HEAD' ? undefined : body.subarray(start, end + 1));
}
