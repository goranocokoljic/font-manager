// The typed client over the fm-api Tauri commands. Every call to Tauri goes
// through here; components and stores never call `invoke` themselves.
import { invoke, isTauri } from '@tauri-apps/api/core';
import { mockClient } from './mock';
import type {
  ActivateRequest,
  ActivationOutcome,
  AppInfo,
  DeactivateRequest,
  FaceDetail,
  FaceId,
} from './types';

/** One method per Tauri command; a rejected promise carries an `ApiError`. */
export interface FmClient {
  getAppInfo(): Promise<AppInfo>;
  getFace(face: FaceId): Promise<FaceDetail>;
  activate(req: ActivateRequest): Promise<ActivationOutcome[]>;
  deactivate(req: DeactivateRequest): Promise<ActivationOutcome[]>;
}

/** The client backed by the Tauri commands in `src-tauri`. */
export const tauriClient: FmClient = {
  getAppInfo: () => invoke<AppInfo>('get_app_info'),
  getFace: (face) => invoke<FaceDetail>('get_face', { face }),
  activate: (req) => invoke<ActivationOutcome[]>('activate', { req }),
  deactivate: (req) => invoke<ActivationOutcome[]>('deactivate', { req }),
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
