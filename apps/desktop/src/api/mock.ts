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
  type FaceSummary,
  type Family,
  type FamilyId,
  type FamilyOrder,
  type FamilySortKey,
  type FamilySummary,
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
  /** Extra families after the sample ones, for long lists (e2e). */
  extraFaces?: number;
  /** Styles in each extra family (1–6, default 1), so families can expand (e2e). */
  stylesPerFamily?: number;
}

/** Id of the first extra face and family; above every sample id. */
export const EXTRA_ID_BASE = 1000;

/** Styles of an extra family, the most common first; a family takes the first N. */
const EXTRA_STYLES = [
  { style: 'Regular', weight: 400, slant: 'upright' },
  { style: 'Bold', weight: 700, slant: 'upright' },
  { style: 'Italic', weight: 400, slant: 'italic' },
  { style: 'Bold Italic', weight: 700, slant: 'italic' },
  { style: 'Light', weight: 300, slant: 'upright' },
  { style: 'Black', weight: 900, slant: 'upright' },
] as const;

/**
 * The faces of extra family `i`, sorted like `Family.faces` (weight, then slant).
 * Every other family is variable, so long lists mix both kinds. With one style per
 * family, face and family ids are both `EXTRA_ID_BASE + i`.
 */
function extraFamily(i: number, styles: number): FaceDetail[] {
  const family = EXTRA_ID_BASE + i;
  const faces = EXTRA_STYLES.slice(0, styles).map((s, k) => {
    const id = EXTRA_ID_BASE + i * styles + k;
    const detail = structuredClone(mockFaces[i % 2 === 0 ? 0 : 1]!);
    const tag = s.style.replace(' ', '');
    detail.face = {
      ...detail.face,
      id,
      family,
      familyName: `Sample ${i + 1}`,
      style: s.style,
      weight: s.weight,
      slant: s.slant,
      postscriptName: `Sample${i + 1}-${tag}`,
    };
    detail.path = `/Users/demo/Fonts/Samples/Sample${i + 1}-${tag}.ttf`;
    detail.hash = String(id).padStart(64, '0');
    return detail;
  });
  const rank = (slant: string) => (slant === 'upright' ? 0 : 1);
  return faces.sort(
    (a, b) => a.face.weight - b.face.weight || rank(a.face.slant) - rank(b.face.slant),
  );
}

/** The list row of a family, from its faces and their details (like the Rust mock). */
function familySummary(faces: FaceSummary[], details: FaceDetail[]): FamilySummary {
  const first = faces[0]!;
  const detail = details[0];
  const active = faces.filter((f) => f.active).length;
  const styles = faces
    .map((f) => ({ weight: f.weight, italic: f.slant !== 'upright' }))
    .sort((a, b) => a.weight - b.weight || Number(a.italic) - Number(b.italic));
  // A variable face spans its `wght` axis; a static one sits at its weight.
  const weights = details.flatMap((d) => {
    const axis = d.axes.find((a) => a.tag === 'wght');
    return [axis?.min ?? d.face.weight, axis?.max ?? d.face.weight];
  });
  const tags = [...new Set(details.flatMap((d) => d.tags))].sort((a, b) =>
    a.toLowerCase().localeCompare(b.toLowerCase()),
  );
  return {
    id: first.family,
    name: first.familyName,
    styleCount: faces.length,
    formats: [...new Set(faces.map((f) => f.format))],
    foundry: detail?.foundry ?? null,
    classification: null,
    isVariable: faces.some((f) => f.isVariable),
    activation: active === 0 ? 'inactive' : active === faces.length ? 'active' : 'partial',
    added: '2026-10-01T00:00:00Z',
    styles,
    weightRange: weights.length ? { min: Math.min(...weights), max: Math.max(...weights) } : null,
    isColor: false,
    version: detail?.version ?? null,
    location: mockSources[0]!.path,
    openLicense: details.find((d) => d.license.openLicense)?.license.openLicense ?? null,
    declaredLicense: details.find((d) => d.license.declared)?.license.declared ?? null,
    tags,
  };
}

/** `a` and `b` split into digit runs (compared as numbers) and other text. */
function compareNatural(a: string, b: string): number {
  const parts = (s: string) => s.toLowerCase().match(/\d+|\D+/g) ?? [];
  const pa = parts(a);
  const pb = parts(b);
  for (let i = 0; i < Math.min(pa.length, pb.length); i++) {
    const x = pa[i]!;
    const y = pb[i]!;
    const dx = /^\d/.test(x);
    const dy = /^\d/.test(y);
    // Like the Rust enum: a number run sorts before a text run.
    const c =
      dx && dy ? Number(x) - Number(y) : dx !== dy ? (dx ? -1 : 1) : x < y ? -1 : x > y ? 1 : 0;
    if (c) return c;
  }
  return pa.length - pb.length;
}

