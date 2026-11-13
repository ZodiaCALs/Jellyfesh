// Why a server cannot be reached from this page, in words a person can act on (docs/web.md,
// "Connecting to your server"). Browsers hide the reason a cross-origin request failed, so the
// page asks twice: a normal (CORS) request, and an opaque one that needs no permission. When only
// the opaque one gets through, the server is there but does not allow this page.

export type Reachability = 'ok' | 'mixed-content' | 'cors' | 'unreachable' | 'invalid';

function isLoopback(url: URL): boolean {
  const host = url.hostname;
  return host === 'localhost' || host.endsWith('.localhost') || host === '[::1]' || /^127\./.test(host);
}

/** Whether an https page may not reach this http URL (mixed content). */
export function isMixedContent(url: URL, pageProtocol = location.protocol): boolean {
  return pageProtocol === 'https:' && url.protocol === 'http:' && !isLoopback(url);
}

/** Probes `url` (no credentials, nothing sent but the request itself). */
export async function diagnose(url: string, timeoutMs = 8000): Promise<Reachability> {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return 'invalid';
  }
  if (!/^https?:$/.test(parsed.protocol)) return 'invalid';
  if (isMixedContent(parsed)) return 'mixed-content';
  const attempt = async (mode: RequestMode) => {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    try {
      await fetch(parsed, { mode, credentials: 'omit', cache: 'no-store', signal: controller.signal });
      return true;
    } catch {
      return false;
    } finally {
      clearTimeout(timer);
    }
  };
  if (await attempt('cors')) return 'ok';
  if (await attempt('no-cors')) return 'cors';
  return 'unreachable';
}
