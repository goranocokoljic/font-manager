import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import { useSelectionStore } from '../stores/selection';
import { useTagsStore } from '../stores/tags';
import HealthBadge from './HealthBadge.vue';
import InspectorInfo from './InspectorInfo.vue';

const copyText = vi.hoisted(() => vi.fn());
vi.mock('../api/clipboard', () => ({ copyText }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));

async function mountWith(face: number | null) {
  const pinia = createPinia();
  setActivePinia(pinia);
  await useSelectionStore().select(face);
  const wrapper = mount(InspectorInfo, { global: { plugins: [pinia] } });
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  setClient(createMockClient());
  copyText.mockResolvedValue(undefined);
});

afterEach(() => {
  setClient(undefined);
  copyText.mockReset();
  vi.useRealTimers();
});

describe('inspector info tab', () => {
  it('says so when nothing is selected', async () => {
    expect((await mountWith(null)).text()).toContain('Nothing selected');
  });

  it('shows every field of the face detail', async () => {
    const text = (await mountWith(1)).text();
    const d = mockFaces[0];
    for (const value of [
      d.face.familyName,
      d.face.style,
      d.face.postscriptName,
      d.legacyFamily!,
      d.version!,
      d.foundry!,
      d.designer!,
      d.copyright!,
      d.path,
      'TrueType (TTF)',
      '121,000 bytes',
      d.hash,
      'Weight',
      '0°',
      'wght Weight: 100–900 (default 400)',
      'Thin, Regular, Bold, Black',
      '1,000',
      '-250',
      '520',
      '720',
      '187',
      'latn, cyrl',
      'kern, liga, smcp, onum, ss01',
      d.license.description!,
      d.license.url!,
      'SIL Open Font License',
      'Installable',
    ]) {
      expect(text).toContain(value);
    }
    for (const label of ['Vendor ID', 'Tags', 'Favorite', 'Rating', 'Notes', 'Face in file']) {
      expect(text).toContain(label);
    }
  });

  it('edits tags with completion, the favorite toggle and the stars', async () => {
    setClient(createMockClient({ faceTags: { 6: ['Brand', 'Display'] } }));
    const wrapper = await mountWith(1);
    await useTagsStore().refresh();
    await flushPromises();
    // Completion offers the library's tags.
    const options = () => wrapper.findAll('#fm-tag-names option').map((o) => o.attributes('value'));
    expect(options()).toEqual(['Brand', 'Display']);

    const input = wrapper.get('input[aria-label="Add a tag"]');
    await input.setValue('Display');
    await wrapper.get('form.add-tag').trigger('submit');
    await flushPromises();
    expect(wrapper.findAll('.tag').map((t) => t.text().replace('×', '').trim())).toEqual([
      'Display',
    ]);
    expect((input.element as HTMLInputElement).value).toBe('');
    // A tag the face has is no longer offered.
    expect(options()).toEqual(['Brand']);
    await wrapper.get('button[aria-label="Remove tag Display"]').trigger('click');
    await flushPromises();
    expect(wrapper.findAll('.tag')).toHaveLength(0);

    const favorite = wrapper.get('button.favorite');
    expect(favorite.attributes('aria-pressed')).toBe('false');
    await favorite.trigger('click');
    await flushPromises();
    expect(wrapper.get('button.favorite').attributes('aria-pressed')).toBe('true');
    expect(useSelectionStore().detail!.favorite).toBe(true);

    await wrapper.get('button[aria-label="4 stars"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[data-testid="rating"]').text()).toBe('4 of 5');
    expect(wrapper.get('button[aria-label="4 stars"]').attributes('aria-pressed')).toBe('true');
    expect(wrapper.findAll('.star.on')).toHaveLength(4);
    // Choosing the same rating clears it.
    await wrapper.get('button[aria-label="4 stars"]').trigger('click');
    await flushPromises();
    expect(wrapper.get('[data-testid="rating"]').text()).toBe('Not rated');
  });

  it('shows why a tag was refused', async () => {
    const wrapper = await mountWith(1);
    await wrapper.get('input[aria-label="Add a tag"]').setValue('x');
    setClient({
      ...createMockClient(),
      tagFaces: () => Promise.reject({ code: 'invalidRequest', message: 'No.', detail: null }),
    });
    await wrapper.get('form.add-tag').trigger('submit');
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toBe('No.');
  });

  it('copies names and the path on click', async () => {
    const wrapper = await mountWith(1);
    for (const value of ['Inter', 'Inter-Regular', mockFaces[0].path]) {
      await wrapper.get(`button[aria-label$=": ${value}"]`).trigger('click');
      await flushPromises();
      expect(copyText).toHaveBeenLastCalledWith(value);
    }
    expect(wrapper.text()).toContain('Copied');
  });

  it('shows and copies the name ID 1 and 2 values apart from the typographic names', async () => {
    const mock = createMockClient();
    setClient({
      ...mock,
      getFace: async (id) => ({
        ...(await mock.getFace(id)),
        legacyFamily: 'Inter Display',
        legacyStyle: 'Book',
      }),
    });
    const wrapper = await mountWith(1);
    const row = (label: string) => {
      const dt = wrapper.findAll('dt').find((el) => el.text() === label);
      return dt?.element.nextElementSibling?.textContent ?? '';
    };
    expect(row('Family (name ID 1)')).toContain('Inter Display');
    expect(row('Style (name ID 2)')).toContain('Book');
    await wrapper.get('button[aria-label^="Copy family (name ID 1)"]').trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith('Inter Display');
    await wrapper.get('button[aria-label^="Copy style (name ID 2)"]').trigger('click');
    await flushPromises();
    expect(copyText).toHaveBeenLastCalledWith('Book');
  });

  it('shows a dash when the name ID 1 and 2 values are absent', async () => {
    const mock = createMockClient();
    setClient({
      ...mock,
      getFace: async (id) => ({
        ...(await mock.getFace(id)),
        legacyFamily: null,
        legacyStyle: null,
      }),
    });
    const wrapper = await mountWith(1);
    expect(wrapper.find('button[aria-label^="Copy family (name ID 1)"]').exists()).toBe(false);
    const dt = wrapper.findAll('dt').find((el) => el.text() === 'Style (name ID 2)');
    expect(dt?.element.nextElementSibling?.textContent?.trim()).toBe('—');
  });

  it('says when the clipboard refuses', async () => {
    copyText.mockRejectedValue(new Error('denied'));
    const wrapper = await mountWith(1);
    await wrapper.get('button[aria-label^="Copy path"]').trigger('click');
    await flushPromises();
    expect(wrapper.text()).toContain('Could not copy');
  });

  it('activates and deactivates the face from the button', async () => {
    const wrapper = await mountWith(1);
    const button = () => wrapper.get('button.activate');
    expect(button().text()).toBe('Activate');
    expect(wrapper.get('[data-testid="activation-state"]').text()).toBe('Not active');
    await button().trigger('click');
    await flushPromises();
    expect(button().text()).toBe('Deactivate');
    expect(button().attributes('aria-pressed')).toBe('true');
    expect(wrapper.get('[data-testid="activation-state"]').text()).toBe('Active for this session');
    await button().trigger('click');
    await flushPromises();
    expect(button().text()).toBe('Activate');
  });

  it('asks about a clashing face, then shows why it was not activated', async () => {
    const wrapper = await mountWith(6);
    await wrapper.get('button.activate').trigger('click');
    await flushPromises();
    const question = wrapper.get('[role="alertdialog"]');
    expect(question.text()).toContain('/System/Library/Fonts/JetBrainsMono-Regular.ttf');
    const choices = question.findAll('button').map((b) => b.text());
    expect(choices).toEqual(['Keep existing', 'Replace for this session', 'Cancel']);
    await question.findAll('button')[0]!.trigger('click');
    await flushPromises();
    expect(wrapper.find('[role="alertdialog"]').exists()).toBe(false);
    expect(wrapper.get('[role="alert"]').text()).toContain(
      '/System/Library/Fonts/JetBrainsMono-Regular.ttf',
    );
  });

  it('disables activation for an unsupported face and shows its health', async () => {
    const wrapper = await mountWith(7);
    expect(wrapper.get('button.activate').attributes('disabled')).toBeDefined();
    const badge = wrapper.get('[data-testid="health-badge"]');
    expect(badge.text()).toContain('Unsupported');
    expect(badge.text()).toContain('Type 1 fonts cannot be activated');
  });

  it('shows the API error when the face cannot be loaded', async () => {
    const wrapper = await mountWith(999);
    expect(wrapper.get('[role="alert"]').text()).toBe('Face 999 was not found.');
  });
});

describe('health badge', () => {
  it('labels each health kind', () => {
    expect(mount(HealthBadge, { props: { health: { kind: 'ok' } } }).text()).toBe('OK');
    const broken = mount(HealthBadge, {
      props: { health: { kind: 'broken', reason: 'The glyf table is truncated.' } },
    });
    expect(broken.text()).toContain('Broken');
    expect(broken.text()).toContain('The glyf table is truncated.');
    expect(broken.get('.badge').classes()).toContain('broken');
  });
});
