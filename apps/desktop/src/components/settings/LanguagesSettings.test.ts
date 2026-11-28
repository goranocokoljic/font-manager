// Settings › Languages (E58.2): the user's languages as rows to reorder, add and remove,
// each change saved at once; "Then the rest" from the API's answer; the line that says who
// leads while none is set; the way to the Duplicates rules.
import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../../api/client';
import { createMockClient, shellLocale } from '../../api/mock';
import { setLocale } from '../../i18n';
import { createAppRouter } from '../../router';
import { useSettingsStore } from '../../stores/settings';
import { pickOption, selectOptions } from '../ui/testing';
import LanguagesSettings from './LanguagesSettings.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

let client: FmClient;

beforeEach(() => {
  setLocale('en');
  shellLocale.value = '';
  client = createMockClient({ moreLanguages: true });
  setClient(client);
});

afterEach(() => {
  shellLocale.value = '';
  setClient(undefined);
  document.body.innerHTML = '';
});

async function mountIt(myLanguages: string[] = []) {
  if (myLanguages.length) await client.updateSettings({ myLanguages });
  const pinia = createPinia();
  setActivePinia(pinia);
  const router = createAppRouter();
  await router.push('/settings?section=languages');
  await useSettingsStore().load();
  const wrapper = mount(LanguagesSettings, {
    global: { plugins: [pinia, router] },
    attachTo: document.body,
  });
  await router.isReady();
  await flushPromises();
  return { wrapper, router };
}
type Wrapper = Awaited<ReturnType<typeof mountIt>>['wrapper'];

const rows = (w: Wrapper) =>
  w.findAll('[data-testid="my-languages"] > li').map((li) => li.get('.name').text());
const saved = async () => (await client.getSettings()).myLanguages;
const picker = (w: Wrapper) => w.get('[data-testid="language-picker"]');

describe('Settings › Languages', () => {
  it('shows each language with its place, name and distinctive letters, and a remove button', async () => {
    const { wrapper } = await mountIt(['sr-Latn', 'en', 'mt']);
    expect(rows(wrapper)).toEqual(['Serbian (Latin)', 'English', 'Maltese']);
    const first = wrapper.get('[data-testid="my-languages"] > li');
    expect(first.get('.position').text()).toBe('1');
    expect(first.get('[data-testid="my-language-glyphs"]').text()).toBe('ČĆĐŠŽčćđšž');
    // Maltese has no distinctive letters in the mock: no empty glyph line.
    const maltese = wrapper.findAll('[data-testid="my-languages"] > li')[2]!;
    expect(maltese.find('[data-testid="my-language-glyphs"]').exists()).toBe(false);
    expect(first.find('[aria-label="Remove Serbian (Latin)"]').exists()).toBe(true);
    wrapper.unmount();
  });

  it('adds a language from the picker over list_languages and saves it at once', async () => {
    const { wrapper } = await mountIt(['sr-Latn']);
    const offered = (await selectOptions(picker(wrapper))).map(([value]) => value);
    // Every preset but the chosen one, in the API's order.
    const order = (await client.listLanguages()).map((p) => p.code);
    expect(offered).toEqual(order.filter((c) => c !== 'sr-Latn'));
    await pickOption(picker(wrapper), 'el');
    await flushPromises();
    expect(await saved()).toEqual(['sr-Latn', 'el']);
    expect(rows(wrapper)).toEqual(['Serbian (Latin)', 'Greek']);
    wrapper.unmount();
  });

  it('reorders by keyboard from the grip and saves the new order at once', async () => {
    const { wrapper } = await mountIt(['sr-Latn', 'en', 'de']);
    const grip = wrapper.get('[aria-label="Move German"]');
    await grip.trigger('keydown', { key: 'Home' });
    await flushPromises();
    expect(rows(wrapper)).toEqual(['German', 'Serbian (Latin)', 'English']);
    expect(await saved()).toEqual(['de', 'sr-Latn', 'en']);
    expect(document.activeElement?.getAttribute('aria-label')).toBe('Move German');
    wrapper.unmount();
  });

  it('removes a language with its x and saves at once', async () => {
    const { wrapper } = await mountIt(['sr-Latn', 'en']);
    await wrapper.get('[aria-label="Remove Serbian (Latin)"]').trigger('click');
    await flushPromises();
    expect(rows(wrapper)).toEqual(['English']);
    expect(await saved()).toEqual(['en']);
    wrapper.unmount();
  });

  it('while none is set, says which language leads now: the system’s', async () => {
    shellLocale.value = 'sr-Latn';
    const { wrapper } = await mountIt();
    expect(wrapper.find('[data-testid="my-languages"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="my-languages-empty"]').text()).toBe(
      'None yet. Until you add one, the language of your system comes first: Serbian (Latin).',
    );
    expect(picker(wrapper).text()).toContain('Add a language');
    wrapper.unmount();
  });

  it('while none is set and the system’s language has no preset, says the most used lead', async () => {
    shellLocale.value = 'xx';
    const { wrapper } = await mountIt();
    expect(wrapper.get('[data-testid="my-languages-empty"]').text()).toBe(
      'None yet. The language of your system is not in the list, so the most used languages come first.',
    );
    wrapper.unmount();
  });

  it('shows the rest from the API’s answer: what completes the first ten, then what is behind "Show more"', async () => {
    // Five languages that are not most used: the most used then fill places 6 to 10, and the
    // next most used one is behind "Show more", before the others from A to Z.
    const { wrapper } = await mountIt(['sr-Cyrl', 'sr-Latn', 'pl', 'el', 'mt']);
    const list = await client.listLanguages();
    const rest = wrapper.get('[data-testid="languages-rest"]');
    const chips = rest
      .findAll('ul')[0]!
      .findAll('li')
      .map((li) => li.text());
    expect(chips).toEqual(list.slice(5, 10).map((p, i) => `${i + 6} ${p.name}`));
    expect(chips[0]).toBe('6 English');
    const behind = rest.get('[data-testid="languages-behind"]');
    const more = list.slice(10).filter((p) => p.group === 'mostUsed');
    const others = list.slice(10).filter((p) => p.group === 'others');
    expect(more.length).toBeGreaterThan(0);
    expect(others.length).toBeGreaterThan(4);
    expect(behind.findAll('li').map((li) => li.text())).toEqual(more.map((p) => p.name));
    expect(behind.get('.others').text()).toBe(
      `then ${others.length} more, A to Z: ${others
        .slice(0, 4)
        .map((p) => p.name)
        .join(', ')} …`,
    );
    wrapper.unmount();
  });

  it('follows a new answer of list_languages after a change', async () => {
    const { wrapper } = await mountIt(['sr-Cyrl', 'sr-Latn', 'pl', 'el', 'mt']);
    await wrapper.get('[aria-label="Remove Maltese"]').trigger('click');
    await flushPromises();
    const first = wrapper
      .get('[data-testid="languages-rest"] ul')
      .findAll('li')
      .map((li) => li.text());
    expect(first[0]).toBe('5 English');
    expect(first).toHaveLength(6);
    wrapper.unmount();
  });

  it('"Open the rules" opens the Duplicates rules', async () => {
    const { wrapper, router } = await mountIt();
    await wrapper.get('[data-testid="languages-open-rules"]').trigger('click');
    await flushPromises();
    expect(router.currentRoute.value.name).toBe('duplicates');
    expect(router.currentRoute.value.query.section).toBe('rules');
    wrapper.unmount();
  });
});
