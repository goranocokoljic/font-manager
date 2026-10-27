import { describe, expect, it } from 'vitest';
import { API_VERSION } from '../src/api';
import {
  FmApiError,
  FmClient,
  NOT_RUNNING,
  NotRunningError,
  endpointFilePath,
} from '../src/client';
import { FAKE_TOKEN, FAKE_URL, type FakeEndpoint, fakeEndpoint } from '../src/testing';

const font = { postscriptName: 'Inter-Regular' };

describe('FmClient', () => {
  it('posts each call to /v1/<method> with the bearer token and the API version', async () => {
    const fake = fakeEndpoint({
      fonts_needed: () => ({ matched: [], unmatched: [font], missing: 1, outcomes: [] }),
      document_closed: () => ({ offer: [3] }),
      deactivate: () => [{ face: 3, outcome: 'done' }],
    });
    const client = new FmClient(fake);

    expect((await client.fontsNeeded({ fonts: [font], document: 'd' })).missing).toBe(1);
    expect((await client.documentClosed({ document: 'd' })).offer).toEqual([3]);
    await client.deactivate({ target: { kind: 'faces', faces: [3] } });

    expect(fake.requests.map((r) => r.url)).toEqual([
      `${FAKE_URL}/v1/fonts_needed`,
      `${FAKE_URL}/v1/document_closed`,
      `${FAKE_URL}/v1/deactivate`,
    ]);
    const { major, minor, patch } = API_VERSION;
    for (const r of fake.requests) {
      expect(r.headers.Authorization).toBe(`Bearer ${FAKE_TOKEN}`);
      expect(r.headers['Fm-Api-Version']).toBe(`${major}.${minor}.${patch}`);
    }
    expect(fake.calls('fonts_needed')).toEqual([{ fonts: [font], document: 'd' }]);
  });

  it('reads endpoint.json on every call, so a new port and token are used', async () => {
    const fake = fakeEndpoint({ document_closed: () => ({ offer: [] }) });
    const client = new FmClient(fake);
    await client.documentClosed({ document: 'd' });
    fake.discovery = JSON.stringify({ url: 'http://127.0.0.1:50000', token: FAKE_TOKEN });
    await client.documentClosed({ document: 'd' });
    expect(fake.requests[1].url).toBe('http://127.0.0.1:50000/v1/document_closed');
  });

  const broken: [string, (f: FakeEndpoint) => void][] = [
    ['there is no endpoint.json', (f) => (f.discovery = null)],
    ['endpoint.json is broken', (f) => (f.discovery = '{')],
    [
      'endpoint.json cannot be read',
      (f) =>
        (f.readEndpointFile = async () => {
          throw new Error('EACCES');
        }),
    ],
    ['nothing listens on the port', (f) => (f.listening = false)],
    ['the token is refused', (f) => (f.discovery = JSON.stringify({ url: FAKE_URL, token: 'x' }))],
  ];
  it.each(broken)('is not running when %s', async (_, breakIt) => {
    const fake = fakeEndpoint({ document_closed: () => ({ offer: [] }) });
    breakIt(fake);
    const err = await new FmClient(fake).documentClosed({ document: 'd' }).catch((e) => e);
    expect(err).toBeInstanceOf(NotRunningError);
    expect(err.message).toBe(NOT_RUNNING);
  });

  it.each([
    'http://localhost:49152',
    'http://192.168.1.10:49152',
    'https://127.0.0.1:49152',
    'http://127.0.0.1.evil.example:80',
    'http://127.0.0.1:49152/elsewhere',
  ])('never sends the token to %s', async (url) => {
    const fake = fakeEndpoint({ document_closed: () => ({ offer: [] }) });
    fake.discovery = JSON.stringify({ url, token: FAKE_TOKEN });
    const err = await new FmClient(fake).documentClosed({ document: 'd' }).catch((e) => e);
    expect(err).toBeInstanceOf(NotRunningError);
    expect(fake.requests).toEqual([]);
  });

  it("turns the app's error answer into an FmApiError with its message", async () => {
    const fake = fakeEndpoint({
      fonts_needed: () => ({ status: 503, body: { code: 'busy', message: 'A scan is running.' } }),
    });
    const err = await new FmClient(fake).fontsNeeded({ fonts: [] }).catch((e) => e);
    expect(err).toBeInstanceOf(FmApiError);
    expect(err.error.code).toBe('busy');
    expect(err.message).toBe('A scan is running.');
  });

  it('reports an answer that is not the local API as internal', async () => {
    const fake = fakeEndpoint();
    fake.fetch = async () => ({ status: 500, text: async () => '<html>' });
    const err = await new FmClient(fake).fontsNeeded({ fonts: [] }).catch((e) => e);
    expect(err).toBeInstanceOf(FmApiError);
    expect(err.error.code).toBe('internal');
  });
});

describe('endpointFilePath', () => {
  it("is endpoint.json in the app's data folder on every OS", () => {
    expect(endpointFilePath('win32', 'C:\\Users\\Ana', { LOCALAPPDATA: 'D:\\Local' })).toBe(
      'D:\\Local\\FontManager\\data\\endpoint.json',
    );
    expect(endpointFilePath('win32', 'C:\\Users\\Ana')).toBe(
      'C:\\Users\\Ana\\AppData\\Local\\FontManager\\data\\endpoint.json',
    );
    expect(endpointFilePath('darwin', '/Users/ana')).toBe(
      '/Users/ana/Library/Application Support/org.fontmanager.app/endpoint.json',
    );
    expect(endpointFilePath('linux', '/home/ana')).toBe(
      '/home/ana/.local/share/font-manager/endpoint.json',
    );
    expect(endpointFilePath('linux', '/home/ana', { XDG_DATA_HOME: '/data' })).toBe(
      '/data/font-manager/endpoint.json',
    );
  });
});
