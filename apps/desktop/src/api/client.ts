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
  CollectionFacesRequest,
  CollectionId,
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
  MoveCollectionRequest,
  Paged,
  RenameCollectionRequest,
  SampleText,
  SampleTextRequest,
  ScanRequest,
  SearchRequest,
  SearchResponse,
  SetSourcePausedRequest,
  Settings,
  SettingsUpdate,
  Source,
  SourceId,
  Tag,
  TagFacesRequest,
  UntagFacesRequest,
  UserMetaUpdate,
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
  /** Every collection, parents before children, siblings in the user's order. */
  listCollections(): Promise<Collection[]>;
  /** Creates a collection; with `rules`, a smart one. */
  createCollection(req: CreateCollectionRequest): Promise<Collection>;
  renameCollection(req: RenameCollectionRequest): Promise<Collection>;
  /** Nests a collection under another one, or reorders it among its siblings. */
  moveCollection(req: MoveCollectionRequest): Promise<Collection>;
  /** Deletes a collection and its children; the fonts are not touched. */
  deleteCollection(collection: CollectionId): Promise<void>;
  /** Adds faces or whole families to a manual collection. */
  addToCollection(req: CollectionFacesRequest): Promise<Collection>;
  removeFromCollection(req: CollectionFacesRequest): Promise<Collection>;
  /** Every tag, A to Z, with its face count. */
  listTags(): Promise<Tag[]>;
  /** Puts tags on faces, creating new ones; resolves to the tags named. */
  tagFaces(req: TagFacesRequest): Promise<Tag[]>;
  /** Takes tags off faces; a tag left on no face is gone. */
  untagFaces(req: UntagFacesRequest): Promise<void>;
  /** Favorite, rating (0 clears it) and notes; fields left out stay. */
  updateUserMeta(req: UserMetaUpdate): Promise<void>;
  /** The URL the webview loads a face's font file from (for `@font-face`). */
  fontUrl(face: FaceId): string;
  /** The text each face previews for a sample: the sample, or a fallback from its coverage. */
  sampleTexts(req: SampleTextRequest): Promise<SampleText[]>;
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
  /** Opens a page of the user docs (`HELP_PAGES` in src/help.ts) in the help window. */
  openHelp(page: string): Promise<void>;
}

/** The client backed by the Tauri commands in `src-tauri`. */
export const tauriClient: FmClient = {
  getAppInfo: () => invoke<AppInfo>('get_app_info'),
  getFace: (face) => invoke<FaceDetail>('get_face', { face }),
  listFamilies: (req) => invoke<Paged<FamilySummary>>('list_families', { req }),
  getFamily: (family) => invoke<Family>('get_family', { family }),
  search: (req) => invoke<SearchResponse>('search', { req }),
  listLanguages: () => invoke<LanguagePreset[]>('list_languages'),
  listCollections: () => invoke<Collection[]>('list_collections'),
  createCollection: (req) => invoke<Collection>('create_collection', { req }),
  renameCollection: (req) => invoke<Collection>('rename_collection', { req }),
  moveCollection: (req) => invoke<Collection>('move_collection', { req }),
  deleteCollection: (collection) => invoke<void>('delete_collection', { collection }),
  addToCollection: (req) => invoke<Collection>('add_to_collection', { req }),
  removeFromCollection: (req) => invoke<Collection>('remove_from_collection', { req }),
  listTags: () => invoke<Tag[]>('list_tags'),
  tagFaces: (req) => invoke<Tag[]>('tag_faces', { req }),
  untagFaces: (req) => invoke<void>('untag_faces', { req }),
  updateUserMeta: (req) => invoke<void>('update_user_meta', { req }),
  fontUrl: (face) => convertFileSrc(String(face), FONT_SCHEME),
  sampleTexts: (req) => invoke<SampleText[]>('sample_texts', { req }),
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
  openHelp: (page) => invoke<void>('open_help', { page }),
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
