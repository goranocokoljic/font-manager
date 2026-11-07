import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient, mockLanguages } from '../api/mock';
import type { Collection, FaceDetail, FaceLanguageCoverage, Family } from '../api/types';
import { useFamilyRailStore } from '../stores/familyRail';
import FamilyRail from './FamilyRail.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const USER_TEXT = 'My own words';
let client: FmClient;

const lang = (code: string, name: string, covered = 10): FaceLanguageCoverage => ({
  code,
  name,
  covered,
  total: 10,
  hasLocl: false,
});
const EIGHT = [
  lang('en', 'English'),
  lang('pl', 'Polish'),
  lang('de', 'German'),
  lang('fr', 'French'),
  lang('cs', 'Czech'),
  lang('hr', 'Croatian'),
  lang('sk', 'Slovak'),
  lang('sl', 'Slovenian'),
  lang('el', 'Greek', 4),
];

async function mountWith(
  opts: {
    languages?: FaceLanguageCoverage[];
    family?: Partial<Family>;
    detail?: Partial<FaceDetail>;
  } = {},
) {
  client = createMockClient({ extraFaces: 20 });
  setClient(client);
  vi.spyOn(client, 'faceCoverage').mockImplementation(async (face) => ({
    face,
    blocks: [],
    languages: opts.languages ?? EIGHT,
  }));
  const pinia = createPinia();
  setActivePinia(pinia);
  const detail = { ...(await client.getFace(1)), ...opts.detail };
  const family = { ...(await client.getFamily(detail.face.family)), ...opts.family };
  const collections: Collection[] = [
    { id: 7, parent: null, name: 'Client brand', rules: null, faceCount: 3 },
    { id: 3, parent: null, name: 'Editorial', rules: null, faceCount: 2 },
    { id: 9, parent: null, name: 'Unused', rules: null, faceCount: 0 },
  ];
  await useFamilyRailStore().load(1);
  const wrapper = mount(FamilyRail, {
    props: {
      face: 1,
      detail,
      family,
      summary: family.summary,
      presets: mockLanguages,
      collections,
      text: USER_TEXT,
      variation: '',
    },
    global: { plugins: [pinia] },
    attachTo: document.body,
  });
  await flushPromises();
  return wrapper;
}
type W = Awaited<ReturnType<typeof mountWith>>;
const chips = (w: W) => w.findAll('[data-testid="rail-language"]');
const chip = (w: W, code: string) => w.get(`[data-testid="rail-language"][data-code="${code}"]`);
const text = (w: W, id: string) => w.get(`[data-testid="${id}"]`).text();

afterEach(() => {
  setClient(undefined);
  document.body.innerHTML = '';
});

