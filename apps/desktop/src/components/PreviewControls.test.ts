import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { mockLanguages } from '../api/mock';
import { ACTIONS } from '../keyboard/keymap';
import { PARAGRAPH } from '../preview/sampleText';
import { useCommandsStore } from '../stores/commands';
import { usePreviewStore } from '../stores/preview';
import { useSearchStore } from '../stores/search';
import PreviewControls from './PreviewControls.vue';
import PreviewSizeSlider from './PreviewSizeSlider.vue';
import { pickOption, selectOptions } from './ui/testing';

beforeEach(() => {
  localStorage.clear();
  setActivePinia(createPinia());
});

function mountControls(view = 'all', props: Record<string, unknown> = {}) {
  return {
    wrapper: mount(PreviewControls, { props: { view, ...props }, attachTo: document.body }),
    store: usePreviewStore(),
  };
}

let mounted: ReturnType<typeof mountControls>['wrapper'] | undefined;
function controls(view = 'all', props: Record<string, unknown> = {}) {
  const got = mountControls(view, props);
  mounted = got.wrapper;
  return got;
}
afterEach(() => {
  mounted?.unmount();
  mounted = undefined;
});

const selectLabelled = (w: ReturnType<typeof mountControls>['wrapper'], name: string) => {
  const label = w.findAll('label').find((l) => l.text() === name)!;
  return w.get(`[role="combobox"]#${label.attributes('for')}`);
};
const field = (w: ReturnType<typeof mountControls>['wrapper']) => w.get('input[type="text"]');

