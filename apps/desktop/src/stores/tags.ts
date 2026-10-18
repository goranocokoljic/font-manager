import { defineStore } from 'pinia';
import { ref, shallowRef, watch } from 'vue';
import { useClient } from '../api/client';
import type { ApiError, Tag, TagId } from '../api/types';
import { useLibraryStore } from './library';

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

  /** The id of the tag called `name`; the API compares names without regard to case. */
  function idOf(name: string): TagId | null {
    const lower = name.toLowerCase();
    return tags.value.find((t) => t.name.toLowerCase() === lower)?.id ?? null;
  }

  /** The name of tag `id`, for a chip. */
  function nameOf(id: TagId): string {
    return tags.value.find((t) => t.id === id)?.name ?? `Tag ${id}`;
  }

  watch(
    () => library.families,
    () => void refresh(),
    { immediate: library.loaded },
  );

  return { tags, error, refresh, idOf, nameOf };
});
