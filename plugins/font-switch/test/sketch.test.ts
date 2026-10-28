import { describe, expect, it, vi } from 'vitest';
import { type SketchSelectedLayer, sketchAdapter } from '../src/sketch';
import {
  HOST_FONTS,
  INTER,
  INTER_DISPLAY,
  LORA_RENAMED,
  SELECT_TEXT_FIRST,
  UNKNOWN,
} from './fixtures';

const textLayer = () => {
  const setFontPostscriptName = vi.fn<(name: string) => void>();
  const layer: SketchSelectedLayer = { type: 'Text', sketchObject: { setFontPostscriptName } };
  return { layer, setFontPostscriptName };
};

function mockSketch(selected: SketchSelectedLayer[]) {
  const fonts = HOST_FONTS.map(([postscriptName, family, style]) => ({
    postscriptName,
    family,
    style,
  }));
  return sketchAdapter({ selectedLayers: () => selected, fonts: () => fonts });
}

describe('Sketch adapter', () => {
  it('gives the selected text layers the font by its PostScript name', async () => {
    const a = textLayer();
    const b = textLayer();
    expect(await mockSketch([a.layer, b.layer]).apply(INTER)).toEqual({
      applied: true,
      font: 'Inter-Regular',
    });
    expect(a.setFontPostscriptName).toHaveBeenCalledWith('Inter-Regular');
    expect(b.setFontPostscriptName).toHaveBeenCalledWith('Inter-Regular');
  });

  it('takes a plain PostScript name and falls back to the family and style', async () => {
    const a = textLayer();
    const adapter = mockSketch([a.layer]);
    expect((await adapter.apply('Inter-Bold')).applied).toBe(true);
    expect(await adapter.apply(LORA_RENAMED)).toEqual({ applied: true, font: 'Lora-BoldV2' });
    expect(a.setFontPostscriptName.mock.calls).toEqual([['Inter-Bold'], ['Lora-Bold']]);
  });

  it('takes the PostScript name before the family and style', async () => {
    const a = textLayer();
    expect((await mockSketch([a.layer]).apply(INTER_DISPLAY)).applied).toBe(true);
    expect(a.setFontPostscriptName).toHaveBeenCalledWith('Inter-Display');
  });

  it.each<[string, SketchSelectedLayer[]]>([
    ['no selection', []],
    ['a shape', [{ type: 'ShapePath', sketchObject: {} }]],
  ])('does nothing for %s', async (_, selected) => {
    expect(await mockSketch(selected).apply(INTER)).toEqual(SELECT_TEXT_FIRST);
  });

  it('says a font the Mac does not have is not active', async () => {
    const a = textLayer();
    expect(await mockSketch([a.layer]).apply(UNKNOWN)).toEqual({
      applied: false,
      message: 'Nope-Regular is not active',
    });
    expect(a.setFontPostscriptName).not.toHaveBeenCalled();
  });
});
