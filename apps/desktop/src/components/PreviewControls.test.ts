import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import { mockLanguages } from '../api/mock';
import { usePreviewStore } from '../stores/preview';
import PreviewControls from './PreviewControls.vue';

beforeEach(() => {
  localStorage.clear();
  setActivePinia(createPinia());
});

function mountControls(view = 'all') {
  return { wrapper: mount(PreviewControls, { props: { view } }), store: usePreviewStore() };
}

describe('PreviewControls', () => {
  it('chooses the sample kind, the pangram language and custom text', async () => {
    const { wrapper, store } = mountControls();
    await wrapper.get('select').setValue('pangram');
    await flushPromises();
    const language = wrapper.findAll('select')[1]!;
    // The picker lists the API's languages that have a pangram, by name.
    const withPangram = mockLanguages.filter((l) => l.pangram);
    expect(language.findAll('option').map((o) => [o.attributes('value'), o.text()])).toEqual(
      withPangram.map((l) => [l.code, l.name]),
    );
    expect(language.findAll('option').some((o) => o.text() === 'Maltese')).toBe(false);
    await language.setValue('sr-Cyrl');
    expect(store.settingsFor('all').language).toBe('sr-Cyrl');
    await wrapper.get('select').setValue('custom');
    await wrapper.get('input[type="text"]').setValue('Handgloves');
    expect(store.settingsFor('all')).toMatchObject({ sample: 'custom', customText: 'Handgloves' });
  });

  it('applies a typed size only inside the bounds and clamps it on change', async () => {
    const { wrapper, store } = mountControls();
    const size = wrapper.get('input[aria-label="Size in pixels"]');
    const type = async (text: string) => {
      (size.element as HTMLInputElement).value = text;
      await size.trigger('input');
    };
    await type('1'); // on the way to "120": not applied yet
    expect(store.settingsFor('all').sizePx).toBe(28);
    await type('120');
    expect(store.settingsFor('all').sizePx).toBe(120);
    (size.element as HTMLInputElement).value = '5000';
    await size.trigger('change');
    expect(store.settingsFor('all').sizePx).toBe(300);
    expect((size.element as HTMLInputElement).value).toBe('300');
  });

  it('sets line height, tracking, alignment, colors and surface', async () => {
    const { wrapper, store } = mountControls();
    const numbers = wrapper.findAll('input[type="number"]');
    await numbers[1]!.setValue('1.6');
    await numbers[2]!.setValue('0.1');
    await wrapper.get('button[aria-pressed="false"]:nth-of-type(2)').trigger('click');
    await wrapper.get('input[aria-label="Text color"]').setValue('#123456');
    await wrapper.get('input[aria-label="Background color"]').setValue('#fedcba');
    await wrapper.findAll('select').at(-1)!.setValue('dark');
    expect(store.settingsFor('all')).toMatchObject({
      lineHeight: 1.6,
      trackingEm: 0.1,
      align: 'center',
      textColor: '#123456',
      backgroundColor: '#fedcba',
      surface: 'dark',
    });
    expect(wrapper.get('button[aria-pressed="true"]').text()).toBe('Center');
  });

  it('changes only its own view, and Reset puts it back to the defaults', async () => {
    const { wrapper, store } = mountControls('collection-3');
    await wrapper.get('input[aria-label="Size slider"]').setValue('200');
    expect(store.settingsFor('collection-3').sizePx).toBe(200);
    expect(store.settingsFor('all').sizePx).toBe(28);
    await wrapper.findAll('button').at(-1)!.trigger('click');
    expect(store.settingsFor('collection-3').sizePx).toBe(28);
  });

  it('sets auto size from a labelled select (Off, x-height, Cap height)', async () => {
    const { wrapper, store } = mountControls('collection-3');
    const label = wrapper.findAll('label').find((l) => l.text() === 'Auto size')!;
    const select = wrapper.get(`select#${label.attributes('for')}`);
    expect(select.findAll('option').map((o) => o.text())).toEqual([
      'Off',
      'x-height',
      'Cap height',
    ]);
    expect((select.element as HTMLSelectElement).value).toBe('off');
    await select.setValue('xHeight');
    expect(store.settingsFor('collection-3').autoSize).toBe('xHeight');
    await select.setValue('capHeight');
    expect(store.settingsFor('collection-3').autoSize).toBe('capHeight');
    expect(store.settingsFor('all').autoSize).toBe('off');
  });
});