describe('PreviewControls', () => {
  it('shows a field, the five chips, a language picker and one options button', async () => {
    const { wrapper } = controls();
    await flushPromises();
    expect(field(wrapper).attributes('placeholder')).toBe('Type to preview');
    expect(wrapper.findAll('.chip').map((c) => c.text())).toEqual([
      'Word',
      'Pangram',
      'Alphabet',
      'Numerals',
      'Paragraph',
    ]);
    expect(wrapper.find('.divider').exists()).toBe(true);
    expect(selectLabelled(wrapper, 'Language').isVisible()).toBe(true);
    const row = wrapper.get('.main');
    expect(row.element.lastElementChild!.querySelector('button')!.getAttribute('aria-label')).toBe(
      'Preview options',
    );
    // Nothing else in the row: no sample picker, size or colour until the popover opens.
    expect(row.findAll('input').map((i) => i.attributes('type'))).toEqual(['text']);
    expect(row.findAll('[role="combobox"]')).toHaveLength(1);
    expect(wrapper.get('.more').isVisible()).toBe(false);
  });

  it('a chip sets the layout and its text, and marks itself', async () => {
    const { wrapper, store } = controls();
    await flushPromises();
    const chip = (name: string) => wrapper.findAll('.chip').find((c) => c.text() === name)!;
    await chip('Paragraph').trigger('click');
    expect(store.settingsFor('all')).toMatchObject({ preset: 'paragraph', sample: 'paragraph' });
    expect((field(wrapper).element as HTMLInputElement).value).toBe(PARAGRAPH);
    expect(chip('Paragraph').attributes('aria-pressed')).toBe('true');
    expect(chip('Word').attributes('aria-pressed')).toBe('false');
    expect(wrapper.get('[data-testid="preview-hint"]').text()).toBe('body text at 17 px');
    await chip('Numerals').trigger('click');
    expect((field(wrapper).element as HTMLInputElement).value).toBe('0123456789');
    expect(wrapper.get('[data-testid="preview-hint"]').text()).toContain('figures');
  });

  it('typing keeps the preset and makes the text custom; the hint counts the fonts', async () => {
    const { wrapper, store } = controls('all', { fontCount: 1137 });
    await flushPromises();
    await wrapper
      .findAll('.chip')
      .find((c) => c.text() === 'Pangram')!
      .trigger('click');
    await field(wrapper).setValue('Handgloves');
    expect(store.settingsFor('all')).toMatchObject({
      preset: 'pangram',
      sample: 'custom',
      customText: 'Handgloves',
    });
    expect(wrapper.get('[data-testid="preview-hint"]').text()).toBe('renders in 1,137 fonts');
  });

  it('focus selects a stock text, so the first key replaces it; own text stays to edit', async () => {
    const { wrapper } = controls();
    await flushPromises();
    await wrapper
      .findAll('.chip')
      .find((c) => c.text() === 'Numerals')!
      .trigger('click');
    const input = field(wrapper).element as HTMLInputElement;
    input.focus();
    expect(input.value.slice(input.selectionStart!, input.selectionEnd!)).toBe('0123456789');
    input.blur();
    await field(wrapper).setValue('Hamburg');
    input.setSelectionRange(0, 0);
    input.blur();
    input.focus();
    expect(input.selectionEnd).toBe(0);
  });

  it('the Word chip with no text left empties the field for the placeholder', async () => {
    const { wrapper } = controls();
    await flushPromises();
    expect((field(wrapper).element as HTMLInputElement).value).not.toBe('');
    await wrapper
      .findAll('.chip')
      .find((c) => c.text() === 'Word')!
      .trigger('click');
    expect((field(wrapper).element as HTMLInputElement).value).toBe('');
  });

  it('lists the recent languages first and every API language; one puts its pangram up', async () => {
    const { wrapper, store } = controls();
    await flushPromises();
    const language = selectLabelled(wrapper, 'Language');
    const before = await selectOptions(language);
    expect(before.map(([code]) => code)).toEqual(mockLanguages.map((l) => l.code));
    await pickOption(language, 'sr-Cyrl');
    expect(store.settingsFor('all')).toMatchObject({
      language: 'sr-Cyrl',
      sample: 'pangram',
      preset: 'pangram',
    });
    const after = await selectOptions(language);
    expect(after[0]![0]).toBe('sr-Cyrl');
    expect(after).toHaveLength(mockLanguages.length);
    expect((field(wrapper).element as HTMLInputElement).value).toBe(
      mockLanguages.find((l) => l.code === 'sr-Cyrl')!.pangram,
    );
  });

  it('picking the language it already shows puts that pangram up too', async () => {
    const { wrapper, store } = controls();
    await flushPromises();
    const language = selectLabelled(wrapper, 'Language');
    store.update('all', { sample: 'custom', customText: 'Hamburg' });
    await flushPromises();
    await pickOption(language, store.settingsFor('all').language);
    expect(store.settingsFor('all')).toMatchObject({ sample: 'pangram', preset: 'pangram' });

    store.applyPreset('all', 'alphabet');
    await flushPromises();
    await pickOption(language, store.settingsFor('all').language);
    expect(store.settingsFor('all')).toMatchObject({ sample: 'pangram', preset: 'pangram' });
  });

  it('cannot choose a language that has no pangram', async () => {
    const { wrapper, store } = controls();
    await flushPromises();
    const language = selectLabelled(wrapper, 'Language');
    expect((await selectOptions(language)).some(([, label]) => label === 'Maltese')).toBe(true);
    await language.trigger('click');
    await flushPromises();
    const maltese = [...document.querySelectorAll('.ui-popover [role="option"]')].find(
      (o) => o.textContent?.trim() === 'Maltese',
    ) as HTMLElement;
    expect(maltese.getAttribute('aria-disabled')).toBe('true');
    maltese.click();
    await flushPromises();
    expect(store.settingsFor('all').language).not.toBe('mt');
  });

  it('opens the options from one button; a press outside or Escape closes them', async () => {
    const { wrapper } = controls();
    const toggle = wrapper.get('button[aria-label="Preview options"]');
    const panel = wrapper.get('.more');
    expect(toggle.attributes('aria-expanded')).toBe('false');
    await toggle.trigger('click');
    expect(panel.isVisible()).toBe(true);
    expect(toggle.attributes('aria-expanded')).toBe('true');
    // A press inside stays open; the panel holds every remaining control.
    wrapper
      .get('input[aria-label="Line height"]')
      .element.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
    await flushPromises();
    expect(panel.isVisible()).toBe(true);
    for (const name of [
      'Size in pixels',
      'Line height',
      'Tracking in em',
      'Text color',
      'Background color',
    ]) {
      expect(panel.find(`input[aria-label="${name}"]`).exists()).toBe(true);
    }
    expect(panel.text()).toContain('Auto size');
    expect(panel.text()).toContain('Metrics');
    expect(panel.text()).toContain('Outline');
    expect(panel.text()).toContain('Surface');
    expect(panel.find('[aria-label="Alignment"]').exists()).toBe(true);

    document.body.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
    await flushPromises();
    expect(panel.isVisible()).toBe(false);

    await toggle.trigger('click');
    await wrapper.get('input[aria-label="Line height"]').trigger('keydown', { key: 'Escape' });
    expect(panel.isVisible()).toBe(false);
    expect(document.activeElement).toBe(toggle.element);
  });

  it('T focuses and selects the field; Escape leaves it for the list', async () => {
    const { wrapper } = controls();
    await flushPromises();
    await field(wrapper).setValue('Hamburg');
    (field(wrapper).element as HTMLInputElement).blur();
    expect(useCommandsStore().run(ACTIONS.previewField)).toBe(true);
    const input = field(wrapper).element as HTMLInputElement;
    expect(document.activeElement).toBe(input);
    expect(input.value.slice(input.selectionStart!, input.selectionEnd!)).toBe('Hamburg');
    await field(wrapper).trigger('keydown', { key: 'Escape' });
    expect(wrapper.emitted('leave')).toHaveLength(1);
    expect(document.activeElement).not.toBe(input);
  });

  it('T leaves the focus alone while a modal dialog is open', async () => {
    const { wrapper } = controls();
    const modal = document.createElement('div');
    modal.setAttribute('role', 'dialog');
    modal.setAttribute('aria-modal', 'true');
    document.body.append(modal);
    expect(useCommandsStore().run(ACTIONS.previewField)).toBe(false);
    expect(document.activeElement).not.toBe(field(wrapper).element);
    modal.remove();
  });

  it('applies a typed size only inside the bounds and clamps it on change', async () => {
    const { wrapper, store } = controls();
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
    const { wrapper, store } = controls();
    await wrapper.get('input[aria-label="Line height"]').setValue('1.6');
    await wrapper.get('input[aria-label="Tracking in em"]').setValue('0.1');
    await wrapper
      .get('[aria-label="Alignment"] button[aria-pressed="false"]:nth-of-type(2)')
      .trigger('click');
    await wrapper.get('input[aria-label="Text color"]').setValue('#123456');
    await wrapper.get('input[aria-label="Background color"]').setValue('#fedcba');
    await pickOption(selectLabelled(wrapper, 'Surface'), 'dark');
    expect(store.settingsFor('all')).toMatchObject({
      lineHeight: 1.6,
      trackingEm: 0.1,
      align: 'center',
      textColor: '#123456',
      backgroundColor: '#fedcba',
      surface: 'dark',
    });
    expect(wrapper.get('[aria-label="Alignment"] button[aria-pressed="true"]').text()).toBe(
      'Center',
    );
  });

  it('chooses the sample kind in the options, with the font name among them', async () => {
    const { wrapper, store } = controls();
    await pickOption(selectLabelled(wrapper, 'Sample'), 'fontName');
    expect(store.settingsFor('all').sample).toBe('fontName');
    expect((field(wrapper).element as HTMLInputElement).value).toBe('');
  });

  it('changes only its own view, and Reset puts it back to the defaults', async () => {
    const { wrapper, store } = controls('collection-3');
    await wrapper.get('input[aria-label="Size in pixels"]').setValue('200');
    expect(store.settingsFor('collection-3').sizePx).toBe(200);
    expect(store.settingsFor('all').sizePx).toBe(28);
    await wrapper.findAll('button').at(-1)!.trigger('click');
    expect(store.settingsFor('collection-3').sizePx).toBe(28);
  });

  it('sets auto size from a labelled select (Off, x-height, Cap height)', async () => {
    const { wrapper, store } = controls('collection-3');
    const select = selectLabelled(wrapper, 'Auto size');
    expect((await selectOptions(select)).map(([, label]) => label)).toEqual([
      'Off',
      'x-height',
      'Cap height',
    ]);
    expect(select.attributes('data-value')).toBe('off');
    await pickOption(select, 'xHeight');
    expect(store.settingsFor('collection-3').autoSize).toBe('xHeight');
    await pickOption(select, 'capHeight');
    expect(store.settingsFor('collection-3').autoSize).toBe('capHeight');
    expect(store.settingsFor('all').autoSize).toBe('off');
  });

  it('has a tracking slider from -0.2 to 1 em next to the number field', async () => {
    const { wrapper, store } = controls();
    const slider = wrapper.get('input[type="range"][aria-label="Tracking slider"]');
    expect(slider.attributes()).toMatchObject({ min: '-0.2', max: '1', step: '0.01' });
    await slider.setValue('0.35');
    expect(store.settingsFor('all').trackingEm).toBe(0.35);
    const number = wrapper.get('input[aria-label="Tracking in em"]');
    expect((number.element as HTMLInputElement).value).toBe('0.35');
    await slider.setValue('-0.2');
    expect(store.settingsFor('all').trackingEm).toBe(-0.2);
    const group = wrapper.get(
      `[role="group"][aria-labelledby="${slider.element.parentElement!.getAttribute('aria-labelledby')}"]`,
    );
    expect(group.text()).toContain('Tracking');
  });

  it('toggles metrics and outline from labelled checkboxes, per view', async () => {
    const { wrapper, store } = controls('collection-3');
    const box = (name: string) =>
      wrapper
        .findAll('label')
        .find((l) => l.text() === name)!
        .get('input[type="checkbox"]');
    expect((box('Metrics').element as HTMLInputElement).checked).toBe(false);
    await box('Metrics').setValue(true);
    expect(store.settingsFor('collection-3').metrics).toBe(true);
    await box('Outline').setValue(true);
    expect(store.settingsFor('collection-3').outline).toBe(true);
    expect(store.settingsFor('all')).toMatchObject({ metrics: false, outline: false });
    await box('Metrics').setValue(false);
    expect(store.settingsFor('collection-3').metrics).toBe(false);
    expect((box('Outline').element as HTMLInputElement).checked).toBe(true);
  });
});

