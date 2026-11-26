// A mock Jellyfin server for Jellyfesh Web's end-to-end tests. It answers the requests the official
// Jellyfin plugin makes, like the Jellyfin API (PascalCase JSON, QueryResult pages, the
// `MediaBrowser` Authorization header, Quick Connect), with the invented library of ./library.mjs.
// `POST /__mock/approve` with `{"Code": "…"}` approves a Quick Connect code, as a person would in
// Jellyfin's own app.

import { randomBytes } from 'node:crypto';
import { createServer } from 'node:http';
import { ALBUMS, ARTISTS, FAVOURITES, LYRICS, NO_DOWNLOAD, PLAYLISTS, TRACK_SECONDS, artistOf, track, tracks } from './library.mjs';
import { png, wav } from './media.mjs';
import { sendRange } from './subsonic.mjs';

export const JELLYFIN_USER = { name: 'demo', password: 'invented-password', id: 'e6000000000000000000000000000001' };
const SERVER_ID = '0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f0f';
const LIBRARY = 'f1000000000000000000000000000001';
const TICKS = 10_000_000;

const id = (prefix, n) => `${prefix}${String(n).padStart(32 - prefix.length, '0')}`;
const artistId = (a) => id('a2', a.id);
const albumId = (a) => id('b3', a.id);
const songId = (t) => id('c4', t.id);
const playlistId = (p) => id('d5', p.id);
/** The account's favourites, by item id (Jellyfin's `IsFavorite`). */
const FAVOURITE_IDS = new Set([
  ...FAVOURITES.songs.map(({ id: key }) => songId(track(key))),
  ...FAVOURITES.albums.map(({ id: key }) => albumId(ALBUMS.find((a) => a.id === key))),
  ...FAVOURITES.artists.map(({ id: key }) => artistId(ARTISTS.find((a) => a.id === key))),
]);

function artistDto(a) {
  return { Name: a.name, Id: artistId(a), Type: 'MusicArtist', IsFolder: true, ImageTags: {} };
}

function albumDto(a) {
  const artist = artistOf(a);
  const credit = [{ Name: artist.name, Id: artistId(artist) }];
  return {
    Name: a.title,
    Id: albumId(a),
    Type: 'MusicAlbum',
    IsFolder: true,
    AlbumArtist: artist.name,
    AlbumArtists: credit,
    Artists: [artist.name],
    ArtistItems: credit,
    ProductionYear: a.year,
    RunTimeTicks: a.tracks.length * TRACK_SECONDS * TICKS,
    ChildCount: a.tracks.length,
    ImageTags: { Primary: `cover-${a.id}` },
    _library: LIBRARY,
  };
}

function songDto(t) {
  const artist = artistOf(t.album);
  const credit = [{ Name: artist.name, Id: artistId(artist) }];
  return {
    Name: t.title,
    Id: songId(t),
    Type: 'Audio',
    MediaType: 'Audio',
    IsFolder: false,
    Album: t.album.title,
    AlbumId: albumId(t.album),
    AlbumPrimaryImageTag: `cover-${t.album.id}`,
    AlbumArtist: artist.name,
    AlbumArtists: credit,
    Artists: [artist.name],
    ArtistItems: credit,
    IndexNumber: t.number,
    ParentIndexNumber: 1,
    ProductionYear: t.album.year,
    RunTimeTicks: TRACK_SECONDS * TICKS,
    ImageTags: {},
    _library: LIBRARY,
    MediaSources: [
      {
        Protocol: 'File',
        Id: songId(t),
        Type: 'Default',
        Container: 'wav',
        Bitrate: 705600,
        Name: t.title,
        SupportsDirectPlay: true,
        SupportsDirectStream: true,
        SupportsTranscoding: true,
        MediaStreams: [{ Type: 'Audio', Codec: 'pcm_s16le', BitRate: 705600, Channels: 1, SampleRate: 44100, Index: 0 }],
      },
    ],
  };
}

function playlistDto(p) {
  return {
    Name: p.title,
    Id: playlistId(p),
    Type: 'Playlist',
    MediaType: 'Audio',
    IsFolder: true,
    RunTimeTicks: p.tracks.length * TRACK_SECONDS * TICKS,
    ChildCount: p.tracks.length,
    ImageTags: {},
  };
}

function all() {
  return [...ALBUMS.map(albumDto), ...tracks().map(songDto), ...PLAYLISTS.map(playlistDto)];
}

