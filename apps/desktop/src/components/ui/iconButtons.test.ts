// E50.6: one icon size everywhere. Every icon-only button of the app is a UiIconButton (28 px
// target, 16 px glyph), so no component may draw one of its own: a `<button>` or `<UiButton>`
// whose content is only an icon (a UiIcon, an inline svg, or a lone symbol such as ×) fails
// here. Three files are the exception: UiIconButton itself, and the segments of a segmented
// control and the steppers of a number field, which are parts of one control, not buttons.
import { describe, expect, it } from 'vitest';

const sources = import.meta.glob<string>('/src/**/*.vue', {
  query: '?raw',
  import: 'default',
  eager: true,
});

/** The icon button itself, and controls whose icon buttons are parts of them. */
const UI_PARTS = new Set([
  '/src/components/ui/UiIconButton.vue',
  '/src/components/ui/UiSegmentedControl.vue',
  '/src/components/ui/UiNumberField.vue',
]);

/** A lone symbol that stands for an icon when it is all a button shows. */
const SYMBOL = /^[×✕✖⋯…+−\-‹›<>↑↓←→▾▸◂▴]$/u;

/** The icon-only buttons of one template: `[line, opening tag]`. */
export function iconOnlyButtons(source: string): Array<[number, string]> {
  const template = source.replace(/<script[\s\S]*?<\/script>|<style[\s\S]*?<\/style>/g, (m) =>
    m.replace(/[^\n]/g, ' '),
  );
  const found: Array<[number, string]> = [];
  const button = /<(button|UiButton)\b((?:[^>"']|"[^"]*"|'[^']*')*)>([\s\S]*?)<\/\1>/g;
  for (const m of template.matchAll(button)) {
    const inner = m[3]!.replace(/<!--[\s\S]*?-->/g, '');
    const icons = (inner.match(/<UiIcon\b|<svg\b/g) ?? []).length;
    const rest = inner
      .replace(/<svg\b[\s\S]*?<\/svg>/g, '')
      .replace(/<span\b[^>]*class="sr-only"[^>]*>[\s\S]*?<\/span>/g, '')
      .replace(/<[^>]*>/g, '')
      .trim();
    if ((icons > 0 && rest === '') || (icons === 0 && SYMBOL.test(rest))) {
      const line = template.slice(0, m.index).split('\n').length;
      found.push([line, `<${m[1]}${m[2]!.replace(/\s+/g, ' ').slice(0, 60)}…>`]);
    }
  }
  return found;
}

describe('icon buttons (E50.6)', () => {
  it('reads the components', () => {
    expect(Object.keys(sources).length).toBeGreaterThan(50);
    expect(Object.keys(sources)).toContain('/src/components/CatalogFamilyItem.vue');
  });

  it('finds an icon-only button and lets a labelled one through', () => {
    const tpl = (body: string) => `<template>${body}</template>`;
    expect(iconOnlyButtons(tpl('<button aria-label="x"><UiIcon name="close" /></button>'))).toEqual(
      [[1, '<button aria-label="x"…>']],
    );
    expect(
      iconOnlyButtons(tpl('<UiButton :a="b > c">\n  <svg><path /></svg>\n</UiButton>')),
    ).toHaveLength(1);
    expect(iconOnlyButtons(tpl('<button aria-label="Close">×</button>'))).toHaveLength(1);
    expect(
      iconOnlyButtons(tpl('<button><UiIcon name="x" /><span class="sr-only">Hi</span></button>')),
    ).toHaveLength(1);
    expect(iconOnlyButtons(tpl('<button><UiIcon name="power" /> {{ label }}</button>'))).toEqual(
      [],
    );
    expect(iconOnlyButtons(tpl('<button>Install</button>'))).toEqual([]);
  });

  it('no component draws an icon-only button outside UiIconButton', () => {
    const offenders = Object.entries(sources)
      .filter(([path]) => !UI_PARTS.has(path))
      .flatMap(([path, source]) =>
        iconOnlyButtons(source).map(([line, tag]) => `${path.slice(5)}:${line} ${tag}`),
      );
    expect(offenders, 'use UiIconButton for these').toEqual([]);
  });
});
