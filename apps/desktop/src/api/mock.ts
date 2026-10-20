// Sample answers for browser mode (Playwright) and unit tests, shaped like
// `fm_api::mock::MockApi`: the same faces, the same activation outcomes. Fixed
// data plus the faces it has activated (and how), so the UI can round-trip.
import { HELP_PAGES } from '../help';
import type { FmClient, FullSettings } from './client';
import {
  API_VERSION,
  type ActivationMode,
  type ActivationOutcome,
  type ApiError,
  type AppFolders,
  type AppInfo,
  type Clash,
  type Collection,
  type CollectionFacesRequest,
  type CollectionId,
  type Conflict,
  type CoverageReport,
  type DuplicateGroup,
  type Event,
  type FaceDetail,
  type FaceId,
  type FaceSummary,
  type FeatureInfo,
  type Family,
  type FamilyId,
  type FamilyOrder,
  type FamilySortKey,
  type FamilyHit,
  type FamilySummary,
  type FilterCount,
  type GlyphBlock,
  type GlyphDetail,
  type GlyphInfo,
  type GlyphPage,
  type GlyphRequest,
  type GlyphsRequest,
  type FilterCounts,
  type Filters,
  type JobId,
  type LanguagePreset,
  type PreviewImage,
  type PreviewRequest,
  type Problem,
  type Resolution,
  type ProjectSet,
  type SearchResponse,
  type SampleText,
  type Source,
  type SourceId,
  type Tag,
  type TagId,
  type Target,
  type VirtualStyle,
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

// Features as fm-core names them (crates/fm-core/src/features.rs).
const FEATURES: Record<string, Omit<FeatureInfo, 'tag'>> = {
  ccmp: { name: 'Glyph composition and decomposition', automatic: true, defaultOn: false },
  kern: { name: 'Kerning', automatic: false, defaultOn: true },
  liga: { name: 'Standard ligatures', automatic: false, defaultOn: true },
  onum: { name: 'Oldstyle figures', automatic: false, defaultOn: false },
  smcp: { name: 'Small capitals', automatic: false, defaultOn: false },
  ss01: { name: 'Stylistic set 1', automatic: false, defaultOn: false },
};
const features = (...tags: string[]): FeatureInfo[] =>
  tags.map((tag) => ({ tag, ...(FEATURES[tag] ?? { name: null }) }));
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
    features: features('ccmp', 'kern', 'liga', 'onum', 'smcp', 'ss01'),
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
    // Its own metrics, so the glyph compare overlay tells the two fonts' lines apart.
    metrics: { unitsPerEm: 1000, ascender: 1020, descender: -300, xHeight: 550, capHeight: 730 },
    axes: [],
    instances: [],
    features: features('kern', 'liga'),
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
    // A 2048-unit em and no declared x-height.
    metrics: { unitsPerEm: 2048, ascender: 1900, descender: -500, xHeight: null, capHeight: 1434 },
    axes: [],
    instances: [],
    features: features('kern', 'liga'),
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

/** `MOCK_NOW` of the Rust mock: when its activations started. */
export const MOCK_NOW = '2026-10-16T09:00:00Z';

/** `Settings::default()` in fm-api. */
export const defaultSettings: FullSettings = {
  activationMode: 'session',
  onConflict: 'keepExisting',
  theme: 'system',
  density: 'comfortable',
  cacheLimitMb: 1024,
  rescanIntervalMinutes: 60,
  reactivateOnLaunch: false,
};

/** The app folders of the Rust mock. */
export const mockFolders: AppFolders = {
  config: '/home/demo/.config/font-manager',
  data: '/home/demo/.local/share/font-manager',
  cache: '/home/demo/.cache/font-manager',
  logs: '/home/demo/.local/state/font-manager/logs',
};

// The characters the mock faces map: basic Latin and Latin-1 (Western European);
// faces that list `cyrl` also Latin Extended-A (Central European) and Serbian Cyrillic.
const range = (from: number, to: number) =>
  Array.from({ length: to - from }, (_, i) => String.fromCodePoint(from + i)).join('');
const LATIN = range(0x20, 0x7f) + range(0xa0, 0x100);
const LATIN_EXT_A = range(0x100, 0x180);
const CYRILLIC = 'абвгдђежзијклљмнњопрстћуфхцчџшАБВГДЂЕЖЗИЈКЛЉМНЊОПРСТЋУФХЦЧЏШ';

/** (script, characters the language needs), like the Rust mock's languages. */
const MOCK_LANGUAGES: [string, string][] = [
  ['Latn', 'abcdefghijklmnopqrstuvwxyz'],
  ['Latn', 'aąbcćdeęfghijklłmnńoóprsśtuwyzźż'],
  ['Latn', 'abcčćdđefghijklmnoprsštuvzž'],
  ['Cyrl', 'абвгдђежзијклљмнњопрстћуфхцчџш'],
];

function mockChars(f: FaceDetail): Set<string> {
  return new Set(f.scripts.includes('cyrl') ? LATIN + LATIN_EXT_A + CYRILLIC : LATIN);
}

/** The blocks the mock's characters fall in, like the Rust mock's: name, first, last. */
const MOCK_BLOCKS: [string, number, number][] = [
  ['Basic Latin', 0x0000, 0x007f],
  ['Latin-1 Supplement', 0x0080, 0x00ff],
  ['Latin Extended-A', 0x0100, 0x017f],
  ['Cyrillic', 0x0400, 0x04ff],
  ['CJK Unified Ideographs', 0x4e00, 0x9fff],
  ['Hangul Syllables', 0xac00, 0xd7af],
];

/** CJK ideographs and Hangul syllables: 32,164 characters, for a face given `cjk`. */
const CJK = range(0x4e00, 0xa000) + range(0xac00, 0xd7a4);

const mockBlock = (cp: number): [string, number, number] =>
  MOCK_BLOCKS.find(([, first, last]) => cp >= first && cp <= last) ?? ['No block', cp, cp];

/** `A` for letters, `uni0416` for the rest, like the Rust mock's names. */
const mockGlyphName = (c: string) =>
  /^[A-Za-z]$/.test(c) ? c : `uni${c.codePointAt(0)!.toString(16).toUpperCase().padStart(4, '0')}`;

/** A simple stand-in for fm-core's search: one character, `U+` or bare hex, a name part. */
function mockGlyphMatches(g: GlyphInfo, query: string): boolean {
  if ([...query].length === 1) return g.codePoint === query.codePointAt(0);
  const hex = query.replace(/^u\+/i, '');
  if (/^[0-9a-f]{1,6}$/i.test(hex) && parseInt(hex, 16) === g.codePoint) return true;
  return !!g.name && g.name.toLowerCase().includes(query.toLowerCase());
}

/** The characters of `text` a mock face lacks, like the Rust mock's `check_coverage`. */
export function mockCoverage(f: FaceDetail, text: string, cjk: boolean): CoverageReport {
  const has = mockChars(f);
  const missing = [...new Set([...text])].filter(
    (c) => !/\s/.test(c) && !has.has(c) && !(cjk && CJK.includes(c)),
  );
  return { face: f.face.id, complete: missing.length === 0, missing, languages: [] };
}

/** One page of a mock face's glyph map: its characters, plus CJK when `cjk`. */
export function mockGlyphPage(f: FaceDetail, req: GlyphsRequest, cjk: boolean): GlyphPage {
  const query = req.query?.trim() ?? '';
  const chars = [...mockChars(f)].join('') + (cjk ? CJK : '');
  const all: GlyphInfo[] = [...chars]
    .map((c, i) => ({
      codePoint: c.codePointAt(0)!,
      glyphId: i + 1,
      name: mockGlyphName(c),
      block: mockBlock(c.codePointAt(0)!)[0],
    }))
    .sort((a, b) => a.codePoint - b.codePoint)
    .filter((g) => req.block == null || g.block === req.block)
    .filter((g) => !query || mockGlyphMatches(g, query));
  const blocks: GlyphBlock[] = [];
  for (const g of all) {
    const last = blocks[blocks.length - 1];
    if (last?.name === g.block) last.count += 1;
    else {
      const [name, first, end] = mockBlock(g.codePoint);
      blocks.push({ name, first, last: end, count: 1 });
    }
  }
  const offset = req.page?.offset ?? 0;
  const limit = Math.min(req.page?.limit ?? 100, 500);
  return {
    items: all.slice(offset, offset + limit),
    total: all.length,
    blocks,
    health: { kind: 'ok' },
    unicodeVersion: '17.0.0',
  };
}

/** A GSUB feature as fm-core names it, for the mock's glyph detail. */
function mockFeature(tag: string): FeatureInfo {
  const names: Record<string, string> = {
    liga: 'Standard ligatures',
    smcp: 'Small capitals',
    onum: 'Oldstyle figures',
    ss01: 'Stylistic set 1',
  };
  return { tag, name: names[tag] ?? null, automatic: false, defaultOn: tag === 'liga' };
}

/**
 * The detail of one character, as the Rust MockApi gives it: the glyph as the glyph
 * map lists it; lower-case letters are replaced by `smcp` and `ss01`, figures by
 * `onum`, `fi`/`fl` ligatures produced by `liga`; CJK is 1,000 units wide.
 */
export function mockGlyphDetail(f: FaceDetail, req: GlyphRequest, cjk: boolean): GlyphDetail {
  // Glyph ids as mockGlyphPage numbers them.
  const all = [...mockChars(f)].join('') + (cjk ? CJK : '');
  const index = [...all].findIndex((c) => c.codePointAt(0) === req.codePoint);
  if (index < 0)
    throw {
      code: 'notFound' as const,
      message: `Character U+${req.codePoint.toString(16).toUpperCase().padStart(4, '0')} was not found.`,
      detail: null,
    };
  const c = String.fromCodePoint(req.codePoint);
  const cp = req.codePoint;
  return {
    codePoint: cp,
    glyphId: index + 1,
    name: mockGlyphName(c),
    advanceWidth: cp >= 0x2e80 ? 1000 : 500 + (cp % 5) * 50,
    producedBy: c === 'ﬁ' || c === 'ﬂ' ? [mockFeature('liga')] : [],
    replacedBy: /[a-z]/.test(c)
      ? [mockFeature('smcp'), mockFeature('ss01')]
      : /[0-9]/.test(c)
        ? [mockFeature('onum')]
        : [],
    health: { kind: 'ok' },
  };
}

/**
 * The rule of `fm_core::sample` on the mock's languages: the sample when the face maps
 * it; else letters of the complete language whose script the face maps most; else
 * its own characters.
 */
export function mockSampleText(f: FaceDetail, text: string): SampleText {
  const chars = mockChars(f);
  const face = f.face.id;
  const missing: string[] = [];
  for (const c of text)
    if (!/[\s\p{Cc}]/u.test(c) && !chars.has(c) && !missing.includes(c)) missing.push(c);
  if (!missing.length) return { face, text, fallback: null, missing };
  let best: { score: number; script: string; letters: string } | undefined;
  for (const [script, letters] of MOCK_LANGUAGES) {
    if (![...letters].every((c) => chars.has(c))) continue;
    const all = new Set(MOCK_LANGUAGES.filter((l) => l[0] === script).flatMap((l) => [...l[1]]));
    const score = [...all].filter((c) => chars.has(c)).length;
    if (!best || score > best.score) best = { score, script, letters };
  }
  if (best)
    return {
      face,
      text: [...best.letters].slice(0, 26).join(''),
      fallback: { kind: 'script', script: best.script },
      missing,
    };
  const own = [...chars].filter((c) => !/\s/u.test(c)).slice(0, 24);
  return { face, text: own.join(' '), fallback: { kind: 'symbols' }, missing };
}

/** A 1×1 transparent PNG: what the Rust mock's `render_preview` returns. */
const PNG_1X1 = [
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
  0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
  0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
  0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
  0x42, 0x60, 0x82,
];

/**
 * A mock thumbnail: in a browser, the text drawn black on transparent in the
 * browser's sans-serif (so e2e screenshots show something); else the 1×1 PNG.
 */
export async function mockPreviewImage(req: PreviewRequest): Promise<PreviewImage> {
  const sizePx = req.sizePx ?? 0;
  if (typeof OffscreenCanvas === 'undefined' || sizePx <= 0)
    return { width: 1, height: 1, png: [...PNG_1X1] };
  const font = `${sizePx}px sans-serif`;
  const probe = new OffscreenCanvas(1, 1).getContext('2d');
  if (!probe) return { width: 1, height: 1, png: [...PNG_1X1] };
  probe.font = font;
  const width = Math.max(1, Math.ceil(probe.measureText(req.text).width));
  const height = Math.max(1, Math.ceil(sizePx * 1.3));
  const canvas = new OffscreenCanvas(width, height);
  const ctx = canvas.getContext('2d');
  if (!ctx) return { width: 1, height: 1, png: [...PNG_1X1] };
  ctx.font = font;
  ctx.fillStyle = req.foreground ?? '#000000';
  ctx.textBaseline = 'middle';
  ctx.fillText(req.text, 0, height / 2);
  const blob = await canvas.convertToBlob({ type: 'image/png' });
  return { width, height, png: [...new Uint8Array(await blob.arrayBuffer())] };
}

const invalid = (message: string): ApiError => ({ code: 'invalidRequest', message, detail: null });

/** Options of a mock client. */
export interface MockOptions {
  /** Delay between the steps of a scan, in ms, so progress is visible (e2e). */
  scanStepMs?: number;
  /** Extra families after the sample ones, for long lists (e2e). */
  extraFaces?: number;
  /** Styles in each extra family (1–6, default 1), so families can expand (e2e). */
  stylesPerFamily?: number;
  /** User tags per face (the sample faces have none). */
  faceTags?: Partial<Record<FaceId, string[]>>;
  /** Problems entries after the ones the sample faces have (files the mock has no face for). */
  extraProblems?: Problem[];
  /** Duplicate groups (files the mock has no face for), keeper first. */
  duplicates?: DuplicateGroup[];
  /** Project sets (none by default). */
  sets?: ProjectSet[];
  /** Faces whose glyph map also holds 32,164 CJK and Hangul characters (e2e). */
  cjkFaces?: FaceId[];
}

/** A sample project set, like the Rust mock's: Inter and JetBrains Mono. */
export const mockSets: ProjectSet[] = [{ id: 1, name: 'Client brand', faces: [1, 6] }];

/**
 * Problems of files outside the sample faces, for browser mode: one of each kind the
 * sample library lacks (a broken file, a duplicate copy, a sidecar conflict), so the
 * Problems view shows every group.
 */
export const mockExtraProblems: Problem[] = [
  {
    kind: 'broken',
    face: null,
    path: '/Users/demo/Fonts/damaged/Quarto-Bold.otf',
    other: null,
    message: "The file is truncated: the 'glyf' table ends before its declared length.",
  },
  {
    kind: 'duplicateFile',
    face: null,
    path: '/Users/demo/Downloads/Quarto-Regular.otf',
    other: '/Users/demo/Fonts/Quarto-Regular.otf',
    message: 'The same file is also at /Users/demo/Fonts/Quarto-Regular.otf.',
  },
  {
    kind: 'sidecarConflict',
    face: null,
    path: '/Users/demo/Fonts/.fontlibrary/meta.json',
    other: null,
    message: 'Another computer set a different rating for Inter Regular.',
  },
];

/**
 * The duplicate group behind the `duplicateFile` entry of `mockExtraProblems`: the
 * library-folder copy is the keeper, the Downloads copy the duplicate.
 */
export const mockDuplicates: DuplicateGroup[] = [
  {
    kind: 'exact',
    postscriptName: 'Quarto-Regular',
    copies: [
      {
        path: '/Users/demo/Fonts/Quarto-Regular.otf',
        size: 81234,
        hash: 'a1'.repeat(32),
        face: null,
        version: 'Version 1.100',
        source: 1,
        inLibraryFolder: true,
        keep: true,
      },
      {
        path: '/Users/demo/Downloads/Quarto-Regular.otf',
        size: 81234,
        hash: 'a1'.repeat(32),
        face: null,
        version: 'Version 1.100',
        source: 1,
        inLibraryFolder: false,
        keep: false,
      },
    ],
  },
];

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

/** Language presets of the mock: a face covers one when it has the preset's script. */
export const mockLanguages: LanguagePreset[] = [
  { code: 'en', name: 'English', script: 'Latn' },
  { code: 'pl', name: 'Polish', script: 'Latn' },
  { code: 'sr-Cyrl', name: 'Serbian (Cyrillic)', script: 'Cyrl' },
  { code: 'el', name: 'Greek', script: 'Grek' },
];

/**
 * True when face `d` passes `q` (the mock's subset of `Filters`, like the Rust mock);
 * `tags` are the ids of its tags.
 */
function passes(d: FaceDetail, active: boolean, tags: TagId[], q: Filters): boolean {
  const f = d.face;
  const covers = (code: string) => {
    const script = mockLanguages.find((l) => l.code === code)?.script.toLowerCase();
    return !!script && d.scripts.includes(script);
  };
  const supports = q.supports;
  return (
    (q.slant == null || q.slant === f.slant) &&
    (q.variable == null || q.variable === f.isVariable) &&
    (q.color == null || !q.color) &&
    (!q.formats?.length || q.formats.includes(f.format)) &&
    (q.features ?? []).every((t) => d.features.some((x) => x.tag === t)) &&
    (q.license == null || d.license.declared === q.license) &&
    (!q.sources?.length || q.sources.includes(1)) &&
    (q.active == null || q.active === active) &&
    (q.problems == null || q.problems === (f.health.kind !== 'ok')) &&
    (q.tags ?? []).every((t) => tags.includes(t)) &&
    (q.favorite == null || q.favorite === d.favorite) &&
    (q.minRating == null || (d.rating ?? 0) >= q.minRating) &&
    (supports?.kind !== 'languages' || supports.codes.every(covers))
  );
}

/** Folders the mock folder picker returns, one per call; `null` is a cancel. */
export const mockPickedFolders: (string | null)[] = [];

/** A mock client with its own activation, source and settings state. */
export function createMockClient(options: MockOptions = {}): FmClient {
  const active = new Map<FaceId, ActivationMode>();
  const sources = structuredClone(mockSources);
  const sets = structuredClone(options.sets ?? []);
  const handlers = new Set<(event: Event) => void>();
  let nextId = 100;
  let picks = 0;
  let settings = { ...defaultSettings };
  const emit = (event: Event) => handlers.forEach((h) => h(event));
  // "Keep this one": like the Rust mock, trashed paths leave the duplicate groups.
  const trashed = new Set<string>();
  const resolutions: Resolution[] = [];
  const duplicates = (): DuplicateGroup[] =>
    structuredClone(options.duplicates ?? [])
      .map((g) => ({ ...g, copies: g.copies.filter((c) => !trashed.has(c.path)) }))
      .filter((g) => g.copies.length > 1);
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
  // The sample faces are copied: tagging and rating edit this client's faces only.
  const faces = [
    ...mockFaces.map((f) => structuredClone(f)),
    ...Array.from({ length: options.extraFaces ?? 0 }, (_, i) => extraFamily(i, styles)).flat(),
  ].map((f) => {
    const tags = options.faceTags?.[f.face.id];
    return tags ? { ...f, tags: [...tags] } : f;
  });
  // Tags by id; a face holds their names (`FaceDetail.tags`), like the Rust mock.
  const tags: { id: TagId; name: string }[] = [];
  // Like fm-core's `COLLATE NOCASE`: only A–Z case is folded ("Äb" and "äb" are two tags).
  const fold = (s: string) => s.replace(/[A-Z]/g, (c) => c.toLowerCase());
  const sameName = (a: string, b: string) => fold(a) === fold(b);
  for (const f of faces)
    for (const name of f.tags)
      if (!tags.some((t) => sameName(t.name, name))) tags.push({ id: ++nextId, name });
  const tagIdsOf = (d: FaceDetail): TagId[] =>
    tags.filter((t) => d.tags.some((n) => sameName(n, t.name))).map((t) => t.id);
  const tagView = (t: { id: TagId; name: string }): Tag => ({
    ...t,
    faceCount: faces.filter((f) => f.tags.some((n) => sameName(n, t.name))).length,
  });
  const changed = (updated: FaceId[]) =>
    emit({ type: 'libraryChanged', source: null, added: [], removed: [], updated });
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
  const faceOrThrow = (id: FaceId) => {
    const f = find(id);
    if (!f) throw notFound(id);
    return f;
  };
  // Family groups a search returns: name matches first, then by name (like the Rust mock).
  const hits = (query: string, q: Filters): FamilyHit[] => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    const out: FamilyHit[] = [];
    for (const [family, details] of byFamily) {
      const matching = details.filter((d) => {
        const hay =
          `${d.face.familyName} ${d.face.style} ${d.face.postscriptName} ${d.foundry ?? ''} ${d.designer ?? ''}`.toLowerCase();
        return (
          words.every((w) => hay.includes(w)) &&
          passes(d, active.has(d.face.id), tagIdsOf(d), q) &&
          (q.collection == null || !!members.get(q.collection)?.has(d.face.id))
        );
      });
      if (!matching.length) continue;
      const nameHit = words.some((w) => details[0]!.face.familyName.toLowerCase().includes(w));
      out.push({
        family: summaryOf(family),
        faces: matching.map((d) => d.face.id),
        score: nameHit ? 1 : 0.5,
        missing: q.supports ? { chars: [], hasLocl: false } : null,
      });
    }
    return out.sort((a, b) => b.score! - a.score! || a.family.name.localeCompare(b.family.name));
  };
  // Per value, the family groups with that value chosen and the other filters kept.
  const countEach = <T>(
    query: string,
    q: Filters,
    values: T[],
    pick: (f: Filters, v: T) => Filters,
  ): FilterCount<T>[] =>
    values
      .map((value) => ({ value, count: hits(query, pick(q, value)).length }))
      .filter((c) => c.count > 0);
  const distinct = <T>(list: T[]) => [...new Set(list)];
  const both = [true, false];
  const counts = (query: string, q: Filters): FilterCounts => ({
    slant: countEach(query, q, distinct(faces.map((d) => d.face.slant)), (f, slant) => ({
      ...f,
      slant,
    })),
    variable: countEach(query, q, both, (f, variable) => ({ ...f, variable })),
    color: countEach(query, q, both, (f, color) => ({ ...f, color })),
    formats: countEach(query, q, distinct(faces.map((d) => d.face.format)), (f, v) => ({
      ...f,
      formats: [v],
    })),
    features: countEach(
      query,
      q,
      distinct(faces.flatMap((d) => d.features.map((x) => x.tag))).sort(),
      (f, v) => ({ ...f, features: distinct([...(f.features ?? []), v]) }),
    ),
    sources: countEach(query, q, [1], (f, v) => ({ ...f, sources: [v] })),
    problems: countEach(query, q, both, (f, problems) => ({ ...f, problems })),
    active: countEach(query, q, both, (f, v) => ({ ...f, active: v })),
    tags: countEach(
      query,
      q,
      tags.map((t) => t.id),
      (f, v) => ({ ...f, tags: distinct([...(f.tags ?? []), v]) }),
    ),
    favorite: countEach(query, q, both, (f, favorite) => ({ ...f, favorite })),
    minRating: countEach(query, q, [1, 2, 3, 4, 5], (f, minRating) => ({ ...f, minRating })),
  });
  // Siblings keep the array's order; the list walks the tree from it.
  const collections: Collection[] = [];
  const members = new Map<CollectionId, Set<FaceId>>();
  const conflict = (name: string) => ({
    code: 'conflict' as const,
    message: `A collection named "${name}" already exists here.`,
    detail: null,
  });
  const findCollection = (id: CollectionId) => {
    const c = collections.find((c) => c.id === id);
    if (!c)
      throw { code: 'notFound' as const, message: `Collection ${id} was not found.`, detail: null };
    return c;
  };
  const nameTaken = (parent: CollectionId | null, name: string, except?: CollectionId) =>
    collections.some((c) => c.parent === parent && c.name === name && c.id !== except);
  // Smart collections are not evaluated (like fm-core until E14.2): their count is 0.
  const viewOf = (c: Collection): Collection => ({
    ...structuredClone(c),
    faceCount: c.rules ? 0 : (members.get(c.id)?.size ?? 0),
  });
  const walk = (parent: CollectionId | null): Collection[] =>
    collections.filter((c) => c.parent === parent).flatMap((c) => [viewOf(c), ...walk(c.id)]);
  // Virtual styles (E10.2), like the Rust mock: in a manual collection or a set.
  const virtualStyles: VirtualStyle[] = [];
  const checkHolder = (collection?: CollectionId | null, set?: number | null) => {
    if (collection != null && set == null) {
      if (findCollection(collection).rules)
        throw invalid(
          'A virtual style goes in a manual collection or a set, not a smart collection.',
        );
      return;
    }
    if (collection == null && set != null) {
      if (!sets.some((s) => s.id === set))
        throw { code: 'notFound' as const, message: `Set ${set} was not found.`, detail: null };
      return;
    }
    throw invalid('Choose one collection or one set for the virtual style.');
  };
  const editMembers = (req: CollectionFacesRequest, add: boolean): Collection => {
    const c = findCollection(req.collection);
    if (c.rules) throw invalid('A smart collection fills itself from its rules.');
    const ids = [...req.faces];
    for (const id of ids) if (!find(id)) throw notFound(id);
    for (const family of req.families ?? []) {
      const list = byFamily.get(family);
      if (!list)
        throw {
          code: 'notFound' as const,
          message: `Family ${family} was not found.`,
          detail: null,
        };
      ids.push(...list.map((d) => d.face.id));
    }
    const set = members.get(c.id) ?? new Set<FaceId>();
    members.set(c.id, set);
    for (const id of ids) {
      if (add) set.add(id);
      else set.delete(id);
    }
    return viewOf(c);
  };
  // Like the Rust mock: faces, families and sets; no target means every active face.
  const ids = (target: Target | null | undefined): FaceId[] => {
    if (!target) return [...active.keys()];
    if (target.kind === 'faces') return target.faces;
    if (target.kind === 'family') {
      const list = byFamily.get(target.family);
      if (!list)
        throw {
          code: 'notFound' as const,
          message: `Family ${target.family} was not found.`,
          detail: null,
        };
      return list.map((d) => d.face.id);
    }
    if (target.kind === 'set') {
      const set = sets.find((s) => s.id === target.set);
      if (!set)
        throw {
          code: 'notFound' as const,
          message: `Set ${target.set} was not found.`,
          detail: null,
        };
      return [...set.faces];
    }
    throw invalid(`The mock cannot act on a ${target.kind}.`);
  };

  return {
    getAppInfo: async () => mockAppInfo,
    sampleTexts: async (req) =>
      req.faces.map((id) => {
        const f = find(id);
        if (!f) throw notFound(id);
        return mockSampleText(f, req.text);
      }),
    listGlyphs: async (req) => {
      const f = find(req.face);
      if (!f) throw notFound(req.face);
      return mockGlyphPage(f, req, options.cjkFaces?.includes(req.face) ?? false);
    },
    getGlyph: async (req) => {
      const f = find(req.face);
      if (!f) throw notFound(req.face);
      return mockGlyphDetail(f, req, options.cjkFaces?.includes(req.face) ?? false);
    },
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
    search: async (req) => {
      const query = req.query ?? '';
      const filters = req.filters ?? {};
      const all = hits(query, filters);
      const offset = req.page?.offset ?? 0;
      const limit = req.page?.limit ?? 100;
      return {
        families: all.slice(offset, offset + limit),
        total: all.length,
        counts: counts(query, filters),
      } satisfies SearchResponse;
    },
    checkCoverage: async (req) => {
      const f = find(req.face);
      if (!f) throw notFound(req.face);
      if (req.target.kind !== 'text') throw invalid('The mock checks the coverage of text only.');
      return mockCoverage(f, req.target.text, options.cjkFaces?.includes(req.face) ?? false);
    },
    listLanguages: async () => structuredClone(mockLanguages),
    createCollection: async (req) => {
      const name = req.name.trim();
      if (!name) throw invalid('Give the collection a name.');
      if (req.parent != null) findCollection(req.parent);
      if (nameTaken(req.parent ?? null, name)) throw conflict(name);
      const c: Collection = {
        id: ++nextId,
        parent: req.parent ?? null,
        name,
        rules: req.rules ?? null,
        faceCount: 0,
      };
      collections.push(c);
      return viewOf(c);
    },
    listCollections: async () => walk(null),
    renameCollection: async (req) => {
      const name = req.name.trim();
      if (!name) throw invalid('Give the collection a name.');
      const c = findCollection(req.collection);
      if (nameTaken(c.parent, name, c.id)) throw conflict(name);
      c.name = name;
      return viewOf(c);
    },
    moveCollection: async (req) => {
      const c = findCollection(req.collection);
      const parent = req.parent ?? null;
      for (let up = parent; up != null; up = findCollection(up).parent)
        if (up === c.id)
          throw invalid('A collection cannot go inside itself or one of its children.');
      if (nameTaken(parent, c.name, c.id)) throw conflict(c.name);
      collections.splice(collections.indexOf(c), 1);
      c.parent = parent;
      const siblings = collections.filter((x) => x.parent === parent);
      const before = siblings[req.position];
      collections.splice(before ? collections.indexOf(before) : collections.length, 0, c);
      return viewOf(c);
    },
    deleteCollection: async (id) => {
      findCollection(id);
      const doomed = new Set([id]);
      // A child can sit before its parent in the array after a move: repeat until stable.
      for (let size = 0; size !== doomed.size;) {
        size = doomed.size;
        for (const c of collections) if (c.parent != null && doomed.has(c.parent)) doomed.add(c.id);
      }
      for (let i = collections.length - 1; i >= 0; i--)
        if (doomed.has(collections[i]!.id)) collections.splice(i, 1);
      for (const d of doomed) members.delete(d);
      for (let i = virtualStyles.length - 1; i >= 0; i--) {
        const c = virtualStyles[i]!.collection;
        if (c != null && doomed.has(c)) virtualStyles.splice(i, 1);
      }
    },
    addToCollection: async (req) => editMembers(req, true),
    removeFromCollection: async (req) => editMembers(req, false),
    listTags: async () =>
      tags.map(tagView).sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase())),
    // Like fm-core: names are trimmed, the same name in another case is the same tag.
    tagFaces: async (req) => {
      const names = req.tags.map((n) => n.trim()).filter(Boolean);
      if (!names.length) throw invalid('Give the tag a name.');
      if (!req.faces.length) throw invalid('Choose the fonts to tag.');
      const targets = req.faces.map(faceOrThrow);
      const named: { id: TagId; name: string }[] = [];
      for (const name of names) {
        let t = tags.find((x) => sameName(x.name, name));
        if (!t) tags.push((t = { id: ++nextId, name }));
        for (const d of targets)
          if (!d.tags.some((n) => sameName(n, t.name))) d.tags = [...d.tags, t.name];
        if (!named.some((x) => x.id === t.id)) named.push(t);
      }
      changed(req.faces);
      return named.map(tagView);
    },
    untagFaces: async (req) => {
      const targets = req.faces.map(faceOrThrow);
      for (const id of req.tags) {
        const t = tags.find((x) => x.id === id);
        if (!t)
          throw { code: 'notFound' as const, message: `Tag ${id} was not found.`, detail: null };
        for (const d of targets) d.tags = d.tags.filter((n) => !sameName(n, t.name));
        if (!faces.some((f) => f.tags.some((n) => sameName(n, t.name))))
          tags.splice(tags.indexOf(t), 1);
      }
      changed(req.faces);
    },
    updateUserMeta: async (req) => {
      if (req.rating != null && req.rating > 5)
        throw invalid('A rating is 1 to 5 stars, or 0 to clear it.');
      const targets = req.faces.map(faceOrThrow);
      for (const d of targets) {
        if (req.favorite != null) d.favorite = req.favorite;
        if (req.rating != null) d.rating = req.rating || null;
        if (req.notes != null) d.notes = req.notes || null;
      }
      changed(req.faces);
    },
    // Same origin as the page; e2e serves fixture fonts on this path.
    fontUrl: (id) => `/fmfont/${id}`,
    renderPreview: async (req) => {
      const sizePx = req.sizePx ?? NaN;
      if (!(sizePx >= 8 && sizePx <= 300))
        throw invalid('Preview size must be between 8 and 300 px.');
      if (!find(req.face)) throw notFound(req.face);
      return mockPreviewImage(req);
    },
    activate: async (req) => {
      const activated: FaceId[] = [];
      const out = ids(req.target).map((id): ActivationOutcome => {
        const f = find(id);
        if (!f) throw notFound(id);
        const health = f.face.health;
        if (health.kind !== 'ok')
          return { face: id, outcome: { status: 'skipped', reason: health.reason } };
        const clash = systemClash[id];
        const replace = req.onConflict === 'replace';
        // "Replace" is for this session only.
        const mode = clash && replace ? 'session' : (req.mode ?? 'session');
        if (active.get(id) === mode) return { face: id, outcome: { status: 'unchanged' } };
        if (clash && !replace)
          return {
            face: id,
            outcome: { status: 'conflict', with: { kind: 'system', path: clash } },
          };
        active.set(id, mode);
        activated.push(id);
        return { face: id, outcome: { status: 'done' } };
      });
      if (activated.length) emit({ type: 'activationChanged', activated, deactivated: [] });
      return out;
    },
    deactivate: async (req) => {
      const deactivated: FaceId[] = [];
      const keep = new Set(req.except ? ids(req.except) : []);
      const out = ids(req.target)
        .filter((id) => !keep.has(id))
        .map((id): ActivationOutcome => {
          if (!active.delete(id))
            return {
              face: id,
              outcome: { status: 'skipped', reason: 'The app did not activate this font.' },
            };
          deactivated.push(id);
          return { face: id, outcome: { status: 'done' } };
        });
      if (deactivated.length) emit({ type: 'activationChanged', activated: [], deactivated });
      return out;
    },
    listActive: async () =>
      [...active]
        .sort(([a], [b]) => a - b)
        .map(([face, mode]) => ({ face, mode, since: MOCK_NOW, document: null })),
    conflicts: async (target) =>
      ids(target).flatMap((id): Conflict[] => {
        const f = find(id);
        if (!f) throw notFound(id);
        const clash = systemClash[id];
        return clash
          ? [
              {
                face: id,
                postscriptName: f.face.postscriptName,
                with: { kind: 'system', path: clash },
              },
            ]
          : [];
      }),
    listConflicts: async () =>
      [...active.keys()]
        .sort((a, b) => a - b)
        .flatMap((id): Clash[] => {
          const clash = systemClash[id];
          const f = find(id);
          return clash && f
            ? [
                {
                  face: id,
                  postscriptName: f.face.postscriptName,
                  with: { kind: 'system', path: clash },
                  winner: 'face',
                },
              ]
            : [];
        }),
    // Like the Rust mock: per face its health and system clash, then the active pairs
    // (none: no two sample faces share a PostScript name), then the extra entries.
    listSets: async () => structuredClone(sets),
    pinInstance: async (req) => {
      const name = req.name.trim();
      if (!name) throw invalid('Give the style a name.');
      checkHolder(req.collection, req.set);
      const f = find(req.face);
      if (!f) throw notFound(req.face);
      const label = `${f.face.familyName} ${f.face.style}`;
      if (!f.axes.length)
        throw invalid(`“${label}” is not a variable font, so it has no instances to pin.`);
      const given = req.coordinates ?? [];
      for (const c of given) {
        const axis = f.axes.find((a) => a.tag === c.tag);
        if (!axis) throw invalid(`“${label}” has no axis “${c.tag}”.`);
        if (c.value == null || c.value < axis.min! || c.value > axis.max!)
          throw invalid(
            `${axis.name} (${axis.tag}) goes from ${axis.min} to ${axis.max}; ${c.value} is outside it.`,
          );
      }
      const collection = req.collection ?? null;
      const set = req.set ?? null;
      if (
        virtualStyles.some(
          (v) =>
            v.face === req.face &&
            (v.collection ?? null) === collection &&
            (v.set ?? null) === set &&
            v.name.toLowerCase() === name.toLowerCase(),
        )
      )
        throw {
          code: 'conflict' as const,
          message: `“${label}” already has a style named “${name}” here.`,
          detail: null,
        };
      const style: VirtualStyle = {
        id: ++nextId,
        face: req.face,
        name,
        coordinates: f.axes.map((a) => ({
          tag: a.tag,
          value: given.find((c) => c.tag === a.tag)?.value ?? a.default,
        })),
        collection,
        set,
      };
      virtualStyles.push(style);
      return structuredClone(style);
    },
    listVirtualStyles: async (req) => {
      checkHolder(req.collection, req.set);
      const collection = req.collection ?? null;
      const set = req.set ?? null;
      return virtualStyles
        .filter((v) => (v.collection ?? null) === collection && (v.set ?? null) === set)
        .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()) || a.id - b.id)
        .map((v) => structuredClone(v));
    },
    deleteVirtualStyle: async (id) => {
      const i = virtualStyles.findIndex((v) => v.id === id);
      if (i < 0)
        throw {
          code: 'notFound' as const,
          message: `Virtual style ${id} was not found.`,
          detail: null,
        };
      virtualStyles.splice(i, 1);
    },
    listProblems: async () => [
      ...faces.flatMap((f): Problem[] => {
        const out: Problem[] = [];
        const health = f.face.health;
        if (health.kind !== 'ok')
          out.push({
            kind: health.kind,
            face: f.face.id,
            path: f.path,
            other: null,
            message: health.reason,
          });
        const clash = systemClash[f.face.id];
        if (clash)
          out.push({
            kind: 'systemConflict',
            face: f.face.id,
            path: f.path,
            other: clash,
            message: `${f.face.postscriptName} has the same PostScript name as a system font.`,
          });
        return out;
      }),
      ...structuredClone(options.extraProblems ?? []).filter(
        (p) => !trashed.has(p.path) && !(p.other !== null && trashed.has(p.other)),
      ),
    ],
    listDuplicates: async () => duplicates(),
    keepCopy: async (req) => {
      // The rule of `KeepCopyRequest::check`: only a copy listed with the keeper moves.
      const groups = duplicates().filter((g) => g.copies.some((c) => c.path === req.keep));
      if (!groups.length) throw invalid(`${req.keep} is not a duplicate any more.`);
      if (!req.trash.length) throw invalid('Choose at least one copy to move to the trash.');
      for (const path of req.trash)
        if (path === req.keep || !groups.some((g) => g.copies.some((c) => c.path === path)))
          throw invalid(`${path} is not a copy of the font you keep.`);
      const dryRun = req.dryRun ?? false;
      const resolution: Resolution = {
        id: dryRun ? null : ++nextId,
        kept: req.keep,
        at: '2026-10-16T09:00:00Z',
        dryRun,
        undone: false,
        moves: req.trash.map((path) => ({
          path,
          status: dryRun ? 'planned' : 'trashed',
          message: null,
        })),
      };
      if (!dryRun) {
        req.trash.forEach((p) => trashed.add(p));
        resolutions.unshift(resolution);
        emit({ type: 'libraryChanged', source: null, added: [], removed: [], updated: [] });
      }
      return structuredClone(resolution);
    },
    listResolutions: async () => structuredClone(resolutions),
    undoResolution: async (id) => {
      const r = resolutions.find((r) => r.id === id);
      if (!r)
        throw { code: 'notFound' as const, message: `Action ${id} was not found.`, detail: null };
      for (const m of r.moves)
        if (m.status === 'trashed') {
          trashed.delete(m.path);
          m.status = 'restored';
        }
      r.undone = true;
      emit({ type: 'libraryChanged', source: null, added: [], removed: [], updated: [] });
      return structuredClone(r);
    },
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
    // Browser mode: the dev server serves the built pages from public/help/.
    openHelp: async (page) => {
      if (!HELP_PAGES.some((p) => p.slug === page))
        throw {
          code: 'notFound' as const,
          message: 'This help page does not exist.',
          detail: null,
        };
      if (typeof window !== 'undefined') window.open(`/help/${page}.html`, '_blank');
    },
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
  extraProblems: mockExtraProblems,
  duplicates: mockDuplicates,
  sets: mockSets,
  extraFaces: countFromUrl('mockFaces', 100_000),
  stylesPerFamily: countFromUrl('mockStyles', EXTRA_STYLES.length) || 1,
  // `?mockCjk=1` gives face 1 a CJK-sized glyph map.
  cjkFaces: countFromUrl('mockCjk', 1) ? [1] : [],
});
