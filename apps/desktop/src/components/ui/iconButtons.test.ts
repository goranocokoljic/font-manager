// E50.6: one icon size everywhere. Every icon-only button of the app is a UiIconButton (28 px
// target, 16 px glyph), so no component may draw one of its own: a `<button>` or `<UiButton>`
// whose content is only an icon (a UiIcon, an inline svg, or a lone symbol such as ×) fails
// here. Three files are the exception: UiIconButton itself, and the segments of a segmented
// control and the steppers of a number field, which are parts of one control, not buttons.
// E54.9 adds the tooltip: the app's own `UiTooltip`, never a native `title`.
import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { defineComponent, h, nextTick } from 'vue';
import UiIconButton from './UiIconButton.vue';
import { tooltipOf, tooltipShortcut } from './testing';
import { HOVER_DELAY_MS, resetTooltips } from './tooltip';

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

// E54.9: the app's one tooltip, on every icon button.
describe('the icon button tooltip (E54.9)', () => {
  let wrappers: VueWrapper[] = [];
  const mountButton = (props: Record<string, unknown> = {}) => {
    const w = mount(UiIconButton, {
      props: { icon: 'similar', label: 'Find similar to Inter', shortcut: 'S', ...props },
      attachTo: document.body,
    });
    wrappers.push(w);
    return w;
  };
  /** The tooltip layer of a button, through its aria-describedby. */
  const layer = (w: VueWrapper) => {
    const ids = (w.attributes('aria-describedby') ?? '').split(' ');
    const el = ids
      .map((id) => document.getElementById(id))
      .find((e) => e?.getAttribute('role') === 'tooltip');
    if (!el) throw new Error('no tooltip');
    return el;
  };
  const shown = (w: VueWrapper) => layer(w).style.display !== 'none';
  const key = (k: string) => window.dispatchEvent(new KeyboardEvent('keydown', { key: k }));
  const press = () => window.dispatchEvent(new Event('pointerdown'));

  beforeEach(() => {
    vi.useFakeTimers();
    resetTooltips();
  });
  afterEach(() => {
    for (const w of wrappers) w.unmount();
    wrappers = [];
    vi.useRealTimers();
  });

  it('has no native title; the tooltip is its description, with the name and the shortcut', () => {
    const w = mountButton();
    expect(w.attributes('title')).toBeUndefined();
    expect(w.attributes('aria-label')).toBe('Find similar to Inter');
    expect(tooltipOf(w)).toBe('Find similar to Inter');
    expect(tooltipShortcut(w)).toBe('S');
    // The description assistive technology reads: the name, a pause, the key.
    expect(layer(w).textContent).toBe('Find similar to Inter, S');
    expect(shown(w)).toBe(false);
  });

  it('says more than the label with a title, and keeps a caller’s own description', () => {
    const w = mountButton({ title: 'Compare is full', 'aria-describedby': 'hint' });
    expect(w.attributes('title')).toBeUndefined();
    expect(tooltipOf(w)).toBe('Compare is full');
    expect(w.attributes('aria-describedby')?.split(' ')[0]).toBe('hint');
  });

  it('comes up after a rest of the pointer, within 400 ms, and goes when it leaves', async () => {
    const w = mountButton();
    await w.trigger('pointerenter', { pointerType: 'mouse' });
    expect(shown(w)).toBe(false);
    expect(HOVER_DELAY_MS).toBeLessThanOrEqual(400);
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    await nextTick();
    expect(shown(w)).toBe(true);
    await w.trigger('pointerleave');
    expect(shown(w)).toBe(false);
  });

  it('a pointer that leaves before the delay shows nothing', async () => {
    const w = mountButton();
    await w.trigger('pointerenter', { pointerType: 'mouse' });
    await w.trigger('pointerleave');
    vi.advanceTimersByTime(1000);
    await nextTick();
    expect(shown(w)).toBe(false);
  });

  it('comes up at once on a keyboard focus, not on a focus by click', async () => {
    const w = mountButton();
    key('Tab');
    await w.trigger('focus');
    expect(shown(w)).toBe(true);
    await w.trigger('blur');
    expect(shown(w)).toBe(false);
    press();
    await w.trigger('focus');
    expect(shown(w)).toBe(false);
  });

  it('hides on a press, by pointer or by key, and on Esc', async () => {
    const w = mountButton();
    key('Tab');
    await w.trigger('focus');
    expect(shown(w)).toBe(true);
    await w.trigger('pointerdown');
    expect(shown(w)).toBe(false);

    await w.trigger('blur');
    key('Tab');
    await w.trigger('focus');
    expect(shown(w)).toBe(true);
    await w.trigger('keydown', { key: 'Enter' });
    expect(shown(w)).toBe(false);

    await w.trigger('blur');
    key('Tab');
    await w.trigger('focus');
    expect(shown(w)).toBe(true);
    key('Escape');
    await nextTick();
    expect(shown(w)).toBe(false);
  });

  it('shows one tooltip at a time; the next one along a row comes up at once', async () => {
    const a = mountButton({ label: 'Activate' });
    const b = mountButton({ label: 'Compare' });
    await a.trigger('pointerenter', { pointerType: 'mouse' });
    vi.advanceTimersByTime(HOVER_DELAY_MS);
    await nextTick();
    expect(shown(a)).toBe(true);
    await a.trigger('pointerleave');
    await b.trigger('pointerenter', { pointerType: 'mouse' });
    expect(shown(b), 'no second wait right after one closed').toBe(true);
    key('Tab');
    await a.trigger('focus');
    expect(shown(a)).toBe(true);
    expect(shown(b), 'the one before closes').toBe(false);
  });

  it('a touch shows no tooltip', async () => {
    const w = mountButton();
    await w.trigger('pointerenter', { pointerType: 'touch' });
    vi.advanceTimersByTime(1000);
    await nextTick();
    expect(shown(w)).toBe(false);
  });

  it('sits above the button, below it without room, inside the window, never over it', async () => {
    const rect = (left: number, top: number, w: number, h: number) =>
      ({
        left,
        top,
        right: left + w,
        bottom: top + h,
        width: w,
        height: h,
        x: left,
        y: top,
      }) as DOMRect;
    const width = vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(120);
    const height = vi.spyOn(HTMLElement.prototype, 'offsetHeight', 'get').mockReturnValue(24);
    const place = async (button: DOMRect) => {
      const w = mountButton();
      vi.spyOn(w.element, 'getBoundingClientRect').mockReturnValue(button);
      key('Tab');
      await w.trigger('focus');
      await nextTick();
      await nextTick();
      const s = layer(w).style;
      const box = rect(parseFloat(s.left), parseFloat(s.top), 120, 24);
      await w.trigger('blur');
      return box;
    };
    const overlaps = (a: DOMRect, b: DOMRect) =>
      a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;

    const mid = rect(400, 300, 32, 32);
    const above = await place(mid);
    expect(above.bottom).toBeLessThanOrEqual(mid.top);
    expect(above.left + 60).toBe(mid.left + 16);
    expect(overlaps(above, mid)).toBe(false);

    const top = rect(400, 2, 32, 32);
    const below = await place(top);
    expect(below.top).toBeGreaterThanOrEqual(top.bottom);
    expect(overlaps(below, top)).toBe(false);

    const edge = rect(window.innerWidth - 20, 300, 32, 32);
    const inside = await place(edge);
    expect(inside.right).toBeLessThanOrEqual(window.innerWidth);
    expect(overlaps(inside, edge)).toBe(false);
    width.mockRestore();
    height.mockRestore();
  });

  it('a count beside the icon stays the button’s only text', () => {
    const Wrap = defineComponent(
      () => () =>
        h(UiIconButton, { icon: 'compare', label: 'Compare, 2 fonts' }, () => h('span', '2')),
    );
    const w = mount(Wrap, { attachTo: document.body });
    expect(w.get('button').text()).toBe('2');
    w.unmount();
  });

  it('a row action is a 32 px target around a 20 px glyph (size="lg")', () => {
    const w = mountButton({ size: 'lg' });
    expect(w.classes()).toContain('lg');
    expect(w.get('svg').attributes('width')).toBe('20');
    expect(w.get('svg').attributes('height')).toBe('20');
    expect(mountButton().get('svg').attributes('width')).toBe('16');
    const style = sources['/src/components/ui/UiIconButton.vue']!;
    expect(style).toMatch(/\.ui-icon-button\.lg \{\s*min-width: 32px;\s*height: 32px;/);
  });
});

// E57.8: × closes. A top bar or a panel head closes with the icon button `close`, never with a
// text button that says "Close". A dialog footer (a `<footer>`, or a `buttons`, `actions` or
// `footer` row) keeps its text button: the catalog download and install dialogs end that way.

/** The text buttons of one template whose label is `common.close` or a `*.close` string. */
export function textCloseButtons(source: string): Array<[number, string]> {
  const template = source.replace(/<script[\s\S]*?<\/script>|<style[\s\S]*?<\/style>/g, (m) =>
    m.replace(/[^\n]/g, ' '),
  );
  const found: Array<[number, string]> = [];
  const button = /<(button|UiButton)\b((?:[^>"']|"[^"]*"|'[^']*')*)>([\s\S]*?)<\/\1>/g;
  for (const m of template.matchAll(button)) {
    const label = m[3]!.replace(/<!--[\s\S]*?-->/g, '').trim();
    if (!/^\{\{\s*t\(\s*'(?:[\w]+\.)*close'\s*\)\s*\}\}$/.test(label)) continue;
    if (inFooter(template.slice(0, m.index))) continue;
    const line = template.slice(0, m.index).split('\n').length;
    found.push([line, `<${m[1]}${m[2]!.replace(/\s+/g, ' ').slice(0, 60)}…>`]);
  }
  return found;
}

const VOID = new Set(['input', 'img', 'br', 'hr', 'meta', 'link', 'col', 'source', 'wbr']);

/** Whether the elements still open at the end of `before` include a dialog footer. */
function inFooter(before: string): boolean {
  const open: Array<{ name: string; footer: boolean }> = [];
  const tag = /<(\/?)([A-Za-z][\w-]*)\b((?:[^>"']|"[^"]*"|'[^']*')*?)(\/?)>/g;
  for (const [, closing, name, attrs, self] of before
    .replace(/<!--[\s\S]*?-->/g, '')
    .matchAll(tag)) {
    if (closing) {
      const i = open.map((e) => e.name).lastIndexOf(name!);
      if (i >= 0) open.length = i;
    } else if (!self && !VOID.has(name!.toLowerCase())) {
      const footer = name === 'footer' || /\bclass="[^"]*\b(buttons|actions|footer)\b/.test(attrs!);
      open.push({ name: name!, footer });
    }
  }
  return open.some((e) => e.footer);
}

describe('× closes (E57.8)', () => {
  const tpl = (body: string) => `<template>${body}</template>`;

  it('finds a text Close in a bar or a panel head, and lets a footer and other words through', () => {
    expect(
      textCloseButtons(
        tpl(
          `<header class="bar">\n<UiButton @click="close">\n  {{ t('common.close') }}\n</UiButton></header>`,
        ),
      ),
    ).toEqual([[2, '<UiButton @click="close"…>']]);
    expect(
      textCloseButtons(
        tpl(`<div class="head"><button>{{ t('duplicatesModule.compare.close') }}</button></div>`),
      ),
    ).toHaveLength(1);
    expect(
      textCloseButtons(
        tpl(`<div class="props"><input /><button>{{ t('common.close') }}</button></div>`),
      ),
    ).toHaveLength(1);
    expect(
      textCloseButtons(
        tpl(`<div class="buttons"><button>{{ t('catalogs.download.close') }}</button></div>`),
      ),
    ).toEqual([]);
    expect(
      textCloseButtons(
        tpl(
          `<div class="actions"><template v-if="a"><b>x</b></template><button>{{ t('catalogs.install.close') }}</button></div>`,
        ),
      ),
    ).toEqual([]);
    expect(
      textCloseButtons(tpl(`<footer><button>{{ t('common.close') }}</button></footer>`)),
    ).toEqual([]);
    // A footer that has closed no longer covers the button after it.
    expect(
      textCloseButtons(
        tpl(
          `<div class="buttons"><i>a</i></div><header><button>{{ t('common.close') }}</button></header>`,
        ),
      ),
    ).toHaveLength(1);
    expect(
      textCloseButtons(tpl(`<header><button>{{ t('common.cancel') }}</button></header>`)),
    ).toEqual([]);
    expect(
      textCloseButtons(tpl(`<header><button>{{ t('x.closeAll') }}</button></header>`)),
    ).toEqual([]);
  });

  it('no top bar or panel head closes with a text button', () => {
    const offenders = Object.entries(sources).flatMap(([path, source]) =>
      textCloseButtons(source).map(([line, tag]) => `${path.slice(5)}:${line} ${tag}`),
    );
    expect(offenders, 'use <UiIconButton icon="close"> for these').toEqual([]);
  });

  it.each([
    ['/src/views/ProblemsView.vue', 'problems.close', true],
    ['/src/views/ConflictsView.vue', 'conflicts.close', true],
    ['/src/components/DuplicateCompare.vue', 'duplicatesModule.compare.close', true],
    ['/src/components/BlockProperties.vue', 'blocks.close', true],
    ['/src/components/ProjectSetsSidebar.vue', 'sets.dismissLabel', false],
  ])('%s closes with the × named by %s', (path, key, esc) => {
    const source = sources[path]!;
    const close = source.match(/<UiIconButton\b[^>]*\bicon="close"[^>]*>/)?.[0];
    expect(close, 'a UiIconButton with the icon close').toBeDefined();
    expect(close).toContain(`:label="t('${key}')"`);
    // The tooltip names the Esc key only where Esc closes it.
    expect(close!.includes('shortcut="Esc"')).toBe(esc);
    expect(close!.includes('aria-keyshortcuts="Escape"')).toBe(esc);
  });

  it('the catalog download and install dialogs keep their text Close in the footer', () => {
    expect(sources['/src/components/CatalogDownload.vue']).toContain(
      "{{ t('catalogs.download.close') }}",
    );
    expect(sources['/src/components/CatalogInstall.vue']).toContain(
      "{{ t('catalogs.install.close') }}",
    );
  });
});