describe('PreviewSizeSlider', () => {
  it('sets the size of its own view and shows a saved size at the end of its range', async () => {
    const slider = mount(PreviewSizeSlider, {
      props: { view: 'all', sizeRange: { min: 24, max: 120, step: 1 } },
    });
    const store = usePreviewStore();
    const input = slider.get('input[aria-label="Size slider"]');
    await input.setValue('60');
    expect(store.settingsFor('all').sizePx).toBe(60);
    expect(store.settingsFor('collection-3').sizePx).toBe(28);
    store.update('all', { sizePx: 200 });
    await flushPromises();
    expect((input.element as HTMLInputElement).value).toBe('120');
    slider.unmount();
  });

  it('is in the row only when the view asks for it', () => {
    expect(controls().wrapper.find('input[aria-label="Size slider"]').exists()).toBe(false);
    mounted?.unmount();
    expect(
      controls('all', { inlineSize: true })
        .wrapper.find('input[aria-label="Size slider"]')
        .exists(),
    ).toBe(true);
  });
});

describe('PreviewControls in paragraph mode (E43.1)', () => {
  const LONG =
    'Ljubazni fenjerdžija čađavog lica hoće da mi pokaže štos, a onda i drugi štos za kraj.';
  const chip = (w: ReturnType<typeof mountControls>['wrapper'], name: string) =>
    w.findAll('.chip').find((c) => c.text() === name)!;

  it('typing your own long text presses the Paragraph chip; a short one keeps Word', async () => {
    const { wrapper, store } = controls();
    await field(wrapper).setValue('Ljubazni');
    expect(chip(wrapper, 'Word').attributes('aria-pressed')).toBe('true');
    expect(chip(wrapper, 'Paragraph').attributes('aria-pressed')).toBe('false');
    await field(wrapper).setValue(LONG);
    expect(store.settingsFor('all').preset).toBe('word');
    expect(chip(wrapper, 'Paragraph').attributes('aria-pressed')).toBe('true');
    expect(chip(wrapper, 'Word').attributes('aria-pressed')).toBe('false');
  });

  it('the size and line height fields set the body size of the rows, not the wall size', async () => {
    const { wrapper, store } = controls('all', { sizeRange: { min: 24, max: 120, step: 1 } });
    await field(wrapper).setValue(LONG);
    const size = wrapper.get('input[aria-label="Size in pixels"]');
    expect((size.element as HTMLInputElement).value).toBe('17');
    // The body's bounds, not the wall's 24–120: 5 clamps to 10.
    (size.element as HTMLInputElement).value = '5';
    await size.trigger('change');
    expect(store.settingsFor('all').bodySizePx).toBe(10);
    await size.setValue('20');
    await wrapper.get('input[aria-label="Line height"]').setValue('1.6');
    expect(store.settingsFor('all')).toMatchObject({
      bodySizePx: 20,
      bodyLineHeight: 1.6,
      sizePx: 28,
      lineHeight: 1.2,
    });
  });

  it('on the wall the same fields keep setting the wall size', async () => {
    const { wrapper, store } = controls('all', { sizeRange: { min: 24, max: 120, step: 1 } });
    await field(wrapper).setValue('Ljubazni');
    await wrapper.get('input[aria-label="Size in pixels"]').setValue('60');
    expect(store.settingsFor('all')).toMatchObject({ sizePx: 60, bodySizePx: 17 });
  });

  it('names the body size in the Paragraph hint', async () => {
    const { wrapper, store } = controls();
    store.update('all', { bodySizePx: 19 });
    await chip(wrapper, 'Paragraph').trigger('click');
    expect(wrapper.get('[data-testid="preview-hint"]').text()).toBe('body text at 19 px');
  });
});

