// The chrome's colors meet WCAG AA in both themes: text 4.5:1, control outlines
// and focus rings 3:1. Reads the palettes from the style sheets, so a color
// change that breaks contrast fails here.
import { describe, expect, it } from 'vitest';
import { COLLECTION_COLORS } from '../api/collectionColors';
import graphiteTheme from '../../../../data/themes/graphite.json';
import indigoTheme from '../../../../data/themes/indigo.json';
import mossTheme from '../../../../data/themes/moss.json';
import paperTheme from '../../../../data/themes/paper.json';
import theme from './theme.css?raw';
import print from './print.css?raw';
import compareSheet from '../components/CompareSheet.vue?raw';
import glyphCompare from '../components/GlyphCompare.vue?raw';

type Palette = Record<string, string>;

/** The `--fm-*` colors declared in the first block opened by `selector`. */
function palette(css: string, selector: string): Palette {
  const start = css.indexOf(`${selector} {`);
  expect(start, selector).toBeGreaterThanOrEqual(0);
  const body = css.slice(start, css.indexOf('}', start));
  return Object.fromEntries(
    [...body.matchAll(/--fm-([a-z0-9-]+):\s*(#[0-9a-f]{6})/gi)].map((m) => [
      m[1]!,
      m[2]!.toLowerCase(),
    ]),
  );
}

const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));

function luminance([r, g, b]: number[]): number {
  const lin = (v: number) => {
    const c = v / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r!) + 0.7152 * lin(g!) + 0.0722 * lin(b!);
}

function contrast(a: number[], b: number[]): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
}

/** `color-mix(in srgb, top p, transparent)` drawn over `under`. */
const tint = (top: string, p: number, under: string) =>
  rgb(top).map((v, i) => v * p + rgb(under)[i]! * (1 - p));

const light = palette(theme, ':root');
const dark = palette(theme, ":root[data-theme='dark']");
// The accent is not a text color: words on it are `on-accent`, the rest `fg` (DESIGN.md).
const TEXT = ['fg', 'muted', 'danger', 'success', 'warning'];

describe('the accent pair (E40.3)', () => {
  it.each([
    ['light', light, '#c56eb2', '#141413'],
    ['dark', dark, '#ab4b96', '#ffffff'],
  ])('%s is the owner’s colors', (_, p, accent, onAccent) => {
    expect(p.accent).toBe(accent);
    expect(p['on-accent']).toBe(onAccent);
  });
});

describe.each([
  ['light', light],
  ['dark', dark],
])('%s theme', (_, p) => {
  it('has every color the checks need', () => {
    for (const k of [
      ...TEXT,
      'accent',
      'on-accent',
      'bg',
      'surface',
      'surface-2',
      'control-border',
    ])
      expect(p[k], k).toBeDefined();
  });

  it.each(TEXT)('%s text is at least 4.5:1 on the three surfaces and the selected row', (k) => {
    const fg = rgb(p[k]!);
    const backgrounds = {
      bg: rgb(p.bg!),
      surface: rgb(p.surface!),
      'surface-2': rgb(p['surface-2']!),
      selected: tint(p.accent!, 0.16, p.bg!),
    };
    for (const [name, bg] of Object.entries(backgrounds))
      expect(contrast(fg, bg), `${k} on ${name}`).toBeGreaterThanOrEqual(4.5);
  });

  it('text on the accent is at least 4.5:1', () => {
    expect(contrast(rgb(p['on-accent']!), rgb(p.accent!))).toBeGreaterThanOrEqual(4.5);
  });

  it('text on the accent tint of a pressed button is at least 4.5:1', () => {
    for (const under of [p.bg!, p.surface!, p['surface-2']!])
      expect(contrast(rgb(p.fg!), tint(p.accent!, 0.16, under))).toBeGreaterThanOrEqual(4.5);
  });

  it('control outlines and the focus ring are at least 3:1', () => {
    for (const bg of [p.bg!, p.surface!, p['surface-2']!]) {
      expect(contrast(rgb(p['control-border']!), rgb(bg))).toBeGreaterThanOrEqual(3);
      expect(contrast(rgb(p.accent!), rgb(bg))).toBeGreaterThanOrEqual(3);
    }
  });
});

describe('text the browser colors by default', () => {
  it('placeholders use the muted color at full opacity', () => {
    // `muted` is in the checked text colors above; the browser default is not.
    const start = theme.indexOf('::placeholder {');
    expect(start).toBeGreaterThanOrEqual(0);
    const rule = theme.slice(start, theme.indexOf('}', start));
    expect(rule).toMatch(/\bcolor:\s*var\(--fm-muted\)/);
    expect(rule).toMatch(/\bopacity:\s*1\b/);
  });
});

