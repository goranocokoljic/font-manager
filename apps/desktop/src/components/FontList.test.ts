import { flushPromises, mount, type VueWrapper } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setClient, useClient } from '../api/client';
import { createMockClient, EXTRA_ID_BASE, mockLanguages, type MockOptions } from '../api/mock';
import { FaceLoader, setFaceLoader, type FontFaceSetLike } from '../preview/faceLoader';
import { ALPHABET } from '../preview/sampleText';
import { useSearchStore } from '../stores/search';
import { clearFaceAxes } from '../preview/variations';
import { useLibraryStore } from '../stores/library';
import { DEFAULT_PREVIEW, type PreviewSettings } from '../stores/preview';
import { useFontListStore } from '../stores/fontList';
import { useSettingsStore } from '../stores/settings';
import FontList from './FontList.vue';

class FakeFace {
  constructor(
    readonly family: string,
    readonly source: string,
  ) {}
  load() {
    return Promise.resolve(this);
  }
}

class FakeSet implements FontFaceSetLike<FakeFace> {
  readonly faces = new Set<FakeFace>();
  add(f: FakeFace) {
    this.faces.add(f);
  }
  delete(f: FakeFace) {
    return this.faces.delete(f);
  }
}

const MAX_IDLE = 16;
let fonts: FakeSet;
let loader: FaceLoader<FakeFace>;

/** The mock API's pangram for `code`. */
const pangramOf = (code: string) => mockLanguages.find((l) => l.code === code)!.pangram!;

beforeEach(() => {
  setActivePinia(createPinia());
  clearFaceAxes();
  fonts = new FakeSet();
  loader = new FaceLoader<FakeFace>({
    fonts,
    create: (family, source) => new FakeFace(family, source),
    url: (face) => `fmfont://localhost/${face}`,
    maxIdle: MAX_IDLE,
  });
  setFaceLoader(loader);
});

afterEach(() => {
  setFaceLoader(undefined);
  setClient(undefined);
});

/** Loads a mock library and mounts the list on it (600 px fallback viewport). */
async function listOf(options: MockOptions, props: Record<string, unknown> = {}) {
  setClient(createMockClient(options));
  await useLibraryStore().load();
  await useSearchStore().loadLanguages();
  const list = mount(FontList, { props, attachTo: document.body });
  await flushPromises();
  return list;
}

const items = (list: VueWrapper) => list.findAll('[role="treeitem"]');

async function scrollTo(list: VueWrapper, top: number) {
  const tree = list.get('[role="tree"]');
  (tree.element as HTMLElement).scrollTop = top;
  await tree.trigger('scroll');
  await flushPromises();
}

