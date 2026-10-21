// The typed client over the fm-api Tauri commands. Every call to Tauri goes
// through here; components and stores never call `invoke` themselves.
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { mockClient } from './mock';
import type {
  ActivateRequest,
  ActivationOutcome,
  ActiveFace,
  AddSourceRequest,
  Clash,
  AppFolders,
  AppInfo,
  Collection,
  CollectionFacesRequest,
  CollectionId,
  Conflict,
  CoverageReport,
  CoverageRequest,
  FaceCoverage,
  CreateCollectionRequest,
  CssValues,
  CssValuesRequest,
  DeactivateRequest,
  DuplicateGroup,
  Event,
  FaceDetail,
  FaceId,
  Family,
  FamilyId,
  FamilyNotesUpdate,
  LicenseDeclaration,
  FamilySummary,
  GlyphDetail,
  GlyphPage,
  GlyphRequest,
  GlyphsRequest,
  JobId,
  KeepCopyRequest,
  LanguagePreset,
  DuplicateLayoutRequest,
  Layout,
  LayoutBounds,
  LayoutEntry,
  LayoutRef,
  LayoutVersion,
  ListFamiliesRequest,
  RestoreLayoutRequest,
  RevertLayoutRequest,
  SavedLayout,
  SaveLayoutRequest,
  MoveCollectionRequest,
  Paged,
  PinInstanceRequest,
  PreviewImage,
  PreviewRequest,
  Problem,
  ProjectSet,
  RenameCollectionRequest,
  Resolution,
  ResolutionId,
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
  Target,
  UntagFacesRequest,
  UserMetaUpdate,
  VirtualStyle,
  VirtualStyleId,
  VirtualStylesRequest,
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
  /** One page of a face's glyph map: its characters by Unicode block, or the ones a search finds. */
  listGlyphs(req: GlyphsRequest): Promise<GlyphPage>;
  /** One character in detail: glyph id and name, advance width, the GSUB features that produce or replace it. */
  getGlyph(req: GlyphRequest): Promise<GlyphDetail>;
  listFamilies(req: ListFamiliesRequest): Promise<Paged<FamilySummary>>;
  getFamily(family: FamilyId): Promise<Family>;
  /** Free text and filters, grouped by family, with the count of every filter value. */
  search(req: SearchRequest): Promise<SearchResponse>;
  /** How well one face covers some languages or text: what it lacks. */
  checkCoverage(req: CoverageRequest): Promise<CoverageReport>;
  /** One face's coverage per Unicode block and per language preset, complete languages first. */
  faceCoverage(face: FaceId): Promise<FaceCoverage>;
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
  /** A family's own notes; an empty string clears them. */
  updateFamilyNotes(req: FamilyNotesUpdate): Promise<void>;
  /** A family's declared license: type, seats, purchase date, receipt (E15.2). */
  declareLicense(req: LicenseDeclaration): Promise<void>;
  /** The URL the webview loads a face's font file from (for `@font-face`). */
  fontUrl(face: FaceId): string;
  /** A thumbnail: text rasterized in one face, cached by face hash and settings (PNG). */
  renderPreview(req: PreviewRequest): Promise<PreviewImage>;
  /** The text each face previews for a sample: the sample, or a fallback from its coverage. */
  sampleTexts(req: SampleTextRequest): Promise<SampleText[]>;
  /** The CSS declarations for a face's moved axes and switched features ("Copy CSS"). */
  cssValues(req: CssValuesRequest): Promise<CssValues>;
  /** The six body-text layout presets (Book, Columns, Magazine, Quote, Characters, Waterfall). */
  layoutPresets(): Promise<Layout[]>;
  /** A layout with every block value clamped into its range. */
  clampLayout(req: Layout): Promise<Layout>;
  /** The ranges `clampLayout` clamps block values into, with their steps (the properties editor). */
  layoutBounds(): Promise<LayoutBounds>;
  /** Every layout template file (presets first); a broken file is listed with the reason. */
  listLayouts(): Promise<LayoutEntry[]>;
  /** One saved layout. */
  getLayout(req: LayoutRef): Promise<SavedLayout>;
  /** Saves a layout to its template file; the content it replaces becomes a version. */
  saveLayout(req: SaveLayoutRequest): Promise<SavedLayout>;
  /** Copies a layout into a new, independent template file with a new name. */
  duplicateLayout(req: DuplicateLayoutRequest): Promise<SavedLayout>;
  /** The saved file again, or (`toOriginal`) a preset's shipped original. */
  revertLayout(req: RevertLayoutRequest): Promise<SavedLayout>;
  /** The kept versions of a layout, newest first. */
  layoutHistory(req: LayoutRef): Promise<LayoutVersion[]>;
  /** Makes a kept version current; what it replaces is kept as a version. */
  restoreLayout(req: RestoreLayoutRequest): Promise<SavedLayout>;
  /** The width in CSS px the layout's text is set at; null when unlocked (follows the window). */
  layoutWidth(req: Layout): Promise<number | null>;
  activate(req: ActivateRequest): Promise<ActivationOutcome[]>;
  deactivate(req: DeactivateRequest): Promise<ActivationOutcome[]>;
  /** Every face the app has active. */
  listActive(): Promise<ActiveFace[]>;
  /** The clashes activating `target` would cause; activates nothing. */
  conflicts(target: Target): Promise<Conflict[]>;
  /** Every clash of an active face, with its winner (the Conflicts view). */
  listConflicts(): Promise<Clash[]>;
  /** Duplicates, broken and legacy files, sidecar and activation conflicts (the Problems view). */
  listProblems(): Promise<Problem[]>;
  /** Duplicate groups, each with its recommended keeper first. */
  listDuplicates(): Promise<DuplicateGroup[]>;
  /** "Keep this one": moves the other copies to the OS trash; `dryRun` only plans. */
  keepCopy(req: KeepCopyRequest): Promise<Resolution>;
  /** The "Keep this one" actions, newest first (the Problems history). */
  listResolutions(): Promise<Resolution[]>;
  /** Puts the copies of an action back from the trash. */
  undoResolution(resolution: ResolutionId): Promise<Resolution>;
  /** Every project set: named groups of faces activated together. */
  listSets(): Promise<ProjectSet[]>;
  /** Pins an instance of a variable face as a virtual style of a manual collection or a set. */
  pinInstance(req: PinInstanceRequest): Promise<VirtualStyle>;
  /** The virtual styles of one manual collection or one set, A to Z. */
  listVirtualStyles(req: VirtualStylesRequest): Promise<VirtualStyle[]>;
  /** Deletes a virtual style; its face stays. */
  deleteVirtualStyle(style: VirtualStyleId): Promise<void>;
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
  /** The native file picker; `null` when the user cancels. */
  pickFile(title: string): Promise<string | null>;
  /** Opens a page of the user docs (`HELP_PAGES` in src/help.ts) in the help window. */
  openHelp(page: string): Promise<void>;
}

