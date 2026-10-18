// The typed client over the fm-api Tauri commands. Every call to Tauri goes
// through here; components and stores never call `invoke` themselves.
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { mockClient } from './mock';
import type {
  ActivateRequest,
  ActivationOutcome,
  AddSourceRequest,
  AppFolders,
  AppInfo,
  Collection,
  CreateCollectionRequest,
  DeactivateRequest,
  Event,
  FaceDetail,
  FaceId,
  Family,
  FamilyId,
  FamilySummary,
  JobId,
  LanguagePreset,
  ListFamiliesRequest,
  Paged,
  ScanRequest,
  SearchRequest,
  SearchResponse,
  SetSourcePausedRequest,
  Settings,
  SettingsUpdate,
  Source,
  SourceId,
} from './types';

/** Settings as a response carries them: every field present. */
export type FullSettings = Required<Settings>;

/** The Tauri event channel `fm_app_lib::EVENT_CHANNEL` forwards fm-api events on. */
export const EVENT_CHANNEL = 'fm://event';

/** The URI scheme `fm_app_lib::FONT_SCHEME` serves font files on, by face id. */
export const FONT_SCHEME = 'fmfont';

/** Stops an event subscription. */
export type Unsubscribe = () => void;

/** One method per Tauri command; a rejected promise carries an `ApiError`. */
export interface FmClient {
  getAppInfo(): Promise<AppInfo>;
  getFace(face: FaceId): Promise<FaceDetail>;
  listFamilies(req: ListFamiliesRequest): Promise<Paged<FamilySummary>>;
  getFamily(family: FamilyId): Promise<Family>;
  /** Free text and filters, grouped by family, with the count of every filter value. */
  search(req: SearchRequest): Promise<SearchResponse>;
  /** The language presets a `supports` filter can name. */
  listLanguages(): Promise<LanguagePreset[]>;
  /** Creates a collection; with `rules`, a smart one. */
  createCollection(req: CreateCollectionRequest): Promise<Collection>;
  /** The URL the webview loads a face's font file from (for `@font-face`). */
  fontUrl(face: FaceId): string;
  activate(req: ActivateRequest): Promise<ActivationOutcome[]>;
  deactivate(req: DeactivateRequest): Promise<ActivationOutcome[]>;
  listSources(): Promise<Source[]>;
  addSource(req: AddSourceRequest): Promise<Source>;
  removeSource(source: SourceId): Promise<void>;
  setSourcePaused(req: SetSourcePausedRequest): Promise<Source>;
  getSettings(): Promise<FullSettings>;
  updateSettings(req: SettingsUpdate): Promise<FullSettings>;
  getAppFolders(): Promise<AppFolders>;
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
  listFamilies: (req) => invoke<Paged<FamilySummary>>('list_families', { req }),
  getFamily: (family) => invoke<Family>('get_family', { family }),
  search: (req) => invoke<SearchResponse>('search', { req }),
  listLanguages: () => invoke<LanguagePreset[]>('list_languages'),
  createCollection: (req) => invoke<Collection>('create_collection', { req }),
  fontUrl: (face) => convertFileSrc(String(face), FONT_SCHEME),
  activate: (req) => invoke<ActivationOutcome[]>('activate', { req }),
  deactivate: (req) => invoke<ActivationOutcome[]>('deactivate', { req }),
  listSources: () => invoke<Source[]>('list_sources'),
  addSource: (req) => invoke<Source>('add_source', { req }),
  removeSource: (source) => invoke<void>('remove_source', { source }),
  setSourcePaused: (req) => invoke<Source>('set_source_paused', { req }),
  getSettings: () => invoke<FullSettings>('get_settings'),
  updateSettings: (req) => invoke<FullSettings>('update_settings', { req }),
  getAppFolders: () => invoke<AppFolders>('get_app_folders'),
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