describe('FontList virtualization', () => {
  // Building and searching a 10,000-family mock takes seconds under a loaded
  // parallel run; the test is about the DOM row count, not speed.
  it('renders only the rows on screen for 10,000 families', { timeout: 30_000 }, async () => {
    const list = await listOf({ extraFaces: 9997 }, { rowHeight: 50, overscan: 2 });
    const rows = items(list);
    // 600 px fallback viewport: 12 rows, +1 partly visible, +2 overscan each side.
    expect(rows).toHaveLength(17);
    expect(rows[0]!.attributes('aria-setsize')).toBe('10000');
    expect(list.get('.spacer').attributes('style')).toContain('height: 500000px');
    await scrollTo(list, 499_400);
    expect(items(list).at(-1)!.attributes('aria-posinset')).toBe('10000');
    list.unmount();
  });

  it('recycles row elements while scrolling instead of creating new ones', async () => {
    const list = await listOf({ extraFaces: 9997 }, { rowHeight: 50, overscan: 2 });
    await scrollTo(list, 5000);
    const before = items(list).map((r) => r.element);
    const keysBefore = before.map((el) => el.getAttribute('data-key'));
    await scrollTo(list, 5150); // three rows further
    const after = items(list).map((r) => r.element);
    expect(after).toHaveLength(before.length);
    // The same elements, three of them now showing the three new rows.
    expect(new Set(after)).toEqual(new Set(before));
    const keysAfter = after.map((el) => el.getAttribute('data-key'));
    expect(keysAfter.filter((k) => !keysBefore.includes(k))).toHaveLength(3);
    // Every row's preview row was patched in place, not remounted.
    const rowsBefore = list.findAll('.row').map((r) => r.element);
    await scrollTo(list, 5300);
    expect(new Set(list.findAll('.row').map((r) => r.element))).toEqual(new Set(rowsBefore));
    list.unmount();
  });

  // 200 scroll steps over 1,000 families: about 4 s on a fast machine, more
  // than the 5 s default on the CI Mac. A leak fails the checks, not the clock.
  it('does not leak font faces while 1,000 families scroll past', { timeout: 30_000 }, async () => {
    const list = await listOf({ extraFaces: 997 }, { rowHeight: 50, overscan: 2 });
    let most = 0;
    const seen = new Set<string>();
    for (let top = 0; top < 50_000; top += 250) {
      await scrollTo(list, top);
      const rendered = list.findAll('.row').length;
      // Only the rows on screen hold a face; at most MAX_IDLE released ones stay loaded.
      expect(loader.inUse).toBe(rendered);
      expect(fonts.faces.size - loader.inUse).toBeLessThanOrEqual(MAX_IDLE);
      most = Math.max(most, fonts.faces.size);
      for (const f of fonts.faces) seen.add(f.family);
    }
    expect(items(list).at(-1)!.attributes('aria-posinset')).toBe('1000');
    expect(seen.size).toBe(1000);
    expect(most).toBeLessThanOrEqual(17 + MAX_IDLE);
    list.unmount();
    expect(loader.inUse).toBe(0);
    loader.trim();
    expect(fonts.faces.size).toBe(0);
  });
});

describe('FontList families and styles', () => {
  it('a family whose font states an open license shows the "Open license" badge', async () => {
    const list = await listOf({});
    const row = (name: string) =>
      items(list).find((r) => r.attributes('aria-label')?.startsWith(`${name},`))!;
    const badge = row('Inter').get('[data-testid="open-license"]');
    expect(badge.text()).toBe('Open license');
    expect(badge.classes()).toContain('open-license');
    expect(badge.attributes('title')).toBe('SIL Open Font License');
    expect(row('Legacy Grotesk').find('[data-testid="open-license"]').exists()).toBe(false);
    list.unmount();
  });

  it('a family row previews its first style and says how many it has', async () => {
    const list = await listOf({ extraFaces: 1, stylesPerFamily: 4 });
    const row = list.get(`[data-key="family:${EXTRA_ID_BASE}"]`);
    expect(row.attributes('aria-expanded')).toBe('false');
    expect(row.attributes('aria-label')).toBe('Sample 1, 4 styles, not active');
    expect(row.get('.row').attributes('data-face')).toBe(String(EXTRA_ID_BASE));
    expect(row.text()).toContain('4 styles');
    list.unmount();
  });

  it('announces the activation state of a style and its family, and follows changes', async () => {
    const list = await listOf({ extraFaces: 1, stylesPerFamily: 4 });
    await list.get(`[data-key="family:${EXTRA_ID_BASE}"] .twisty`).trigger('click');
    await flushPromises();
    const label = (key: string) => list.get(`[data-key="${key}"]`).attributes('aria-label');
    const faces = useLibraryStore().faces.get(EXTRA_ID_BASE)!;
    const bold = faces.find((f) => f.style === 'Bold')!;
    await useClient().activate({ target: { kind: 'faces', faces: [bold.id] }, mode: 'session' });
    await flushPromises();
    expect(label(`face:${bold.id}`)).toBe('Sample 1 Bold, active for this session');
    expect(label(`family:${EXTRA_ID_BASE}`)).toBe('Sample 1, 4 styles, partly active');
    const others = faces.filter((f) => f.id !== bold.id).map((f) => f.id);
    await useClient().activate({ target: { kind: 'faces', faces: others }, mode: 'permanent' });
    await flushPromises();
    expect(label(`face:${others[0]}`)).toMatch(/^Sample 1 \S.*, active permanently$/);
    expect(label(`family:${EXTRA_ID_BASE}`)).toBe('Sample 1, 4 styles, active');
    await useLibraryStore().stop();
    list.unmount();
  });

  it('expanding a family shows its styles inline under it', async () => {
    const list = await listOf({ extraFaces: 2, stylesPerFamily: 4 });
    await list.get(`[data-key="family:${EXTRA_ID_BASE}"] .twisty`).trigger('click');
    await flushPromises();
    const rows = items(list);
    expect(rows.map((r) => r.attributes('aria-label'))).toEqual([
      'Inter, 1 style, not active',
      'JetBrains Mono, 1 style, not active',
      'Legacy Grotesk, 1 style, not active',
      'Sample 1, 4 styles, not active',
      'Sample 1 Regular, not active',
      'Sample 1 Italic, not active',
      'Sample 1 Bold, not active',
      'Sample 1 Bold Italic, not active',
      'Sample 2, 4 styles, not active',
    ]);
    expect(rows[3]!.attributes('aria-expanded')).toBe('true');
    expect(rows[5]!.attributes()).toMatchObject({
      'aria-level': '2',
      'aria-posinset': '2',
      'aria-setsize': '4',
    });
    expect(rows[5]!.get('.row').attributes('data-face')).toBe(String(EXTRA_ID_BASE + 2));
    await list.get(`[data-key="family:${EXTRA_ID_BASE}"] .twisty`).trigger('click');
    expect(items(list)).toHaveLength(5);
    list.unmount();
  });
});

