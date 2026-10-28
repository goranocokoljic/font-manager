import { describe, expect, it } from 'vitest';
import type { FontSwitchPayload } from '../api/types';
import {
  dragKeys,
  FONT_SWITCH_MIME,
  FONTS_MIME,
  isFontDrag,
  leftWindow,
  pickOf,
  readFontDrop,
  startFontDrag,
} from './drag';

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

  it('carries the Font Switch payload as JSON, plain text and a URI list (E34.1)', () => {
    const payload: FontSwitchPayload = {
      postscriptName: 'Inter-Regular',
      family: 'Inter',
      style: 'Regular',
      fileUrl: 'file:///Users/demo/Fonts/Inter/Inter%5Bwght%5D.ttf',
    };
    const dataTransfer = transfer();
    startFontDrag({ dataTransfer } as DragEvent, ['face:1', 'family:2'], payload);
    expect(dataTransfer.types).toEqual([
      FONTS_MIME,
      FONT_SWITCH_MIME,
      'text/plain',
      'text/uri-list',
    ]);
    expect(FONT_SWITCH_MIME).toBe('application/x-fm-font-switch');
    expect(JSON.parse(dataTransfer.getData(FONT_SWITCH_MIME))).toEqual(payload);
    expect(dataTransfer.getData('text/plain')).toBe('Inter-Regular');
    expect(dataTransfer.getData('text/uri-list')).toBe(payload.fileUrl);
    // Collections and tags still read the rows from it.
    const drop = { dataTransfer } as DragEvent;
    expect(isFontDrag(drop)).toBe(true);
    expect(readFontDrop(drop)).toEqual({ faces: [1], families: [2] });
  });

  it('carries only the rows without a payload', () => {
    const dataTransfer = transfer();
    startFontDrag({ dataTransfer } as DragEvent, ['face:1']);
    expect(dataTransfer.types).toEqual([FONTS_MIME]);
  });

  it('tells a drag leaving the window from one moving between elements', () => {
    const leave = (relatedTarget: unknown, clientX: number, clientY: number) =>
      leftWindow({ relatedTarget, clientX, clientY } as unknown as DragEvent, 800, 600);
    expect(leave(null, -1, 300)).toBe(true);
    expect(leave(null, 400, 0)).toBe(true);
    expect(leave(null, 800, 300)).toBe(true);
    expect(leave(null, 400, 600)).toBe(true);
    // Inside the viewport, or into another element: still in the window.
    expect(leave(null, 400, 300)).toBe(false);
    expect(leave({}, -1, 300)).toBe(false);
  });
});
