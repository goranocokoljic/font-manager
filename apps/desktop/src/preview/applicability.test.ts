import { describe, expect, it } from 'vitest';
import { t } from '../i18n';
import { VIEW_MODES, type ViewMode } from '../stores/views';
import {
  applicability,
  PREVIEW_PRESETS,
  previewPreset,
  type PreviewPreset,
  type PresetSource,
} from './applicability';

/** The expected table (E51.4), written out by hand: [size applies, language applies]. */
const EXPECTED: Record<ViewMode, Record<PreviewPreset, [boolean, boolean]>> = {
  list: {
    pangram: [true, true],
    word: [true, false],
    custom: [true, false],
    alphabet: [true, false],
    numerals: [true, false],
    paragraph: [false, false],
    letters: [true, false],
  },
  grid: {
    pangram: [true, true],
    word: [true, false],
    custom: [true, false],
    alphabet: [true, false],
    numerals: [true, false],
    paragraph: [false, false],
    letters: [true, false],
  },
  table: {
    pangram: [false, true],
    word: [false, false],
    custom: [false, false],
    alphabet: [false, false],
    numerals: [false, false],
    paragraph: [false, false],
    letters: [false, false],
  },
  waterfall: {
    pangram: [false, true],
    word: [false, false],
    custom: [false, false],
    alphabet: [false, false],
    numerals: [false, false],
    paragraph: [false, false],
    letters: [false, false],
  },
  // The Layout mode's toolbar sizes the split's list, as in List.
  layout: {
    pangram: [true, true],
    word: [true, false],
    custom: [true, false],
    alphabet: [true, false],
    numerals: [true, false],
    paragraph: [false, false],
    letters: [true, false],
  },
};

describe('applicability', () => {
  it('walks 5 view modes × 7 presets', () => {
    expect(VIEW_MODES).toHaveLength(5);
    expect(PREVIEW_PRESETS).toHaveLength(7);
  });

  for (const mode of VIEW_MODES) {
    for (const preset of PREVIEW_PRESETS) {
      const [size, language] = EXPECTED[mode][preset];
      it(`${mode} × ${preset}: size ${size ? 'on' : 'off'}, language ${language ? 'on' : 'off'}`, () => {
        const a = applicability(mode, preset);
        expect(a.size.applies).toBe(size);
        expect(a.language.applies).toBe(language);
      });
    }
  }

  it('gives every control that is off a reason that exists in the catalog', () => {
    for (const mode of [...VIEW_MODES, 'layoutPane'] as const) {
      for (const preset of PREVIEW_PRESETS) {
        for (const state of Object.values(applicability(mode, preset))) {
          if (state.applies) continue;
          expect(t(state.reason)).not.toBe(state.reason);
        }
      }
    }
  });

  it('names why and what to do', () => {
    const waterfall = applicability('waterfall', 'word').size;
    expect(waterfall.applies || t(waterfall.reason)).toBe(
      'Size is fixed in Waterfall — switch to List or Grid to change it',
    );
    const language = applicability('list', 'word').language;
    expect(language.applies || t(language.reason)).toBe(
      'Language applies to the Pangram preset — switch to Pangram',
    );
    // Off for two causes, the view mode's wins: the strip, not Paragraph, holds the size.
    const table = applicability('table', 'paragraph').size;
    expect(table.applies || table.reason).toBe('preview.inactive.sizeTable');
    const paragraph = applicability('grid', 'paragraph').size;
    expect(paragraph.applies || paragraph.reason).toBe('preview.inactive.sizeParagraph');
  });

  it('turns both off in the Layout pane, whatever the preset', () => {
    for (const preset of PREVIEW_PRESETS) {
      const a = applicability('layoutPane', preset);
      expect(a.size.applies).toBe(false);
      expect(a.language.applies).toBe(false);
    }
  });
});

describe('previewPreset', () => {
  const src = (patch: Partial<PresetSource>): PresetSource => ({
    preset: 'word',
    sample: 'custom',
    customText: '',
    ...patch,
  });

  it('reads each chip as its own preset', () => {
    expect(previewPreset(src({ preset: 'pangram', sample: 'pangram' }))).toBe('pangram');
    expect(previewPreset(src({ preset: 'word', sample: 'custom' }))).toBe('word');
    expect(previewPreset(src({ preset: 'alphabet', sample: 'alphabet' }))).toBe('alphabet');
    expect(previewPreset(src({ preset: 'numerals', sample: 'numerals' }))).toBe('numerals');
    expect(previewPreset(src({ preset: 'paragraph', sample: 'paragraph' }))).toBe('paragraph');
    expect(previewPreset(src({ preset: 'letters', sample: 'letters' }))).toBe('letters');
  });

  it('reads the sample, not the chip: the pangram on the Word chip is Pangram', () => {
    expect(previewPreset(src({ preset: 'word', sample: 'pangram' }))).toBe('pangram');
    expect(previewPreset(src({ preset: 'pangram', sample: 'alphabet' }))).toBe('alphabet');
  });

  it('reads your own text typed over another chip as custom', () => {
    expect(previewPreset(src({ preset: 'alphabet', sample: 'custom', customText: 'Hi' }))).toBe(
      'custom',
    );
    expect(previewPreset(src({ preset: 'pangram', sample: 'paragraph' }))).toBe('custom');
  });

  it('reads your own long text as Paragraph: the reading rows set it at the body size', () => {
    const long =
      'A text of your own, long enough to be read as body text in rows rather than as a word or two.';
    expect(previewPreset(src({ preset: 'word', sample: 'custom', customText: long }))).toBe(
      'paragraph',
    );
    expect(previewPreset(src({ preset: 'paragraph', sample: 'custom', customText: 'Hi' }))).toBe(
      'paragraph',
    );
  });

  it('reads Lorem and the font name as Word', () => {
    expect(previewPreset(src({ sample: 'lorem' }))).toBe('word');
    expect(previewPreset(src({ sample: 'fontName' }))).toBe('word');
  });
});