describe('FontList keyboard and selection', () => {
  async function keyed() {
    const list = await listOf({ extraFaces: 2, stylesPerFamily: 4 });
    const tree = list.get('[role="tree"]');
    const press = async (key: string, mods: KeyboardEventInit = {}) => {
      await tree.trigger('keydown', { key, ...mods });
      await flushPromises();
    };
    const active = () => tree.attributes('aria-activedescendant');
    const selected = () =>
      list.findAll('[aria-selected="true"]').map((r) => r.attributes('data-key'));
    return { list, tree, press, active, selected };
  }

  it('arrows, Home and End move the cursor and select the row', async () => {
    const { list, press, active, selected } = await keyed();
    await press('ArrowDown');
    expect(active()).toBe('fm-row-family-1');
    await press('ArrowDown');
    await press('ArrowDown');
    await press('ArrowUp');
    expect(active()).toBe('fm-row-family-3');
    expect(selected()).toEqual(['family:3']);
    await press('End');
    expect(active()).toBe(`fm-row-family-${EXTRA_ID_BASE + 1}`);
    await press('Home');
    expect(active()).toBe('fm-row-family-1');
    expect(list.emitted('cursorFace')!.at(-1)).toEqual([1]);
    list.unmount();
  });

  it('Enter expands and collapses a family; Right and Left do too', async () => {
    const { list, press, active } = await keyed();
    await press('End');
    await press('ArrowUp'); // Sample 1
    await press('Enter');
    expect(list.get(`#fm-row-family-${EXTRA_ID_BASE}`).attributes('aria-expanded')).toBe('true');
    expect(items(list)).toHaveLength(9);
    await press('Enter');
    expect(items(list)).toHaveLength(5);
    await press('ArrowRight');
    expect(items(list)).toHaveLength(9);
    await press('ArrowRight'); // into the first style
    expect(active()).toBe(`fm-row-face-${EXTRA_ID_BASE}`);
    await press('ArrowLeft'); // back to the family
    await press('ArrowLeft'); // collapse
    expect(items(list)).toHaveLength(5);
    list.unmount();
  });

  it('Enter on a style opens it; the cursor face follows the row', async () => {
    const { list, press } = await keyed();
    await press('End');
    await press('ArrowUp');
    await press('ArrowRight');
    await press('ArrowDown');
    await press('ArrowDown'); // Italic
    await press('Enter');
    expect(list.emitted('open')).toEqual([[EXTRA_ID_BASE + 2]]);
    expect(list.emitted('cursorFace')!.at(-1)).toEqual([EXTRA_ID_BASE + 2]);
    list.unmount();
  });

  it('Shift+arrows select a range; Ctrl+arrows move alone; Ctrl+Space toggles; Ctrl+A all', async () => {
    const { list, press, selected } = await keyed();
    await press('ArrowDown');
    await press('ArrowDown', { shiftKey: true });
    await press('ArrowDown', { shiftKey: true });
    expect(selected()).toEqual(['family:1', 'family:3', 'family:4']);
    await press('ArrowDown', { ctrlKey: true });
    await press('ArrowDown', { metaKey: true });
    expect(selected()).toHaveLength(3);
    await press(' ', { ctrlKey: true });
    expect(selected()).toEqual(['family:1', 'family:3', 'family:4', `family:${EXTRA_ID_BASE + 1}`]);
    await press(' '); // plain Space is quick-look: the list leaves it alone
    expect(selected()).toHaveLength(4);
    await press('a', { ctrlKey: true });
    expect(selected()).toHaveLength(5);
    list.unmount();
  });

  it('clicks select one row, Ctrl/Cmd+click toggles, Shift+click selects a range', async () => {
    const { list, tree, selected } = await keyed();
    const row = (key: string) => list.get(`[data-key="${key}"]`);
    await row('family:3').trigger('click');
    await row(`family:${EXTRA_ID_BASE + 1}`).trigger('click', { ctrlKey: true });
    expect(selected()).toEqual(['family:3', `family:${EXTRA_ID_BASE + 1}`]);
    await row('family:1').trigger('click', { metaKey: true });
    await row('family:1').trigger('click', { metaKey: true });
    expect(selected()).toHaveLength(2);
    await row('family:4').trigger('click');
    await row(`family:${EXTRA_ID_BASE + 1}`).trigger('click', { shiftKey: true });
    expect(selected()).toEqual([
      'family:4',
      `family:${EXTRA_ID_BASE}`,
      `family:${EXTRA_ID_BASE + 1}`,
    ]);
    expect(document.activeElement).toBe(tree.element);
    list.unmount();
  });

  it('scrolls the cursor row into view', async () => {
    const list = await listOf({ extraFaces: 997 }, { rowHeight: 50, overscan: 2 });
    const tree = list.get('[role="tree"]');
    await tree.trigger('keydown', { key: 'End' });
    await flushPromises();
    expect((tree.element as HTMLElement).scrollTop).toBe(1000 * 50 - 600);
    expect(tree.attributes('aria-activedescendant')).toBe(`fm-row-family-${EXTRA_ID_BASE + 996}`);
    list.unmount();
  });
});

