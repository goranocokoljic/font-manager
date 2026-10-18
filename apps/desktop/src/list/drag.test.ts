import { describe, expect, it } from 'vitest';
import { dragKeys, FONTS_MIME, pickOf, readFontDrop, startFontDrag } from './drag';

/** A `DataTransfer` stand-in for jsdom, which has none. */
function transfer(): DataTransfer {
  const data = new Map<string, string>();
  return {
    effectAllowed: 'none',
    get types() {
      return [...data.keys()];
    },
    setData: (type: string, value: string) => void data.set(type, value),
    getData: (type: string) => data.get(type) ?? '',
  } as unknown as DataTransfer;
}

describe('font drag', () => {
  it('drags the selection when the row is in it, else the row alone', () => {
    const selected = new Set(['family:1', 'face:7']);
    expect(dragKeys(selected, 'face:7')).toEqual(['family:1', 'face:7']);
    expect(dragKeys(selected, 'family:3')).toEqual(['family:3']);
  });

  it('turns family rows into families and style rows into faces', () => {
    expect(pickOf(['family:1', 'face:7', 'face:8', 'other:2'])).toEqual({
      faces: [7, 8],
      families: [1],
    });
  });

  it('round-trips through the drag data and ignores a foreign payload', () => {
    const dataTransfer = transfer();
    startFontDrag({ dataTransfer } as DragEvent, ['family:4', 'face:9']);
    expect(dataTransfer.effectAllowed).toBe('copy');
    expect(readFontDrop({ dataTransfer } as DragEvent)).toEqual({ faces: [9], families: [4] });

    const foreign = transfer();
    foreign.setData(FONTS_MIME, '{"faces":["x", 2.5, 3]}');
    expect(readFontDrop({ dataTransfer: foreign } as DragEvent)).toEqual({
      faces: [3],
      families: [],
    });
    const broken = transfer();
    broken.setData(FONTS_MIME, 'not json');
    expect(readFontDrop({ dataTransfer: broken } as DragEvent)).toBeNull();
  });
});
