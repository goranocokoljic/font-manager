// The typed client over the fm-api Tauri commands. Every call to Tauri goes
// through here; components and stores never call `invoke` themselves.
import { convertFileSrc, invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { catalogFontPath } from './catalogFont';
import { mockClient } from './mock';
import type {
  Catalog,
  CatalogDownload,
  CatalogFamily,
  CatalogId,
  CatalogRef,
  CatalogUpdate,
  CheckUpdatesRequest,
  DownloadFamilyRequest,
  DownloadFamilyResponse,
  ListCatalogFamiliesRequest,
  SetCatalogEnabledRequest,
  ActivateRequest,
  ActivationOutcome,
  ActiveFace,
  AddSourceRequest,
  Clash,
  AppFolders,
  EndpointInfo,
  McpConfirmationAnswer,
  McpGrantUpdate,
  McpStatus,
  McpUpdate,
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
  Comparison,
  ComparisonEntry,
  ComparisonRef,
  DeactivateRequest,
  DuplicateGroup,
  Event,
  FaceDetail,
  FaceId,
  Family,
  FamilyId,
  FamilyNotesUpdate,
  LicenseDeclaration,
  LicenseInventoryRequest,
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
  QuickCollection,
  QuickCollectionRequest,
  RenameCollectionRequest,
  RenameComparisonRequest,
  Resolution,
  ResolutionId,
  SampleText,
  SampleTextRequest,
  SaveComparisonRequest,
  ScanRequest,
  SearchRequest,
  SearchResponse,
  SelectionTags,
  SelectionTagsRequest,
  SetSourcePausedRequest,
  Settings,
  SettingsUpdate,
  SimilarFace,
  SimilarRequest,
  PairingRequest,
  PairingSuggestion,
  SizeFactor,
  SizeFactorRequest,
  UpdateCollectionRulesRequest,
  Source,
  SourceId,
  Tag,
  TagFacesRequest,
  Target,
  UntagFacesRequest,
  InstallUpdateRequest,
  UpdateCheck,
  UpdateSupport,
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
  /** Faces that look like one face, closest first, each with its score and reasons (E23.3). */
  findSimilar(req: SimilarRequest): Promise<SimilarFace[]>;
  /** Faces of other families that pair with one face by rule, best first, with reasons (E23.4). */
  suggestPairings(req: PairingRequest): Promise<PairingSuggestion[]>;
  /** Every collection, parents before children, siblings in the user's order. */
  listCollections(): Promise<Collection[]>;
  /** Creates a collection; with `rules`, a smart one. */
  createCollection(req: CreateCollectionRequest): Promise<Collection>;
  renameCollection(req: RenameCollectionRequest): Promise<Collection>;
  /** Replaces the rule of a smart collection (E14.1). */
  updateCollectionRules(req: UpdateCollectionRulesRequest): Promise<Collection>;
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
  /** The tags the faces of a selection carry, with how many carry each (E13.4). */
  selectionTags(req: SelectionTagsRequest): Promise<SelectionTags>;
  /** Favorite, rating (0 clears it) and notes; fields left out stay. */
  updateUserMeta(req: UserMetaUpdate): Promise<void>;
  /** A family's own notes; an empty string clears them. */
  updateFamilyNotes(req: FamilyNotesUpdate): Promise<void>;
  /** A family's declared license: type, seats, purchase date, receipt (E15.2). */
  declareLicense(req: LicenseDeclaration): Promise<void>;
  /** The Quick Collection (E13.3): its faces, first added first. */
  quickCollection(): Promise<QuickCollection>;
  /** Adds faces to the end of the Quick Collection; resolves to it. */
  addToQuickCollection(req: QuickCollectionRequest): Promise<QuickCollection>;
  /** Drops faces from the Quick Collection; resolves to what is left. */
  removeFromQuickCollection(req: QuickCollectionRequest): Promise<QuickCollection>;
  /** Empties the Quick Collection; the fonts are not touched. */
  clearQuickCollection(): Promise<void>;
  /**
   * "Export license inventory…" (E15.4): asks where to save with the native save dialog,
   * suggesting `fileName`, and writes the CSV of one collection (or the whole library)
   * there. Resolves to the number of families written; `null` when the user cancels.
   */
  saveLicenseInventory(req: LicenseInventoryRequest, fileName: string): Promise<number | null>;
  /** The URL the webview loads a face's font file from (for `@font-face`). */
  fontUrl(face: FaceId): string;
  /** A thumbnail: text rasterized in one face, cached by face hash and settings (PNG). */
  renderPreview(req: PreviewRequest): Promise<PreviewImage>;
  /** The text each face previews for a sample: the sample, or a fallback from its coverage. */
  sampleTexts(req: SampleTextRequest): Promise<SampleText[]>;
  /** The auto size factor of each face for a basis (x-height or cap height), in order. */
  sizeFactors(req: SizeFactorRequest): Promise<SizeFactor[]>;
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
  /** Every online catalog (E22.1), on or off, with the state of its cache. */
  listCatalogs(): Promise<Catalog[]>;
  /** Turns a catalog on or off; makes no request. */
  setCatalogEnabled(req: SetCatalogEnabledRequest): Promise<Catalog>;
  /** Fetches the catalog's list (the only call that goes to the network); refused while off. */
  refreshCatalog(req: CatalogRef): Promise<Catalog>;
  /** A page of a catalog's cached families by name; makes no request. */
  listCatalogFamilies(req: ListCatalogFamiliesRequest): Promise<Paged<CatalogFamily>>;
  /**
   * The URL the webview loads a catalog family's preview file from (`@font-face`); the
   * core fetches it into the cache folder the first time, only while the catalog is on.
   */
  catalogFontUrl(id: CatalogId, family: string): string;
  /**
   * Downloads a family into a watched source (E22.3): its files go into a subfolder named
   * after it, an existing file is never replaced, and the source is rescanned (the returned
   * job's scan events follow). The only other call besides a refresh that goes to the network.
   */
  downloadFamily(req: DownloadFamilyRequest): Promise<DownloadFamilyResponse>;
  /** Every family downloaded from a catalog, newest first, with where its files are now. */
  listCatalogDownloads(): Promise<CatalogDownload[]>;
  /**
   * The downloaded families with a newer version in their catalog, or no longer in it
   * (E22.4). Refreshes the catalogs that are on and have downloads first (requests: only on
   * the user's "Check for updates"), unless `cachedOnly` (right after a Refresh).
   */
  checkUpdates(req: CheckUpdatesRequest): Promise<CatalogUpdate[]>;
  /** Every saved comparison (E13.2), by name. */
  listComparisons(): Promise<ComparisonEntry[]>;
  /** One saved comparison: the faces the library still shows, and the missing ones by name. */
  getComparison(req: ComparisonRef): Promise<Comparison>;
  /** Saves a comparison (2–4 faces, a name), new or over `req.id`. */
  saveComparison(req: SaveComparisonRequest): Promise<Comparison>;
  /** Renames a saved comparison. */
  renameComparison(req: RenameComparisonRequest): Promise<ComparisonEntry>;
  /** Deletes a saved comparison; the fonts are not touched. */
  deleteComparison(req: ComparisonRef): Promise<void>;
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
  /** The local API endpoint: its address (while it runs) and the token (Settings → Local API). */
  endpointInfo(): Promise<EndpointInfo>;
  /** Makes a new local API token; the old one fails at once. */
  regenerateEndpointToken(): Promise<EndpointInfo>;
  /** The MCP server: whether it is on, its URL and the client config snippets (Settings → AI assistants). */
  mcpStatus(): Promise<McpStatus>;
  /** Turns the MCP server on or off; the next request sees it. */
  setMcpEnabled(req: McpUpdate): Promise<McpStatus>;
  /** Allows or refuses one MCP tool that changes something; the next call sees it. */
  setMcpGrant(req: McpGrantUpdate): Promise<McpStatus>;
  /** The user's Allow or Deny for an AI assistant's waiting call (the confirmation toast). */
  answerMcpConfirmation(req: McpConfirmationAnswer): Promise<void>;
  scan(req: ScanRequest): Promise<JobId>;
  /** Subscribes to the fm-api event stream; the UI subscribes, it does not poll. */
  onEvent(handler: (event: Event) => void): Promise<Unsubscribe>;
  /** The native folder picker; `null` when the user cancels. */
  pickFolder(title: string): Promise<string | null>;
  /** The native file picker; `null` when the user cancels. */
  pickFile(title: string): Promise<string | null>;
  /** Opens a page of the user docs (`HELP_PAGES` in src/help.ts) in the help window. */
  openHelp(page: string): Promise<void>;
  /** Opens an `https://` address in the default browser; other schemes are refused. */
  openUrl(url: string): Promise<void>;
  /**
   * Opens the OS print dialog for this window's page (Pairing Studio's "Print...", E24.4);
   * the page's print stylesheet decides what prints. PDF is the dialog's save-as-PDF.
   */
  printPage(): Promise<void>;
  /**
   * Tells the native shell (Help menu, tray) which catalog locale the UI shows, so
   * its labels follow the Language setting.
   */
  setShellLocale(locale: string): Promise<void>;
  /** Whether this copy updates itself; where not, Settings hides the update section. */
  updateSupport(): Promise<UpdateSupport>;
  /** Asks the release server once (a user action, or "Check automatically" at start). */
  checkForUpdate(): Promise<UpdateCheck>;
  /** Installs the update the last check found; the app quits, installs and restarts. */
  installUpdate(req: InstallUpdateRequest): Promise<void>;
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
  findSimilar: (req) => invoke<SimilarFace[]>('find_similar', { req }),
  suggestPairings: (req) => invoke<PairingSuggestion[]>('suggest_pairings', { req }),
  listCollections: () => invoke<Collection[]>('list_collections'),
  createCollection: (req) => invoke<Collection>('create_collection', { req }),
  renameCollection: (req) => invoke<Collection>('rename_collection', { req }),
  updateCollectionRules: (req) => invoke<Collection>('update_collection_rules', { req }),
  moveCollection: (req) => invoke<Collection>('move_collection', { req }),
  deleteCollection: (collection) => invoke<void>('delete_collection', { collection }),
  addToCollection: (req) => invoke<Collection>('add_to_collection', { req }),
  removeFromCollection: (req) => invoke<Collection>('remove_from_collection', { req }),
  listTags: () => invoke<Tag[]>('list_tags'),
  tagFaces: (req) => invoke<Tag[]>('tag_faces', { req }),
  untagFaces: (req) => invoke<void>('untag_faces', { req }),
  selectionTags: (req) => invoke<SelectionTags>('selection_tags', { req }),
  updateUserMeta: (req) => invoke<void>('update_user_meta', { req }),
  updateFamilyNotes: (req) => invoke<void>('update_family_notes', { req }),
  declareLicense: (req) => invoke<void>('declare_license', { req }),
  quickCollection: () => invoke<QuickCollection>('quick_collection'),
  addToQuickCollection: (req) => invoke<QuickCollection>('add_to_quick_collection', { req }),
  removeFromQuickCollection: (req) =>
    invoke<QuickCollection>('remove_from_quick_collection', { req }),
  clearQuickCollection: () => invoke<void>('clear_quick_collection'),
  saveLicenseInventory: (req, fileName) =>
    invoke<number | null>('save_license_inventory', { req, fileName }),
  fontUrl: (face) => convertFileSrc(String(face), FONT_SCHEME),
  renderPreview: (req) => invoke<PreviewImage>('render_preview', { req }),
  sampleTexts: (req) => invoke<SampleText[]>('sample_texts', { req }),
  sizeFactors: (req) => invoke<SizeFactor[]>('size_factors', { req }),
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
  listCatalogs: () => invoke<Catalog[]>('list_catalogs'),
  setCatalogEnabled: (req) => invoke<Catalog>('set_catalog_enabled', { req }),
  refreshCatalog: (req) => invoke<Catalog>('refresh_catalog', { req }),
  listCatalogFamilies: (req) => invoke<Paged<CatalogFamily>>('list_catalog_families', { req }),
  catalogFontUrl: (id, family) => convertFileSrc(catalogFontPath(id, family), FONT_SCHEME),
  downloadFamily: (req) => invoke<DownloadFamilyResponse>('download_family', { req }),
  listCatalogDownloads: () => invoke<CatalogDownload[]>('list_catalog_downloads'),
  checkUpdates: (req) => invoke<CatalogUpdate[]>('check_updates', { req }),
  listComparisons: () => invoke<ComparisonEntry[]>('list_comparisons'),
  getComparison: (req) => invoke<Comparison>('get_comparison', { req }),
  saveComparison: (req) => invoke<Comparison>('save_comparison', { req }),
  renameComparison: (req) => invoke<ComparisonEntry>('rename_comparison', { req }),
  deleteComparison: (req) => invoke<void>('delete_comparison', { req }),
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
  endpointInfo: () => invoke<EndpointInfo>('endpoint_info'),
  regenerateEndpointToken: () => invoke<EndpointInfo>('regenerate_endpoint_token'),
  mcpStatus: () => invoke<McpStatus>('mcp_status'),
  setMcpEnabled: (req) => invoke<McpStatus>('set_mcp_enabled', { req }),
  setMcpGrant: (req) => invoke<McpStatus>('set_mcp_grant', { req }),
  answerMcpConfirmation: (req) => invoke<void>('answer_mcp_confirmation', { req }),
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
  openUrl: (url) => invoke<void>('open_url', { url }),
  printPage: () => invoke<void>('print_page'),
  setShellLocale: (locale) => invoke<void>('set_shell_locale', { locale }),
  updateSupport: () => invoke<UpdateSupport>('update_support'),
  checkForUpdate: () => invoke<UpdateCheck>('check_for_update'),
  installUpdate: (req) => invoke<void>('install_update', { req }),
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