describe('FontList preview settings', () => {
  const settings = (patch: Partial<PreviewSettings>): PreviewSettings => ({
    ...DEFAULT_PREVIEW,
    ...patch,
  });

  it('applies the sample, size, spacing, alignment and colors to every row', async () => {
    const list = await listOf(
      {},
      {
        preview: settings({
          sample: 'alphabet',
          sizePx: 96,
          lineHeight: 1.5,
          trackingEm: 0.05,
          align: 'end',
          textColor: '#aa0000',
          backgroundColor: '#00ff00',
          surface: 'dark',
        }),
      },
    );
    const samples = list.findAll('.sample');
    expect(samples).toHaveLength(3);
    for (const sample of samples) {
      const style = (sample.element as HTMLElement).style;
      expect(sample.text()).toBe(ALPHABET);
      expect(style.fontSize).toBe('96px');
      expect(style.lineHeight).toBe('1.5');
      expect(style.letterSpacing).toBe('0.05em');
      expect(style.textAlign).toBe('end');
      expect(style.color).toBe('rgb(170, 0, 0)');
    }
    const tree = list.get('[role="tree"]');
    expect((tree.element as HTMLElement).style.backgroundColor).toBe('rgb(0, 255, 0)');
    expect(tree.classes()).toContain('surface-dark');
    // 96 px × 1.5 + label and padding.
    expect(list.get('[role="treeitem"]').attributes('style')).toContain('height: 174px');
    list.unmount();
  });

  it('updates all rows live when the settings change', async () => {
    const list = await listOf({}, { preview: settings({}) });
    expect(list.get('[role="tree"]').classes()).not.toContain('surface-light');
    await list.setProps({
      preview: settings({ sample: 'pangram', language: 'fr', sizePx: 12, surface: 'light' }),
    });
    for (const sample of list.findAll('.sample')) {
      expect(sample.text()).toBe(pangramOf('fr'));
      expect((sample.element as HTMLElement).style.fontSize).toBe('12px');
    }
    expect(list.get('[role="tree"]').classes()).toContain('surface-light');
    list.unmount();
  });

  it('shows each face its own name for the font-name sample', async () => {
    const list = await listOf({}, { preview: settings({ sample: 'fontName' }) });
    const texts = list.findAll('.sample').map((s) => s.text());
    expect(texts).toEqual(['Inter Regular', 'JetBrains Mono Regular', 'Legacy Grotesk Regular']);
    list.unmount();
  });
});

