import { createConnection, createServer, type Server, type Socket } from 'node:net';
import { tmpdir } from 'node:os';
import { afterEach, describe, expect, it } from 'vitest';
import { API_VERSION } from '../src/api';
import {
  FmApiError,
  FmClient,
  NotRunningError,
  TransportUnavailableError,
  firstAvailable,
  httpTransport,
} from '../src/client';
import { IPC_VERSION, ipcPath, ipcTransport } from '../src/ipc';
import { DocumentWatcher } from '../src/session';
import { fakeEndpoint } from '../src/testing';

// A fake of the app's IPC endpoint (ADR 0016) on a real pipe (Windows) or
// Unix socket (macOS, Linux): it answers `hello` and the handlers, line by
// line, and records every request.
interface Line {
  v: number;
  id: unknown;
  method: string;
  params?: unknown;
}
type Handler = (params: unknown) => unknown;
interface FakeIpc {
  path: string;
  lines: Line[];
  close(): Promise<void>;
}
/** Takes the line over when it returns true: the fake then sends nothing itself. */
type OnLine = (line: Line, socket: Socket) => boolean;

let serial = 0;
const servers: FakeIpc[] = [];

function freePath(): string {
  serial += 1;
  const name = `fm-test-${process.pid}-${serial}`;
  return process.platform === 'win32' ? `\\\\.\\pipe\\${name}` : `${tmpdir()}/${name}.sock`;
}

async function fakeIpc(
  handlers: Record<string, Handler>,
  options: { major?: number; onLine?: OnLine } = {},
): Promise<FakeIpc> {
  const path = freePath();
  const lines: Line[] = [];
  const sockets: Socket[] = [];
  const server: Server = createServer((socket) => {
    sockets.push(socket);
    socket.setEncoding('utf8');
    socket.on('error', () => {});
    let buffer = '';
    socket.on('data', (chunk) => {
      buffer += chunk;
      let end: number;
      while ((end = buffer.indexOf('\n')) >= 0) {
        const line = JSON.parse(buffer.slice(0, end)) as Line;
        buffer = buffer.slice(end + 1);
        lines.push(line);
        if (options.onLine?.(line, socket)) continue;
        const reply = (body: object) =>
          socket.write(`${JSON.stringify({ v: IPC_VERSION, id: line.id, ...body })}\n`);
        if (line.method === 'hello') {
          reply({ result: { ...API_VERSION, major: options.major ?? API_VERSION.major } });
          continue;
        }
        const handler = handlers[line.method];
        if (!handler) {
          reply({ error: { code: 'unsupported', message: `No method ${line.method}.` } });
          continue;
        }
        try {
          reply({ result: handler(line.params) });
        } catch (e) {
          reply({ error: e });
        }
      }
    });
  });
  await new Promise<void>((resolve) => server.listen(path, resolve));
  const fake: FakeIpc = {
    path,
    lines,
    close: () =>
      new Promise((resolve) => {
        for (const s of sockets) s.destroy();
        server.close(() => resolve());
      }),
  };
  servers.push(fake);
  return fake;
}

// Answers `hello`, then drops the connection on the request.
const dropAfterHello: OnLine = (line, socket) => {
  if (line.method === 'hello') return false;
  socket.destroy();
  return true;
};

afterEach(async () => {
  await Promise.all(servers.splice(0).map((s) => s.close()));
});

const connect = (path: string) => createConnection(path);

/** The error a call failed with. */
async function rejection(call: Promise<unknown>): Promise<FmApiError & NotRunningError> {
  try {
    await call;
  } catch (e) {
    return e as FmApiError & NotRunningError;
  }
  throw new Error('the call did not fail');
}
const font = { postscriptName: 'Inter-Regular' };

