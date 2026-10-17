// Sample answers for browser mode (Playwright) and unit tests, shaped like
// `fm_api::mock::MockApi`: the same faces, the same activation outcomes. Fixed
// data plus the faces it has activated (and how), so the UI can round-trip.
import type { FmClient, FullSettings } from './client';
import {
  API_VERSION,
  type ActivationMode,
  type ActivationOutcome,
  type ApiError,
  type AppFolders,
  type AppInfo,
  type Event,
  type FaceDetail,
  type FaceId,
  type JobId,
  type Source,
  type SourceId,
  type Target,
} from './types';

export const mockAppInfo: AppInfo = {
  name: 'Font Manager (mock)',
  version: '0.1.0',
  apiVersion: API_VERSION,
  platform: 'linux',
};

const ofl = {
  description: 'SIL Open Font License 1.1',
  url: 'https://openfontlicense.org',
  openLicense: 'ofl',
  embedding: 'installable',
  declared: null,
} as const;

const metrics = { unitsPerEm: 1000, ascender: 900, descender: -250, xHeight: 520, capHeight: 720 };

const user = { tags: [], favorite: false, rating: null, notes: null };

/** Faces 1, 6 and 7 of the Rust mock: variable, system clash, unsupported. */
export const mockFaces: FaceDetail[] = [
  {
    face: {
      id: 1,
      family: 1,
      familyName: 'Inter',
      style: 'Regular',
      postscriptName: 'Inter-Regular',
      weight: 400,
      width: 5,
      slant: 'upright',
      format: 'ttf',
      isVariable: true,
      active: null,
      health: { kind: 'ok' },
    },
    path: '/Users/demo/Fonts/Inter/Inter[wght].ttf',
    collectionIndex: 0,
    fileSize: 121000,
    hash: '1'.padStart(64, '0'),
    version: 'Version 1.000',
    copyright: 'Copyright Inter Authors',
    designer: 'Rasmus Andersson',
    vendorId: null,
    license: ofl,
    metrics,
    axes: [{ tag: 'wght', name: 'Weight', min: 100, default: 400, max: 900 }],
    instances: (
      [
        ['Thin', 100],
        ['Regular', 400],
        ['Bold', 700],
        ['Black', 900],
      ] as const
    ).map(([name, value]) => ({ name, coordinates: [{ tag: 'wght', value }] })),
    features: ['kern', 'liga', 'smcp', 'onum', 'ss01'].map((tag) => ({ tag, name: null })),
    scripts: ['latn', 'cyrl'],
    ...user,
    legacyFamily: 'Inter',
    legacyStyle: 'Regular',
    foundry: 'rsms',
    italicAngle: 0,
    codePointCount: 187,
  },
  {
    face: {
      id: 6,
      family: 3,
      familyName: 'JetBrains Mono',
      style: 'Regular',
      postscriptName: 'JetBrainsMono-Regular',
      weight: 400,
      width: 5,
      slant: 'upright',
      format: 'ttf',
      isVariable: false,
      active: null,
      health: { kind: 'ok' },
    },
    path: '/Users/demo/Fonts/JetBrainsMono-Regular.ttf',
    collectionIndex: 0,
    fileSize: 126000,
    hash: '6'.padStart(64, '0'),
    version: 'Version 1.000',
    copyright: 'Copyright JetBrains Mono Authors',
    designer: 'Philipp Nurullin',
    vendorId: null,
    license: ofl,
    metrics,
    axes: [],
    instances: [],
    features: ['kern', 'liga'].map((tag) => ({ tag, name: null })),
    scripts: ['latn', 'cyrl'],
    ...user,
    legacyFamily: 'JetBrains Mono',
    legacyStyle: 'Regular',
    foundry: 'JetBrains',
    italicAngle: 0,
    codePointCount: 133,
  },
  {
    face: {
      id: 7,
      family: 4,
      familyName: 'Legacy Grotesk',
      style: 'Regular',
      postscriptName: 'LegacyGrotesk',
      weight: 400,
      width: 5,
      slant: 'upright',
      format: 'type1',
      isVariable: false,
      active: null,
      health: { kind: 'unsupported', reason: 'Type 1 fonts cannot be activated; convert to OTF.' },
    },
    path: '/Users/demo/Fonts/legacy/LEGGR___.PFB',
    collectionIndex: 0,
    fileSize: 127000,
    hash: '7'.padStart(64, '0'),
    version: 'Version 1.000',
    copyright: 'Copyright Legacy Grotesk Authors',
    designer: 'Unknown',
    vendorId: null,
    license: {
      description: null,
      url: null,
      openLicense: null,
      embedding: 'restricted',
      declared: null,
    },
    metrics,
    axes: [],
    instances: [],
    features: ['kern', 'liga'].map((tag) => ({ tag, name: null })),
    scripts: ['latn'],
    ...user,
    legacyFamily: 'Legacy Grotesk',
    legacyStyle: 'Regular',
    foundry: 'Old Foundry',
    italicAngle: 0,
    codePointCount: 73,
  },
];

