import { describe, expect, it } from 'vitest';
import { type InDesignFont, type InDesignSelected, inDesignAdapter } from '../src/indesign';
import { HOST_FONTS, INTER, LORA_RENAMED, SELECT_TEXT_FIRST, UNKNOWN } from './fixtures';

// InDesign's collections are `length` plus `item(i)`, not arrays.
function collection<T>(items: T[]) {
  return { length: items.length, item: (i: number) => items[i] };
}

function mockInDesign(selection: InDesignSelected[]) {
  const fonts: InDesignFont[] = HOST_FONTS.map(([postscriptName, fontFamily, fontStyleName]) => ({
    postscriptName,
    fontFamily,
    fontStyleName,
  }));
  const app = { selection: collection(selection), fonts: collection(fonts) };
  return { fonts, adapter: inDesignAdapter(app) };
}

const text = (constructorName = 'Text'): InDesignSelected => ({
  constructorName,
  appliedFont: 'Minion',
});

describe('InDesign adapter', () => {
  it('gives selected text the font with the PostScript name', async () => {
    const words = text('Word');
    const { adapter, fonts } = mockInDesign([words]);
    expect(await adapter.apply(INTER)).toEqual({ applied: true, font: 'Inter-Regular' });
    expect(words.appliedFont).toBe(fonts[0]);
  });

  it('takes a plain PostScript name', async () => {
    const words = text();
    const { adapter, fonts } = mockInDesign([words]);
    expect(await adapter.apply('Inter-Bold')).toEqual({ applied: true, font: 'Inter-Bold' });
    expect(words.appliedFont).toBe(fonts[1]);
  });

  it('falls back to the family and style', async () => {
    const words = text('TextStyleRange');
    const { adapter, fonts } = mockInDesign([words]);
    expect(await adapter.apply(LORA_RENAMED)).toEqual({ applied: true, font: 'Lora-BoldV2' });
    expect(words.appliedFont).toBe(fonts[2]);
  });

  it('gives the text of a selected text frame the font', async () => {
    const inside: InDesignSelected = { appliedFont: 'Minion' };
    const frame: InDesignSelected = { constructorName: 'TextFrame', texts: collection([inside]) };
    const { adapter, fonts } = mockInDesign([frame]);
    expect((await adapter.apply(INTER)).applied).toBe(true);
    expect(inside.appliedFont).toBe(fonts[0]);
  });

  it.each<[string, InDesignSelected[]]>([
    ['no selection', []],
    ['a rectangle', [{ constructorName: 'Rectangle', appliedFont: 'Minion' }]],
    ['an insertion point', [{ constructorName: 'InsertionPoint', appliedFont: 'Minion' }]],
  ])('does nothing for %s', async (_, selection) => {
    const { adapter } = mockInDesign(selection);
    expect(await adapter.apply(INTER)).toEqual(SELECT_TEXT_FIRST);
    for (const s of selection) expect(s.appliedFont).toBe('Minion');
  });

  it('does nothing without a document', async () => {
    const adapter = inDesignAdapter({ selection: null, fonts: collection([]) });
    expect(await adapter.apply(INTER)).toEqual(SELECT_TEXT_FIRST);
  });

  it('says a font InDesign does not know is not active', async () => {
    const words = text();
    const { adapter } = mockInDesign([words]);
    expect(await adapter.apply(UNKNOWN)).toEqual({
      applied: false,
      message: 'Nope-Regular is not active',
    });
    expect(await adapter.apply('Missing-Font')).toEqual({
      applied: false,
      message: 'Missing-Font is not active',
    });
    expect(words.appliedFont).toBe('Minion');
  });
});
