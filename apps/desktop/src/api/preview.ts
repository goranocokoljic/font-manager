// Decoding of `render_preview`'s raw IPC answer (E35.5). The Tauri command
// sends the thumbnail as bytes, not as JSON: one number per byte left hundreds
// of KB of garbage per thumbnail in the page's heap.
import type { ApiError, PreviewImage } from './types';

/**
 * Bytes before the PNG: the width, then the height, each a little-endian u32.
 * Matches `PREVIEW_HEADER_LEN` in `src-tauri/src/lib.rs`.
 */
export const PREVIEW_HEADER_LEN = 8;

/** A rendered preview as the client gives it: `PreviewImage` with the PNG as bytes. */
export type PreviewBytes = Omit<PreviewImage, 'png'> & { png: Uint8Array };

/**
 * Reads the raw answer: an `ArrayBuffer` from Tauri's IPC, or a number array
 * when the webview falls back to the message channel. The PNG is a view into
 * the answer, not a copy.
 */
export function decodePreview(raw: ArrayBuffer | ArrayBufferView | number[]): PreviewBytes {
  const bytes = Array.isArray(raw)
    ? Uint8Array.from(raw)
    : raw instanceof ArrayBuffer
      ? new Uint8Array(raw)
      : new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength);
  if (bytes.byteLength < PREVIEW_HEADER_LEN) {
    const error: ApiError = {
      code: 'internal',
      message: 'The app sent a preview it could not read.',
      detail: `render_preview answered ${bytes.byteLength} bytes, fewer than the ${PREVIEW_HEADER_LEN}-byte header`,
    };
    throw error;
  }
  const head = new DataView(bytes.buffer, bytes.byteOffset, PREVIEW_HEADER_LEN);
  return {
    width: head.getUint32(0, true),
    height: head.getUint32(4, true),
    png: bytes.subarray(PREVIEW_HEADER_LEN),
  };
}