describe('family page rail', () => {
  it('"Supports N languages": complete ones only, six chips, then "+N more" expands', async () => {
    const w = await mountWith();
    expect(w.get('#fm-rail-languages').text()).toBe('Supports 8 languages');
    expect(chips(w).map((c) => c.text())).toEqual([
      'English',
      'Polish',
      'German',
      'French',
      'Czech',
      'Croatian',
    ]);
    const more = w.get('[data-testid="rail-languages-more"]');
    expect(more.text()).toBe('+ 2 more');
    await more.trigger('click');
    expect(chips(w)).toHaveLength(8);
    expect(w.find('[data-testid="rail-languages-more"]').exists()).toBe(false);
    expect(w.text()).not.toContain('Greek');
  });

  it('a chip previews the pangram and takes the accent; again, or back to my text, restores', async () => {
    const w = await mountWith();
    const rail = useFamilyRailStore();
    await chip(w, 'pl').trigger('click');
    expect(rail.language).toBe('pl');
    expect(rail.textOf(mockLanguages)).toBe('Pchnąć w tę łódź jeża lub ośm skrzyń fig');
    expect(chip(w, 'pl').attributes('aria-pressed')).toBe('true');
    expect(chip(w, 'en').attributes('aria-pressed')).toBe('false');
    expect(text(w, 'rail-language-chosen')).toContain(
      'Every specimen on this page now shows Polish.',
    );
    await chip(w, 'pl').trigger('click');
    expect(rail.language).toBeNull();
    expect(w.find('[data-testid="rail-language-chosen"]').exists()).toBe(false);

    await chip(w, 'de').trigger('click');
    await w.get('[data-testid="rail-my-text"]').trigger('click');
    expect(rail.language).toBeNull();
    expect(chip(w, 'de').attributes('aria-pressed')).toBe('false');
  });

  it("under the chips, the language's distinctive glyphs at 22 px in the face", async () => {
    const w = await mountWith();
    const line = w.get('[data-testid="family-rail-sample"]');
    // Nothing chosen: the first complete language with a glyph line (English).
    expect(line.text()).toBe('AaGgQqRrWw');
    await chip(w, 'pl').trigger('click');
    expect(line.text()).toBe('Ąćęłńóśźż');
    // The 22 px size is a scoped style jsdom does not apply: e2e/family-rail.spec.ts checks it.
    // Never a fallback font: FamilySample hides the line until the face has loaded.
    expect(line.classes()).toContain('family-sample');
  });

  it('no complete language: no chips, and the line shows the page text', async () => {
    const w = await mountWith({ languages: [lang('el', 'Greek', 3)] });
    expect(w.find('[data-testid="rail-languages"]').exists()).toBe(false);
    expect(text(w, 'family-rail-sample')).toBe(USER_TEXT);
  });

  it('"Show only fonts that can do Polish" is offered once a language is chosen', async () => {
    const w = await mountWith();
    expect(w.find('[data-testid="rail-filter-wall"]').exists()).toBe(false);
    await chip(w, 'pl').trigger('click');
    const offer = w.get('[data-testid="rail-filter-wall"]');
    expect(offer.text()).toBe('Show only fonts that can do Polish');
    expect(offer.attributes('aria-pressed')).toBe('false');
    await offer.trigger('click');
    expect(offer.attributes('aria-pressed')).toBe('true');
    expect(useFamilyRailStore().takeWallLanguage()).toBe('pl');
  });

  it('"Pairs well with": the top two with one reason each, and Open in Pairing Studio', async () => {
    const w = await mountWith();
    const rows = w.findAll('[data-testid="rail-pairing"]');
    expect(rows).toHaveLength(2);
    const top = useFamilyRailStore().pairings[0]!;
    expect(rows[0]!.get('.pair-name').text()).toBe(top.face.familyName);
    expect(rows[0]!.get('.pair-reason').text()).toBe(top.reasons[0]);
    await w.get('[data-testid="rail-open-studio"]').trigger('click');
    expect(w.emitted('studio')).toEqual([[top]]);
  });

  it('facts: designer, foundry, licence with web and embed marks, format, files, size, used in', async () => {
    const w = await mountWith({
      family: { collections: [3, 7], fileCount: 2, fileBytes: 1_100_000 },
    });
    expect(text(w, 'rail-designer')).not.toBe('—');
    const licence = w.get('[data-testid="rail-licence"]');
    const marks = licence.findAll('.mark');
    expect(marks.map((m) => m.attributes('data-mark'))).toEqual(['web', 'embed']);
    expect(marks[0]!.attributes('aria-label')).toMatch(/^web: (not )?allowed$/);
    expect(text(w, 'rail-format')).toContain('2 files · 1.1 MB');
    // In the sidebar's order; a collection that does not hold the family is not listed.
    const used = w.get('[data-testid="rail-used-in"]').findAll('button');
    expect(used.map((b) => b.text())).toEqual(['Client brand', 'Editorial']);
    await used[1]!.trigger('click');
    expect(w.emitted('collection')).toEqual([[3]]);
  });

  it('a restricted font is marked not for embedding', async () => {
    const base = await (async () => {
      const c = createMockClient({});
      return (await c.getFace(1)).license;
    })();
    const w = await mountWith({ detail: { license: { ...base, embedding: 'restricted' } } });
    const embed = w.get('[data-testid="rail-licence"] [data-mark="embed"]');
    expect(embed.attributes('data-ok')).toBe('false');
    expect(embed.text()).toBe('embed ✗');
  });

  it('"Technical details" is the last control of the rail', async () => {
    const w = await mountWith();
    // In document order: the wrapper of a many-rooted component lists its roots out of order.
    const buttons = [...document.body.querySelectorAll('button')];
    expect(buttons.at(-1)?.textContent?.trim()).toBe('Technical details');
    expect(w.exists()).toBe(true);
  });
});
