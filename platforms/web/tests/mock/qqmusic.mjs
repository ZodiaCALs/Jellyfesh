// A mock of WeChat's QR sign-in and of the QQ Music web player's API, for the relay's end-to-end
// test (tests/e2e/relay.spec.ts). It answers what the unofficial QQ Music plugin asks when it is
// built with its `mock-service` feature (which sends every request to `<mock>/<host>/<path>`),
// the way plugins/community/qqmusic/tests/mock answers it, from the same invented library
// (plugins/community/qqmusic/tests/fixtures/library.json) and generated gradient covers.
//
// Like the services as Jellyfesh Web meets them, it sends no CORS headers, so a page cannot read
// its answers, and it wants what a page cannot send: the web player's Referer and a browser's
// User-Agent on every request, and the session in Cookie for signed-in calls. A request without
// them is answered as the service would refuse it, and recorded. GET /__mock/requests lists what
// it received (for the test, which is not a page: no CORS needed).

import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { png } from './media.mjs';

const LIBRARY = JSON.parse(
  readFileSync(new URL('../../../../plugins/community/qqmusic/tests/fixtures/library.json', import.meta.url), 'utf8'),
);
const ACCOUNT = LIBRARY.account;
export const WECHAT_APP_ID = 'wx48db31d50e334801';

/** g_tk of a session key, as the web player computes it. */
function gtk(key) {
  let hash = 5381;
  for (const unit of key) hash = (hash + ((hash << 5) & 0xffffffff) + unit.charCodeAt(0)) & 0xffffffff;
  return (hash >>> 0) & 0x7fffffff;
}

function song(mid) {
  const found = LIBRARY.songs.find((s) => s.mid === mid);
  if (!found) return null;
  const { stream: _stream, result: _result, ...rest } = found;
  return rest;
}

function search(param) {
  const words = String(param.query ?? '').toLowerCase().split(/\s+/).filter(Boolean);
  const matches = (text) => words.length > 0 && words.every((w) => text.toLowerCase().includes(w));
  const names = (list) => (list ?? []).map((s) => s.name).join(' ');
  const body = { song: { list: [] }, singer: { list: [] }, album: { list: [] }, songlist: { list: [] } };
  let found = [];
  let key = 'song';
  switch (param.search_type) {
    case 0:
      found = LIBRARY.songs.filter((s) => matches(`${s.name} ${names(s.singer)}`)).map((s) => song(s.mid));
      break;
    case 1:
      key = 'singer';
      found = LIBRARY.singers.filter((s) => matches(s.singerName ?? ''));
      break;
    case 2:
      key = 'album';
      found = LIBRARY.albums
        .filter((a) => matches(`${a.albumName} ${names(a.singer_list)}`))
        .map(({ songs, ...a }) => ({ ...a, song_count: songs.length }));
      break;
    default:
      found = [];
  }
  const page = Math.max(1, param.page_num ?? 1);
  const size = param.num_per_page ?? 10;
  body[key].list = found.slice((page - 1) * size, page * size);
  return { code: 0, data: { body, meta: { sum: found.length, curpage: page, perpage: size } } };
}