describe('palettes stay in step', () => {
  it('the OS dark mode uses the dark palette', () => {
    const media = theme.slice(theme.indexOf('@media (prefers-color-scheme: dark)'));
    expect(palette(media, ":root:not([data-theme='light'])")).toEqual(dark);
  });

  it.each([
    ['.surface-light', light],
    ['.surface-dark', dark],
  ])('the list surface %s uses the theme colors', (selector, p) => {
    for (const [k, v] of Object.entries(palette(theme, selector))) expect(v, k).toBe(p[k]);
  });
});

// E19.1: every chrome color token is checked, and the tokens declared outside
// theme.css (Compare's two colors, the print palette) meet contrast too.

/** Colors with no contrast duty: row and panel separators (WCAG 1.4.11 exempts them). */
const DECORATIVE = ['border'];
const COLLECTION = COLLECTION_COLORS.map((key) => `collection-${key}`);
const CHECKED = [
  ...TEXT,
  ...COLLECTION,
  'accent',
  'on-accent',
  'bg',
  'surface',
  'surface-2',
  'control-border',
];

describe('every color token', () => {
  it.each([
    ['light', light],
    ['dark', dark],
  ])('of the %s palette is checked or declared decorative', (_, p) => {
    for (const k of Object.keys(p)) expect([...CHECKED, ...DECORATIVE], k).toContain(k);
  });
});

