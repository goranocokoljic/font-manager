import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { PairingCard } from '../stores/familyPairings';
import FamilyPairings from './FamilyPairings.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

const HEADLINE = 'Headline words';
let client: FmClient;

async function mountWith(extraFaces = 20, face: number | null = 1) {
  client = createMockClient({ extraFaces });
  setClient(client);
  const pinia = createPinia();
  setActivePinia(pinia);
  const wrapper = mount(FamilyPairings, {
    props: { face, text: HEADLINE },
    global: { plugins: [pinia] },
  });
  await flushPromises();
  return wrapper;
}
type W = Awaited<ReturnType<typeof mountWith>>;
const cards = (w: W) => w.findAll('[data-testid="pairing-card"]');
const segment = (w: W, label: string) =>
  w
    .get('[role="group"]')
    .findAll('button')
    .find((b) => b.text() === label) ?? w.get('nothing');

afterEach(() => setClient(undefined));

describe('family page pairings section', () => {
  it('shows up to eight cards: the family at 30 px over the candidate at 14 px, name and reasons', async () => {
    const w = await mountWith();
    const all = cards(w);
    expect(all).toHaveLength(8);
    const first = all[0]!;
    expect(first.attributes('data-headline')).toBe('1');
    expect(first.attributes('data-body')).toBe('1000');
    expect(first.get('.headline').text()).toBe(HEADLINE);
    // A short preview text reads as the stock passage in the body.
    expect(first.get('.body').text().length).toBeGreaterThan(80);
    expect(first.get('.name').text()).toBe('Sample 1');
    expect(first.get('.reasons').text()).toBe(
      'lower contrast for body · same designer · x-height within 1 %',
    );
  });

  it('the switch asks for headline faces and swaps the parts', async () => {
    const w = await mountWith();
    const spy = vi.spyOn(client, 'suggestPairings');
    expect(segment(w, 'for headlines').attributes('aria-pressed')).toBe('true');
    await segment(w, 'for body').trigger('click');
    await flushPromises();
    expect(spy).toHaveBeenCalledWith({ face: 1, limit: 8, role: 'headline' });
    const first = cards(w)[0]!;
    expect(first.attributes('data-headline')).toBe('1000');
    expect(first.attributes('data-body')).toBe('1');
    expect(first.get('.reasons').text()).toBe('same designer · x-height within 1 %');
  });

  it('a click on a card asks to open it in the Studio', async () => {
    const w = await mountWith();
    await cards(w)[1]!.trigger('click');
    const opened = w.emitted('open')?.[0]?.[0] as PairingCard;
    expect(opened.headline).toBe(1);
    expect(opened.body).toBe(1002);
  });

  it('with no candidate: one line and a way to browse catalogs', async () => {
    const w = await mountWith(0);
    expect(cards(w)).toHaveLength(0);
    const empty = w.get('[data-testid="pairings-empty"]');
    expect(empty.findAll('p')).toHaveLength(1);
    expect(empty.text()).toContain('No font in your library pairs with this family yet.');
    await w.get('[data-testid="pairings-browse"]').trigger('click');
    expect(w.emitted('browse')).toHaveLength(1);
  });

  it('says why when the suggestions could not be read', async () => {
    client = createMockClient({ extraFaces: 20 });
    vi.spyOn(client, 'suggestPairings').mockRejectedValue({
      code: 'io',
      message: 'disk gone',
      detail: null,
    });
    setClient(client);
    const pinia = createPinia();
    setActivePinia(pinia);
    const w = mount(FamilyPairings, {
      props: { face: 1, text: HEADLINE },
      global: { plugins: [pinia] },
    });
    await flushPromises();
    expect(w.get('[role="alert"]').text()).toContain('disk gone');
    expect(w.find('[data-testid="pairings-empty"]').exists()).toBe(false);
  });
});