describe('FontList dynamic previews', () => {
  const custom = (customText: string): PreviewSettings => ({
    ...DEFAULT_PREVIEW,
    sample: 'custom',
    customText,
  });
  const rows = (list: VueWrapper) =>
    items(list).map((item) => ({
      text: item.get('.sample').text(),
      hint: item.find('[data-testid="fallback-hint"]'),
    }));

  it('a face that lacks characters of the sample previews its own letters, with a hint', async () => {
    // Inter and JetBrains Mono map Cyrillic; Legacy Grotesk maps basic Latin only.
    const list = await listOf({}, { preview: custom('Љубав') });
    const [inter, mono, legacy] = rows(list);
    expect(inter!.text).toBe('Љубав');
    expect(inter!.hint.exists()).toBe(false);
    expect(mono!.text).toBe('Љубав');
    expect(mono!.hint.exists()).toBe(false);
    expect(legacy!.text).toBe('abcdefghijklmnopqrstuvwxyz');
    expect(legacy!.hint.text()).toBe('Fallback');
    expect(legacy!.hint.attributes('title')).toBe(
      'This font lacks Љ у б а в, so it shows its own letters.',
    );
    list.unmount();
  });

  it('follows the sample live: covered text drops the fallback again', async () => {
    const list = await listOf({}, { preview: custom('Љубав') });
    expect(rows(list)[2]!.hint.exists()).toBe(true);
    await list.setProps({ preview: custom('Love') });
    await flushPromises();
    expect(rows(list).map((r) => r.text)).toEqual(['Love', 'Love', 'Love']);
    expect(list.find('[data-testid="fallback-hint"]').exists()).toBe(false);
    list.unmount();
  });

  it('asks the API once per sample for the rows on screen', async () => {
    const mock = createMockClient();
    const calls: { faces: number[]; text: string }[] = [];
    setClient({
      ...mock,
      sampleTexts: (req) => {
        calls.push({ faces: [...req.faces], text: req.text });
        return mock.sampleTexts(req);
      },
    });
    await useLibraryStore().load();
    const list = mount(FontList, { props: { preview: custom('Љубав') }, attachTo: document.body });
    await flushPromises();
    expect(calls).toEqual([{ faces: [1, 6, 7], text: 'Љубав' }]);
    await list.setProps({ preview: { ...custom('Љубав'), sizePx: 40 } });
    await flushPromises();
    expect(calls).toHaveLength(1);
    list.unmount();
  });
});

