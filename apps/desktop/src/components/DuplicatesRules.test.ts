import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { KeeperPolicy, PreviewKeeperPolicyRequest } from '../api/types';
import { setLocale } from '../i18n';
import { createAppRouter } from '../router';
import { useDuplicatesStore } from '../stores/duplicates';
import { useSettingsStore } from '../stores/settings';
import DuplicatesRules from './DuplicatesRules.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let client: FmClient;
let previews: PreviewKeeperPolicyRequest[];

beforeEach(() => {
  setLocale('en');
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
  previews = [];
  // Numbers no other text on the page has, so each one is the preview's own.
  const real = client.previewKeeperPolicy.bind(client);
  vi.spyOn(client, 'previewKeeperPolicy').mockImplementation(async (req) => {
    previews.push(structuredClone(req));
    const answer = await real(req);
    return { ...answer, withKeeper: 17, keepBoth: 4, changed: 9 };
  });
});

afterEach(() => setClient(undefined));

/** Past the preview's short wait, and every answer in. */
async function settle(): Promise<void> {
  await new Promise((r) => setTimeout(r, 160));
  await flushPromises();
}

async function mountIt(policy?: KeeperPolicy) {
  if (policy) await client.updateSettings({ keeperPolicy: policy });
  await useSettingsStore().load();
  await useDuplicatesStore().load();
  const router = createAppRouter();
  await router.push('/duplicates?section=rules');
  const wrapper = mount(DuplicatesRules, {
    global: { plugins: [router] },
    attachTo: document.body,
  });
  await settle();
  return wrapper;
}
type Wrapper = Awaited<ReturnType<typeof mountIt>>;

const ruleList = (w: Wrapper) => w.get('[data-testid="dup-rules"]');
/** The rule names in the list, in order, with "off" for a rule that is turned off. */
const ruleRows = (w: Wrapper) =>
  ruleList(w)
    .findAll(':scope > li')
    .map((li) => {
      const name = li.get('.rule-name');
      return name.classes('off') ? `${name.text()} (off)` : name.text();
    });
const byLabel = (w: Wrapper, label: string) => w.get(`[aria-label="${label}"]`);
const saveButton = (w: Wrapper) => w.get('[data-testid="dup-policy-save"]');
const lastPreview = () => previews.at(-1)!.policy;

