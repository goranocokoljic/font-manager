// E46.3: the drawer, the popovers (every UiSelect list), the selection bar and the toasts move
// through the motion helpers only, with the props of the motion table (dev-docs/redesign/MOTION.md),
// and keep a fade under reduced motion. No keyframes or transition of their own.
import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import UiDrawer from '../components/ui/UiDrawer.vue';
import UiPopover from '../components/ui/UiPopover.vue';
import FmFade from './FmFade.vue';
import FmSlide from './FmSlide.vue';

const SOURCES = import.meta.glob<string>(
  [
    '../components/ui/UiDrawer.vue',
    '../components/ui/UiPopover.vue',
    '../components/ui/UiSelect.vue',
    '../components/SelectionBar.vue',
    '../components/ToastMessage.vue',
    '../components/McpConfirmToast.vue',
    '../components/WallCountLine.vue',
    '../components/TableColumnsMenu.vue',
    '../components/PreviewControls.vue',
    '../views/HomeView.vue',
  ],
  { query: '?raw', import: 'default', eager: true },
);
const source = (file: string) => SOURCES[`../${file}`] ?? '';
const styles = (file: string) =>
  [...source(file).matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)]
    .map((m) => m[1]!.replace(/\/\*[\s\S]*?\*\//g, ''))
    .join('\n');

/**
 * What a component moves by itself: keyframes, animations, Vue transitions of its own. (A
 * toggle's own 120 ms transition, such as the select's chevron, is not an overlay moment.)
 */
function ownMotion(file: string): string[] {
  const css = styles(file);
  return [
    ...[...css.matchAll(/@keyframes\s+[\w-]+/g)].map((m) => m[0]),
    ...[...css.matchAll(/(^|[\s;{])(animation[\w-]*\s*:(?!\s*none\b)[^;}]+)/g)].map((m) =>
      m[2]!.trim(),
    ),
    ...[...template(file).matchAll(/<Transition(Group)?[\s>]/g)].map((m) => m[0].trim()),
  ];
}

/** A file's template, so a helper named in a comment does not count. */
function template(file: string): string {
  return source(file).match(/<template>([\s\S]*)<\/template>/)?.[1] ?? '';
}

/** The opening tags of `helper` in a file's template, attributes in one line. */
const tags = (file: string, helper: string) =>
  [...template(file).matchAll(new RegExp(`<${helper}\\b([^>]*)>`, 'g'))].map((m) =>
    m[1]!.replace(/\s+/g, ' ').trim(),
  );

describe('the overlay moments go through the helpers (E46.3)', () => {
  it('finds what a component moves by itself, so the check below can fail', () => {
    SOURCES['../probe.vue'] = `<template><Transition name="x"><p/></Transition></template>
<style>.a { animation: rise var(--fm-motion-fast); } @keyframes rise { from { opacity: 0 } }
.c { animation: none; }</style>`;
    expect(ownMotion('probe.vue')).toEqual([
      '@keyframes rise',
      'animation: rise var(--fm-motion-fast)',
      '<Transition',
    ]);
    delete SOURCES['../probe.vue'];
  });

  it.each([
    'components/ui/UiDrawer.vue',
    'components/ui/UiPopover.vue',
    'components/ui/UiSelect.vue',
    'components/SelectionBar.vue',
    'components/ToastMessage.vue',
    'components/McpConfirmToast.vue',
    'components/WallCountLine.vue',
    'components/TableColumnsMenu.vue',
    'components/PreviewControls.vue',
  ])('%s has no keyframes, animation or Vue transition of its own', (file) => {
    expect(source(file)).not.toBe('');
    expect(ownMotion(file)).toEqual([]);
  });

  it('the drawer slides its full width from the start edge over a faded scrim', () => {
    expect(tags('components/ui/UiDrawer.vue', 'FmSlide')).toEqual([
      'from="start" distance="100%" reduced="fade"',
    ]);
    expect(tags('components/ui/UiDrawer.vue', 'FmFade')).toEqual(['reduced="fade"']);
    expect(styles('components/ui/UiDrawer.vue')).toMatch(
      /\.scrim \{[^}]*background-color: var\(--fm-scrim-drawer\);[^}]*pointer-events: none;/,
    );
  });

  it('a popover, and so every UiSelect list, rises 4 px at the fast speed', () => {
    expect(tags('components/ui/UiPopover.vue', 'FmSlide')).toEqual([
      'from="bottom" distance="4px" speed="fast" reduced="fade"',
    ]);
    expect(source('components/ui/UiSelect.vue')).toMatch(/<UiPopover\b/);
  });

  // The popovers that live in the page, not in a UiPopover layer: the sort menu, the table's
  // columns menu and the Preview options. Each panel is the slide's one child.
  it.each([
    ['components/WallCountLine.vue', 'v-if="open"', 'menu'],
    ['components/TableColumnsMenu.vue', 'v-if="open"', 'panel'],
    ['components/PreviewControls.vue', 'v-show="optionsOpen"', 'more'],
  ])('%s rises its popover 4 px at the fast speed', (file, shown, cls) => {
    expect(tags(file, 'FmSlide')).toEqual([
      'from="bottom" distance="4px" speed="fast" reduced="fade"',
    ]);
    const slide = template(file).match(/<FmSlide\b[^>]*>\s*<div\b([^>]*)>/)?.[1] ?? '';
    expect(slide).toContain(shown);
    expect(slide).toMatch(new RegExp(`class="${cls}"`));
  });

  it('the selection bar and both toasts rise 16 px at the move speed', () => {
    expect(
      tags('views/HomeView.vue', 'FmSlide').filter((t) => t.includes('distance="16px"')),
    ).toEqual(['from="bottom" distance="16px" reduced="fade"']);
    expect(source('views/HomeView.vue')).toMatch(
      /<FmSlide from="bottom" distance="16px" reduced="fade">\s*<SelectionBar\b/,
    );
    for (const file of ['components/ToastMessage.vue', 'components/McpConfirmToast.vue']) {
      expect(tags(file, 'FmSlide')).toEqual([
        'from="bottom" distance="16px" reduced="fade" @before-leave="retireToast"',
      ]);
    }
  });
});

describe('the drawer and the popover in the component tree', () => {
  it('the drawer shows its panel in the slide and the scrim in the fade', async () => {
    const wrapper = mount(UiDrawer, {
      props: { open: true, label: 'Filters' },
      attachTo: document.body,
    });
    await nextTick();
    const slide = wrapper.findComponent(FmSlide);
    expect(slide.props()).toMatchObject({ from: 'start', distance: '100%', reduced: 'fade' });
    expect(slide.props('speed')).toBe('move');
    expect(wrapper.findComponent(FmFade).props()).toMatchObject({
      speed: 'move',
      reduced: 'fade',
    });
    expect(document.querySelector('[data-testid="drawer-scrim"]')).not.toBeNull();
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
    await wrapper.setProps({ open: false });
    expect(document.querySelector('[data-testid="drawer-scrim"]')).toBeNull();
    wrapper.unmount();
  });

  it('the popover rises in the fast slide', async () => {
    const anchor = document.createElement('button');
    document.body.append(anchor);
    const wrapper = mount(UiPopover, {
      props: { open: true, anchor },
      slots: { default: '<p>menu</p>' },
      attachTo: document.body,
    });
    await nextTick();
    expect(wrapper.findComponent(FmSlide).props()).toMatchObject({
      from: 'bottom',
      distance: '4px',
      speed: 'fast',
      reduced: 'fade',
    });
    expect(document.querySelector('[data-ui-popover]')).not.toBeNull();
    wrapper.unmount();
    anchor.remove();
  });
});
