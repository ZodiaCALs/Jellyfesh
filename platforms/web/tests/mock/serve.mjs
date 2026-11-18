// Starts the mock servers for the end-to-end tests and for trying the app by hand:
//   Subsonic (CORS on)  http://127.0.0.1:4610   user "demo", password "invented-password"
//   Jellyfin            http://127.0.0.1:4620   the same; Quick Connect codes approve with
//                                               POST /__mock/approve {"Code": "…"}
//   Subsonic (no CORS)  http://127.0.0.1:4630   for the page's CORS diagnosis
//   QQ Music (no CORS)  http://127.0.0.1:4640   WeChat's QR sign-in and the web player's API, as
//                                               the unofficial plugin's mock-service build asks
//                                               them; reached only through a relay (tools/jf-relay)
// Everything they serve is invented: generated sine tones, gradient covers, made-up names.
//
// Usage: node tests/mock/serve.mjs   (ports from MOCK_SUBSONIC_PORT, MOCK_JELLYFIN_PORT, MOCK_NOCORS_PORT,
// MOCK_QQMUSIC_PORT)

import { startJellyfin } from './jellyfin.mjs';
import { startQqMusic } from './qqmusic.mjs';
import { startSubsonic } from './subsonic.mjs';

const port = (name, fallback) => Number(process.env[name] ?? fallback);

const subsonic = await startSubsonic({ port: port('MOCK_SUBSONIC_PORT', 4610) });
const jellyfin = await startJellyfin({ port: port('MOCK_JELLYFIN_PORT', 4620) });
const nocors = await startSubsonic({ port: port('MOCK_NOCORS_PORT', 4630), cors: false });
const qqmusic = await startQqMusic({ port: port('MOCK_QQMUSIC_PORT', 4640) });

for (const [name, server] of [
  ['Subsonic', subsonic],
  ['Jellyfin', jellyfin],
  ['Subsonic without CORS', nocors],
  ['QQ Music and WeChat, without CORS', qqmusic],
]) {
  const address = server.address();
  console.log(`mock ${name}: http://127.0.0.1:${address.port}`);
}
