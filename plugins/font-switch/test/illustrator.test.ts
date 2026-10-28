import type { ExtendScript } from '@fm/plugin-shared';
import { ES5_PLUS, codeOnly } from '@fm/plugin-shared/testing';
import { describe, expect, it, vi } from 'vitest';
import { illustratorAdapter, illustratorFontList } from '../src/illustrator';
import { ILLUSTRATOR_JSX } from '../src/illustrator-jsx';
import {
  HOST_FONTS,
  INTER,
  INTER_DISPLAY,
  LORA_RENAMED,
  SELECT_TEXT_FIRST,
  UNKNOWN,
} from './fixtures';

interface MockTextFont {
  name: string;
  family: string;
  style: string;
}
interface MockRange {
  typename: 'TextRange';
  length: number;
  characterAttributes: { textFont: MockTextFont | string };
}
type MockItem = { typename: 'TextFrame'; textRange: MockRange } | { typename: 'PathItem' };

const range = (length = 5): MockRange => ({
  typename: 'TextRange',
  length,
  characterAttributes: { textFont: 'MyriadPro-Regular' },
});
const frame = (): { typename: 'TextFrame'; textRange: MockRange } => ({
  typename: 'TextFrame',
  textRange: range(),
});

/**
 * A mocked Illustrator behind `evalScript`: the real `ILLUSTRATOR_JSX` runs
 * against a fake `app`; its functions exist only once the adapter has
 * loaded it, as in the ExtendScript engine.
 */
function mockIllustrator(selection: MockRange | MockItem[] | null, documents = 1) {
  const textFonts = Object.assign(
    HOST_FONTS.map(([name, family, style]) => ({ name, family, style })),
    {
      getByName(name: string) {
        const found = textFonts.find((f) => f.name === name);
        if (!found) throw new Error(`No such font: ${name}`);
        return found;
      },
    },
  );
  const app = {
    documents: { length: documents },
    activeDocument: { selection },
    textFonts,
  };
  let loaded = '';
  const scripts: string[] = [];
  const script: ExtendScript = {
    evalScript: vi.fn(async (source: string) => {
      scripts.push(source);
      if (source === ILLUSTRATOR_JSX) {
        loaded = source;
        return 'undefined';
      }
      return String(new Function('app', `${loaded}\nreturn ${source};`)(app));
    }),
  };
  return { adapter: illustratorAdapter(script), textFonts, scripts, script };
}

describe('Illustrator adapter', () => {
  it('gives the selected text the font, after loading its script once', async () => {
    const selected = range();
    const { adapter, textFonts, scripts } = mockIllustrator(selected);
    expect(await adapter.apply(INTER)).toEqual({ applied: true, font: 'Inter-Regular' });
    expect(selected.characterAttributes.textFont).toBe(textFonts[0]);
    await adapter.apply(INTER);
    expect(scripts.filter((s) => s === ILLUSTRATOR_JSX)).toHaveLength(1);
  });

  it('gives the text of selected text frames the font and leaves other objects alone', async () => {
    const a = frame();
    const b = frame();
    const { adapter, textFonts } = mockIllustrator([a, { typename: 'PathItem' }, b]);
    expect((await adapter.apply('Inter-Bold')).applied).toBe(true);
    expect(a.textRange.characterAttributes.textFont).toBe(textFonts[1]);
    expect(b.textRange.characterAttributes.textFont).toBe(textFonts[1]);
  });

  it('takes the PostScript name before the family and style', async () => {
    const selected = range();
    const { adapter, textFonts } = mockIllustrator(selected);
    expect((await adapter.apply(INTER_DISPLAY)).applied).toBe(true);
    expect(selected.characterAttributes.textFont).toBe(textFonts[3]);
  });

  it('falls back to the family and style', async () => {
    const selected = range();
    const { adapter, textFonts } = mockIllustrator(selected);
    expect(await adapter.apply(LORA_RENAMED)).toEqual({ applied: true, font: 'Lora-BoldV2' });
    expect(selected.characterAttributes.textFont).toBe(textFonts[2]);
  });

  it.each<[string, MockRange | MockItem[] | null, number]>([
    ['no selection', [], 1],
    ['no document', null, 0],
    ['a path', [{ typename: 'PathItem' }], 1],
    ['an insertion point', range(0), 1],
  ])('does nothing for %s', async (_, selection, documents) => {
    const { adapter, scripts } = mockIllustrator(selection, documents);
    expect(await adapter.apply(INTER)).toEqual(SELECT_TEXT_FIRST);
    expect(scripts.some((s) => s.startsWith('fmFontSwitchApply'))).toBe(false);
  });

  it('says a font Illustrator does not know is not active', async () => {
    const selected = range();
    const { adapter, scripts } = mockIllustrator(selected);
    expect(await adapter.apply(UNKNOWN)).toEqual({
      applied: false,
      message: 'Nope-Regular is not active',
    });
    expect(selected.characterAttributes.textFont).toBe('MyriadPro-Regular');
    expect(scripts.some((s) => s.startsWith('fmFontSwitchApply'))).toBe(false);
  });

  it('loads its script again after a failed load', async () => {
    const { adapter, script } = mockIllustrator(range());
    const run = vi.mocked(script.evalScript);
    const real = run.getMockImplementation()!;
    run.mockImplementationOnce(async () => {
      throw new Error('ExtendScript failed');
    });
    await expect(adapter.apply(INTER)).rejects.toThrow('ExtendScript failed');
    run.mockImplementation(real);
    expect((await adapter.apply(INTER)).applied).toBe(true);
  });

  it('reads the font lines of fmFontSwitchFonts', () => {
    expect(illustratorFontList('Inter-Regular\tInter\tRegular\n\nLora-Bold\tLora\tBold')).toEqual([
      { postscriptName: 'Inter-Regular', family: 'Inter', style: 'Regular' },
      { postscriptName: 'Lora-Bold', family: 'Lora', style: 'Bold' },
    ]);
    expect(illustratorFontList('')).toEqual([]);
  });

  it.each(ES5_PLUS)('has no %s in its ExtendScript (ES3)', (_, pattern) => {
    expect(codeOnly(ILLUSTRATOR_JSX)).not.toMatch(pattern);
  });
});