describe('ipcTransport', () => {
  it('says hello, then sends the request in the ADR 0016 envelope', async () => {
    const fake = await fakeIpc({
      fonts_needed: () => ({ matched: [], unmatched: [font], missing: 1, outcomes: [] }),
    });
    const client = new FmClient(ipcTransport({ path: fake.path, connect }));
    const answer = await client.fontsNeeded({ fonts: [font], document: 'd' });
    expect(answer.missing).toBe(1);
    expect(fake.lines).toEqual([
      { v: 1, id: 0, method: 'hello' },
      { v: 1, id: 1, method: 'fonts_needed', params: { fonts: [font], document: 'd' } },
    ]);
  });

  it('serves every call on a connection of its own', async () => {
    const fake = await fakeIpc({ document_closed: () => ({ offer: [7] }), deactivate: () => [] });
    const client = new FmClient(ipcTransport({ path: fake.path, connect }));
    expect((await client.documentClosed({ document: 'd' })).offer).toEqual([7]);
    await client.deactivate({ target: { kind: 'faces', faces: [7] } });
    expect(fake.lines.map((l) => l.method)).toEqual([
      'hello',
      'document_closed',
      'hello',
      'deactivate',
    ]);
  });

  it("turns an error reply into an FmApiError with the app's message", async () => {
    const fake = await fakeIpc({
      fonts_needed: () => {
        throw { code: 'busy', message: 'A scan is running.', detail: null };
      },
    });
    const err = await rejection(
      ipcTransport({ path: fake.path, connect }).call('fonts_needed', { fonts: [] }),
    );
    expect(err).toBeInstanceOf(FmApiError);
    expect(err.error.code).toBe('busy');
    expect(err.message).toBe('A scan is running.');
  });

  it('refuses an app of another API major before it sends the request', async () => {
    const fake = await fakeIpc({ deactivate: () => [] }, { major: API_VERSION.major + 1 });
    const err = await rejection(
      ipcTransport({ path: fake.path, connect }).call('deactivate', {
        target: { kind: 'faces', faces: [1] },
      }),
    );
    expect(err).toBeInstanceOf(FmApiError);
    expect(err.error.code).toBe('unsupported');
    expect(fake.lines.map((l) => l.method)).toEqual(['hello']);
  });

  it('reads replies split over chunks and skips blank lines', async () => {
    const fake = await fakeIpc(
      {},
      {
        onLine: (line, socket) => {
          const result = line.method === 'hello' ? API_VERSION : { offer: [1, 2] };
          const text = `\n${JSON.stringify({ v: 1, id: line.id, result })}\n`;
          socket.write(text.slice(0, 5));
          setTimeout(() => socket.write(text.slice(5)), 5);
          return true;
        },
      },
    );
    const answer = await ipcTransport({ path: fake.path, connect }).call('document_closed', {
      document: 'd',
    });
    expect(answer).toEqual({ offer: [1, 2] });
  });

  it('refuses a reply in another envelope version', async () => {
    const fake = await fakeIpc(
      {},
      {
        onLine: (line, socket) => {
          socket.write(`${JSON.stringify({ v: 2, id: line.id, result: API_VERSION })}\n`);
          return true;
        },
      },
    );
    const err = await rejection(
      ipcTransport({ path: fake.path, connect }).call('document_closed', { document: 'd' }),
    );
    expect(err).toBeInstanceOf(FmApiError);
    expect(err.error.code).toBe('internal');
    expect(fake.lines.map((l) => l.method)).toEqual(['hello']);
  });

  it('is unavailable, and sends nothing, when nothing listens on the pipe', async () => {
    const err = await rejection(
      ipcTransport({ path: freePath(), connect }).call('document_closed', { document: 'd' }),
    );
    expect(err).toBeInstanceOf(TransportUnavailableError);
    expect(err).toBeInstanceOf(NotRunningError);
  });

  it('is not running when the app closes the pipe before it answers', async () => {
    const fake = await fakeIpc({}, { onLine: dropAfterHello });
    const err = await rejection(
      ipcTransport({ path: fake.path, connect }).call('document_closed', { document: 'd' }),
    );
    expect(err).toBeInstanceOf(NotRunningError);
    expect(err).not.toBeInstanceOf(TransportUnavailableError);
  });

  it('is not running when the app does not answer in time', async () => {
    const fake = await fakeIpc({}, { onLine: () => true });
    const err = await rejection(
      ipcTransport({ path: fake.path, connect, timeoutMs: 50 }).call('document_closed', {
        document: 'd',
      }),
    );
    expect(err).toBeInstanceOf(NotRunningError);
    expect(err.detail).toMatch(/50 ms/);
  });
});

