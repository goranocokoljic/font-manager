// The chrome's colors meet WCAG AA in both themes: text 4.5:1, control outlines
// and focus rings 3:1. Reads the palettes from the style sheets, so a color
// change that breaks contrast fails here.
import { describe, expect, it } from 'vitest';
import fontList from '../components/FontList.vue?raw';
import theme from './theme.css?raw';

type Palette = Record<string, string>;

/** The `--fm-*` colors declared in the first block opened by `selector`. */
function palette(css: string, selector: string): Palette {
  const start = css.indexOf(`${selector} {`);
  expect(start, selector).toBeGreaterThanOrEqual(0);
  const body = css.slice(start, css.indexOf('}', start));
  return Object.fromEntries(
    [...body.matchAll(/--fm-([a-z-]+):\s*(#[0-9a-f]{6})/gi)].map((m) => [
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
const TEXT = ['fg', 'muted', 'danger', 'accent', 'success', 'warning'];

describe.each([
  ['light', light],
  ['dark', dark],
])('%s theme', (_, p) => {
  it('has every color the checks need', () => {
    for (const k of [...TEXT, 'bg', 'surface', 'control-border']) expect(p[k], k).toBeDefined();
  });

  it.each(TEXT)('%s text is at least 4.5:1 on the background, surface and selected row', (k) => {
    const fg = rgb(p[k]!);
    const backgrounds = {
      bg: rgb(p.bg!),
      surface: rgb(p.surface!),
      selected: tint(p.accent!, 0.16, p.bg!),
    };
    for (const [name, bg] of Object.entries(backgrounds))
      expect(contrast(fg, bg), `${k} on ${name}`).toBeGreaterThanOrEqual(4.5);
  });

  it('button text on the accent is at least 4.5:1', () => {
    expect(contrast(rgb(p.bg!), rgb(p.accent!))).toBeGreaterThanOrEqual(4.5);
  });

  it('control outlines and the focus ring are at least 3:1', () => {
    for (const bg of [p.bg!, p.surface!]) {
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
    for (const [k, v] of Object.entries(palette(fontList, selector))) expect(v, k).toBe(p[k]);
  });
});
