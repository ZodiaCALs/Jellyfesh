// Download in Jellyfesh Web (ADR-0025 §5; docs/ui.md, Allowed differences): a song from a source
// that allows downloads saves its original file through the browser, named by the default
// template's file part ("07 Lanterns.flac", jf-web's rules shared with the apps). There is no
// queue, Downloads page or offline library on the web, and nothing is uploaded. A server that
// refuses the account (the plugin says so, 401 or 403, or an answer that is not a file, such as
// Subsonic's HTTP 200 with error 50 in JSON) is reported as the core words it: "Your {service}
// server doesn't allow downloads for this account." Nothing that is not a file is ever saved.

import type { App } from './app';
import type { DownloadInfo, Item } from './core/types';
import { t } from './i18n/index.svelte';
import { CallError } from './plugins/host';
import { headerOf, relayFetch, type RelayConfig } from './plugins/relay';
import { explain } from './state/sources.svelte';

/** The server refused the account a download (the core's `DownloadError::NotAllowed`). */
class NotAllowed extends Error {}

/** A content type that is an answer rather than a file: JSON, XML, HTML or other text. */
function isAnswer(type: string): boolean {
  return /^(text\/|application\/([\w.-]+\+)?(json|xml)\b)/i.test(type.trim());
}

/** Whether a body's first bytes are text (`{`, `[` or `<`, as JSON, XML or HTML start) rather than audio. */
async function looksLikeText(blob: Blob): Promise<boolean> {
  const head = new Uint8Array(await blob.slice(0, 64).arrayBuffer());
  const first = head.find((byte) => byte !== 0x20 && byte !== 0x09 && byte !== 0x0a && byte !== 0x0d && byte !== 0xef && byte !== 0xbb && byte !== 0xbf);
  return first === 0x7b || first === 0x5b || first === 0x3c;
}

/** A fetched body, if it is a file: refusals and answers that are not files throw. */
async function fileOf(status: number, type: string, body: () => Promise<Blob>): Promise<Blob> {
  if (status === 401 || status === 403) throw new NotAllowed();
  if (status < 200 || status > 299) throw new Error(t('The server answered {status}.', { status }));
  if (isAnswer(type)) throw new NotAllowed();
  const blob = await body();
  if (blob.size === 0 || (await looksLikeText(blob))) throw new NotAllowed();
  return blob;
}

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
    const type = headerOf(answer, 'content-type') ?? '';
    return fileOf(answer.status, type, async () => new Blob([await response.arrayBuffer()], { type }));
  }
  const response = await fetch(info.url, { headers, mode: 'cors', credentials: 'omit', cache: 'no-store' });
  return fileOf(response.status, response.headers.get('content-type') ?? '', () => response.blob());
}

/** Hands a file to the browser to save as `name` (Download, Export Playlist…). */
export function saveFile(blob: Blob, name: string) {
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
    saveFile(blob, app.jf.download_name(JSON.stringify(item), source.name, info.container));
  } catch (error) {
    // The plugin answers `unsupported` when the server refuses the account (WIT 1.3).
    const refused = error instanceof NotAllowed || (error instanceof CallError && error.plugin === 'Unsupported');
    const reason = refused ? t('Your {service} server doesn’t allow downloads for this account.', { service: source.name }) : await explain(error, source);
    toast(t('Couldn’t download “{title}”: {reason}', { title: item.title, reason }));
  }
}
