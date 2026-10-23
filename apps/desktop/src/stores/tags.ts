import { defineStore } from 'pinia';
import { ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, Tag, TagId } from '../api/types';
import { t } from '../i18n';
import { useLibraryStore } from './library';

/**
 * True when the API takes `a` and `b` as one tag: they differ only in the case
 * of A–Z (fm-core's `tag.name` is `COLLATE NOCASE`, which folds ASCII only, so
 * "Äb" and "äb" are two tags).
 */
export function sameTagName(a: string, b: string): boolean {
  const fold = (s: string) => s.replace(/[A-Z]/g, (c) => c.toLowerCase());
  return fold(a) === fold(b);
}

/**
 * Every tag the library has, A to Z, with its face count, as the API lists it.
 * The Info tab completes tag names from it and the filter chips name tag ids
 * with it. It lists again whenever the library changes (tagging is a change).
 */
export const useTagsStore = defineStore('tags', () => {
  const library = useLibraryStore();
  const tags = shallowRef<Tag[]>([]);
  const error = ref<ApiError | null>(null);

  async function refresh(): Promise<void> {
    try {
      tags.value = await useClient().listTags();
      error.value = null;
    } catch (e) {
      error.value = e as ApiError;
    }
  }

  /**
   * The id of the tag called `name`: the one with exactly this name (a face
   * detail lists names as stored), else the one the API takes as the same name.
   */
  function idOf(name: string): TagId | null {
    const tag =
      tags.value.find((t) => t.name === name) ?? tags.value.find((t) => sameTagName(t.name, name));
    return tag?.id ?? null;
  }

  /** The name of tag `id`, for a chip. */
  function nameOf(id: TagId): string {
    return (
      tags.value.find((tag) => tag.id === id)?.name ?? t('tags.fallbackName', { id: String(id) })
    );
  }

  watch(
    () => library.families,
    () => void refresh(),
    { immediate: library.loaded },
  );

  return { tags, error, refresh, idOf, nameOf };
});