/** The system font the Rust mock gives face 6 the same PostScript name as. */
const systemClash: Partial<Record<FaceId, string>> = {
  6: '/System/Library/Fonts/JetBrainsMono-Regular.ttf',
};

/** The sources of the Rust mock: one watched folder and the system fonts. */
export const mockSources: Source[] = [
  { id: 1, path: '/Users/demo/Fonts', kind: 'watched', paused: false, faceCount: 7 },
  { id: 2, path: '/System/Library/Fonts', kind: 'system', paused: false, faceCount: 0 },
];

/** `Settings::default()` in fm-api. */
export const defaultSettings: FullSettings = {
  activationMode: 'session',
  onConflict: 'keepExisting',
  theme: 'system',
  density: 'comfortable',
  cacheLimitMb: 1024,
  rescanIntervalMinutes: 60,
};

/** The app folders of the Rust mock. */
export const mockFolders: AppFolders = {
  config: '/home/demo/.config/font-manager',
  data: '/home/demo/.local/share/font-manager',
  cache: '/home/demo/.cache/font-manager',
  logs: '/home/demo/.local/state/font-manager/logs',
};

const invalid = (message: string): ApiError => ({ code: 'invalidRequest', message, detail: null });

/** Options of a mock client. */
export interface MockOptions {
  /** Delay between the steps of a scan, in ms, so progress is visible (e2e). */
  scanStepMs?: number;
}

/** Folders the mock folder picker returns, one per call; `null` is a cancel. */
export const mockPickedFolders: (string | null)[] = [];