describe('firstAvailable', () => {
  const http = () => fakeEndpoint({ document_closed: () => ({ offer: [9] }) });

  it('uses the pipe when the app listens on it, and not HTTP', async () => {
    const fake = await fakeIpc({ document_closed: () => ({ offer: [3] }) });
    const endpoint = http();
    const client = new FmClient(
      firstAvailable(ipcTransport({ path: fake.path, connect }), httpTransport(endpoint)),
    );
    expect((await client.documentClosed({ document: 'd' })).offer).toEqual([3]);
    expect(endpoint.requests).toEqual([]);
  });

  it('uses HTTP when the pipe cannot be opened', async () => {
    const endpoint = http();
    const client = new FmClient(
      firstAvailable(ipcTransport({ path: freePath(), connect }), httpTransport(endpoint)),
    );
    expect((await client.documentClosed({ document: 'd' })).offer).toEqual([9]);
    expect(endpoint.calls('document_closed')).toEqual([{ document: 'd' }]);
  });

  it('never sends a request again over HTTP once the pipe took it', async () => {
    const fake = await fakeIpc({}, { onLine: dropAfterHello });
    const endpoint = http();
    const err = await new FmClient(
      firstAvailable(ipcTransport({ path: fake.path, connect }), httpTransport(endpoint)),
    )
      .deactivate({ target: { kind: 'faces', faces: [1] } })
      .catch((e) => e);
    expect(err).toBeInstanceOf(NotRunningError);
    expect(endpoint.requests).toEqual([]);
  });

  it('is not running when no transport can reach the app', async () => {
    const endpoint = http();
    endpoint.listening = false;
    const err = await new FmClient(
      firstAvailable(ipcTransport({ path: freePath(), connect }), httpTransport(endpoint)),
    )
      .documentClosed({ document: 'd' })
      .catch((e) => e);
    expect(err).toBeInstanceOf(NotRunningError);
  });

  it('drives a DocumentWatcher over the pipe', async () => {
    const fake = await fakeIpc({
      document_closed: () => ({ offer: [4, 5] }),
      deactivate: () => [],
    });
    const asked: string[] = [];
    const watcher = new DocumentWatcher(new FmClient(ipcTransport({ path: fake.path, connect })), {
      notify: (m) => asked.push(`notify ${m}`),
      confirm: async (m) => (asked.push(m), true),
    });
    await watcher.closed({ key: 'k', name: 'Poster.ai' });
    expect(asked).toEqual(['Deactivate 2 fonts used only by Poster.ai?']);
    expect(fake.lines.filter((l) => l.method === 'deactivate')[0].params).toEqual({
      target: { kind: 'faces', faces: [4, 5] },
    });
  });
});

describe('ipcPath', () => {
  it("is the app's pipe or socket name on every OS (ADR 0016)", () => {
    expect(ipcPath('win32', 'C:\\Users\\Ana', { USERNAME: 'ana.b' })).toBe(
      '\\\\.\\pipe\\org.fontmanager.app-ana.b',
    );
    expect(ipcPath('win32', 'C:\\Users\\Ana', { USERNAME: 'Ana \u0160mit$' })).toBe(
      '\\\\.\\pipe\\org.fontmanager.app-Ana_\u0160mit_',
    );
    expect(ipcPath('win32', 'C:\\Users\\Ana')).toBe('\\\\.\\pipe\\org.fontmanager.app-user');
    expect(ipcPath('darwin', '/Users/ana', { XDG_RUNTIME_DIR: '/run/user/501' })).toBe(
      '/Users/ana/Library/Application Support/org.fontmanager.app/fm.sock',
    );
    expect(ipcPath('linux', '/home/ana', { XDG_RUNTIME_DIR: '/run/user/1000' })).toBe(
      '/run/user/1000/font-manager/fm.sock',
    );
    expect(ipcPath('linux', '/home/ana', { XDG_RUNTIME_DIR: 'relative' })).toBe(
      '/home/ana/.local/share/font-manager/fm.sock',
    );
    expect(ipcPath('linux', '/home/ana', { XDG_DATA_HOME: '/data' })).toBe(
      '/data/font-manager/fm.sock',
    );
  });
});
