// The typed client over the fm-api Tauri commands. Every call to Tauri goes
// through here; components and stores never call `invoke` themselves.
import { invoke, isTauri } from '@tauri-apps/api/core';
import { mockClient } from './mock';
import type {
  ActivateRequest,
  ActivationOutcome,
  AddSourceRequest,
  AppFolders,
  AppInfo,
  DeactivateRequest,
  FaceDetail,
  FaceId,
  SetSourcePausedRequest,
  Settings,
  SettingsUpdate,
  Source,
  SourceId,
} from './types';

/** Settings as a response carries them: every field present. */
export type FullSettings = Required<Settings>;

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
  getSettings(): Promise<FullSettings>;
  updateSettings(req: SettingsUpdate): Promise<FullSettings>;
  getAppFolders(): Promise<AppFolders>;
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
  getSettings: () => invoke<FullSettings>('get_settings'),
  updateSettings: (req) => invoke<FullSettings>('update_settings', { req }),
  getAppFolders: () => invoke<AppFolders>('get_app_folders'),
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
