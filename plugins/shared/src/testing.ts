// A fake of the app's local API for the plugins' unit tests: it plays the
// discovery file and the endpoint, and records every request.

import type { ClientHost, HttpAnswer } from './client';

export { ES5_PLUS, codeOnly } from './es3';

export const FAKE_URL = 'http://127.0.0.1:49152';
export const FAKE_TOKEN = 'a'.repeat(64);

export interface FakeRequest {
  url: string;
  method: string;
  headers: Record<string, string>;
  body: unknown;
}

type Handler = (body: unknown) => { status: number; body: unknown } | unknown;

export interface FakeEndpoint extends ClientHost {
  requests: FakeRequest[];
  /** The calls of one service method, by their request bodies. */
  calls(method: string): unknown[];
  /** `endpoint.json`; `null` removes it. */
  discovery: string | null;
  /** When false, every `fetch` fails as with no listener on the port. */
  listening: boolean;
}

export function fakeEndpoint(handlers: Record<string, Handler> = {}): FakeEndpoint {
  const fake: FakeEndpoint = {
    requests: [],
    discovery: JSON.stringify({ url: FAKE_URL, token: FAKE_TOKEN, pid: 4242 }),
    listening: true,
    calls: (method) => fake.requests.filter((r) => r.method === method).map((r) => r.body),
    readEndpointFile: async () => fake.discovery,
    fetch: async (url, init): Promise<HttpAnswer> => {
      if (!fake.listening) throw new TypeError('Network request failed');
      const method = url.slice(url.lastIndexOf('/') + 1);
      const body: unknown = JSON.parse(init.body);
      fake.requests.push({ url, method, headers: init.headers, body });
      const answer = (status: number, value: unknown): HttpAnswer => ({
        status,
        text: async () => JSON.stringify(value),
      });
      if (init.headers.Authorization !== `Bearer ${FAKE_TOKEN}`) {
        return answer(401, { code: 'permissionDenied', message: 'The token is wrong.' });
      }
      const handler = handlers[method];
      if (!handler) return answer(404, { code: 'notFound', message: `No route ${method}.` });
      const result = handler(body);
      if (result && typeof result === 'object' && 'status' in result && 'body' in result) {
        return answer(result.status as number, result.body);
      }
      return answer(200, result);
    },
  };
  return fake;
}