describe('Duplicates › languages and rules (E56.14)', () => {
  it('shows the guard above the rules as always on, not as a row', async () => {
    const w = await mountIt();
    const guard = w.get('[data-testid="dup-guard"]');
    expect(guard.text()).toContain('Language guard');
    expect(guard.text()).toContain('Always on');
    expect(guard.text()).toContain('never recommended for removal');
    expect(guard.find('[role="switch"]').exists()).toBe(false);
    expect(guard.find('button').exists()).toBe(false);
    // Above the rules in the page, and no row of the list names it.
    const pos = guard.element.compareDocumentPosition(ruleList(w).element);
    expect(pos & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(ruleRows(w).some((r) => /guard/i.test(r))).toBe(false);
    w.unmount();
  });

  it('lists the saved rules in order, then the rules turned off; the path rule ends it', async () => {
    const w = await mountIt();
    expect(ruleRows(w)).toEqual([
      'Covers more of my languages',
      'Newest version',
      'Format, in my order',
      'In the library folder',
      'Has local forms (locl) for more of my languages (off)',
      'Variable over static (off)',
      'Most glyphs (off)',
      'In a folder I prefer (off)',
      'Carries more of my favorites, ratings, tags, notes, collections and sets (off)',
      'Active, or asked for by a document (off)',
    ]);
    expect(w.get('.path-rule').text()).toContain('the path, A to Z');
    w.unmount();
  });

  it('moves a rule by keyboard, keeps the focus on it and saves only on Save', async () => {
    const update = vi.spyOn(client, 'updateSettings');
    const w = await mountIt();
    expect(saveButton(w).attributes('disabled')).toBeDefined();
    const grip = byLabel(w, 'Move Covers more of my languages');
    (grip.element as HTMLElement).focus();
    await grip.trigger('keydown', { key: 'ArrowDown' });
    await flushPromises();
    expect(ruleRows(w).slice(0, 2)).toEqual(['Newest version', 'Covers more of my languages']);
    expect(document.activeElement).toBe(byLabel(w, 'Move Covers more of my languages').element);
    expect(w.text()).toContain('Covers more of my languages moved to place 2 of 10.');
    await byLabel(w, 'Move Most glyphs').trigger('keydown', { key: 'Home' });
    await settle();
    expect(ruleRows(w)[0]).toBe('Most glyphs (off)');

    // The preview tried the edited policy; nothing was saved.
    expect(lastPreview().rules).toEqual([
      'newestVersion',
      'myLanguages',
      'format',
      'libraryFolder',
      'path',
    ]);
    expect(update).not.toHaveBeenCalled();
    expect(w.get('[data-testid="dup-policy-unsaved"]').text()).toBe('Not saved yet');
    expect(w.get('[data-testid="dup-policy-preview"]').text()).toBe(
      'With these rules, 17 sets get a keeper, 4 sets are “keep both”, and 9 recommendations change.',
    );

    await saveButton(w).trigger('click');
    await settle();
    expect(update).toHaveBeenCalledTimes(1);
    expect((await client.getSettings()).keeperPolicy?.rules).toEqual([
      'newestVersion',
      'myLanguages',
      'format',
      'libraryFolder',
      'path',
    ]);
    expect(w.find('[data-testid="dup-policy-unsaved"]').exists()).toBe(false);
    w.unmount();
  });

  it('reorders the rules by dragging one onto another', async () => {
    const w = await mountIt();
    const rows = ruleList(w).findAll(':scope > li');
    await rows[0]!.trigger('dragstart');
    await rows[2]!.trigger('dragover');
    await rows[2]!.trigger('drop');
    await settle();
    expect(ruleRows(w).slice(0, 3)).toEqual([
      'Newest version',
      'Format, in my order',
      'Covers more of my languages',
    ]);
    expect(lastPreview().rules?.slice(0, 3)).toEqual(['newestVersion', 'format', 'myLanguages']);
    w.unmount();
  });

  it('turns a rule off and on; a rule turned off leaves the policy', async () => {
    const w = await mountIt();
    await byLabel(w, 'Use the rule “Newest version”').trigger('click');
    await settle();
    expect(lastPreview().rules).toEqual(['myLanguages', 'format', 'libraryFolder', 'path']);
    await byLabel(w, 'Use the rule “Most glyphs”').trigger('click');
    await settle();
    expect(lastPreview().rules).toEqual([
      'myLanguages',
      'format',
      'libraryFolder',
      'mostGlyphs',
      'path',
    ]);
    expect(byLabel(w, 'Use the rule “Most glyphs”').attributes('aria-checked')).toBe('true');
    w.unmount();
  });

  it('opens the format order as its own short list to reorder, remove and add', async () => {
    const w = await mountIt();
    const open = w
      .findAll('button')
      .find((b) => b.text() === '5 formats' && b.attributes('aria-expanded') === 'false')!;
    await open.trigger('click');
    const formats = w.get('[data-testid="dup-formats"]');
    const names = () => formats.findAll('li .rule-name').map((n) => n.text());
    expect(names()).toEqual([
      'OpenType CFF (OTF)',
      'TrueType (TTF)',
      'WOFF2',
      'WOFF',
      'PostScript Type 1',
    ]);
    await byLabel(w, 'Move WOFF2').trigger('keydown', { key: 'ArrowUp' });
    await byLabel(w, 'Remove PostScript Type 1').trigger('click');
    await settle();
    expect(names()).toEqual(['OpenType CFF (OTF)', 'WOFF2', 'TrueType (TTF)', 'WOFF']);
    expect(lastPreview().formatOrder).toEqual(['otf', 'woff2', 'ttf', 'woff']);
    // A move inside the format list leaves the rules where they are.
    expect(ruleRows(w)[2]).toBe('Format, in my order');
    w.unmount();
  });

  it('adds, orders and removes preferred folders', async () => {
    vi.spyOn(client, 'pickFolder')
      .mockResolvedValueOnce('/Fonts/Archive')
      .mockResolvedValueOnce('/Fonts/Best');
    const w = await mountIt();
    await w
      .findAll('button')
      .find((b) => b.text() === '0 folders')!
      .trigger('click');
    const list = () => w.get('[data-testid="dup-folders"]');
    expect(list().text()).toContain('No folder is set.');
    const add = () =>
      list()
        .findAll('button')
        .find((b) => b.text() === 'Add a folder…')!;
    await add().trigger('click');
    await flushPromises();
    await add().trigger('click');
    await flushPromises();
    await byLabel(w, 'Move /Fonts/Best').trigger('keydown', { key: 'ArrowUp' });
    await settle();
    expect(lastPreview().preferredFolders).toEqual(['/Fonts/Best', '/Fonts/Archive']);
    await byLabel(w, 'Remove /Fonts/Archive').trigger('click');
    await settle();
    expect(lastPreview().preferredFolders).toEqual(['/Fonts/Best']);
    w.unmount();
  });

  it('loads the default policy with Back to the default, and saves it only on Save', async () => {
    const w = await mountIt({ rules: ['mostGlyphs', 'path'], formatOrder: ['woff'] });
    expect(ruleRows(w)[0]).toBe('Most glyphs');
    expect(ruleRows(w)[1]).toContain('(off)');
    const update = vi.spyOn(client, 'updateSettings');
    await w
      .findAll('button')
      .find((b) => b.text() === 'Back to the default')!
      .trigger('click');
    await settle();
    expect(ruleRows(w).slice(0, 5)).toEqual([
      'Covers more of my languages',
      'Newest version',
      'Format, in my order',
      'In the library folder',
      'Has local forms (locl) for more of my languages (off)',
    ]);
    expect(update).not.toHaveBeenCalled();
    expect((await client.getSettings()).keeperPolicy?.rules).toEqual(['mostGlyphs', 'path']);
    await saveButton(w).trigger('click');
    await settle();
    const saved = (await client.getSettings()).keeperPolicy;
    expect(saved?.rules).toEqual([
      'myLanguages',
      'newestVersion',
      'format',
      'libraryFolder',
      'path',
    ]);
    expect(saved?.formatOrder).toEqual(['otf', 'ttf', 'woff2', 'woff', 'type1']);
    w.unmount();
  });

  it('shows the languages without editing them, and links to Settings › Languages (E58.2)', async () => {
    let w = await mountIt();
    expect(w.text()).toContain('No language is set.');
    expect(w.text()).toContain('Set your languages under Settings › Languages');
    w.unmount();
    await client.updateSettings({ myLanguages: ['sr-Latn', 'pl'] });
    w = await mountIt();
    const chips = w.get('[data-testid="dup-rules-languages"]');
    expect(chips.findAll('li').map((li) => li.text())).toEqual(['Serbian (Latin)', 'Polish']);
    // No editor: no picker, no remove buttons among the languages.
    expect(w.find('[aria-label="Add a language"]').exists()).toBe(false);
    expect(chips.findAll('button')).toHaveLength(0);
    const link = w.get('[data-testid="dup-rules-change-languages"]');
    expect(link.attributes('href')).toBe('#/settings?section=languages');
    expect(w.text()).not.toContain('Set your languages under');
    w.unmount();
  });
});
