import { afterEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from './client';
import { decodePreview, PREVIEW_HEADER_LEN } from './preview';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, isTauri: () => false }));

afterEach(() => invoke.mockReset());

/** A 1×1 transparent PNG, as the Rust mock renders. */
const PNG = Uint8Array.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
  0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
  0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
  0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
  0x42, 0x60, 0x82,
]);

/** What `preview_bytes` in src-tauri sends: width and height (u32 LE), then the PNG. */
function answer(width: number, height: number, png: Uint8Array): ArrayBuffer {
  const out = new Uint8Array(PREVIEW_HEADER_LEN + png.length);
  const head = new DataView(out.buffer);
  head.setUint32(0, width, true);
  head.setUint32(4, height, true);
  out.set(png, PREVIEW_HEADER_LEN);
  return out.buffer;
}

describe("render_preview's raw answer (E35.5)", () => {
  it('reads the 8-byte header as little-endian width and height', () => {
    // Bytes the Rust test `preview_bytes_puts_width_and_height_before_the_png` asserts.
    const got = decodePreview(Uint8Array.of(4, 3, 2, 1, 44, 1, 0, 0, 0x89, 0x50, 0x4e, 0x47, 9));
    expect(got.width).toBe(0x01020304);
    expect(got.height).toBe(300);
    expect([...got.png]).toEqual([0x89, 0x50, 0x4e, 0x47, 9]);
  });

  it('gives back the PNG it was sent, byte for byte, and a Blob of it holds the same bytes', async () => {
    const got = decodePreview(answer(173, 47, PNG));
    expect(got.width).toBe(173);
    expect(got.height).toBe(47);
    expect(got.png).toBeInstanceOf(Uint8Array);
    expect(got.png).toEqual(PNG);
    const blob = new Blob([new Uint8Array(got.png)], { type: 'image/png' });
    expect(new Uint8Array(await blob.arrayBuffer())).toEqual(PNG);
  });

  it('reads a view into a larger buffer from its own offset', () => {
    const big = new Uint8Array(5 + PREVIEW_HEADER_LEN + PNG.length);
    big.set(new Uint8Array(answer(2, 3, PNG)), 5);
    const got = decodePreview(big.subarray(5));
    expect([got.width, got.height]).toEqual([2, 3]);
    expect(got.png).toEqual(PNG);
  });

  it('reads the number array of the message-channel fallback', () => {
    const got = decodePreview([...new Uint8Array(answer(9, 10, PNG))]);
    expect([got.width, got.height]).toEqual([9, 10]);
    expect(got.png).toEqual(PNG);
  });

  it('takes an answer with a header and no PNG as an empty image', () => {
    const got = decodePreview(answer(0, 0, new Uint8Array()));
    expect(got).toEqual({ width: 0, height: 0, png: new Uint8Array() });
  });

  it('rejects an answer shorter than the header with an internal API error', () => {
    expect(() => decodePreview(new ArrayBuffer(7))).toThrow(
      expect.objectContaining({ code: 'internal' }),
    );
  });

  it("is what the Tauri client's renderPreview returns, in fm-api's field names", async () => {
    invoke.mockResolvedValueOnce(answer(120, 40, PNG));
    const req = { face: 3, text: 'Ag', sizePx: 36 };
    const got = await tauriClient.renderPreview(req);
    expect(invoke).toHaveBeenCalledWith('render_preview', { req });
    expect(Object.keys(got).sort()).toEqual(['height', 'png', 'width']);
    expect(got.width).toBe(120);
    expect(got.height).toBe(40);
    expect(got.png).toEqual(PNG);
  });

  it('passes an API error from the command through', async () => {
    const error = { code: 'invalidRequest', message: 'Too big.', detail: null };
    invoke.mockRejectedValueOnce(error);
    await expect(tauriClient.renderPreview({ face: 3, text: 'Ag', sizePx: 400 })).rejects.toBe(
      error,
    );
  });
});