function find(itemId) {
  const lower = itemId.toLowerCase();
  return [...all(), ...ARTISTS.map(artistDto)].find((i) => i.Id === lower) ?? null;
}

/** Fields only sent when asked for (`fields=`), and the mock's own `_` keys removed. */
function shape(item, fields) {
  const out = {};
  for (const [k, v] of Object.entries(item)) if (!k.startsWith('_')) out[k] = v;
  if (!fields.includes('MediaSources')) delete out.MediaSources;
  if (!fields.includes('ChildCount')) delete out.ChildCount;
  if (fields.includes('PlayAccess')) out.PlayAccess = 'Full';
  // The account's EnableContentDownloading policy, per item.
  if (fields.includes('CanDownload')) out.CanDownload = item.Type !== 'Audio' || !NO_DOWNLOAD.some((key) => songId(track(key)) === item.Id);
  out.ServerId = SERVER_ID;
  return out;
}

function page(list, params) {
  const start = Number(params.get('startIndex') ?? 0);
  const limit = params.has('limit') ? Number(params.get('limit')) : list.length;
  return { items: list.slice(start, start + limit), start, total: list.length };
}

function sortItems(list, params) {
  const keys = (params.get('sortBy') ?? '').split(',').filter(Boolean);
  const descending = (params.get('sortOrder') ?? '').startsWith('Descending');
  if (keys.length === 0) return list;
  const value = (item, key) =>
    key === 'IndexNumber' || key === 'ParentIndexNumber' || key === 'ProductionYear'
      ? (item[key] ?? 0)
      : String(item.Name ?? '').toLowerCase();
  return [...list].sort((a, b) => {
    for (const key of keys) {
      const [x, y] = [value(a, key), value(b, key)];
      if (x < y) return descending ? 1 : -1;
      if (x > y) return descending ? -1 : 1;
    }
    return 0;
  });
}

/** `Authorization: MediaBrowser k="v", …` (AuthorizationContext.GetParts). */
function authParts(header) {
  if (!header) return null;
  const space = header.indexOf(' ');
  if (space < 0 || header.slice(0, space).toLowerCase() !== 'mediabrowser') return null;
  const parts = {};
  for (const match of header.slice(space + 1).matchAll(/(\w+)="([^"]*)"/g)) parts[match[1]] = decodeURIComponent(match[2].replace(/\+/g, ' '));
  return parts;
}