/** The `--fm-compare-*` colors a component declares. */
function compareColors(css: string): string[] {
  return [...css.matchAll(/--fm-compare-\d:\s*(#[0-9a-f]{6})/gi)].map((m) => m[1]!.toLowerCase());
}

// E44.3: a dot on the sidebar and a swatch in a popover; the dot is the only mark of the colour.
describe('collection colors', () => {
  it.each([
    ['light', light],
    ['dark', dark],
  ])('are all there and at least 3:1 on the %s surfaces', (_, p) => {
    for (const k of COLLECTION) {
      expect(p[k], k).toBeDefined();
      for (const bg of [p.bg!, p.surface!, p['surface-2']!])
        expect(contrast(rgb(p[k]!), rgb(bg)), `${k} on ${bg}`).toBeGreaterThanOrEqual(3);
    }
  });

  it.each([
    ['light', light],
    ['dark', dark],
  ])('leave the text of the %s strip at least 4.5:1 on their 10 % tint', (_, p) => {
    for (const k of COLLECTION)
      expect(contrast(rgb(p.fg!), tint(p[k]!, 0.1, p.bg!)), k).toBeGreaterThanOrEqual(4.5);
  });
});

// E47.1: the named themes replace six tokens and keep the rest of `theme.css`, so the colors
// they keep must stay legible on the surfaces they bring.
describe.each(
  [paperTheme, graphiteTheme, indigoTheme, mossTheme].flatMap((t) => [
    [t.id, 'light', t.light, light],
    [t.id, 'dark', t.dark, dark],
  ]) as [string, string, typeof paperTheme.light, Palette][],
)('theme %s, %s', (_, __, tokens, base) => {
  const surfaces = [tokens.bg, tokens.surface, tokens['surface-2']];

  it('has text at least 4.5:1 on the accent', () => {
    expect(contrast(rgb(tokens['on-accent']), rgb(tokens.accent))).toBeGreaterThanOrEqual(4.5);
  });

  it('has a focus ring and control outlines at least 3:1 on the surfaces', () => {
    for (const bg of surfaces) {
      expect(contrast(rgb(tokens.accent), rgb(bg)), bg).toBeGreaterThanOrEqual(3);
      expect(contrast(rgb(base['control-border']!), rgb(bg)), bg).toBeGreaterThanOrEqual(3);
    }
  });

  it.each(TEXT)('keeps %s text at least 4.5:1 on the surfaces and the selected row', (k) => {
    const fg = rgb(base[k]!);
    for (const bg of surfaces) expect(contrast(fg, rgb(bg)), bg).toBeGreaterThanOrEqual(4.5);
    expect(contrast(fg, tint(tokens.accent, 0.16, tokens.bg))).toBeGreaterThanOrEqual(4.5);
  });

  it('keeps text at least 4.5:1 on the accent tint of a pressed button', () => {
    for (const under of surfaces)
      expect(contrast(rgb(base.fg!), tint(tokens.accent, 0.16, under))).toBeGreaterThanOrEqual(4.5);
  });

  it('keeps the collection colors at least 3:1 and their strip text at least 4.5:1', () => {
    for (const k of COLLECTION) {
      for (const bg of surfaces)
        expect(contrast(rgb(base[k]!), rgb(bg)), `${k} on ${bg}`).toBeGreaterThanOrEqual(3);
      expect(contrast(rgb(base.fg!), tint(base[k]!, 0.1, tokens.bg)), k).toBeGreaterThanOrEqual(
        4.5,
      );
    }
  });

  it('keeps the Compare colors at least 3:1 on the background and surface', () => {
    for (const c of compareColors(compareSheet))
      for (const bg of [tokens.bg, tokens.surface])
        expect(contrast(rgb(c), rgb(bg)), `${c} on ${bg}`).toBeGreaterThanOrEqual(3);
  });
});

describe('the Paper theme', () => {
  /** `rgb(0 0 0 / 9%)` as `#00000017`, the form the theme files use. */
  function hairline(css: string, selector: string): string {
    const start = css.indexOf(`${selector} {`);
    const body = css.slice(start, css.indexOf('}', start));
    const m = /--fm-border:\s*rgb\((\d+) (\d+) (\d+) \/ (\d+)%\)/.exec(body);
    expect(m, selector).not.toBeNull();
    const hex = (v: number) => Math.round(v).toString(16).padStart(2, '0');
    const [r, g, b] = [m![1]!, m![2]!, m![3]!].map((v) => hex(Number(v)));
    return `#${r}${g}${b}${hex((Number(m![4]) / 100) * 255)}`;
  }

  it.each([
    ['light', paperTheme.light, light, ':root'],
    ['dark', paperTheme.dark, dark, ":root[data-theme='dark']"],
  ] as const)('is the %s palette of theme.css', (_, tokens, p, selector) => {
    for (const k of ['accent', 'on-accent', 'bg', 'surface', 'surface-2'] as const)
      expect(tokens[k], k).toBe(p[k]);
    expect(tokens.border).toBe(hairline(theme, selector));
  });
});

describe('Compare colors', () => {
  it('are the same in the Compare sheet and the glyph compare', () => {
    expect(compareColors(compareSheet)).toHaveLength(2);
    expect(compareColors(glyphCompare)).toEqual(compareColors(compareSheet));
  });

  it.each([
    ['light', light],
    ['dark', dark],
  ])('are at least 3:1 on the %s background and surface (1.4.11)', (_, p) => {
    for (const c of compareColors(compareSheet))
      for (const bg of [p.bg!, p.surface!])
        expect(contrast(rgb(c), rgb(bg)), `${c} on ${bg}`).toBeGreaterThanOrEqual(3);
  });
});

// E40.3: where the accent goes. A style sheet is read as rules, so a new component is held
// to the same two checks without being named here.
const SOURCES = Object.entries(
  import.meta.glob<string>(['/src/**/*.vue', '/src/**/*.css'], {
    query: '?raw',
    import: 'default',
    eager: true,
  }),
).map(([path, source]) => [path.replace(/^\/src\//, ''), source] as const);

/** The innermost `selector { declarations }` rules of a style sheet. */
const rules = (source: string) =>
  [...source.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({
    selector: m[1]!.replace(/\/\*[\s\S]*?\*\//g, '').trim(),
    body: m[2]!,
  }));

/** The rule of `file` for exactly `selector`. */
function ruleOf(file: string, selector: string): string {
  const source = SOURCES.find(([path]) => path === file)?.[1];
  expect(source, file).toBeDefined();
  const rule = rules(source!).find((r) => r.selector.replace(/\s+/g, ' ') === selector);
  expect(rule, `${file}: ${selector}`).toBeDefined();
  return rule!.body;
}

const ACCENT_FILL = /(^|[\s;])background(-color)?:\s*var\(--fm-accent\)/;
const TEXT_COLOR = /(^|[\s;])color:\s*([^;]+)/;

describe('the accent in the style sheets', () => {
  it('finds the style sheets', () => {
    expect(SOURCES.length).toBeGreaterThan(50);
  });

  it('is never a text color', () => {
    const found = SOURCES.flatMap(([path, source]) =>
      rules(source)
        .filter((r) => TEXT_COLOR.exec(r.body)?.[2]?.trim() === 'var(--fm-accent)')
        .map((r) => `${path}: ${r.selector}`),
    );
    expect(found).toEqual([]);
  });

  it('has on-accent text wherever a rule fills with it and sets a text color', () => {
    const found = SOURCES.flatMap(([path, source]) =>
      rules(source)
        .filter((r) => ACCENT_FILL.test(r.body) && TEXT_COLOR.test(r.body))
        .filter((r) => TEXT_COLOR.exec(r.body)?.[2]?.trim() !== 'var(--fm-on-accent)')
        .map((r) => `${path}: ${r.selector}`),
    );
    expect(found).toEqual([]);
  });

  it('never draws an accent focus ring inside an accent fill', () => {
    const parts = (selector: string) => selector.split(',').map((s) => s.trim());
    const found = SOURCES.flatMap(([path, source]) => {
      const all = rules(source);
      const filled = all.filter((r) => ACCENT_FILL.test(r.body)).flatMap((r) => parts(r.selector));
      return all
        .filter((r) => /outline:[^;]*var\(--fm-accent\)/.test(r.body))
        .filter((r) => /outline-offset:\s*-/.test(r.body))
        .flatMap((r) => parts(r.selector))
        .filter((s) => filled.some((f) => s === `${f}:focus-visible`))
        .map((s) => `${path}: ${s}`);
    });
    expect(found).toEqual([]);
  });

  it.each([
    ['primary button', 'components/UpdateDialog.vue', '.button.primary'],
    ['primary button', 'views/ProblemsView.vue', '.button.primary'],
    ['activate button', 'components/InspectorInfo.vue', '.activate'],
    ['count badge', 'components/ui/UiBadge.vue', '.tone-accent'],
    ['pressed chip', 'components/FilterChips.vue', ".chip[aria-pressed='true']"],
    ['toast action', 'components/ToastMessage.vue', '.action'],
    ['active segment', 'components/ui/UiSegmentedControl.vue', ".seg[aria-pressed='true']"],
    ['active Compare mode', 'components/CompareSheet.vue', '.mode:has(input:checked)'],
    ['selected glyph', 'components/InspectorGlyphs.vue', ".cell[aria-selected='true']"],
  ])('%s fills with the accent and writes on-accent (%s)', (_, file, selector) => {
    const body = ruleOf(file, selector);
    expect(body).toMatch(ACCENT_FILL);
    expect(TEXT_COLOR.exec(body)?.[2]?.trim()).toBe('var(--fm-on-accent)');
  });

  it.each([
    ['focus ring', 'styles/theme.css', ':focus-visible', /outline:\s*2px solid var\(--fm-accent\)/],
    [
      'selection ring',
      'components/FontGrid.vue',
      '.viewport:focus .slot.cursor .card',
      /box-shadow:\s*inset 0 0 0 2px var\(--fm-accent\)/,
    ],
    ['switch on', 'components/InspectorFeatures.vue', ".switch[aria-checked='true']", ACCENT_FILL],
    ['slider thumb', 'styles/theme.css', "input[type='range']::-webkit-slider-thumb", ACCENT_FILL],
    [
      'Firefox slider thumb',
      'styles/theme.css',
      "input[type='range']::-moz-range-thumb",
      ACCENT_FILL,
    ],
    ['activation dot', 'components/ActivationDot.vue', '.dot.active', ACCENT_FILL],
    [
      'check box',
      'styles/theme.css',
      "input[type='checkbox']:checked, input[type='radio']:checked",
      ACCENT_FILL,
    ],
  ])('%s takes the accent (%s)', (_, file, selector, pattern) => {
    expect(ruleOf(file, selector)).toMatch(pattern);
  });

  it('draws the check mark, radio dot and switch knob in on-accent', () => {
    expect(ruleOf('styles/theme.css', "input[type='checkbox']::before")).toContain(
      'background-color: var(--fm-on-accent)',
    );
    expect(ruleOf('styles/theme.css', "input[type='radio']::before")).toContain(
      'background-color: var(--fm-on-accent)',
    );
    expect(
      ruleOf('components/InspectorFeatures.vue', ".switch[aria-checked='true'] .knob"),
    ).toContain('background-color: var(--fm-on-accent)');
  });
});

describe('the print palette', () => {
  it('has text at least 4.5:1 on the paper', () => {
    const p = palette(print, '[data-print-target]');
    for (const k of ['fg', 'muted'])
      expect(contrast(rgb(p[k]!), rgb(p.bg!)), k).toBeGreaterThanOrEqual(4.5);
  });
});
