// What the relay's end-to-end test runs: the relay itself (tools/jf-relay, built with cargo) and the
// unofficial QQ Music plugin built the way its own integration tests build it (the
// `mock-service` feature: every request goes to the origin in its `mock_origin` setting, under the
// service's host name), packed unsigned with the test copy of its manifest (the allowlist is only
// that origin; the schema gains the setting), as plugins/community/testsupport does. Its id is the
// published plugin's, so Jellyfesh Web treats it as the QQ Music it is.

import { type ChildProcess, execFileSync, spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { swap, zip } from './packages';

const WEB = fileURLToPath(new URL('../..', import.meta.url));
const ROOT = resolve(WEB, '../..');
const TARGET = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : join(ROOT, 'target');

function cargo(args: string[], env: Record<string, string> = {}) {
  execFileSync('cargo', args, { cwd: ROOT, stdio: ['ignore', 'ignore', 'inherit'], env: { ...process.env, CARGO_INCREMENTAL: '0', ...env } });
}

/** The relay binary, built (debug) if needed. */
export function relayBinary(): string {
  cargo(['build', '--quiet', '--package', 'jf-relay']);
  return join(TARGET, 'debug', process.platform === 'win32' ? 'jf-relay.exe' : 'jf-relay');
}

/** The mock-service build of the unofficial QQ Music plugin as a `.jfpkg`, unsigned. */
export function mockQqMusicPackage(scratch: string): Buffer {
  cargo(['build', '--quiet', '--package', 'jf-community-qqmusic', '--target', 'wasm32-unknown-unknown', '--features', 'mock-service'], {
    CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS: '--cfg getrandom_backend="custom"',
  });
  const module = join(TARGET, 'wasm32-unknown-unknown', 'debug', 'jf_community_qqmusic.wasm');
  const component = join(scratch, 'qqmusic-mock.wasm');
  // A core module made with wit-bindgen becomes a component as `jf-plugin build` makes one.
  execFileSync(join(WEB, 'node_modules', '.bin', 'jco'), ['new', module, '-o', component], { stdio: 'inherit' });
  const dir = join(ROOT, 'plugins', 'community', 'qqmusic');
  const manifest = swap(
    readFileSync(join(dir, 'manifest.toml'), 'utf8'),
    /^allow = \[[^\]]*\]$/m,
    'allow = ["${settings.mock_origin}"]',
  );
  const schema = JSON.parse(readFileSync(join(dir, 'settings.schema.json'), 'utf8'));
  schema.properties.mock_origin = { type: 'string', format: 'uri', title: 'Mock service (tests only)' };
  return zip(
    new Map([
      ['manifest.toml', Buffer.from(manifest, 'utf8')],
      ['plugin.wasm', readFileSync(component)],
      ['settings.schema.json', Buffer.from(JSON.stringify(schema, null, 2), 'utf8')],
    ]),
  );
}

/** A running relay. */
export interface Relay {
  address: string;
  token: string;
  /** What it logged (with --verbose: methods, hosts and statuses). */
  log: string[];
  stop(): void;
}

/** Starts the relay on a free loopback port for `origin`, forwarding also to `allow`. */
export async function startRelay(binary: string, origin: string, allow: string[]): Promise<Relay> {
  const token = 'e2e-relay-token-0123456789';
  const log: string[] = [];
  const child: ChildProcess = spawn(binary, ['--listen', '127.0.0.1:0', '--origin', origin, '--verbose', ...allow.flatMap((a) => ['--allow', a])], {
    env: { ...process.env, JF_RELAY_TOKEN: token },
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  const address = await new Promise<string>((resolveAddress, reject) => {
    const timer = setTimeout(() => reject(new Error(`the relay did not start:\n${log.join('\n')}`)), 30_000);
    let buffered = '';
    child.stderr!.on('data', (chunk: Buffer) => {
      buffered += chunk.toString('utf8');
      const lines = buffered.split('\n');
      buffered = lines.pop() ?? '';
      for (const line of lines) {
        log.push(line);
        const found = /listening on (http:\/\/127\.0\.0\.1:\d+)/.exec(line);
        if (found) {
          clearTimeout(timer);
          resolveAddress(found[1]);
        }
      }
    });
    child.on('exit', (code) => {
      clearTimeout(timer);
      reject(new Error(`the relay stopped (${code}):\n${log.join('\n')}`));
    });
  });
  return { address, token, log, stop: () => child.kill() };
}