describe('PreviewControls with a detected language (E43.3)', () => {
  const SERBIAN = 'Ljubazni fenjerdžija čađavog lica hoće da mi pokaže štos.';

  it("names the language the API detects in the field's text", async () => {
    const { wrapper } = controls('all', { fontCount: 212 });
    const search = useSearchStore();
    // After the controls' own language load, so these names are the ones read.
    await flushPromises();
    search.languages = [
      {
        code: 'sr-Latn',
        name: 'Serbian (Latin)',
        script: 'Latn',
        pangram: null,
        sampleGlyphs: null,
      },
    ];
    await field(wrapper).setValue(SERBIAN);
    // Not detected yet: the plain note.
    expect(wrapper.get('[data-testid="preview-hint"]').text()).toBe('renders in 212 fonts');
    search.coverText = SERBIAN;
    search.detected = [{ code: 'sr-Latn', confidence: 0.9 }];
    await flushPromises();
    expect(wrapper.get('[data-testid="preview-hint"]').text()).toBe(
      'Serbian (Latin) detected · showing fonts that render every glyph',
    );
  });

  it('keeps the plain note when the detection is for another text', async () => {
    const { wrapper } = controls('all', { fontCount: 212 });
    const search = useSearchStore();
    await field(wrapper).setValue(SERBIAN);
    search.coverText = 'Ђ';
    search.detected = [{ code: 'sr-Cyrl', confidence: 0.9 }];
    await flushPromises();
    expect(wrapper.get('[data-testid="preview-hint"]').text()).toBe('renders in 212 fonts');
  });
});
