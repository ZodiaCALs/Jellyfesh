// Download in Jellyfesh Web (ADR-0025 §5; docs/ui.md, Allowed differences): a song from a source
// that allows downloads saves its original file through the browser, named by the default
// template's file part ("07 Lanterns.flac", jf-web's rules shared with the apps). There is no
// queue, Downloads page or offline library on the web, and nothing is uploaded.

import type { App } from './app';
import type { DownloadInfo, Item } from './core/types';
import { t } from './i18n/index.svelte';
import { headerOf, relayFetch, type RelayConfig } from './plugins/relay';
import { explain } from './state/sources.svelte';

/** Whether Download is offered for `item` of `plugin`: a playable song of an on source that allows it. */
export function canDownload(app: App, plugin: string, item: Item): boolean {
  if (item.kind !== 'Track' || !item.playable) return false;
  const source = app.sources.get(plugin);
  return source !== undefined && source.blocked === null && source.declares('Download');
}

/** The file's bytes, with the headers the plugin gave, through the relay when its service needs one. */
async function fetchFile(info: DownloadInfo, relay: RelayConfig | null): Promise<Blob> {
  const headers = info.headers.map((h) => [h.name, h.value] as [string, string]);
  if (relay) {
    const { answer, response } = await relayFetch(relay, info.url, headers);
    if (answer.status !== 200) throw new Error(t('The server answered {status}.', { status: answer.status }));
    return new Blob([await response.arrayBuffer()], { type: headerOf(answer, 'content-type') ?? '' });
  }
  const response = await fetch(info.url, { headers, mode: 'cors', credentials: 'omit', cache: 'no-store' });
  if (!response.ok) throw new Error(t('The server answered {status}.', { status: response.status }));
  return response.blob();
}

/** Hands a file to the browser to save as `name`. */
function save(blob: Blob, name: string) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.hidden = true;
  document.body.append(link);
  link.click();
  link.remove();
  // The browser has taken the file once the click is handled; the URL can go a little later.
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

/** Download: saves the song's original file through the browser. */
export async function downloadSong(app: App, plugin: string, item: Item, toast: (text: string) => void) {
  const source = app.sources.get(plugin);
  if (!source) return;
  toast(t('Downloading “{title}”…', { title: item.title }));
  try {
    const info = await source.download(item);
    const blob = await fetchFile(info, source.relay);
    save(blob, app.jf.download_name(JSON.stringify(item), source.name, info.container));
  } catch (error) {
    toast(t('Couldn’t download “{title}”: {reason}', { title: item.title, reason: await explain(error, source) }));
  }
}
