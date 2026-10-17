// The typed client over the fm-api Tauri commands. Every call to Tauri goes
// through here; components and stores never call `invoke` themselves.
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { mockClient } from './mock';
import type {
  ActivateRequest,
  ActivationOutcome,
  AddSourceRequest,
  AppInfo,
  DeactivateRequest,
  Event,
  FaceDetail,
  FaceId,
  JobId,
  ScanRequest,
  SetSourcePausedRequest,
  Source,
  SourceId,
} from './types';

/** The Tauri event channel `fm_app_lib::EVENT_CHANNEL` forwards fm-api events on. */
export const EVENT_CHANNEL = 'fm://event';

/** Stops an event subscription. */
export type Unsubscribe = () => void;

/** One method per Tauri command; a rejected promise carries an `ApiError`. */
export interface FmClient {
  getAppInfo(): Promise<AppInfo>;
  getFace(face: FaceId): Promise<FaceDetail>;
  activate(req: ActivateRequest): Promise<ActivationOutcome[]>;
  deactivate(req: DeactivateRequest): Promise<ActivationOutcome[]>;
  listSources(): Promise<Source[]>;
  addSource(req: AddSourceRequest): Promise<Source>;
  removeSource(source: SourceId): Promise<void>;
  setSourcePaused(req: SetSourcePausedRequest): Promise<Source>;
  scan(req: ScanRequest): Promise<JobId>;
  /** Subscribes to the fm-api event stream; the UI subscribes, it does not poll. */
  onEvent(handler: (event: Event) => void): Promise<Unsubscribe>;
  /** The native folder picker; `null` when the user cancels. */
  pickFolder(title: string): Promise<string | null>;
}

/** The client backed by the Tauri commands in `src-tauri`. */
export const tauriClient: FmClient = {
  getAppInfo: () => invoke<AppInfo>('get_app_info'),
  getFace: (face) => invoke<FaceDetail>('get_face', { face }),
  activate: (req) => invoke<ActivationOutcome[]>('activate', { req }),
  deactivate: (req) => invoke<ActivationOutcome[]>('deactivate', { req }),
  listSources: () => invoke<Source[]>('list_sources'),
  addSource: (req) => invoke<Source>('add_source', { req }),
  removeSource: (source) => invoke<void>('remove_source', { source }),
  setSourcePaused: (req) => invoke<Source>('set_source_paused', { req }),
  scan: (req) => invoke<JobId>('scan', { req }),
  onEvent: (handler) => listen<Event>(EVENT_CHANNEL, (e) => handler(e.payload)),
  pickFolder: async (title) => {
    const path = await open({ directory: true, multiple: false, title });
    return typeof path === 'string' ? path : null;
  },
};

let current: FmClient | undefined;

/** The active client: Tauri in the app, the mock in a plain browser (e2e). */
export function useClient(): FmClient {
  current ??= isTauri() ? tauriClient : mockClient;
  return current;
}

/** Replaces the active client (tests). */
export function setClient(client: FmClient | undefined): void {
  current = client;
}