export function startJellyfin({ port = 0, host = '127.0.0.1' } = {}) {
  const sessions = new Map();
  const quick = [];
  const audio = new Map();
  const covers = new Map();

  function user() {
    return {
      Name: JELLYFIN_USER.name,
      ServerId: SERVER_ID,
      Id: JELLYFIN_USER.id,
      HasPassword: true,
      HasConfiguredPassword: true,
      Policy: { IsAdministrator: false, IsDisabled: false, EnableMediaPlayback: true, EnableAudioPlaybackTranscoding: true },
    };
  }

  function issue(parts) {
    const token = randomBytes(16).toString('hex');
    sessions.set(token, { deviceId: parts.DeviceId, revoked: false });
    return {
      User: user(),
      SessionInfo: { Id: `s${token}`, UserId: JELLYFIN_USER.id, UserName: JELLYFIN_USER.name, Client: parts.Client, DeviceId: parts.DeviceId },
      AccessToken: token,
      ServerId: SERVER_ID,
    };
  }

  function quickResult(request, parts) {
    return {
      Authenticated: request.approved,
      Secret: request.secret,
      Code: request.code,
      DeviceId: request.deviceId,
      DeviceName: request.device,
      AppName: parts.Client,
      AppVersion: parts.Version,
      DateAdded: '2026-11-09T12:00:00.0000000Z',
    };
  }

  const server = createServer((req, res) => {
    const url = new URL(req.url ?? '/', `http://${req.headers.host}`);
    const params = url.searchParams;
    const path = url.pathname;
    const headers = {
      'access-control-allow-origin': '*',
      'access-control-allow-methods': 'GET, POST, DELETE, OPTIONS',
      'access-control-allow-headers': 'Authorization, Content-Type, Range, X-Emby-Authorization',
      'access-control-expose-headers': 'Content-Range, Accept-Ranges, Content-Length',
    };
    const send = (status, body, type = 'application/json') => {
      res.writeHead(status, { ...headers, 'content-type': type, 'cache-control': 'no-store' });
      res.end(typeof body === 'string' || Buffer.isBuffer(body) ? body : JSON.stringify(body));
    };
    if (req.method === 'OPTIONS') return send(204, '', 'text/plain');
    let raw = '';
    req.on('data', (chunk) => (raw += chunk));
    req.on('end', () => {
      const body = (() => {
        try {
          return raw ? JSON.parse(raw) : {};
        } catch {
          return {};
        }
      })();
      // Public endpoints.
      if (req.method === 'GET' && path === '/System/Info/Public') {
        return send(200, {
          LocalAddress: `http://${req.headers.host}`,
          ServerName: 'Mock Jellyfin (tests)',
          Version: '10.11.0',
          ProductName: 'Jellyfin Server',
          OperatingSystem: '',
          Id: SERVER_ID,
          StartupWizardCompleted: true,
        });
      }
      if (req.method === 'GET' && path === '/QuickConnect/Enabled') return send(200, true);
      const image = /^\/Items\/([0-9a-f]+)\/Images\/Primary$/i.exec(path);
      if (req.method === 'GET' && image) {
        const item = find(image[1]);
        const album = item?.Type === 'MusicAlbum' ? ALBUMS.find((a) => albumId(a) === item.Id) : null;
        if (!album) return send(404, 'Error processing request.', 'text/plain');
        const size = Math.min(600, Math.max(32, Number(params.get('fillWidth') ?? params.get('maxWidth') ?? 300)));
        const key = `${album.id}:${size}`;
        if (!covers.has(key)) covers.set(key, png(size, album.cover.from, album.cover.to));
        return send(200, covers.get(key), 'image/png');
      }
      if (req.method === 'POST' && path === '/__mock/approve') {
        const request = quick.find((q) => q.code === String(body.Code ?? ''));
        if (!request) return send(404, 'Unknown code', 'text/plain');
        request.approved = true;
        return send(204, '', 'text/plain');
      }
      const parts = authParts(req.headers.authorization);
      if (!parts) return send(401, '', 'text/plain');
      if (['Client', 'Device', 'DeviceId', 'Version'].some((k) => !parts[k])) return send(400, 'Error processing request.', 'text/plain');
      if (req.method === 'POST' && path === '/Users/AuthenticateByName') {
        if (body.Username !== JELLYFIN_USER.name || body.Pw !== JELLYFIN_USER.password) {
          return send(401, 'Error processing request.', 'text/plain');
        }
        return send(200, issue(parts));
      }
      if (req.method === 'POST' && path === '/QuickConnect/Initiate') {
        const n = quick.length + 1;
        const request = {
          secret: randomBytes(32).toString('hex').toUpperCase(),
          code: String(482913 + n * 7),
          deviceId: parts.DeviceId,
          device: parts.Device,
          approved: false,
        };
        quick.push(request);
        return send(200, quickResult(request, parts));
      }
      if (req.method === 'GET' && path === '/QuickConnect/Connect') {
        const request = quick.find((q) => q.secret === params.get('secret'));
        return request ? send(200, quickResult(request, parts)) : send(404, 'Unknown secret', 'text/plain');
      }
      if (req.method === 'POST' && path === '/Users/AuthenticateWithQuickConnect') {
        const request = quick.find((q) => q.secret === body.Secret);
        if (!request || !request.approved) return send(404, 'Error processing request.', 'text/plain');
        return send(200, issue({ ...parts, DeviceId: request.deviceId, Device: request.device }));
      }
      // Signed-in endpoints.
      const session = sessions.get(parts.Token ?? '');
      if (!session || session.revoked || session.deviceId !== parts.DeviceId) return send(401, '', 'text/plain');
      const fields = (params.get('fields') ?? '').split(',').filter(Boolean);
      if (req.method === 'POST' && path === '/Sessions/Logout') {
        session.revoked = true;
        return send(204, '', 'text/plain');
      }
      if (req.method === 'GET' && path === '/Users/Me') return send(200, user());
      if (req.method === 'GET' && path === '/UserViews') {
        return send(200, {
          Items: [{ Name: 'Music', Id: LIBRARY, Type: 'CollectionFolder', CollectionType: 'music', IsFolder: true, ServerId: SERVER_ID }],
          TotalRecordCount: 1,
          StartIndex: 0,
        });
      }
      if (req.method === 'GET' && (path === '/Artists' || path === '/Artists/AlbumArtists')) {
        let list = ARTISTS.map(artistDto);
        const term = params.get('searchTerm');
        if (term) list = list.filter((a) => a.Name.toLowerCase().includes(term.toLowerCase()));
        if (params.get('isFavorite') === 'true') list = list.filter((a) => FAVOURITE_IDS.has(a.Id));
        const { items, start, total } = page(sortItems(list, params), params);
        return send(200, { Items: items.map((i) => shape(i, fields)), TotalRecordCount: total, StartIndex: start });
      }
      if (req.method === 'GET' && path === '/Items') {
        const kinds = (params.get('includeItemTypes') ?? '').split(',').filter(Boolean);
        let list = all();
        if (kinds.length) list = list.filter((i) => kinds.includes(i.Type));
        const ids = (params.get('ids') ?? '').split(',').filter(Boolean).map((x) => x.toLowerCase());
        if (ids.length) list = list.filter((i) => ids.includes(i.Id));
        const parent = params.get('parentId');
        if (parent) {
          const node = find(parent);
          if (node?.Type === 'MusicAlbum') list = list.filter((i) => i.AlbumId === node.Id);
          else if (parent.toLowerCase() !== LIBRARY) return send(404, 'Error processing request.', 'text/plain');
        }
        const term = params.get('searchTerm');
        if (term) list = list.filter((i) => i.Name.toLowerCase().includes(term.toLowerCase()));
        if (params.get('isFavorite') === 'true') list = list.filter((i) => FAVOURITE_IDS.has(i.Id));
        const artistIds = (params.get('artistIds') ?? params.get('albumArtistIds') ?? '').split(',').filter(Boolean);
        if (artistIds.length) {
          list = list.filter((i) => [...(i.AlbumArtists ?? []), ...(i.ArtistItems ?? [])].some((a) => artistIds.includes(a.Id)));
        }
        const { items, start, total } = page(sortItems(list, params), params);
        return send(200, { Items: items.map((i) => shape(i, fields)), TotalRecordCount: total, StartIndex: start });
      }
      const segments = path.split('/').filter(Boolean);
      if (segments[0] === 'Playlists' && segments[2] === 'Items') {
        const playlist = PLAYLISTS.find((p) => playlistId(p) === segments[1]?.toLowerCase());
        if (!playlist) return send(404, 'Error processing request.', 'text/plain');
        const { items, start, total } = page(playlist.tracks.map((t) => songDto(track(t))), params);
        return send(200, { Items: items.map((i) => shape(i, fields)), TotalRecordCount: total, StartIndex: start });
      }
      if (req.method === 'GET' && segments[0] === 'Items' && segments[2] === 'Download') {
        // The original file, with the MediaBrowser authorization header (checked above).
        const t = tracks().find((x) => songId(x) === segments[1]?.toLowerCase());
        if (!t) return send(404, 'Error processing request.', 'text/plain');
        if (NO_DOWNLOAD.includes(t.id)) return send(403, '', 'text/plain');
        if (!audio.has(t.id)) audio.set(t.id, wav(TRACK_SECONDS, t.frequency));
        return sendRange(req, res, headers, audio.get(t.id), 'audio/wav');
      }
      if (segments[0] === 'Items' && segments[2] === 'InstantMix') {
        return send(200, { Items: tracks().slice(0, 3).map((t) => shape(songDto(t), fields)), TotalRecordCount: 3, StartIndex: 0 });
      }
      if (segments[0] === 'Audio' && segments[2] === 'Lyrics') {
        const t = tracks().find((x) => songId(x) === segments[1]?.toLowerCase());
        const lines = t ? LYRICS[t.id] : null;
        if (!t || !lines) return send(404, 'Error processing request.', 'text/plain');
        return send(200, {
          Metadata: { Artist: artistOf(t.album).name, Album: t.album.title, Title: t.title, IsSynced: true },
          Lyrics: lines.map(([start, text]) => ({ Text: text.replace(/<[^>]*>/g, '').replace(/\s+/g, ' ').trim(), Start: start * 10_000 })),
        });
      }
      if (segments[0] === 'Audio' && segments[2] === 'universal') {
        const t = tracks().find((x) => songId(x) === segments[1]?.toLowerCase());
        if (!t) return send(404, 'Error processing request.', 'text/plain');
        if (!audio.has(t.id)) audio.set(t.id, wav(TRACK_SECONDS, t.frequency));
        return sendRange(req, res, headers, audio.get(t.id), 'audio/wav');
      }
      return send(404, '', 'text/plain');
    });
  });
  return new Promise((resolve) => server.listen(port, host, () => resolve(server)));
}
