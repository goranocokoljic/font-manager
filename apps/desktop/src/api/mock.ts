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
  type Classification,
  type CssValues,
  type CssValuesRequest,
  type Clash,
  type Collection,
  type CollectionFacesRequest,
  type CollectionId,
  type Comparison,
  type ComparisonEntry,
  type ComparisonId,
  type CompareMode,
  type Conflict,
  type CoverageReport,
  type FaceCoverage,
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
  type LicenseInfo,
  type Layout,
  type LayoutBounds,
  type LayoutEntry,
  type LayoutVersion,
  type SavedLayout,
  type OpenLicense,
  type PreviewImage,
  type PreviewRequest,
  type Problem,
  type Resolution,
  type RuleGroup,
  type ProjectSet,
  type QuickCollection,
  type SearchResponse,
  type SampleText,
  type Source,
  type SourceId,
  type Tag,
  type TagId,
  type Target,
  type TextBlock,
  type VirtualStyle,
  type WebUse,
} from './types';

export const mockAppInfo: AppInfo = {
  name: 'Font Manager (mock)',
  version: '0.1.0',
  apiVersion: API_VERSION,
  platform: 'linux',
  // What fm-core says on a Linux desktop without a StatusNotifier host (lifecycle.rs).
  platformNotes: [
    'This desktop shows no tray items (no StatusNotifier host), so closing the window quits Font Manager and ends session activations. KDE Plasma and GNOME with the AppIndicator extension show the tray item.',
    'Apps that are already running can take up to 30 seconds to see a font you activate; some see it only after a restart.',
  ],
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

/**
 * A COLR color face with two CPAL palettes (E16.3), only in browser mode with
 * `?mockColor=1` (the Rust mock has no color fonts): the first named, the second
 * not, so the picker shows "Palette 2" for it.
 */
export const mockColorFace: FaceDetail = {
  ...structuredClone(mockFaces[1]!),
  face: {
    ...mockFaces[1]!.face,
    id: 8,
    family: 8,
    familyName: 'Color Play',
    postscriptName: 'ColorPlay-Regular',
  },
  path: '/Users/demo/Fonts/ColorPlay-Regular.ttf',
  hash: '8'.padStart(64, '0'),
  copyright: 'Copyright Color Play Authors',
  designer: 'Mock Designer',
  legacyFamily: 'Color Play',
  foundry: 'Mock Foundry',
  palettes: [
    {
      index: 0,
      name: 'Daylight',
      colors: ['#a00000', '#a10010', '#a20020', '#a30030', '#a40040'],
      lightBackground: true,
      darkBackground: false,
    },
    {
      index: 1,
      name: null,
      colors: ['#a0101080', '#a11010', '#a21020', '#a31030', '#a41040'],
      lightBackground: false,
      darkBackground: true,
    },
  ],
};

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
  keepInTray: true,
  // Offered on Windows, macOS and Linux (ADR 0011), off until turned on.
  startAtLogin: false,
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

/**
 * The blocks the mock's characters fall in, like the Rust mock's: name, first, last
 * and assigned code points (Unicode 17.0).
 */
const MOCK_BLOCKS: [string, number, number, number][] = [
  ['Basic Latin', 0x0000, 0x007f, 128],
  ['Latin-1 Supplement', 0x0080, 0x00ff, 128],
  ['Latin Extended-A', 0x0100, 0x017f, 128],
  ['Cyrillic', 0x0400, 0x04ff, 256],
  ['CJK Unified Ideographs', 0x4e00, 0x9fff, 20992],
  ['Hangul Syllables', 0xac00, 0xd7af, 11172],
];

/** (code, name, characters) of the presets the mock's coverage list counts, like the Rust mock's. */
const MOCK_PRESETS: [string, string, string][] = [
  ['en', 'English', 'abcdefghijklmnopqrstuvwxyz'],
  ['pl', 'Polish', 'aąbcćdeęfghijklłmnńoóprsśtuwyzźż'],
  ['sr-Latn', 'Serbian (Latin)', 'abcčćdđefghijklmnoprsštuvzž'],
  ['sr-Cyrl', 'Serbian (Cyrillic)', 'абвгдђежзијклљмнњопрстћуфхцчџш'],
];

/** CJK ideographs and Hangul syllables: 32,164 characters, for a face given `cjk`. */
const CJK = range(0x4e00, 0xa000) + range(0xac00, 0xd7a4);

const mockBlock = (cp: number): [string, number, number] => {
  const b = MOCK_BLOCKS.find(([, first, last]) => cp >= first && cp <= last);
  return b ? [b[0], b[1], b[2]] : ['No block', cp, cp];
};

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

/** What a mock face lacks of the presets `codes`, like the Rust mock's `check_coverage`. */
export function mockLanguageCoverage(f: FaceDetail, codes: string[]): CoverageReport {
  const has = mockChars(f);
  const all: string[] = [];
  const languages = codes.map((code) => {
    const preset = MOCK_PRESETS.find((p) => p[0] === code);
    if (!preset) throw invalid(`Unknown language preset "${code}".`);
    const letters = [...preset[2]];
    const missing = letters.filter((c) => !has.has(c));
    for (const c of missing) if (!all.includes(c)) all.push(c);
    return {
      code,
      covered: letters.length - missing.length,
      total: letters.length,
      missing,
      hasLocl: code === 'sr-Cyrl' && has.has('б'),
    };
  });
  return { face: f.face.id, complete: all.length === 0, missing: all, languages };
}

/**
 * A mock face's coverage list, like the Rust mock's `face_coverage`: its characters per
 * block, and per preset, complete ones first, then partial by the share covered.
 */
export function mockFaceCoverage(f: FaceDetail, cjk: boolean): FaceCoverage {
  const cps = [...mockChars(f)].map((c) => c.codePointAt(0)!);
  if (cjk) cps.push(...[...CJK].map((c) => c.codePointAt(0)!));
  const blocks = MOCK_BLOCKS.map(([name, first, last, assigned]) => ({
    name,
    first,
    last,
    covered: Math.min(cps.filter((cp) => cp >= first && cp <= last).length, assigned),
    assigned,
  })).filter((b) => b.covered > 0);
  const languages = mockLanguageCoverage(
    f,
    MOCK_PRESETS.map((p) => p[0]),
  )
    .languages.filter((l) => l.covered > 0)
    .map(({ code, covered, total, hasLocl }) => ({
      code,
      name: MOCK_PRESETS.find((p) => p[0] === code)![1],
      covered,
      total,
      hasLocl,
    }))
    .sort(
      (a, b) =>
        Number(a.covered !== a.total) - Number(b.covered !== b.total) ||
        b.covered * a.total - a.covered * b.total,
    );
  return { face: f.face.id, blocks, languages };
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
 * The rule of `fm_core::css`: axes moved from their default in the font's axis
 * order, switchable features away from their default sorted by tag, `normal`
 * for none. Tags that are not four printable ASCII characters are left out.
 */
export function mockCssValues(f: FaceDetail, req: CssValuesRequest): CssValues {
  const num = (v: number) => {
    const s = v.toFixed(3).replace(/\.?0+$/, '');
    return s === '-0' ? '0' : s;
  };
  const tag = (t: string) =>
    /^[\x20-\x7e]{4}$/.test(t) ? `"${t.replace(/["\\]/g, '\\$&')}"` : null;
  const decl = (p: string, parts: string[]) =>
    `${p}: ${parts.length ? parts.join(', ') : 'normal'};`;
  const axes = f.axes.flatMap((a) => {
    const given = req.variations?.find((v) => v.tag === a.tag)?.value;
    if (given == null || !Number.isFinite(given)) return [];
    const v = num(Math.min(a.max ?? Infinity, Math.max(a.min ?? -Infinity, given)));
    const t = tag(a.tag);
    return (a.default != null && v === num(a.default)) || !t ? [] : [`${t} ${v}`];
  });
  const features = f.features
    .filter((x) => !x.automatic)
    .flatMap((x) => {
      const on = req.features?.find((s) => s.tag === x.tag)?.on;
      const t = tag(x.tag);
      return on == null || on === x.defaultOn || !t ? [] : [[x.tag, `${t} ${on ? 1 : 0}`]];
    })
    .sort((a, b) => (a[0]! < b[0]! ? -1 : a[0]! > b[0]! ? 1 : 0))
    .map((x) => x[1]!);
  return {
    variationSettings: decl('font-variation-settings', axes),
    featureSettings: decl('font-feature-settings', features),
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
  /** Saved comparisons (none by default); a face the mock lacks reopens as missing. */
  comparisons?: MockComparison[];
  /**
   * "Start at login": `off` (the default, as on Windows, macOS and Linux) accepts
   * changes, `refuse` refuses them as an OS policy would, `unsupported` is a
   * system without a login entry (a Flatpak sandbox), where the switch is hidden.
   */
  startAtLogin?: 'off' | 'refuse' | 'unsupported';
  /** The OS `getAppInfo` reports (`linux` by default). */
  platform?: AppInfo['platform'];
  /** Adds the color face `mockColorFace` (family "Color Play", E16.3). */
  colorFace?: boolean;
}

/** A saved comparison as the mock keeps it: each face with its name when saved. */
export interface MockComparison {
  id: ComparisonId;
  name: string;
  mode: CompareMode;
  sampleText: string;
  faces: { id: FaceId; name: string }[];
}

/**
 * A sample saved comparison for browser mode: Inter, JetBrains Mono and a face the
 * library no longer has, so reopening it names one missing font.
 */
export const mockComparisons: MockComparison[] = [
  {
    id: 1,
    name: 'Headline shortlist',
    mode: 'stacked',
    sampleText: 'Quarterly results',
    faces: [
      { id: 1, name: 'Inter Regular' },
      { id: 9001, name: 'Retired Sans Bold' },
      { id: 6, name: 'JetBrains Mono Regular' },
    ],
  },
];

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
    added: MOCK_ADDED,
    styles,
    weightRange: weights.length ? { min: Math.min(...weights), max: Math.max(...weights) } : null,
    isColor: details.some((d) => (d.palettes?.length ?? 0) > 0),
    version: detail?.version ?? null,
    location: mockSources[0]!.path,
    openLicense: details.find((d) => d.license.openLicense)?.license.openLicense ?? null,
    declaredLicense: details.find((d) => d.license.declared)?.license.declared ?? null,
    tags,
    notes: detail?.familyNotes ?? null,
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

/** When every mock face was added (`FamilySummary.added`). */
const MOCK_ADDED = '2026-10-01T00:00:00Z';

/** Language presets of the mock: a face covers one when it has the preset's script. */
export const mockLanguages: LanguagePreset[] = [
  { code: 'en', name: 'English', script: 'Latn' },
  { code: 'pl', name: 'Polish', script: 'Latn' },
  { code: 'sr-Cyrl', name: 'Serbian (Cyrillic)', script: 'Cyrl' },
  { code: 'el', name: 'Greek', script: 'Grek' },
];

const OPEN_LICENSE_LABELS: Record<OpenLicense, string> = {
  ofl: 'OFL',
  apache: 'Apache 2.0',
  ufl: 'UFL',
  mit: 'MIT',
};

/**
 * The mock's copy of `fm_core::license::web_use`, like the Rust mock: declared
 * Web allows; declared Desktop, Trial or Personal-only, then restricted or
 * preview-and-print embedding, do not; an open license allows; nothing else does.
 */
export function mockWebUse(l: LicenseInfo): WebUse {
  const no = (reason: string): WebUse => ({ ok: false, reason });
  switch (l.declared) {
    case 'web':
      return { ok: true, reason: 'Declared web license' };
    case 'desktop':
      return no('Declared desktop license');
    case 'trial':
      return no('Declared trial license');
    case 'personalOnly':
      return no('Declared for personal use only');
  }
  if (l.embedding === 'restricted') return no('Embedding restricted, no web license declared');
  if (l.embedding === 'previewAndPrint') {
    return no('Embedding preview & print only, no web license declared');
  }
  if (l.openLicense)
    return { ok: true, reason: `Open license (${OPEN_LICENSE_LABELS[l.openLicense]})` };
  return no('No web license declared');
}

/**
 * The class of a face's family, like the Rust mock's families: Inter and Legacy
 * Grotesk are sans serif, JetBrains Mono is monospace; an extra family takes the
 * class of the sample face it copies.
 */
export function mockClassification(d: FaceDetail): Classification {
  const family = d.face.family;
  const model = family >= EXTRA_ID_BASE ? ((family - EXTRA_ID_BASE) % 2 === 0 ? 1 : 3) : family;
  return model === 3 ? 'mono' : 'sans';
}

/** True when `d` reaches every weight from `min` to `max` (a variable face by its `wght` axis). */
function reachesWeight(d: FaceDetail, min: number, max: number): boolean {
  const axis = d.axes.find((a) => a.tag === 'wght');
  const lo = axis?.min ?? d.face.weight;
  const hi = axis?.max ?? d.face.weight;
  return lo <= max && hi >= min;
}

/**
 * True when face `d` passes `q` (the mock's subset of `Filters`, like the Rust mock);
 * `tags` are the ids of its tags; `activated`: it was ever activated (the mock's clock
 * is fixed, so that is within any `activatedWithinDays` window).
 */
function passes(
  d: FaceDetail,
  active: boolean,
  activated: boolean,
  tags: TagId[],
  q: Filters,
): boolean {
  const f = d.face;
  const covers = (code: string) => {
    const script = mockLanguages.find((l) => l.code === code)?.script.toLowerCase();
    return !!script && d.scripts.includes(script);
  };
  const supports = q.supports;
  return (
    (!q.classification?.length || q.classification.includes(mockClassification(d))) &&
    (q.weight == null || reachesWeight(d, q.weight.min, q.weight.max)) &&
    (q.width == null || (f.width >= q.width.min && f.width <= q.width.max)) &&
    (q.foundry == null || q.foundry === d.foundry) &&
    (q.slant == null || q.slant === f.slant) &&
    (q.variable == null || q.variable === f.isVariable) &&
    (q.color == null || !q.color) &&
    (!q.formats?.length || q.formats.includes(f.format)) &&
    (q.features ?? []).every((t) => d.features.some((x) => x.tag === t)) &&
    (q.license == null || d.license.declared === q.license) &&
    (!q.sources?.length || q.sources.includes(1)) &&
    (q.active == null || q.active === active) &&
    (q.activatedWithinDays == null || activated) &&
    (q.problems == null || q.problems === (f.health.kind !== 'ok')) &&
    (q.tags ?? []).every((t) => tags.includes(t)) &&
    (q.favorite == null || q.favorite === d.favorite) &&
    (q.minRating == null || (d.rating ?? 0) >= q.minRating) &&
    (q.addedWithinDays == null ||
      Date.parse(MOCK_ADDED) >= Date.parse(MOCK_NOW) - q.addedWithinDays * 86_400_000) &&
    (q.embedding == null || q.embedding === d.license.embedding) &&
    (q.openLicense == null || q.openLicense === (d.license.openLicense != null)) &&
    (q.webOk == null || q.webOk === mockWebUse(d.license).ok) &&
    (supports?.kind !== 'languages' || supports.codes.every(covers))
  );
}

/** Folders the mock folder picker returns, one per call; `null` is a cancel. */
export const mockPickedFolders: (string | null)[] = [];

/** Files the mock file picker returns, one per call; `null` is a cancel. */
export const mockPickedFiles: (string | null)[] = [];

/** Where the mock save dialog saves, one per call; `null` is a cancel. */
export const mockPickedSaves: (string | null)[] = [];

/** What the mock "Export license inventory…" saved, oldest first. */
export const mockSavedInventories: {
  path: string;
  collection: CollectionId | null;
  rows: number;
}[] = [];

/** A mock client with its own activation, source and settings state. */
const layoutBlock = (
  text: string,
  sizePt: number,
  lineHeight: number,
  more: Partial<TextBlock> = {},
): TextBlock => ({
  text,
  sizePt,
  lineHeight,
  tracking: 0,
  paragraphSpacingPt: 0,
  firstLineIndentEm: 0,
  columns: 1,
  align: 'start',
  overflow: 'wrap',
  ...more,
});

const MOCK_BODY =
  'Typography gives language a durable visual form. It exists to honor content, and a page set with care keeps the reader reading.\n' +
  'Good type does not call attention to itself. Letters sit evenly on the line and the eye moves from one line to the next without effort.\n' +
  'A body face holds up at small sizes and over long passages. Its spacing is even and its italic follows the voice of the text.';

/** The waterfall preset's sizes in points, largest first (as `fm_core::layout`). */
export const MOCK_WATERFALL_SIZES_PT = [72, 48, 36, 24, 18, 14, 12, 9];

/** The mock's copy of the core presets (`fm_core::layout`): the same six in the same shape. */
export const MOCK_LAYOUT_PRESETS: readonly Layout[] = (
  [
    {
      preset: 'book',
      name: 'Book',
      blocks: [
        layoutBlock('Chapter One', 24, 1.2, { align: 'center', paragraphSpacingPt: 18 }),
        layoutBlock(MOCK_BODY, 11, 1.5, { firstLineIndentEm: 1.5, align: 'justify' }),
      ],
    },
    {
      preset: 'columns',
      name: 'Columns',
      blocks: [
        layoutBlock('Chapter One', 18, 1.2, { paragraphSpacingPt: 12 }),
        layoutBlock(MOCK_BODY, 10, 1.4, { paragraphSpacingPt: 6, columns: 3, align: 'justify' }),
      ],
    },
    {
      preset: 'magazine',
      name: 'Magazine',
      blocks: [
        layoutBlock('The Quiet Return of the Printed Page', 48, 1.05, {
          tracking: -20,
          paragraphSpacingPt: 12,
        }),
        layoutBlock('Readers are going back to paper.', 16, 1.35, { paragraphSpacingPt: 18 }),
        layoutBlock(MOCK_BODY, 10, 1.45, { firstLineIndentEm: 1, columns: 2, align: 'justify' }),
      ],
    },
    {
      preset: 'quote',
      name: 'Quote',
      blocks: [
        layoutBlock('“Type is a beautiful group of letters.”', 36, 1.15, {
          align: 'center',
          paragraphSpacingPt: 18,
        }),
        layoutBlock('Matthew Carter', 12, 1.3, { tracking: 100, align: 'center' }),
      ],
    },
    {
      preset: 'characters',
      name: 'Characters',
      blocks: ['ABCDEFGHIJKLMNOPQRSTUVWXYZ', 'abcdefghijklmnopqrstuvwxyz', '0123456789 &@#%?!'].map(
        (t) => layoutBlock(t, 36, 1.2, { tracking: 50, paragraphSpacingPt: 12 }),
      ),
    },
    {
      preset: 'waterfall',
      name: 'Waterfall',
      blocks: MOCK_WATERFALL_SIZES_PT.map((size) =>
        layoutBlock('The quick brown fox jumps over the lazy dog', size, 1.2, { overflow: 'clip' }),
      ),
    },
  ] as Layout[]
).map((l) => ({ ...l, widthLocked: true, widthPx: null }));

/** The ranges and steps of `fm_core::layout::bounds` (the fm-api mock has the same). */
export const MOCK_LAYOUT_BOUNDS: LayoutBounds = {
  sizePt: { min: 8, max: 600, default: 12, step: 1, perPx: 0.5 },
  lineHeight: { min: 0.1, max: 3, default: 1.3, step: 0.05, perPx: 0.01 },
  tracking: { min: -200, max: 1000, default: 0, step: 10, perPx: 2 },
  paragraphSpacingPt: { min: 0, max: 600, default: 0, step: 1, perPx: 0.5 },
  firstLineIndentEm: { min: 0, max: 20, default: 0, step: 0.5, perPx: 0.05 },
  columns: { min: 1, max: 3 },
};

/** Clamps like `fm_core::layout`; a value that is not a finite number takes the default. */
export function mockClampLayout(layout: Layout, bounds: LayoutBounds = MOCK_LAYOUT_BOUNDS): Layout {
  const fit = (
    v: number | null,
    r: { min: number | null; max: number | null; default: number | null },
  ) =>
    v !== null && Number.isFinite(v)
      ? Math.min(r.max ?? v, Math.max(r.min ?? v, v))
      : (r.default ?? 0);
  const b = bounds;
  return {
    ...layout,
    widthPx: mockWidthPx(layout.widthPx),
    blocks: layout.blocks.map((x) => ({
      ...x,
      sizePt: fit(x.sizePt, b.sizePt),
      lineHeight: fit(x.lineHeight, b.lineHeight),
      tracking: fit(x.tracking, b.tracking),
      paragraphSpacingPt: fit(x.paragraphSpacingPt, b.paragraphSpacingPt),
      firstLineIndentEm: fit(x.firstLineIndentEm, b.firstLineIndentEm),
      columns: Math.round(fit(x.columns, { ...b.columns, default: b.columns.min })),
    })),
  };
}

/** A locked width's range in CSS pixels (`fm_core::layout::WIDTH_PX`). */
export const MOCK_WIDTH_PX = { min: 160, max: 20_000 };

/** A locked width inside its range; one that is not a finite number is dropped (as the core). */
function mockWidthPx(width: number | null | undefined): number | null {
  if (width === null || width === undefined || !Number.isFinite(width)) return null;
  return Math.min(MOCK_WIDTH_PX.max, Math.max(MOCK_WIDTH_PX.min, width));
}

/**
 * `fm_core::layout::layout_width`: the locked width, else 33 em at the body block's size (the
 * block with the most text, the first on a tie; 4/3 px per point); null when unlocked.
 */
export function mockLayoutWidth(layout: Layout): number | null {
  if (layout.widthLocked === false) return null;
  const chosen = mockWidthPx(layout.widthPx);
  if (chosen !== null) return chosen;
  const body = layout.blocks.reduce<TextBlock | null>(
    (best, b) => (best && [...best.text].length >= [...b.text].length ? best : b),
    null,
  );
  const {
    min = 8,
    max = 600,
    default: fallback = 12,
  } = MOCK_LAYOUT_BOUNDS.sizePt as {
    min: number;
    max: number;
    default: number;
  };
  const size = body?.sizePt ?? fallback;
  const fitted = Number.isFinite(size) ? Math.min(max, Math.max(min, size)) : fallback;
  return mockWidthPx(((fitted * 96) / 72) * 33);
}

/** How many earlier versions of one template the mock keeps (as the core). */
export const MOCK_MAX_LAYOUT_VERSIONS = 20;
const MOCK_PRESET_IDS: string[] = MOCK_LAYOUT_PRESETS.map((l) => l.preset ?? '');

type MockLayoutContent = { layout: Layout } | { broken: string };
type MockLayoutFile = {
  id: string;
  content: MockLayoutContent;
  versions: { number: number; content: MockLayoutContent }[];
};

/** A file-name id for `name`, as the core makes it. */
function mockSlug(name: string): string {
  const slug = name
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, '-')
    .replace(/^-+|-+$/g, '');
  return slug || 'layout';
}

/**
 * The layout template files of the mock (`fm_core::layout::store`): the six presets and one
 * hand-edited file that is not valid JSON. A save that changes the file keeps what it replaces
 * as a version, the newest 20 kept.
 */
function createMockLayouts(): Pick<
  FmClient,
  | 'listLayouts'
  | 'getLayout'
  | 'saveLayout'
  | 'duplicateLayout'
  | 'revertLayout'
  | 'layoutHistory'
  | 'restoreLayout'
> {
  const files: MockLayoutFile[] = [
    ...MOCK_LAYOUT_PRESETS.map((l) => ({
      id: l.preset ?? '',
      content: { layout: structuredClone(l) },
      versions: [],
    })),
    {
      id: 'draft',
      content: { broken: 'Not valid JSON: EOF while parsing an object at line 3 column 0' },
      versions: [],
    },
  ];
  const err = (
    code: 'notFound' | 'invalidRequest' | 'conflict',
    message: string,
    detail: string | null = null,
  ) => ({ code, message, detail });
  const file = (id: string) => {
    const f = files.find((x) => x.id === id);
    if (!f) throw err('notFound', 'That layout no longer exists.');
    return f;
  };
  const content = (f: MockLayoutFile): Layout => {
    if ('broken' in f.content)
      throw err(
        'invalidRequest',
        'This layout file cannot be read. Fix it by hand or save over it.',
        f.content.broken,
      );
    return structuredClone(f.content.layout);
  };
  const presetOfId = (id: string): Layout['preset'] =>
    MOCK_LAYOUT_PRESETS.find((l) => l.preset === id)?.preset ?? null;
  const entry = (f: MockLayoutFile): LayoutEntry =>
    'broken' in f.content
      ? { id: f.id, name: f.id, preset: presetOfId(f.id), broken: f.content.broken }
      : { id: f.id, name: f.content.layout.name, preset: f.content.layout.preset, broken: null };
  const save = (id: string, raw: Layout): SavedLayout => {
    if (!id || id.startsWith('.') || /[/\\:]/.test(id))
      throw err('invalidRequest', 'That is not a layout name the app can use.');
    if (!raw.name.trim()) throw err('invalidRequest', 'A layout needs a name.');
    const layout = mockClampLayout(raw);
    const f = files.find((x) => x.id === id);
    if (!f) {
      files.push({ id, content: { layout }, versions: [] });
    } else if (
      !('layout' in f.content) ||
      JSON.stringify(f.content.layout) !== JSON.stringify(layout)
    ) {
      const number = (f.versions.at(-1)?.number ?? 0) + 1;
      f.versions.push({ number, content: f.content });
      f.content = { layout };
      f.versions.splice(0, Math.max(0, f.versions.length - MOCK_MAX_LAYOUT_VERSIONS));
    }
    return { id, layout: structuredClone(layout) };
  };
  const rank = (e: LayoutEntry) => {
    const i = MOCK_PRESET_IDS.indexOf(e.id);
    return i < 0 ? MOCK_PRESET_IDS.length : i;
  };
  return {
    listLayouts: async () =>
      files
        .map(entry)
        .sort(
          (a, b) =>
            rank(a) - rank(b) ||
            a.name.toLowerCase().localeCompare(b.name.toLowerCase()) ||
            a.id.localeCompare(b.id),
        ),
    getLayout: async ({ id }) => ({ id, layout: content(file(id)) }),
    saveLayout: async ({ id, layout }) => save(id, layout),
    duplicateLayout: async ({ id, name }) => {
      const source = content(file(id));
      const taken = files.map((f) => entry(f).name.toLowerCase());
      let copyName = name?.trim() ?? '';
      if (copyName && taken.includes(copyName.toLowerCase()))
        throw err('conflict', `There is already a layout named “${copyName}”.`);
      for (let i = 1; !copyName; i++) {
        const n = i === 1 ? `${source.name} copy` : `${source.name} copy ${i}`;
        if (!taken.includes(n.toLowerCase())) copyName = n;
      }
      const base = mockSlug(copyName);
      let newId = base;
      for (let i = 2; files.some((f) => f.id === newId); i++) newId = `${base}-${i}`;
      return save(newId, { ...source, name: copyName });
    },
    revertLayout: async ({ id, toOriginal }) => {
      const f = file(id);
      if (!toOriginal) return { id, layout: content(f) };
      const preset = 'layout' in f.content ? f.content.layout.preset : presetOfId(id);
      const original = MOCK_LAYOUT_PRESETS.find((l) => l.preset === preset);
      if (!preset || !original)
        throw err(
          'invalidRequest',
          'Only a layout that started from a preset can be reset to its original.',
        );
      const name = 'layout' in f.content ? f.content.layout.name : original.name;
      return save(id, { ...structuredClone(original), name });
    },
    layoutHistory: async ({ id }): Promise<LayoutVersion[]> =>
      [...file(id).versions].reverse().map((v) => ({
        number: v.number,
        savedAt: '2026-10-16T09:00:00Z',
        name: 'layout' in v.content ? v.content.layout.name : null,
        broken: 'broken' in v.content ? v.content.broken : null,
      })),
    restoreLayout: async ({ id, number }) => {
      const v = file(id).versions.find((x) => x.number === number);
      if (!v) throw err('notFound', 'That version is no longer kept.');
      if ('broken' in v.content)
        throw err('invalidRequest', 'This version cannot be restored: it is not a valid layout.');
      return save(id, structuredClone(v.content.layout));
    },
  };
}

export function createMockClient(options: MockOptions = {}): FmClient {
  const active = new Map<FaceId, ActivationMode>();
  /** Faces ever activated, active now or not. */
  const everActivated = new Set<FaceId>();
  const sources = structuredClone(mockSources);
  const sets = structuredClone(options.sets ?? []);
  const handlers = new Set<(event: Event) => void>();
  let nextId = 100;
  let picks = 0;
  let settings: FullSettings = {
    ...defaultSettings,
    startAtLogin: options.startAtLogin === 'unsupported' ? null : false,
  };
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
    ...(options.colorFace ? [structuredClone(mockColorFace)] : []),
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
  // The Quick Collection (E13.3), first added first, like the Rust mock.
  let quick: FaceId[] = [];
  const quickView = (): QuickCollection => ({
    faces: quick.flatMap((id) => {
      const f = find(id);
      return f && !trashed.has(f.path) ? [summary(f)] : [];
    }),
  });
  // Named comparisons (E13.2), like the Rust mock: a face it lacks (or trashed) is missing.
  let comparisons = structuredClone(options.comparisons ?? []);
  const comparisonEntry = (c: MockComparison): ComparisonEntry => ({
    id: c.id,
    name: c.name,
    mode: c.mode,
    faceCount: c.faces.length,
  });
  const comparisonOrThrow = (id: ComparisonId) => {
    const c = comparisons.find((x) => x.id === id);
    if (!c) throw { code: 'notFound', message: `Comparison ${id} was not found.`, detail: null };
    return c;
  };
  const comparisonView = (c: MockComparison): Comparison => {
    const shown = c.faces.map((x) => ({ ...x, f: find(x.id) }));
    return {
      id: c.id,
      name: c.name,
      mode: c.mode,
      sampleText: c.sampleText,
      faces: shown.flatMap(({ f }) => (f && !trashed.has(f.path) ? [summary(f)] : [])),
      missing: shown
        .filter(({ f }) => !f || trashed.has(f.path))
        .map(({ id, name }) => ({ id, name })),
    };
  };
  const comparisonName = (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) throw invalid('A comparison needs a name.');
    return trimmed;
  };
  // Family groups a search returns: name matches first, then by name (like the Rust mock).
  const faceHits = (words: string[], d: FaceDetail, q: Filters, rule?: RuleGroup | null) => {
    const hay =
      `${d.face.familyName} ${d.face.style} ${d.face.postscriptName} ${d.foundry ?? ''} ${d.designer ?? ''}`.toLowerCase();
    return (
      words.every((w) => hay.includes(w)) &&
      matches(d, q, []) &&
      (!rule || passesGroup(d, rule, []))
    );
  };
  const wordsOf = (query: string) => query.toLowerCase().split(/\s+/).filter(Boolean);
  const hits = (query: string, q: Filters, rule?: RuleGroup | null): FamilyHit[] => {
    const words = wordsOf(query);
    const out: FamilyHit[] = [];
    for (const [family, details] of byFamily) {
      const matching = details.filter((d) => faceHits(words, d, q, rule));
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
  // The number of family groups `hits` returns, without building their rows: the
  // counts run it once per filter value, so a 10,000-family list stays quick.
  const hitCount = (query: string, q: Filters, rule?: RuleGroup | null): number => {
    const words = wordsOf(query);
    let n = 0;
    for (const details of byFamily.values())
      if (details.some((d) => faceHits(words, d, q, rule))) n++;
    return n;
  };
  // Per value, the family groups with that value chosen and the other filters kept.
  const countEach = <T>(
    query: string,
    q: Filters,
    values: T[],
    pick: (f: Filters, v: T) => Filters,
    rule?: RuleGroup | null,
  ): FilterCount<T>[] =>
    values
      .map((value) => ({ value, count: hitCount(query, pick(q, value), rule) }))
      .filter((c) => c.count > 0);
  const distinct = <T>(list: T[]) => [...new Set(list)];
  const both = [true, false];
  const counts = (query: string, q: Filters, rule?: RuleGroup | null): FilterCounts => ({
    classification: countEach(
      query,
      q,
      distinct(faces.map(mockClassification)),
      (f, v) => ({ ...f, classification: [v] }),
      rule,
    ),
    weight: countEach(
      query,
      q,
      [100, 200, 300, 400, 500, 600, 700, 800, 900],
      (f, v) => ({ ...f, weight: { min: v - 50, max: v + 49 } }),
      rule,
    ),
    width: countEach(
      query,
      q,
      [1, 2, 3, 4, 5, 6, 7, 8, 9],
      (f, v) => ({ ...f, width: { min: v, max: v } }),
      rule,
    ),
    license: countEach(
      query,
      q,
      distinct(faces.flatMap((d) => (d.license.declared ? [d.license.declared] : []))),
      (f, license) => ({ ...f, license }),
      rule,
    ),
    foundry: countEach(
      query,
      q,
      distinct(faces.flatMap((d) => (d.foundry ? [d.foundry] : []))).sort(),
      (f, foundry) => ({ ...f, foundry }),
      rule,
    ),
    slant: countEach(
      query,
      q,
      distinct(faces.map((d) => d.face.slant)),
      (f, slant) => ({ ...f, slant }),
      rule,
    ),
    variable: countEach(query, q, both, (f, variable) => ({ ...f, variable }), rule),
    color: countEach(query, q, both, (f, color) => ({ ...f, color }), rule),
    formats: countEach(
      query,
      q,
      distinct(faces.map((d) => d.face.format)),
      (f, v) => ({ ...f, formats: [v] }),
      rule,
    ),
    features: countEach(
      query,
      q,
      distinct(faces.flatMap((d) => d.features.map((x) => x.tag))).sort(),
      (f, v) => ({ ...f, features: distinct([...(f.features ?? []), v]) }),
      rule,
    ),
    sources: countEach(query, q, [1], (f, v) => ({ ...f, sources: [v] }), rule),
    problems: countEach(query, q, both, (f, problems) => ({ ...f, problems }), rule),
    active: countEach(query, q, both, (f, v) => ({ ...f, active: v }), rule),
    tags: countEach(
      query,
      q,
      tags.map((t) => t.id),
      (f, v) => ({ ...f, tags: distinct([...(f.tags ?? []), v]) }),
      rule,
    ),
    favorite: countEach(query, q, both, (f, favorite) => ({ ...f, favorite }), rule),
    minRating: countEach(query, q, [1, 2, 3, 4, 5], (f, minRating) => ({ ...f, minRating }), rule),
    embedding: countEach(
      query,
      q,
      distinct(faces.map((d) => d.license.embedding)),
      (f, embedding) => ({ ...f, embedding }),
      rule,
    ),
    openLicense: countEach(query, q, both, (f, openLicense) => ({ ...f, openLicense }), rule),
    webOk: countEach(query, q, both, (f, webOk) => ({ ...f, webOk }), rule),
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
  // Smart collections (E14.2), like fm-core: the rule runs on every ask, never stored;
  // a rule that leads back to its own collection is `invalidRequest`.
  const ruleOf = (c: Collection): RuleGroup | null =>
    c.ruleGroup ?? (c.rules ? { op: 'and', rules: [{ kind: 'filters', filters: c.rules }] } : null);
  const refsOf = (g: RuleGroup): CollectionId[] =>
    (g.rules ?? []).flatMap((r) =>
      r.kind === 'group'
        ? refsOf(r.group)
        : r.filters.collection != null
          ? [r.filters.collection]
          : [],
    );
  // Refuses a rule as fm-core does (E14.1): an empty group at any depth, or a
  // weight or width range whose minimum is above its maximum.
  const checkShape = (g: RuleGroup): void => {
    if (!g.rules?.length)
      throw invalid("Every group of a smart collection's rules needs at least one rule.");
    for (const r of g.rules) {
      if (r.kind === 'group') checkShape(r.group);
      else if (r.filters.weight && r.filters.weight.min > r.filters.weight.max)
        throw invalid('The lowest weight of a rule is above its highest.');
      else if (r.filters.width && r.filters.width.min > r.filters.width.max)
        throw invalid('The narrowest width of a rule is above its widest.');
    }
  };
  const loop = (c: Collection) =>
    invalid(`The rules of "${c.name}" include the collection itself.`);
  const inCollection = (d: FaceDetail, id: CollectionId, path: CollectionId[]): boolean => {
    const c = collections.find((x) => x.id === id);
    if (!c) return false;
    const rule = ruleOf(c);
    if (!rule) return !!members.get(id)?.has(d.face.id);
    if (path.includes(id)) throw loop(c);
    return passesGroup(d, rule, [...path, id]);
  };
  const passesGroup = (d: FaceDetail, g: RuleGroup, path: CollectionId[]): boolean => {
    const results = (g.rules ?? []).map((r) =>
      r.kind === 'group' ? passesGroup(d, r.group, path) : matches(d, r.filters, path),
    );
    return g.op === 'and' ? results.every(Boolean) : !results.length || results.some(Boolean);
  };
  const matches = (d: FaceDetail, q: Filters, path: CollectionId[]): boolean =>
    passes(d, active.has(d.face.id), everActivated.has(d.face.id), tagIdsOf(d), q) &&
    (q.collection == null || inCollection(d, q.collection, path));
  // Refuses a stored rule of `id` that reaches a collection twice on one path.
  const checkLoops = (id: CollectionId, path: CollectionId[]) => {
    const c = collections.find((x) => x.id === id);
    const rule = c && ruleOf(c);
    if (!c || !rule) return;
    if (path.includes(id)) throw loop(c);
    for (const ref of refsOf(rule)) checkLoops(ref, [...path, id]);
  };
  const viewOf = (c: Collection): Collection => {
    let faceCount = members.get(c.id)?.size ?? 0;
    if (ruleOf(c)) {
      try {
        faceCount = [...byFamily.values()].flat().filter((d) => inCollection(d, c.id, [])).length;
      } catch {
        faceCount = 0; // a rule that loops holds nothing
      }
    }
    return { ...structuredClone(c), faceCount };
  };
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
  // The faces and every face of whole families, each once, the faces first (like fm-core).
  const facesOf = (ids: FaceId[], families: FamilyId[] = []): FaceId[] => {
    for (const id of ids) if (!find(id)) throw notFound(id);
    const out = new Set(ids);
    for (const family of families) {
      const list = byFamily.get(family);
      if (!list)
        throw {
          code: 'notFound' as const,
          message: `Family ${family} was not found.`,
          detail: null,
        };
      for (const d of list) out.add(d.face.id);
    }
    return [...out];
  };
  const editMembers = (req: CollectionFacesRequest, add: boolean): Collection => {
    const c = findCollection(req.collection);
    if (c.rules) throw invalid('A smart collection fills itself from its rules.');
    const ids = facesOf(req.faces, req.families);
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
    getAppInfo: async () =>
      options.platform ? { ...mockAppInfo, platform: options.platform } : mockAppInfo,
    layoutPresets: async () => structuredClone([...MOCK_LAYOUT_PRESETS]),
    clampLayout: async (req) => mockClampLayout(req),
    layoutBounds: async () => structuredClone(MOCK_LAYOUT_BOUNDS),
    layoutWidth: async (req) => mockLayoutWidth(req),
    ...createMockLayouts(),
    cssValues: async (req) => {
      const f = find(req.face);
      if (!f) throw notFound(req.face);
      return mockCssValues(f, req);
    },
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
      detail.license.webUse = mockWebUse(detail.license);
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
      const rule = req.rule ?? null;
      const all = hits(query, filters, rule);
      const offset = req.page?.offset ?? 0;
      const limit = req.page?.limit ?? 100;
      return {
        families: all.slice(offset, offset + limit),
        total: all.length,
        counts: counts(query, filters, rule),
      } satisfies SearchResponse;
    },
    checkCoverage: async (req) => {
      const f = find(req.face);
      if (!f) throw notFound(req.face);
      if (req.target.kind === 'languages') return mockLanguageCoverage(f, req.target.codes);
      return mockCoverage(f, req.target.text, options.cjkFaces?.includes(req.face) ?? false);
    },
    faceCoverage: async (face) => {
      const f = find(face);
      if (!f) throw notFound(face);
      return mockFaceCoverage(f, options.cjkFaces?.includes(face) ?? false);
    },
    listLanguages: async () => structuredClone(mockLanguages),
    createCollection: async (req) => {
      const name = req.name.trim();
      if (!name) throw invalid('Give the collection a name.');
      if (req.parent != null) findCollection(req.parent);
      if (nameTaken(req.parent ?? null, name)) throw conflict(name);
      if (req.rules && req.ruleGroup)
        throw invalid('Give a smart collection rules or a rule group, not both.');
      const group: RuleGroup | null =
        req.ruleGroup ??
        (req.rules ? { op: 'and', rules: [{ kind: 'filters', filters: req.rules }] } : null);
      if (group) checkShape(group);
      const id = ++nextId;
      for (const ref of group ? refsOf(group) : []) if (ref !== id) findCollection(ref);
      const only = group?.op === 'and' && group.rules?.length === 1 ? group.rules[0] : null;
      const c: Collection = {
        id,
        parent: req.parent ?? null,
        name,
        rules: group ? (only?.kind === 'filters' ? only.filters : {}) : null,
        ruleGroup: group,
        faceCount: 0,
      };
      collections.push(c);
      try {
        checkLoops(id, []);
      } catch (e) {
        collections.pop();
        throw e;
      }
      return viewOf(c);
    },
    listCollections: async () => walk(null),
    updateCollectionRules: async (req) => {
      checkShape(req.ruleGroup);
      const c = findCollection(req.collection);
      if (!ruleOf(c))
        throw invalid('A manual collection has no rules; make a smart collection instead.');
      for (const ref of refsOf(req.ruleGroup)) if (ref !== c.id) findCollection(ref);
      const before = { rules: c.rules, ruleGroup: c.ruleGroup };
      const only = req.ruleGroup.op === 'and' && req.ruleGroup.rules?.length === 1;
      const first = only ? req.ruleGroup.rules![0] : null;
      c.ruleGroup = structuredClone(req.ruleGroup);
      c.rules = first?.kind === 'filters' ? structuredClone(first.filters) : {};
      try {
        checkLoops(c.id, []);
      } catch (e) {
        Object.assign(c, before);
        throw e;
      }
      return viewOf(c);
    },
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
      const ids = facesOf(req.faces, req.families);
      if (!ids.length) throw invalid('Choose the fonts to tag.');
      const targets = ids.map(faceOrThrow);
      const named: { id: TagId; name: string }[] = [];
      for (const name of names) {
        let t = tags.find((x) => sameName(x.name, name));
        if (!t) tags.push((t = { id: ++nextId, name }));
        for (const d of targets)
          if (!d.tags.some((n) => sameName(n, t.name))) d.tags = [...d.tags, t.name];
        if (!named.some((x) => x.id === t.id)) named.push(t);
      }
      changed(ids);
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
    // Like fm-core: the tags at least one face carries, A to Z, and whether all do.
    selectionTags: async (req) => {
      const ids = facesOf(req.faces, req.families);
      const targets = ids.map(faceOrThrow);
      const byName = [...tags].sort((a, b) =>
        a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
      );
      return {
        faces: ids,
        tags: byName.flatMap((t) => {
          const n = targets.filter((d) => d.tags.some((x) => sameName(x, t.name))).length;
          return n ? [{ tag: t.id, faceCount: n, all: n === ids.length }] : [];
        }),
      };
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
    // The family's notes live on each of its faces' details here.
    updateFamilyNotes: async (req) => {
      const details = byFamily.get(req.family);
      if (!details)
        throw {
          code: 'notFound' as const,
          message: `Family ${req.family} was not found.`,
          detail: null,
        };
      for (const d of details) d.familyNotes = req.notes || null;
      changed(details.map((d) => d.face.id));
    },
    // No file system: the save is recorded in `mockSavedInventories`, one row per family
    // the collection's search finds, as fm-core lists them.
    saveLicenseInventory: async (req, fileName) => {
      const collection = req.collection ?? null;
      if (collection != null) findCollection(collection);
      const path = mockPickedSaves.length
        ? mockPickedSaves.shift()!
        : `/Users/demo/Documents/${fileName}`;
      if (path === null) return null;
      const rows = hits('', { collection }).length;
      mockSavedInventories.push({ path, collection, rows });
      return rows;
    },
    // Like the Rust mock: no file system, an empty path is a missing file and
    // the copy is a made-up path in the chosen place.
    declareLicense: async (req) => {
      const details = byFamily.get(req.family);
      if (!details)
        throw {
          code: 'notFound' as const,
          message: `Family ${req.family} was not found.`,
          detail: null,
        };
      if (req.seats != null && req.seats < 0)
        throw invalid('The number of seats cannot be below 0.');
      const date = req.purchaseDate || null;
      if (date !== null && !/^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/.test(date))
        throw invalid(`"${date}" is not a date. Write the purchase date as YYYY-MM-DD.`);
      const change = req.receipt ?? { kind: 'keep' };
      if (change.kind === 'attach' && !change.path) throw invalid('The receipt file is not there.');
      for (const d of details) {
        d.license.declared = req.licenseType ?? null;
        d.license.seats = req.seats ?? null;
        d.license.purchaseDate = date;
        if (change.kind === 'clear') d.license.receipt = null;
        if (change.kind === 'attach') {
          const name = change.path.split(/[\\/]/).pop();
          d.license.receipt =
            change.place === 'nextToFont'
              ? `/fonts/.fontlibrary/receipts/${name}`
              : `/app-data/receipts/${name}`;
        }
      }
      changed(details.map((d) => d.face.id));
    },
    quickCollection: async () => quickView(),
    addToQuickCollection: async (req) => {
      req.faces.forEach(faceOrThrow);
      for (const id of req.faces) if (!quick.includes(id)) quick.push(id);
      return quickView();
    },
    removeFromQuickCollection: async (req) => {
      quick = quick.filter((id) => !req.faces.includes(id));
      return quickView();
    },
    clearQuickCollection: async () => {
      quick = [];
    },
    listComparisons: async () =>
      comparisons
        .map(comparisonEntry)
        .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()) || a.id - b.id),
    getComparison: async ({ id }) => comparisonView(comparisonOrThrow(id)),
    saveComparison: async (req) => {
      const name = comparisonName(req.name);
      if (req.faces.length < 2 || req.faces.length > 4)
        throw invalid(`A comparison holds 2 to 4 fonts, not ${req.faces.length}.`);
      if (new Set(req.faces).size !== req.faces.length)
        throw invalid('A comparison names each font once.');
      const faces = req.faces.map((id) => {
        const f = faceOrThrow(id);
        return { id, name: `${f.face.familyName} ${f.face.style}` };
      });
      const saved: MockComparison = {
        id: req.id ?? ++nextId,
        name,
        mode: req.mode ?? 'sideBySide',
        sampleText: req.sampleText ?? '',
        faces,
      };
      if (req.id != null) {
        const at = comparisons.indexOf(comparisonOrThrow(req.id));
        comparisons[at] = saved;
      } else comparisons.push(saved);
      return comparisonView(saved);
    },
    renameComparison: async ({ id, name }) => {
      const c = comparisonOrThrow(id);
      c.name = comparisonName(name);
      return comparisonEntry(c);
    },
    deleteComparison: async ({ id }) => {
      comparisonOrThrow(id);
      comparisons = comparisons.filter((c) => c.id !== id);
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
        everActivated.add(id);
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
    pickFile: async () =>
      mockPickedFiles.length
        ? mockPickedFiles.shift()!
        : `/Users/demo/Downloads/receipt-${++picks}.pdf`,
    getSettings: async () => ({ ...settings }),
    // The same ranges as `Settings::apply` in fm-api.
    updateSettings: async (req) => {
      const mb = req.cacheLimitMb;
      if (mb != null && (mb < 64 || mb > 16384))
        throw invalid('The cache size must be between 64 and 16384 MB.');
      const min = req.rescanIntervalMinutes;
      if (min != null && min > 1440)
        throw invalid('The rescan interval must be between 0 (off) and 1440 minutes.');
      // The same rule as `Settings::apply` and the login entry in fm-core.
      if (req.startAtLogin != null && settings.startAtLogin == null)
        throw {
          code: 'unsupported' as const,
          message: 'Start at login is not available on this system.',
          detail: null,
        };
      if (
        req.startAtLogin != null &&
        req.startAtLogin !== settings.startAtLogin &&
        options.startAtLogin === 'refuse'
      )
        throw {
          code: 'permissionDenied' as const,
          message: req.startAtLogin
            ? 'Start at login could not be turned on: the system refused the login entry.'
            : 'Start at login could not be turned off: the system refused to remove the login entry.',
          detail: 'blocked by policy',
        };
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

function platformFromUrl(): AppInfo['platform'] | undefined {
  if (typeof location === 'undefined') return undefined;
  const p = new URLSearchParams(location.search).get('mockPlatform');
  return p === 'windows' || p === 'macos' || p === 'linux' ? p : undefined;
}

// Browser mode (e2e) steps scans slowly enough to see the inline progress.
export const mockClient: FmClient = createMockClient({
  scanStepMs: 150,
  extraProblems: mockExtraProblems,
  duplicates: mockDuplicates,
  sets: mockSets,
  // `?mockComparisons=1` starts with a saved comparison that names a missing font.
  comparisons: countFromUrl('mockComparisons', 1) ? mockComparisons : [],
  extraFaces: countFromUrl('mockFaces', 100_000),
  stylesPerFamily: countFromUrl('mockStyles', EXTRA_STYLES.length) || 1,
  // `?mockCjk=1` gives face 1 a CJK-sized glyph map.
  cjkFaces: countFromUrl('mockCjk', 1) ? [1] : [],
  // "Start at login" is offered; `?mockStartAtLogin=2` refuses changes, `=3` hides it.
  startAtLogin: (['off', 'refuse', 'unsupported'] as const)[
    countFromUrl('mockStartAtLogin', 3) - 1
  ],
  // `?mockPlatform=windows|macos|linux` sets the OS the app reports.
  platform: platformFromUrl(),
  // `?mockColor=1` adds a color font with two palettes.
  colorFace: countFromUrl('mockColor', 1) > 0,
});
