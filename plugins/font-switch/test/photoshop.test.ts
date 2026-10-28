import { describe, expect, it, vi } from 'vitest';
import { type PhotoshopSelectedLayer, photoshopAdapter } from '../src/photoshop';
import { HOST_FONTS, INTER, LORA_RENAMED, SELECT_TEXT_FIRST, UNKNOWN } from './fixtures';

const textLayer = (): PhotoshopSelectedLayer => ({
  kind: 'text',
  textItem: { characterStyle: { font: 'MyriadPro-Regular' } },
});

function mockPhotoshop(activeLayers: PhotoshopSelectedLayer[] | null, withDocument = true) {
  let modal = false;
  const changedOutsideModal: unknown[] = [];
  type Modal = (run: () => Promise<void>, options: { commandName: string }) => Promise<void>;
  const executeAsModal = vi.fn<Modal>(async (run) => {
    modal = true;
    try {
      await run();
    } finally {
      modal = false;
    }
  });
  // A layer's font may only change inside `executeAsModal`, as in Photoshop.
  for (const layer of activeLayers ?? []) {
    const style = layer.textItem?.characterStyle;
    if (!style) continue;
    let font = style.font;
    Object.defineProperty(style, 'font', {
      get: () => font,
      set: (v: unknown) => {
        if (!modal) changedOutsideModal.push(v);
        font = v;
      },
    });
  }
  const app = {
    activeDocument: withDocument ? { activeLayers } : null,
    fonts: HOST_FONTS.map(([postScriptName, family, style]) => ({ postScriptName, family, style })),
  };
  const adapter = photoshopAdapter({ app, core: { executeAsModal } });
  return { adapter, executeAsModal, changedOutsideModal };
}

const fontOf = (l: PhotoshopSelectedLayer) => l.textItem?.characterStyle?.font;

describe('Photoshop adapter', () => {
  it('gives the selected text layers the font, in one modal step', async () => {
    const layers = [textLayer(), textLayer()];
    const { adapter, executeAsModal, changedOutsideModal } = mockPhotoshop(layers);
    expect(await adapter.apply(INTER)).toEqual({ applied: true, font: 'Inter-Regular' });
    expect(layers.map(fontOf)).toEqual(['Inter-Regular', 'Inter-Regular']);
    expect(executeAsModal).toHaveBeenCalledTimes(1);
    expect(executeAsModal.mock.calls[0][1]).toEqual({ commandName: 'Font Switch' });
    expect(changedOutsideModal).toEqual([]);
  });

  it('takes a plain PostScript name and falls back to the family and style', async () => {
    const layer = textLayer();
    const { adapter } = mockPhotoshop([layer]);
    expect((await adapter.apply('Inter-Bold')).applied).toBe(true);
    expect(fontOf(layer)).toBe('Inter-Bold');
    expect(await adapter.apply(LORA_RENAMED)).toEqual({ applied: true, font: 'Lora-BoldV2' });
    expect(fontOf(layer)).toBe('Lora-Bold');
  });

  it('leaves the other layers of a mixed selection alone', async () => {
    const pixels: PhotoshopSelectedLayer = { kind: 'pixel' };
    const layer = textLayer();
    const { adapter } = mockPhotoshop([pixels, layer]);
    expect((await adapter.apply(INTER)).applied).toBe(true);
    expect(fontOf(layer)).toBe('Inter-Regular');
    expect(pixels).toEqual({ kind: 'pixel' });
  });

  it.each<[string, PhotoshopSelectedLayer[] | null, boolean]>([
    ['no selected layer', [], true],
    ['no document', null, false],
    ['a pixel layer', [{ kind: 'pixel' }], true],
  ])('does nothing for %s', async (_, layers, doc) => {
    const { adapter, executeAsModal } = mockPhotoshop(layers, doc);
    expect(await adapter.apply(INTER)).toEqual(SELECT_TEXT_FIRST);
    expect(executeAsModal).not.toHaveBeenCalled();
  });

  it('says a font Photoshop does not know is not active', async () => {
    const layer = textLayer();
    const { adapter, executeAsModal } = mockPhotoshop([layer]);
    expect(await adapter.apply(UNKNOWN)).toEqual({
      applied: false,
      message: 'Nope-Regular is not active',
    });
    expect(fontOf(layer)).toBe('MyriadPro-Regular');
    expect(executeAsModal).not.toHaveBeenCalled();
  });
});