describe('FontList auto size (E16.2)', () => {
  const auto = (autoSize: PreviewSettings['autoSize']): PreviewSettings => ({
    ...DEFAULT_PREVIEW,
    autoSize,
  });
  const sizes = (list: VueWrapper) =>
    list
      .findAll('.sample')
      .map((s) => Number(parseFloat((s.element as HTMLElement).style.fontSize).toFixed(3)));
  const px = (n: number) => Number(n.toFixed(3));

  function counting(options: MockOptions = {}) {
    const mock = createMockClient(options);
    const calls: { faces: number[]; basis: string }[] = [];
    setClient({
      ...mock,
      sizeFactors: (req) => {
        calls.push({ faces: [...req.faces], basis: req.basis });
        return mock.sizeFactors(req);
      },
    });
    return calls;
  }

  it('scales each row by its face factor; a face without the metric keeps the size', async () => {
    const calls = counting();
    await useLibraryStore().load();
    const list = mount(FontList, { props: { preview: auto('xHeight') }, attachTo: document.body });
    await flushPromises();
    // Faces 1, 6, 7: x-height 520/1000, 550/1000, none. 28 px × 0.5 / em.
    expect(sizes(list)).toEqual([px((28 * 0.5) / 0.52), px((28 * 0.5) / 0.55), 28]);
    expect(calls).toEqual([{ faces: [1, 6, 7], basis: 'xHeight' }]);

    await list.setProps({ preview: auto('capHeight') });
    await flushPromises();
    // Cap height 720/1000, 730/1000, 1434/2048. 28 px × 0.7 / em.
    expect(sizes(list)).toEqual([
      px((28 * 0.7) / 0.72),
      px((28 * 0.7) / 0.73),
      px((28 * 0.7) / (1434 / 2048)),
    ]);
    expect(calls).toHaveLength(2);

    await list.setProps({ preview: auto('off') });
    await flushPromises();
    expect(sizes(list)).toEqual([28, 28, 28]);
    expect(calls).toHaveLength(2);
    list.unmount();
  });

  it('asks once for the page on screen, not per row, and not again on a size change', async () => {
    const calls = counting({ extraFaces: 997 });
    await useLibraryStore().load();
    const list = mount(FontList, {
      props: { preview: auto('xHeight'), rowHeight: 50, overscan: 2 },
      attachTo: document.body,
    });
    await flushPromises();
    expect(items(list)).toHaveLength(17);
    expect(calls).toHaveLength(1);
    expect(calls[0]!.faces.length).toBeLessThanOrEqual(17);
    await list.setProps({ preview: { ...auto('xHeight'), sizePx: 60 } });
    await flushPromises();
    expect(calls).toHaveLength(1);
    await scrollTo(list, 50 * 400);
    expect(calls).toHaveLength(2);
    expect(calls[1]!.faces.length).toBeLessThanOrEqual(17);
    list.unmount();
  });

  it('asks nothing while auto size is off', async () => {
    const calls = counting();
    await useLibraryStore().load();
    const list = mount(FontList, { props: { preview: auto('off') }, attachTo: document.body });
    await flushPromises();
    expect(calls).toEqual([]);
    list.unmount();
  });
});

describe('FontList density', () => {
  it('pads rows by the density, keeps the sample size and the cursor row in view', async () => {
    const list = await listOf(
      { extraFaces: 997 },
      { preview: { ...DEFAULT_PREVIEW }, overscan: 2 },
    );
    const tree = list.get('[role="tree"]');
    const el = tree.element as HTMLElement;
    const settings = useSettingsStore();
    await settings.load();
    await flushPromises();
    // 28 px × 1.2 sample line + 30 px label and padding.
    expect(items(list)[0]!.attributes('style')).toContain('height: 64px');
    expect(list.get('.spacer').attributes('style')).toContain(`height: ${1000 * 64}px`);
    const comfortable = items(list).length;

    const store = useFontListStore();
    store.click(store.rows[500]!.key, 'replace');
    el.scrollTop = 500 * 64 - 128;
    await tree.trigger('scroll');
    await flushPromises();

    await settings.setDensity('compact');
    await flushPromises();
    expect(items(list)[0]!.attributes('style')).toContain('height: 54px');
    expect(list.get('.spacer').attributes('style')).toContain(`height: ${1000 * 54}px`);
    expect(el.scrollTop).toBe(500 * 54 - 128);
    expect(list.get('.slot.cursor').attributes('data-key')).toBe(store.rows[500]!.key);
    // The sample keeps the size the user chose.
    expect(list.get('.sample').attributes('style')).toContain('font-size: 28px');
    // Only a window of rows, a few more than before.
    expect(items(list).length).toBeGreaterThan(comfortable);
    expect(items(list).length).toBeLessThanOrEqual(Math.ceil(600 / 54) + 1 + 4);
    list.unmount();
  });
});
