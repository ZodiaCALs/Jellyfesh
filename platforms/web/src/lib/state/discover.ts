// A source's Discover page (ADR-0022 §2), as the core makes it: the plugin's own shelves, or, for
// a source without them (or when they fail), a page made from its suggestions ("For You") and the
// folders of its browse root. jf-web's discover rules (core/jf-web/src/discover.rs, after the
// core's) decide what to ask for and put the answers together; this makes the calls.

import type { DiscoverPage, Item, ItemPage } from '../core/types';
import type { Jf } from '../core/wasm';
import type { Plugin } from '../plugins/host';

type Answer = { ok: Item[] } | { err: string };

interface Limits {
  call_ms: number;
  shelf_items: number;
  root_items: number;
}

/** The page for `plugin`; `explain` words a failed call for people. */
export async function discoverPage(jf: Jf, plugin: Plugin, explain: (error: unknown) => Promise<string>): Promise<DiscoverPage> {
  const limits = JSON.parse(jf.discover_limits()) as Limits;
  if (plugin.declares('Discover')) {
    try {
      return JSON.parse(jf.discover_own_page(JSON.stringify(await plugin.discover())));
    } catch {
      // As the core: a page is made from what else the source has.
    }
  }
  const answer = async (call: () => Promise<ItemPage>): Promise<Answer> => {
    try {
      return { ok: (await call()).items };
    } catch (error) {
      return { err: await explain(error) };
    }
  };
  const forYou = plugin.declares('Recommendations') ? answer(() => plugin.forUser(limits.shelf_items)) : Promise.resolve(null);
  let root: Answer | null = null;
  let folders: [Item, Answer][] = [];
  if (plugin.declares('Browse')) {
    root = await answer(() => plugin.browse(null, null, limits.root_items));
    if ('ok' in root) {
      const wanted = JSON.parse(jf.discover_root_folders(JSON.stringify(root.ok))) as Item[];
      folders = await Promise.all(
        wanted.map(async (folder): Promise<[Item, Answer]> => [folder, await answer(() => plugin.browse(folder, null, limits.shelf_items))]),
      );
    }
  }
  return JSON.parse(jf.discover_made_page(plugin.name, JSON.stringify({ for_you: await forYou, root, folders })));
}
