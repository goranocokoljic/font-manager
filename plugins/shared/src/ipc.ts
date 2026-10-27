// The app's IPC endpoint (ADR 0016) as a client transport: the named pipe or
// Unix socket only this user can open, newline-delimited JSON, no token. The
// host gives it a connector (Node's `net.createConnection`, in CEP), so the
// same code runs in the plugins and in the tests.

import { API_VERSION } from './api';
import {
  FmApiError,
  NotRunningError,
  TransportUnavailableError,
  toApiError,
  type Transport,
} from './client';

/** The part of a Node `net.Socket` the transport uses. */
export interface IpcSocket {
  setEncoding(encoding: 'utf8'): unknown;
  on(event: 'connect' | 'close', listener: () => void): unknown;
  on(event: 'data', listener: (chunk: string) => void): unknown;
  on(event: 'error', listener: (error: Error) => void): unknown;
  write(data: string): unknown;
  end(): unknown;
  destroy(): unknown;
}

/** What the host gives the IPC transport. */
export interface IpcHost {
  /** The pipe or socket name, from `ipcPath`. */
  path: string;
  /** Node's `net.createConnection`, as the host has it. */
  connect(path: string): IpcSocket;
  /** How long one call may wait for its answer; 30 s when not given. */
  timeoutMs?: number;
}

/** The envelope's major version this client speaks (ADR 0016). */
export const IPC_VERSION = 1;

const PIPE_PREFIX = '\\\\.\\pipe\\org.fontmanager.app-';
const SOCKET_FILE = 'fm.sock';

/** The app's pipe or socket name, as `AppPaths::ipc_name` makes it (ADR 0016). */
export function ipcPath(
  platform: string,
  home: string,
  env: Record<string, string | undefined> = {},
): string {
  switch (platform) {
    case 'win32': {
      const user = (env.USERNAME ?? '').replace(/[^\p{Alphabetic}\p{N}\-_.]/gu, '_');
      return `${PIPE_PREFIX}${user === '' ? 'user' : user}`;
    }
    case 'darwin':
      return `${home}/Library/Application Support/org.fontmanager.app/${SOCKET_FILE}`;
    default: {
      const run = env.XDG_RUNTIME_DIR;
      if (run?.startsWith('/')) return `${run}/font-manager/${SOCKET_FILE}`;
      const data = env.XDG_DATA_HOME?.startsWith('/') ? env.XDG_DATA_HOME : `${home}/.local/share`;
      return `${data}/font-manager/${SOCKET_FILE}`;
    }
  }
}

interface Reply {
  v?: unknown;
  id?: unknown;
  result?: unknown;
  error?: unknown;
}

/** One connection: lines out, answers in order. */
class Connection {
  private buffer = '';
  private readonly waiting: { resolve(r: Reply): void; reject(e: Error): void }[] = [];
  private failure: Error | undefined;

  private constructor(private readonly socket: IpcSocket) {
    socket.setEncoding('utf8');
    socket.on('data', (chunk: string) => this.received(chunk));
    socket.on('error', (e: Error) => this.fail(new NotRunningError(`the pipe failed: ${e}`)));
    socket.on('close', () => this.fail(new NotRunningError('the app closed the pipe')));
  }

  /** Opened, or `TransportUnavailableError` when nothing listens there. */
  static open(host: IpcHost): Promise<Connection> {
    return new Promise((resolve, reject) => {
      let socket: IpcSocket;
      try {
        socket = host.connect(host.path);
      } catch (e) {
        reject(new TransportUnavailableError(`no pipe at ${host.path}: ${String(e)}`));
        return;
      }
      let connected = false;
      socket.on('connect', () => {
        connected = true;
        resolve(new Connection(socket));
      });
      socket.on('error', (e: Error) => {
        if (connected) return;
        socket.destroy();
        reject(new TransportUnavailableError(`no pipe at ${host.path}: ${e}`));
      });
    });
  }

  request(id: number, method: string, params?: unknown): Promise<Reply> {
    if (this.failure) return Promise.reject(this.failure);
    const answer = new Promise<Reply>((resolve, reject) => this.waiting.push({ resolve, reject }));
    const line = { v: IPC_VERSION, id, method, ...(params === undefined ? {} : { params }) };
    this.socket.write(`${JSON.stringify(line)}\n`);
    return answer;
  }

  close(): void {
    this.failure ??= new NotRunningError('closed');
    this.socket.end();
    this.socket.destroy();
  }

  private received(chunk: string): void {
    this.buffer += chunk;
    let end: number;
    while ((end = this.buffer.indexOf('\n')) >= 0) {
      const line = this.buffer.slice(0, end).trim();
      this.buffer = this.buffer.slice(end + 1);
      if (line === '') continue;
      let reply: Reply;
      try {
        reply = JSON.parse(line) as Reply;
      } catch {
        reply = { error: toApiError(null, 'the reply is not JSON') };
      }
      this.waiting.shift()?.resolve(reply);
    }
  }

  private fail(e: Error): void {
    this.failure ??= e;
    for (const w of this.waiting.splice(0)) w.reject(this.failure);
  }
}

function answerOf(reply: Reply, id: number): unknown {
  if (reply.v !== IPC_VERSION || reply.id !== id) {
    const error = reply.error ?? null;
    throw new FmApiError(toApiError(error, `reply v=${String(reply.v)} id=${String(reply.id)}`));
  }
  if ('error' in reply) throw new FmApiError(toApiError(reply.error, 'IPC error reply'));
  return reply.result;
}

function withTimeout<T>(work: Promise<T>, ms: number, onTimeout: () => void): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      onTimeout();
      reject(new NotRunningError(`no answer within ${ms} ms`));
    }, ms);
  });
  return Promise.race([work, timeout]).finally(() => clearTimeout(timer));
}

/**
 * The IPC endpoint as a transport: one connection per call, `hello` first so
 * a server of another API major is refused before the request is sent.
 */
export function ipcTransport(host: IpcHost): Transport {
  return {
    async call<T>(method: string, params: unknown): Promise<T> {
      const conn = await Connection.open(host);
      const work = (async () => {
        const hello = answerOf(await conn.request(0, 'hello'), 0) as { major?: unknown };
        if (hello?.major !== API_VERSION.major) {
          throw new FmApiError({
            code: 'unsupported',
            message: 'This plugin does not fit the version of Font Manager that is running.',
            detail: `app API major ${String(hello?.major)}, plugin ${API_VERSION.major}`,
          });
        }
        return answerOf(await conn.request(1, method, params), 1) as T;
      })();
      try {
        return await withTimeout(work, host.timeoutMs ?? 30_000, () => conn.close());
      } finally {
        conn.close();
      }
    },
  };
}
