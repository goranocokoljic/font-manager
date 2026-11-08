import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { useClient } from '../api/client';
import { errorMessage, t } from '../i18n';
import type { FontPick } from './collections';
import { useAppStore } from './app';
import { useToastStore } from './toast';

/**
 * True when a pointer at (`x`, `y`) is on or past an edge of the `width` × `height`
 * viewport: the moment a held "Files" grip starts the native file drag.
 */
export function pastEdge(x: number, y: number, width: number, height: number): boolean {
  return x <= 0 || y <= 0 || x >= width - 1 || y >= height - 1;
}

/**
 * Dragging font files out of the window (E44.4, ADR 0022). The selection bar's
 * "Files" grip is held and moved; nothing is dragged inside the window. When the
 * pointer reaches the window edge, the app starts a native OS file drag of the
 * selection's files, where the user keeps them (never a copy), and the app the
 * user drops them on copies them. The row drag stays the HTML drag of ADR 0017.
 *
 * Whether this system can do it comes from fm-api (`AppInfo.fileDrag`); where it
 * cannot, the grip is disabled and its tooltip says why.
 */
export const useFileDragStore = defineStore('fileDrag', () => {
  const app = useAppStore();

  /** The drag works here. */
  const available = computed(() => app.info?.fileDrag?.available === true);
  /** Why it does not, for the grip's tooltip. */
  const reason = computed(() => app.info?.fileDrag?.reason ?? t('selection.bar.filesUnavailable'));
  /** The grip is held and the pointer has not reached the edge yet. */
  const holding = ref(false);
  /** A native drag was asked for and has not answered yet. */
  const outgoing = ref(false);

  /** The grip went down: wait for the pointer to reach the window edge. */
  function hold(): boolean {
    if (!available.value || outgoing.value) return false;
    holding.value = true;
    return true;
  }

  /** The grip was let go inside the window (or lost the pointer): nothing is dragged. */
  function release(): void {
    holding.value = false;
  }

  /**
   * The held pointer reached the window edge: starts the native drag of `pick`'s files.
   * Toasts when files were left out (gone from disk) or the drag failed.
   */
  async function dragOut(pick: FontPick): Promise<void> {
    if (!holding.value || outgoing.value) return;
    holding.value = false;
    outgoing.value = true;
    const toast = useToastStore();
    try {
      const files = await useClient().startFileDrag({
        faces: pick.faces,
        families: pick.families,
      });
      if (!files.paths.length) toast.show(t('selection.bar.filesNone'));
      else if (files.missing) toast.show(t('selection.bar.filesMissing', { count: files.missing }));
    } catch (e) {
      const reason = errorMessage(e) || t('selection.bar.filesFailedReason');
      toast.show(t('selection.bar.filesFailed', { reason }));
    } finally {
      outgoing.value = false;
    }
  }

  return { available, reason, holding, outgoing, hold, release, dragOut };
});
