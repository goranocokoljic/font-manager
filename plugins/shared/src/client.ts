// The plugins' client of the app's local API (ADR 0015): it finds the
// running app through `endpoint.json` and calls `POST /v1/<method>` with the
// bearer token. Transport only: the host gives it a file reader and `fetch`,
// so the same code runs in UXP and in the tests.

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

function apiError(text: string, status: number): ApiError {
  try {
    const value = JSON.parse(text) as Partial<ApiError>;
    if (typeof value.code === 'string' && typeof value.message === 'string') {
      return value as ApiError;
    }
  } catch {
    // Not the local API's answer: reported below.
  }
  return {
    code: 'internal',
    message: 'Font Manager gave an answer the plugin does not understand.',
    detail: `HTTP ${status}`,
  };
}

/** The calls the plugins make. */
export class FmClient {
  constructor(private readonly host: ClientHost) {}

  fontsNeeded(req: FontsNeededRequest): Promise<FontsNeededResponse> {
    return this.call('fonts_needed', req);
  }

  documentClosed(req: DocumentClosedRequest): Promise<DocumentClosedResponse> {
    return this.call('document_closed', req);
  }

  deactivate(req: DeactivateRequest): Promise<ActivationOutcome[]> {
    return this.call('deactivate', req);
  }

  // The file is read on every call: the port changes from run to run and
  // Regenerate replaces the token.
  private async call<T>(method: string, body: unknown): Promise<T> {
    let text: string | null;
    try {
      text = await this.host.readEndpointFile();
    } catch (e) {
      throw new NotRunningError(`endpoint.json cannot be read: ${String(e)}`);
    }
    if (text === null) throw new NotRunningError('no endpoint.json');
    const { url, token } = parseDiscovery(text);
    const { major, minor, patch } = API_VERSION;
    let answer: HttpAnswer;
    try {
      answer = await this.host.fetch(`${url}/v1/${method}`, {
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
}
