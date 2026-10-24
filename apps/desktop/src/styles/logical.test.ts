// The chrome mirrors in a right-to-left language (E19.3) because its CSS uses
// logical properties: `margin-inline-start`, not `margin-left`. This fails on a
// physical left/right property in a .css file, a .vue <style> block or a
// template `:style` binding, unless ALLOWED names it with a reason.
import { describe, expect, it } from 'vitest';

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

/**
 * Physical declarations that stay, as `file: declaration` (the declaration as
 * `physicalDeclarations` reports it), each with the reason it must not mirror.
 */
const ALLOWED: Record<string, string> = {
  // The thumbnail is a picture of the sample text, and the sample's direction is
  // the text's, not the chrome's (criterion 3): a Latin sample starts on the left.
  'components/FontGrid.vue: mask-position: left center': 'thumbnail follows its text',
  'components/FontGrid.vue: -webkit-mask-position: left center': 'thumbnail follows its text',
};

/** Properties that name a physical side. */
const PHYSICAL_PROPERTY =
  /^(-webkit-)?(left|right|(margin|padding|border|scroll-margin|scroll-padding)-(left|right)(-\w+)?|border-(top|bottom)-(left|right)-radius)$/;
/** Properties whose `left`/`right` value is a physical side. */
const SIDE_VALUE_PROPERTY =
  /^(-webkit-)?(text-align|float|clear|transition(-property)?|(background|mask|object)-position(-x)?|transform-origin)$/;
/** Shorthands whose four values run top, right, bottom, left. */
const BOX_SHORTHAND = /^(margin|padding|inset|border-width|border-style|border-color)$/;

/** camelCase style-binding keys that name a physical side (and a quoted value, if any). */
const PHYSICAL_BINDING =
  /(?:^|[{,\s])(left|right|(?:margin|padding|border)(?:Left|Right)\w*|textAlign)\s*:\s*(?:'([^']*)')?/g;

/** The physical declarations of a stylesheet, as `property: value`. */
export function physicalDeclarations(css: string): string[] {
  const out: string[] = [];
  const plain = css.replace(/\/\*[\s\S]*?\*\//g, '');
  for (const m of plain.matchAll(/([-\w]+)\s*:\s*([^;{}]+)(?=[;}])/g)) {
    const property = m[1]!.toLowerCase();
    const value = m[2]!.trim();
    if (PHYSICAL_PROPERTY.test(property)) out.push(`${property}: ${value}`);
    else if (SIDE_VALUE_PROPERTY.test(property) && /\b(left|right)\b/.test(value))
      out.push(`${property}: ${value}`);
    else if (BOX_SHORTHAND.test(property)) {
      const parts = value.split(/\s+/);
      // Four values with a different right and left are a physical side.
      if (parts.length === 4 && parts[1] !== parts[3]) out.push(`${property}: ${value}`);
    }
  }
  return out;
}

/** The physical keys of a .vue template's `:style` bindings (`textAlign` only set to left/right). */
export function physicalBindings(source: string): string[] {
  const template = source.match(/<template>([\s\S]*)<\/template>/)?.[1] ?? '';
  const out: string[] = [];
  for (const binding of template.matchAll(/(?::|v-bind:)style="([^"]*)"/g)) {
    for (const m of binding[1]!.matchAll(PHYSICAL_BINDING)) {
      const [, key, value] = m;
      if (key === 'textAlign' && value !== 'left' && value !== 'right') continue;
      out.push(key === 'textAlign' ? `${key}: ${value}` : key!);
    }
  }
  return out;
}

/** The stylesheet text of a source: a .css file whole, a .vue file's <style> blocks. */
function stylesOf(file: string, source: string): string {
  if (file.endsWith('.css')) return source;
  return [...source.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]).join('\n');
}

describe('logical properties', () => {
  it('finds physical properties, side values, uneven shorthands and style bindings', () => {
    expect(
      physicalDeclarations(`
.a { margin-left: 4px; padding-inline-start: 8px; left: 0; inset-inline-end: 0; }
.b { text-align: right; text-align: start; float: left; transition: left 0.1s; }
.c { padding: 8px 8px 8px 16px; padding: 0 8px 0 8px; margin: 0 auto; border-right: 1px solid; }
/* left: 0 in a comment is fine */
.d { border-top-left-radius: 4px; border-start-start-radius: 4px; mask-position: left center; }
`),
    ).toEqual([
      'margin-left: 4px',
      'left: 0',
      'text-align: right',
      'float: left',
      'transition: left 0.1s',
      'padding: 8px 8px 8px 16px',
      'border-right: 1px solid',
      'border-top-left-radius: 4px',
      'mask-position: left center',
    ]);
    expect(
      physicalBindings(`<template>
  <span :style="{ left: \`\${x}%\`, width: '2px' }" />
  <span :style="{ insetInlineStart: '0', marginLeft: '4px' }" />
  <span :style="{ textAlign: props.align }" />
  <span :style="{ textAlign: 'right' }" />
</template>`),
    ).toEqual(['left', 'marginLeft', 'textAlign: right']);
  });

  it('no stylesheet or style binding in src uses a physical left/right outside the allow-list', () => {
    expect(Object.keys(SOURCES).length).toBeGreaterThan(40);
    const found = Object.entries(SOURCES).flatMap(([file, source]) => [
      ...physicalDeclarations(stylesOf(file, source)).map((d) => `${file}: ${d}`),
      ...(file.endsWith('.vue') ? physicalBindings(source).map((b) => `${file}: ${b}`) : []),
    ]);
    expect(found.filter((hit) => !(hit in ALLOWED))).toEqual([]);
    // An entry that no longer matches anything is stale: remove it.
    expect(Object.keys(ALLOWED).filter((entry) => !found.includes(entry))).toEqual([]);
  });
});
