// The plugins' client of the running app. Over HTTP (ADR 0015) it finds the
// app through `endpoint.json` and calls `POST /v1/<method>` with the bearer
// token; over the pipe or socket (ADR 0016, `ipc.ts`) it needs neither.
// Transport only: the host gives it a file reader and `fetch`, or a socket
// connector, so the same code runs in UXP, CEP and the tests.

import {
  API_VERSION,
  type ActivationOutcome,
  type ApiError,
  type DeactivateRequest,
  type DocumentClosedRequest,
  type DocumentClosedResponse,
  type FontsNeededRequest,
  type FontsNeededResponse,
} from './api';

/** The one message a plugin shows when it cannot reach the app. */
export const NOT_RUNNING = 'Font Manager is not running';

/** An answer of `fetch`, the part the client reads. */
export interface HttpAnswer {
  status: number;
  text(): Promise<string>;
}

/** What the host gives the client. */
export interface ClientHost {
  /** The text of `endpoint.json`, or `null` when there is no such file. */
  readEndpointFile(): Promise<string | null>;
  /** `fetch`, as the host has it. */
  fetch(
    url: string,
    init: { method: 'POST'; headers: Record<string, string>; body: string },
  ): Promise<HttpAnswer>;
}

/** The app is not running, or it refused the token. */
export class NotRunningError extends Error {
  constructor(readonly detail: string) {
    super(NOT_RUNNING);
    this.name = 'NotRunningError';
  }
}

/** The pipe or socket could not be opened: nothing was sent, so another transport may try. */
export class TransportUnavailableError extends NotRunningError {
  constructor(detail: string) {
    super(detail);
    this.name = 'TransportUnavailableError';
  }
}

/** The app answered with an `ApiError`. */
export class FmApiError extends Error {
  constructor(readonly error: ApiError) {
    super(error.message);
    this.name = 'FmApiError';
  }
}

/** Where the app publishes `endpoint.json`: its data folder (ADR 0007). */
export function endpointFilePath(
  platform: string,
  home: string,
  env: Record<string, string | undefined> = {},
): string {
  switch (platform) {
    case 'win32': {
      const local = env.LOCALAPPDATA ?? `${home}\\AppData\\Local`;
      return `${local}\\FontManager\\data\\endpoint.json`;
    }
    case 'darwin':
      return `${home}/Library/Application Support/org.fontmanager.app/endpoint.json`;
    default: {
      const data = env.XDG_DATA_HOME?.startsWith('/') ? env.XDG_DATA_HOME : `${home}/.local/share`;
      return `${data}/font-manager/endpoint.json`;
    }
  }
}

/** The app's address and token from `endpoint.json`. */
interface Discovery {
  url: string;
  token: string;
}

// The endpoint binds 127.0.0.1 only; the token is never sent anywhere else.
const LOOPBACK = /^http:\/\/127\.0\.0\.1:\d{1,5}$/;

function parseDiscovery(text: string): Discovery {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new NotRunningError('endpoint.json is not JSON');
  }
  const { url, token } = (value ?? {}) as Record<string, unknown>;
  if (typeof url !== 'string' || typeof token !== 'string' || token === '') {
    throw new NotRunningError('endpoint.json has no url or token');
  }
  if (!LOOPBACK.test(url)) {
    throw new NotRunningError(`endpoint.json names ${url}, not 127.0.0.1`);
  }
  return { url, token };
}

/** `value` as the app's `ApiError`, or an `internal` one naming `detail`. */
export function toApiError(value: unknown, detail: string): ApiError {
  const e = (value ?? {}) as Partial<ApiError>;
  if (typeof e.code === 'string' && typeof e.message === 'string') return e as ApiError;
  return {
    code: 'internal',
    message: 'Font Manager gave an answer the plugin does not understand.',
    detail,
  };
}

function apiError(text: string, status: number): ApiError {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    // Not the local API's answer: reported as internal.
  }
  return toApiError(value, `HTTP ${status}`);
}

/**
 * One way to the app: calls a service method by its Rust name with its
 * request value. Throws `NotRunningError` when the app cannot be reached and
 * `FmApiError` when it answers with an error.
 */
export interface Transport {
  call<T>(method: string, params: unknown): Promise<T>;
}

/** The local API over HTTP (ADR 0015). */
export function httpTransport(host: ClientHost): Transport {
  return { call: (method, params) => httpCall(host, method, params) };
}

/**
 * The first transport that can be opened: the next one is tried only when
 * one throws `TransportUnavailableError`, so a request is never sent twice.
 */
export function firstAvailable(...transports: Transport[]): Transport {
  return {
    async call<T>(method: string, params: unknown): Promise<T> {
      let unavailable: TransportUnavailableError | undefined;
      for (const t of transports) {
        try {
          return await t.call<T>(method, params);
        } catch (e) {
          if (!(e instanceof TransportUnavailableError)) throw e;
          unavailable = e;
        }
      }
      throw unavailable ?? new NotRunningError('no transport');
    },
  };
}

function isTransport(value: Transport | ClientHost): value is Transport {
  return typeof (value as Partial<Transport>).call === 'function';
}

/** The calls the plugins make. */
export class FmClient {
  private readonly transport: Transport;

  /** A `ClientHost` means the local API over HTTP. */
  constructor(transport: Transport | ClientHost) {
    this.transport = isTransport(transport) ? transport : httpTransport(transport);
  }

  fontsNeeded(req: FontsNeededRequest): Promise<FontsNeededResponse> {
    return this.call('fonts_needed', req);
  }

  documentClosed(req: DocumentClosedRequest): Promise<DocumentClosedResponse> {
    return this.call('document_closed', req);
  }

  deactivate(req: DeactivateRequest): Promise<ActivationOutcome[]> {
    return this.call('deactivate', req);
  }

  private call<T>(method: string, params: unknown): Promise<T> {
    return this.transport.call<T>(method, params);
  }
}

// The file is read on every call: the port changes from run to run and
// Regenerate replaces the token.
async function httpCall<T>(host: ClientHost, method: string, body: unknown): Promise<T> {
  let text: string | null;
  try {
    text = await host.readEndpointFile();
  } catch (e) {
    throw new NotRunningError(`endpoint.json cannot be read: ${String(e)}`);
  }
  if (text === null) throw new NotRunningError('no endpoint.json');
  const { url, token } = parseDiscovery(text);
  const { major, minor, patch } = API_VERSION;
  let answer: HttpAnswer;
  try {
    answer = await host.fetch(`${url}/v1/${method}`, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
        'Fm-Api-Version': `${major}.${minor}.${patch}`,
      },
      body: JSON.stringify(body),
    });
  } catch (e) {
    throw new NotRunningError(`no answer from ${url}: ${String(e)}`);
  }
  if (answer.status === 401) throw new NotRunningError('the token was refused');
  const reply = await answer.text();
  if (answer.status < 200 || answer.status > 299) {
    throw new FmApiError(apiError(reply, answer.status));
  }
  try {
    return JSON.parse(reply) as T;
  } catch {
    throw new FmApiError(apiError('', answer.status));
  }
}
