// The page's side of the install worker (./install.worker.ts): started for an install and stopped
// when it is done, so the 4 MB transpiler stays out of memory otherwise.

import type { PackageInfo } from '../core/types';
import type { InstallReply, InstallRequest } from './install.worker';

export interface Prepared {
  info: PackageInfo;
  module: Uint8Array;
  cores: [string, Uint8Array][];
}

/** Reads and checks a package and transpiles its component for this browser. */
export function preparePackage(bytes: Uint8Array): Promise<Prepared> {
  const worker = new Worker(new URL('./install.worker.ts', import.meta.url), { type: 'module', name: 'plugin install' });
  return new Promise<Prepared>((resolve, reject) => {
    worker.onmessage = (event: MessageEvent<InstallReply>) => {
      const reply = event.data;
      if (reply.ok) resolve({ info: reply.info, module: reply.module, cores: reply.cores });
      else reject(new Error(reply.message));
    };
    worker.onerror = (event) => reject(new Error(event.message || 'The plugin could not be prepared for this browser.'));
    const request: InstallRequest = { id: 1, bytes };
    worker.postMessage(request);
  }).finally(() => worker.terminate());
}