/** The value a row has for `by`; `null` when it has none. Mirrors `FamilyOrder` in fm-api. */
function sortValue(f: FamilySummary, by: FamilySortKey): string | number | null {
  const text = (s: string | null | undefined) => (s?.trim() ? s.trim().toLowerCase() : null);
  switch (by) {
    case 'activation':
      return ['inactive', 'partial', 'active'].indexOf(f.activation);
    case 'name':
      return f.name.toLowerCase();
    case 'styleCount':
      return f.styleCount;
    case 'weight':
      return f.weightRange ? f.weightRange.min * 65536 + f.weightRange.max : null;
    case 'format':
      return f.formats.length
        ? `${[...f.formats].sort().join(',')} ${Number(f.isVariable)} ${Number(f.isColor ?? false)}`
        : null;
    case 'version':
      return text(f.version);
    case 'manufacturer':
      return text(f.foundry);
    case 'location':
      return text(f.location);
    case 'license':
      return f.declaredLicense || f.openLicense
        ? `${f.declaredLicense ?? '~'} ${f.openLicense ?? '~'}`
        : null;
    case 'dateAdded':
      return f.added || null;
    case 'tags':
      return f.tags?.length
        ? f.tags
            .map((t) => t.toLowerCase())
            .sort()
            .join('\u0001')
        : null;
  }
}

/** Sorts like `FamilyOrder::sort`: missing values last, ties by name, then id. */
function sortFamilies(rows: FamilySummary[], order: FamilyOrder): FamilySummary[] {
  const keyed = rows.map((row) => ({ row, value: sortValue(row, order.by) }));
  const cmp = (x: string | number, y: string | number) =>
    order.by === 'version'
      ? compareNatural(String(x), String(y))
      : typeof x === 'number' && typeof y === 'number'
        ? x - y
        : x < y
          ? -1
          : x > y
            ? 1
            : 0;
  keyed.sort((a, b) => {
    const byValue =
      a.value === null || b.value === null
        ? Number(a.value === null) - Number(b.value === null)
        : cmp(a.value, b.value) * (order.descending ? -1 : 1);
    const an = a.row.name.toLowerCase();
    const bn = b.row.name.toLowerCase();
    return byValue || (an < bn ? -1 : an > bn ? 1 : 0) || a.row.id - b.row.id;
  });
  return keyed.map((k) => k.row);
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
  // Emits a scan of `files` files: the start, progress steps, then the finish.
  const runScan = (job: JobId, source: SourceId | null, files: number) => {
    const steps = [...Array(files + 1).keys()];
    const step = options.scanStepMs ?? 0;
    setTimeout(() => emit({ type: 'scanStarted', job, source }), 0);
    steps.forEach((done, i) =>
      setTimeout(() => emit({ type: 'scanProgress', job, done, total: files }), i * step),
    );
    setTimeout(
      () => emit({ type: 'scanFinished', job, added: 0, changed: 0, removed: 0, problems: 1 }),
      steps.length * step,
    );
  };
  const styles = Math.min(EXTRA_STYLES.length, Math.max(1, options.stylesPerFamily ?? 1));
  const faces = [
    ...mockFaces,
    ...Array.from({ length: options.extraFaces ?? 0 }, (_, i) => extraFamily(i, styles)).flat(),
  ];
  // Indexes, so long lists (10,000 families) answer quickly.
  const byId = new Map(faces.map((f) => [f.face.id, f]));
  const byFamily = new Map<FamilyId, FaceDetail[]>();
  for (const f of faces) {
    const list = byFamily.get(f.face.family);
    if (list) list.push(f);
    else byFamily.set(f.face.family, [f]);
  }
  const familyIds = [...byFamily.keys()];
  const find = (id: FaceId) => byId.get(id);
  const summary = (f: FaceDetail): FaceSummary => ({
    ...structuredClone(f.face),
    active: active.get(f.face.id) ?? null,
  });
  const familyFaces = (family: FamilyId) => (byFamily.get(family) ?? []).map(summary);
  const summaryOf = (family: FamilyId) =>
    familySummary(familyFaces(family), byFamily.get(family) ?? []);
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
    // Families in id order (the sample ones first), or in `order`; `sort` is not mocked.
    listFamilies: async (req) => {
      const offset = req.page?.offset ?? 0;
      const limit = req.page?.limit ?? 100;
      if (req.order) {
        const all = sortFamilies(familyIds.map(summaryOf), req.order);
        return { items: all.slice(offset, offset + limit), total: all.length };
      }
      return {
        items: familyIds.slice(offset, offset + limit).map(summaryOf),
        total: familyIds.length,
      };
    },
    getFamily: async (id) => {
      const list = familyFaces(id);
      if (!list.length)
        throw { code: 'notFound' as const, message: `Family ${id} was not found.`, detail: null };
      const designer = find(list[0]!.id)!.designer;
      return { summary: summaryOf(id), designer, faces: list } satisfies Family;
    },
    // Same origin as the page; e2e serves fixture fonts on this path.
    fontUrl: (id) => `/fmfont/${id}`,
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
      runScan(job, req.source ?? null, files);
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

/**
 * A positive whole number from the page URL, before the hash: `?mockFaces=1000` adds
 * that many extra families, `?mockStyles=4` gives each of them four styles (e2e).
 */
function countFromUrl(name: string, max: number): number {
  if (typeof location === 'undefined') return 0;
  const n = Number(new URLSearchParams(location.search).get(name));
  return Number.isInteger(n) && n > 0 ? Math.min(n, max) : 0;
}

// Browser mode (e2e) steps scans slowly enough to see the inline progress.
export const mockClient: FmClient = createMockClient({
  scanStepMs: 150,
  extraFaces: countFromUrl('mockFaces', 100_000),
  stylesPerFamily: countFromUrl('mockStyles', EXTRA_STYLES.length) || 1,
});
