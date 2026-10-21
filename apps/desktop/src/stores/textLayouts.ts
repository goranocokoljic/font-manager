import { defineStore } from 'pinia';
import { ref } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, Layout, LayoutBounds, LayoutPreset, TextBlock } from '../api/types';

/** The block values the layout view changes in place (E12.2). */
export type BlockEdit = Partial<Pick<TextBlock, 'sizePt' | 'lineHeight' | 'text'>>;

/**
 * The body-text layout presets and the ranges of their block values, as fm-api
 * returns them (loaded once), plus this session's edits to each preset: a
 * block resized or retyped in the layout view stays so until the app closes.
 */
export const useTextLayoutsStore = defineStore('textLayouts', () => {
  const presets = ref<Layout[]>([]);
  const bounds = ref<LayoutBounds | null>(null);
  const edited = ref<Partial<Record<LayoutPreset, Layout>>>({});
  const error = ref<ApiError | null>(null);
  let pending: Promise<void> | null = null;
  let edits = 0;

  /** Fetches the presets and the bounds; a second call while one is in flight waits for it. */
  function load(): Promise<void> {
    if (presets.value.length && bounds.value) return Promise.resolve();
    const client = useClient();
    pending ??= Promise.all([client.layoutPresets(), client.layoutBounds()])
      .then(
        ([list, b]) => {
          presets.value = list;
          bounds.value = b;
          error.value = null;
        },
        (e: unknown) => {
          error.value = e as ApiError;
        },
      )
      .finally(() => {
        pending = null;
      });
    return pending;
  }

  /** The layout of a preset, with this session's edits, once loaded. */
  const byPreset = (preset: LayoutPreset): Layout | null =>
    edited.value[preset] ?? presets.value.find((l) => l.preset === preset) ?? null;

  /**
   * Changes one block of a preset. The edit shows at once; the core then
   * clamps it, and its answer replaces the edit unless a newer one came first.
   */
  async function editBlock(preset: LayoutPreset, index: number, edit: BlockEdit): Promise<void> {
    const base = byPreset(preset);
    if (!base?.blocks[index]) return;
    const next: Layout = {
      ...base,
      blocks: base.blocks.map((b, i) => (i === index ? { ...b, ...edit } : b)),
    };
    edited.value = { ...edited.value, [preset]: next };
    const mine = ++edits;
    try {
      const clamped = await useClient().clampLayout(next);
      if (mine === edits) edited.value = { ...edited.value, [preset]: clamped };
    } catch (e) {
      error.value = e as ApiError;
    }
  }

  return { presets, bounds, error, load, byPreset, editBlock };
});
