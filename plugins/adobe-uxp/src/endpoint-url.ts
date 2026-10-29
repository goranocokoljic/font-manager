// The address the plugin calls. UXP lets a plugin call only the hosts its
// manifest names, and its check drops every name without a top-level
// domain before matching, so `127.0.0.1` can never be allowed; `localhost`
// is the one name it treats as a domain (E30.0 spike, Photoshop 26.4). The
// app's `endpoint.json` always says `http://127.0.0.1:<port>`, and the
// endpoint takes `localhost:<port>` as its Host too (ADR 0015).

const LOOPBACK = 'http://127.0.0.1:';

/** The URL with `127.0.0.1` replaced by `localhost`; any other URL as it is. */
export function uxpUrl(url: string): string {
  return url.startsWith(LOOPBACK) ? `http://localhost:${url.slice(LOOPBACK.length)}` : url;
}