/** The client backed by the Tauri commands in `src-tauri`. */
export const tauriClient: FmClient = {
  getAppInfo: () => invoke<AppInfo>('get_app_info'),
  getFace: (face) => invoke<FaceDetail>('get_face', { face }),
  listGlyphs: (req) => invoke<GlyphPage>('list_glyphs', { req }),
  getGlyph: (req) => invoke<GlyphDetail>('get_glyph', { req }),
  listFamilies: (req) => invoke<Paged<FamilySummary>>('list_families', { req }),
  getFamily: (family) => invoke<Family>('get_family', { family }),
  search: (req) => invoke<SearchResponse>('search', { req }),
  checkCoverage: (req) => invoke<CoverageReport>('check_coverage', { req }),
  faceCoverage: (face) => invoke<FaceCoverage>('face_coverage', { face }),
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
  updateFamilyNotes: (req) => invoke<void>('update_family_notes', { req }),
  declareLicense: (req) => invoke<void>('declare_license', { req }),
  fontUrl: (face) => convertFileSrc(String(face), FONT_SCHEME),
  renderPreview: (req) => invoke<PreviewImage>('render_preview', { req }),
  sampleTexts: (req) => invoke<SampleText[]>('sample_texts', { req }),
  cssValues: (req) => invoke<CssValues>('css_values', { req }),
  layoutPresets: () => invoke<Layout[]>('layout_presets'),
  clampLayout: (req) => invoke<Layout>('clamp_layout', { req }),
  layoutBounds: () => invoke<LayoutBounds>('layout_bounds'),
  listLayouts: () => invoke<LayoutEntry[]>('list_layouts'),
  getLayout: (req) => invoke<SavedLayout>('get_layout', { req }),
  saveLayout: (req) => invoke<SavedLayout>('save_layout', { req }),
  duplicateLayout: (req) => invoke<SavedLayout>('duplicate_layout', { req }),
  revertLayout: (req) => invoke<SavedLayout>('revert_layout', { req }),
  layoutHistory: (req) => invoke<LayoutVersion[]>('layout_history', { req }),
  restoreLayout: (req) => invoke<SavedLayout>('restore_layout', { req }),
  layoutWidth: (req) => invoke<number | null>('layout_width', { req }),
  activate: (req) => invoke<ActivationOutcome[]>('activate', { req }),
  deactivate: (req) => invoke<ActivationOutcome[]>('deactivate', { req }),
  listActive: () => invoke<ActiveFace[]>('list_active'),
  conflicts: (target) => invoke<Conflict[]>('conflicts', { target }),
  listConflicts: () => invoke<Clash[]>('list_conflicts'),
  listProblems: () => invoke<Problem[]>('list_problems'),
  listDuplicates: () => invoke<DuplicateGroup[]>('list_duplicates'),
  keepCopy: (req) => invoke<Resolution>('keep_copy', { req }),
  listResolutions: () => invoke<Resolution[]>('list_resolutions'),
  undoResolution: (resolution) => invoke<Resolution>('undo_resolution', { resolution }),
  listSets: () => invoke<ProjectSet[]>('list_sets'),
  pinInstance: (req) => invoke<VirtualStyle>('pin_instance', { req }),
  listVirtualStyles: (req) => invoke<VirtualStyle[]>('list_virtual_styles', { req }),
  deleteVirtualStyle: (style) => invoke<void>('delete_virtual_style', { style }),
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
  pickFile: async (title) => {
    const path = await open({ directory: false, multiple: false, title });
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