/** A mock client with its own activation, source and settings state. */
export function createMockClient(options: MockOptions = {}): FmClient {
  const active = new Map<FaceId, ActivationMode>();
  const sources = structuredClone(mockSources);
  const handlers = new Set<(event: Event) => void>();
  let nextId = 100;
  let picks = 0;
  let settings = { ...defaultSettings };
  const emit = (event: Event) => handlers.forEach((h) => h(event));
  const findSource = (id: SourceId) => {
    const s = sources.find((s) => s.id === id);
    if (!s)
      throw { code: 'notFound' as const, message: `Source ${id} was not found.`, detail: null };
    return s;
  };
  // Emits a scan of `files` files: progress steps, then the finish.
  const runScan = (job: JobId, files: number) => {
    const steps = [...Array(files + 1).keys()];
    const step = options.scanStepMs ?? 0;
    steps.forEach((done, i) =>
      setTimeout(() => emit({ type: 'scanProgress', job, done, total: files }), i * step),
    );
    setTimeout(
      () => emit({ type: 'scanFinished', job, added: 0, removed: 0, problems: 1 }),
      steps.length * step,
    );
  };
  const find = (id: FaceId) => mockFaces.find((f) => f.face.id === id);
  const notFound = (id: FaceId) => ({
    code: 'notFound' as const,
    message: `Face ${id} was not found.`,
    detail: null,
  });
  const ids = (target: Target | null | undefined): FaceId[] =>
    target?.kind === 'faces' ? target.faces : [...active.keys()];

  return {
    getAppInfo: async () => mockAppInfo,
    getFace: async (id) => {
      const f = find(id);
      if (!f) throw notFound(id);
      const detail = structuredClone(f);
      detail.face.active = active.get(id) ?? null;
      return detail;
    },
    activate: async (req) =>
      ids(req.target).map((id): ActivationOutcome => {
        const f = find(id);
        if (!f) throw notFound(id);
        const health = f.face.health;
        if (health.kind !== 'ok')
          return { face: id, outcome: { status: 'skipped', reason: health.reason } };
        const mode = req.mode ?? 'session';
        if (active.get(id) === mode) return { face: id, outcome: { status: 'unchanged' } };
        const clash = systemClash[id];
        if (clash && req.onConflict !== 'replace')
          return {
            face: id,
            outcome: { status: 'conflict', with: { kind: 'system', path: clash } },
          };
        active.set(id, mode);
        return { face: id, outcome: { status: 'done' } };
      }),
    deactivate: async (req) =>
      ids(req.target).map((id): ActivationOutcome =>
        active.delete(id)
          ? { face: id, outcome: { status: 'done' } }
          : {
              face: id,
              outcome: { status: 'skipped', reason: 'The app did not activate this font.' },
            },
      ),
    listSources: async () => structuredClone(sources),
    addSource: async (req) => {
      if (!req.path.trim())
        throw { code: 'invalidRequest', message: 'Choose a folder to add.', detail: null };
      if (sources.some((s) => s.path === req.path))
        throw { code: 'conflict', message: 'This folder is already in the library.', detail: null };
      const source: Source = {
        id: ++nextId,
        path: req.path,
        kind: 'watched',
        paused: false,
        faceCount: 0,
      };
      sources.push(source);
      return structuredClone(source);
    },
    removeSource: async (id) => {
      if (findSource(id).kind === 'system')
        throw {
          code: 'permissionDenied',
          message: 'System fonts cannot be removed.',
          detail: null,
        };
      sources.splice(
        sources.findIndex((s) => s.id === id),
        1,
      );
      emit({ type: 'libraryChanged', source: id, added: [], removed: [], updated: [] });
    },
    setSourcePaused: async (req) => {
      const s = findSource(req.source);
      s.paused = req.paused;
      return structuredClone(s);
    },
    scan: async (req) => {
      const files =
        req.source == null
          ? sources.reduce((n, s) => n + s.faceCount, 0)
          : findSource(req.source).faceCount;
      const job = ++nextId;
      runScan(job, files);
      return job;
    },
    onEvent: async (handler) => {
      handlers.add(handler);
      return () => handlers.delete(handler);
    },
    pickFolder: async () =>
      mockPickedFolders.length ? mockPickedFolders.shift()! : `/Users/demo/Added ${++picks}`,
    getSettings: async () => ({ ...settings }),
    // The same ranges as `Settings::apply` in fm-api.
    updateSettings: async (req) => {
      const mb = req.cacheLimitMb;
      if (mb != null && (mb < 64 || mb > 16384))
        throw invalid('The cache size must be between 64 and 16384 MB.');
      const min = req.rescanIntervalMinutes;
      if (min != null && min > 1440)
        throw invalid('The rescan interval must be between 0 (off) and 1440 minutes.');
      const next = { ...settings };
      for (const [k, v] of Object.entries(req)) if (v != null) Object.assign(next, { [k]: v });
      settings = next;
      return { ...settings };
    },
    getAppFolders: async () => ({ ...mockFolders }),
  };
}

// Browser mode (e2e) steps scans slowly enough to see the inline progress.
export const mockClient: FmClient = createMockClient({ scanStepMs: 150 });
