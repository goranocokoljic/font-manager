// Three durations and one curve (dev-docs/redesign/MOTION.md). Motion stays one thing to
// change, and one thing the OS setting turns off, because components only read the tokens.
// This fails on a literal duration or curve in a transition or animation anywhere in src.
import { describe, expect, it } from 'vitest';
import theme from './theme.css?raw';

/** Every .vue and .css file under src, by its path from src (`components/X.vue`). */
const SOURCES = Object.fromEntries(
  Object.entries(
    import.meta.glob<string>(['../**/*.vue', '../**/*.css'], {
      query: '?raw',
      import: 'default',
      eager: true,
    }),
  ).map(([path, source]) => [path.replace(/^\.\.\//, ''), source]),
);

/** Every script that could animate an element by hand. */
const SCRIPTS = Object.fromEntries(
  Object.entries(
    import.meta.glob<string>(['../**/*.vue', '../**/*.ts', '!../**/*.test.ts'], {
      query: '?raw',
      import: 'default',
      eager: true,
    }),
  ).map(([path, source]) => [path.replace(/^\.\.\//, ''), source]),
);

const MOTION_PROPERTY = /^(-webkit-)?(transition|animation)(-(duration|delay|timing-function))?$/;
const LITERAL =
  /(^|[\s,(])[\d.]+m?s\b|\b(ease|ease-in|ease-out|ease-in-out|linear|step-start|step-end)\b|\b(cubic-bezier|steps)\(/;

/** The motion declarations of `css` that name a duration or a curve instead of a token. */
export function literalMotion(css: string): string[] {
  const plain = css.replace(/\/\*[\s\S]*?\*\//g, '');
  const out: string[] = [];
  for (const m of plain.matchAll(/([-\w]+)\s*:\s*([^;{}]+)(?=[;}])/g)) {
    const property = m[1]!.toLowerCase();
    const value = m[2]!.trim();
    if (!MOTION_PROPERTY.test(property)) continue;
    // A token's name is not a literal (`--fm-ease`), but a fallback after it is.
    if (LITERAL.test(value.replace(/--[\w-]+/g, ''))) {
      out.push(`${property}: ${value.replace(/\s+/g, ' ')}`);
    }
  }
  return out;
}

/** Motion set on an element itself: a template style attribute, or a script. */
export function inlineMotion(source: string): string[] {
  const template = source.match(/<template>([\s\S]*)<\/template>/)?.[1] ?? '';
  return [
    ...[...template.matchAll(/style="[^"]*(transition|animation)[^"]*"/gi)].map((m) => m[0]),
    ...[...source.matchAll(/\.animate\(|\.style\.(transition|animation)\w*/g)].map((m) => m[0]),
  ];
}

/** The stylesheet text of a source: a .css file whole, a .vue file's <style> blocks. */
function stylesOf(file: string, source: string): string {
  if (file.endsWith('.css')) return source;
  return [...source.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]).join('\n');
}

/** The declarations of a `:root` block in `css`, as name → value. */
function rootTokens(css: string): Record<string, string> {
  const block = css.match(/:root\s*\{([^}]*)\}/)?.[1] ?? '';
  return Object.fromEntries(
    [...block.replace(/\/\*[\s\S]*?\*\//g, '').matchAll(/(--fm-[\w-]+)\s*:\s*([^;]+);/g)].map(
      (m) => [m[1]!, m[2]!.trim()],
    ),
  );
}

describe('motion tokens', () => {
  const base = rootTokens(theme);

  it('are the three durations and the one curve of the spec', () => {
    expect(base['--fm-motion-fast']).toBe('120ms');
    expect(base['--fm-motion-move']).toBe('200ms');
    expect(base['--fm-motion-page']).toBe('280ms');
    expect(base['--fm-ease']).toBe('cubic-bezier(0.2, 0, 0, 1)');
    expect(Object.keys(base).filter((name) => name.startsWith('--fm-motion-'))).toHaveLength(3);
  });

  it('all go to 0ms under prefers-reduced-motion: reduce', () => {
    const block = theme.match(/@media \(prefers-reduced-motion: reduce\) \{([\s\S]*?)\n\}/)?.[1];
    expect(rootTokens(block ?? '')).toEqual({
      '--fm-motion-fast': '0ms',
      '--fm-motion-move': '0ms',
      '--fm-motion-page': '0ms',
    });
  });

  it('the reduced values come after the base values, so they win', () => {
    expect(theme.indexOf('--fm-motion-page: 0ms')).toBeGreaterThan(
      theme.indexOf('--fm-motion-page: 280ms'),
    );
  });
});

describe('no literal motion', () => {
  it('finds literal durations and curves, and leaves tokens alone', () => {
    expect(
      literalMotion(`
.a { animation: rise 120ms ease-out; }
.b { transition: opacity var(--fm-motion-fast) var(--fm-ease); }
.c { transition: transform var(--fm-motion-move, 200ms) var(--fm-ease); }
.d { transition-duration: 0.2s; }
.e { transition-timing-function: cubic-bezier(.2, 0, 0, 1); }
.f { animation: spin var(--fm-motion-page) linear infinite; }
.g { transition: none; animation: none; }
.h {
  transition:
    transform var(--fm-motion-move) var(--fm-ease),
    opacity 1s var(--fm-ease);
}
.i { --fm-motion-fast: 120ms; --fm-ease: ease-in; width: calc(1px + 2ms); }
/* transition: opacity 1s; in a comment is fine */
`),
    ).toEqual([
      'animation: rise 120ms ease-out',
      'transition: transform var(--fm-motion-move, 200ms) var(--fm-ease)',
      'transition-duration: 0.2s',
      'transition-timing-function: cubic-bezier(.2, 0, 0, 1)',
      'animation: spin var(--fm-motion-page) linear infinite',
      'transition: transform var(--fm-motion-move) var(--fm-ease), opacity 1s var(--fm-ease)',
    ]);
    expect(
      inlineMotion(`<template><p style="transition: opacity 1s" /><p :style="{ width }" /></template>
<script>el.animate([], 200); el.style.transitionDuration = '1s'; el.style.setProperty('--x', 1);</script>`),
    ).toEqual(['style="transition: opacity 1s"', '.animate(', '.style.transitionDuration']);
  });

  it('no stylesheet in src declares its own duration or curve', () => {
    expect(Object.keys(SOURCES).length).toBeGreaterThan(40);
    const found = Object.entries(SOURCES).flatMap(([file, source]) =>
      literalMotion(stylesOf(file, source)).map((d) => `${file}: ${d}`),
    );
    expect(found).toEqual([]);
  });

  it('no template or script in src animates an element by hand', () => {
    expect(Object.keys(SCRIPTS).length).toBeGreaterThan(40);
    const found = Object.entries(SCRIPTS).flatMap(([file, source]) =>
      inlineMotion(source).map((d) => `${file}: ${d}`),
    );
    expect(found).toEqual([]);
  });
});
