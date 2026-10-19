import { defineStore } from 'pinia';
import { shallowReactive, watch } from 'vue';
import { useClient } from '../api/client';
import type { FaceId } from '../api/types';
import { useLibraryStore } from './library';

/** Thumbnails kept; past this many the oldest are dropped (and asked again when needed). */
export const MAX_THUMBNAILS = 600;
/** Renders running at once; the rest wait their turn. */
export const MAX_RENDERS = 6;

/** A rendered thumbnail, ready for CSS: an object URL and its size in CSS px. */
export interface Thumbnail {
  url: string;
  width: number;
  height: number;
}

const key = (face: FaceId, text: string, sizePx: number, scale: number) =>
  `${face}\u0000${sizePx}\u0000${scale}\u0000${text}`;

/** The device pixel ratio a thumbnail renders at, so it is sharp on HiDPI screens. */
function deviceScale(): number {
  const dpr = typeof window === 'undefined' ? 1 : window.devicePixelRatio;
  return Number.isFinite(dpr) && dpr > 0 ? Math.min(dpr, 3) : 1;
}

/**
 * Grid thumbnails: text rasterized in a face by the API (`render_preview`, which
 * caches by face hash and settings, E4.3), black on transparent. Cards ask for the
 * faces on screen only; at most `MAX_RENDERS` renders run at once. Kept as object
 * URLs (revoked when dropped) until the library changes; a failed face is not asked
 * again until then.
 */
export const useThumbnailsStore = defineStore('thumbnails', () => {
  /** Thumbnails by (face, text, size, scale). Reading one key tracks it. */
  const ready = shallowReactive(new Map<string, Thumbnail>());
  /** Keys asked for and not answered yet, or failed. */
  const asked = new Set<string>();
  const failed = shallowReactive(new Set<string>());
  let queue: { k: string; face: FaceId; text: string; sizePx: number; scale: number }[] = [];
  let running = 0;
  /** Bumped by every reset; answers to an older round are dropped. */
  let round = 0;

  /** The thumbnail of `face` showing `text` at `sizePx` CSS px, if it has arrived. */
  function get(face: FaceId, text: string, sizePx: number): Thumbnail | undefined {
    return ready.get(key(face, text, sizePx, deviceScale()));
  }

  /** Whether the thumbnail could not be rendered. */
  function hasFailed(face: FaceId, text: string, sizePx: number): boolean {
    return failed.has(key(face, text, sizePx, deviceScale()));
  }

  /** Asks for a thumbnail unless it is known or asked already. */
  function want(face: FaceId, text: string, sizePx: number): void {
    if (!text) return;
    const scale = deviceScale();
    const k = key(face, text, sizePx, scale);
    // Reading `ready` here lets a caller's effect ask again after a reset.
    if (ready.has(k) || asked.has(k)) return;
    asked.add(k);
    queue.push({ k, face, text, sizePx, scale });
    pump();
  }

  function keep(k: string, thumb: Thumbnail): void {
    ready.set(k, thumb);
    for (const [old, t] of ready) {
      if (ready.size <= MAX_THUMBNAILS) break;
      ready.delete(old);
      asked.delete(old);
      URL.revokeObjectURL(t.url);
    }
  }

  function pump(): void {
    while (running < MAX_RENDERS && queue.length) {
      const job = queue.shift()!;
      const mine = round;
      running++;
      const px = Math.min(300, Math.max(8, Math.round(job.sizePx * job.scale)));
      void useClient()
        .renderPreview({ face: job.face, text: job.text, sizePx: px })
        .then(
          (img) => {
            if (mine !== round) return;
            const blob = new Blob([new Uint8Array(img.png)], { type: 'image/png' });
            const ratio = px / job.sizePx;
            keep(job.k, {
              url: URL.createObjectURL(blob),
              width: img.width / ratio,
              height: img.height / ratio,
            });
          },
          () => {
            if (mine === round) failed.add(job.k);
          },
        )
        .finally(() => {
          if (mine === round) running--;
          pump();
        });
    }
  }

  /** Drops every thumbnail: the library changed, so a face's file may have. */
  function reset(): void {
    round++;
    running = 0;
    queue = [];
    asked.clear();
    failed.clear();
    for (const t of ready.values()) URL.revokeObjectURL(t.url);
    ready.clear();
  }

  watch(() => useLibraryStore().families, reset);

  return { get, hasFailed, want, reset };
});