export function startQqMusic({ port = 0, host = '127.0.0.1' } = {}) {
  /** Every request: what the test checks about the headers that reached the service. */
  const requests = [];
  const polls = new Map();
  const codes = new Set();
  const keys = new Set();
  let pages = 0;
  let issued = 0;

  function api(req, res, body, seen) {
    let request;
    try {
      request = JSON.parse(body.toString('utf8'));
    } catch {
      return json(res, { code: 500001 });
    }
    const comm = request.comm ?? {};
    const key = comm.authst ?? '';
    const cookie = req.headers.cookie ?? '';
    const signedIn = key !== '' && keys.has(key) && comm.uin === ACCOUNT.musicid && cookie.includes(`qm_keyst=${key}`);
    seen.signedIn = signedIn;
    seen.calls = [];
    const answer = { code: 0, ts: 1700000000000 };
    for (const [name, part] of Object.entries(request)) {
      if (!name.startsWith('req_')) continue;
      seen.calls.push(`${part.module}.${part.method}`);
      answer[name] = call(part.module, part.method, part.param ?? {}, comm, signedIn);
    }
    if (comm.g_tk !== gtk(key)) seen.problem = `g_tk ${comm.g_tk} for key ${key}`;
    return json(res, answer);
  }

  function call(module, method, param, comm, signedIn) {
    switch (`${module}.${method}`) {
      case 'music.login.LoginServer.Login': {
        if (!param.code || !codes.has(param.code) || param.strAppid !== WECHAT_APP_ID || comm.tmeLoginType !== 1) {
          return { code: 1000, msg: 'mock: unknown WeChat code' };
        }
        codes.delete(param.code);
        issued += 1;
        const musickey = `W_X_mock_key_${String(issued).padStart(4, '0')}`;
        keys.add(musickey);
        return {
          code: 0,
          data: {
            musicid: ACCOUNT.musicid,
            str_musicid: String(ACCOUNT.musicid),
            musickey,
            refresh_key: `mock_refresh_${issued}`,
            refresh_token: 'mock_refresh_token',
            keyExpiresIn: 259200,
            encryptUin: 'mock-encrypted-uin',
            loginType: 1,
          },
        };
      }
      case 'music.UserInfo.userInfoServer.GetLoginUserInfo':
        return signedIn ? { code: 0, data: { info: { nick: ACCOUNT.nick, uin: ACCOUNT.musicid } } } : { code: 1000, msg: 'mock: not signed in' };
      case 'music.search.SearchCgiService.DoSearchForQQMusicDesktop':
        return search(param);
      default:
        return { code: 500001, msg: 'mock: a call the mock does not know' };
    }
  }

  const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://mock');
    if (url.pathname === '/__mock/requests') return json(res, requests);
    const chunks = [];
    req.on('data', (chunk) => chunks.push(chunk));
    req.on('end', () => {
      const [, service, ...rest] = url.pathname.split('/');
      const path = `/${rest.join('/')}`;
      const seen = {
        host: service,
        path,
        method: req.method,
        referer: req.headers.referer ?? null,
        userAgent: req.headers['user-agent'] ?? null,
        origin: req.headers.origin ?? null,
        hasCookie: Boolean(req.headers.cookie),
      };
      requests.push(seen);
      // The web player's headers, which a page cannot set: without them the service says no.
      if (!seen.userAgent?.startsWith('Mozilla/5.0') || !seen.referer?.startsWith('https://')) {
        if (!service.endsWith('y.gtimg.cn')) {
          seen.refused = true;
          res.writeHead(403, { 'content-type': 'text/plain' });
          return res.end('mock: not a browser of this site');
        }
      }
      const body = Buffer.concat(chunks);
      if (service === 'open.weixin.qq.com' && path === '/connect/qrconnect') {
        pages += 1;
        res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
        return res.end(
          `<!DOCTYPE html><html><body><div class="wrp_code"><img class="qrcode" src="/connect/qrcode/091MockUuid${String(pages).padStart(4, '0')}" /></div></body></html>`,
        );
      }
      if (service === 'lp.open.weixin.qq.com' && path === '/connect/l/qrconnect') {
        const uuid = url.searchParams.get('uuid') ?? '';
        const count = polls.get(uuid) ?? 0;
        polls.set(uuid, count + 1);
        // Waiting, scanned, then confirmed with a code.
        const status = [408, 404, 405][Math.min(count, 2)];
        let code = '';
        if (status === 405) {
          code = `081MockWxCode${String(codes.size + 1).padStart(4, '0')}`;
          codes.add(code);
        }
        res.writeHead(200, { 'content-type': 'application/javascript' });
        return res.end(`window.wx_errcode=${status};window.wx_code='${code}';`);
      }
      if (service === 'u.y.qq.com' && path === '/cgi-bin/musics.fcg' && req.method === 'POST') return api(req, res, body, seen);
      if (service === 'y.gtimg.cn' || service === 'qpic.y.qq.com') {
        const image = png(64, [0x2b, 0x6c, 0xb0], [0xe0, 0x7a, 0x5f]);
        res.writeHead(200, { 'content-type': 'image/png', 'content-length': image.length });
        return res.end(image);
      }
      res.writeHead(404, { 'content-type': 'text/plain' });
      res.end('not found');
    });
  });
  return new Promise((resolve) => server.listen(port, host, () => resolve(server)));
}

function json(res, value) {
  const body = JSON.stringify(value);
  res.writeHead(200, { 'content-type': 'application/json' });
  res.end(body);
}
