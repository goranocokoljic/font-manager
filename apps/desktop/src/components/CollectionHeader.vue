<script setup lang="ts">
// The open collection's mini specimen strip: its first families, each name set in its own
// face, on a band of the collection's colour (E44.3). Hidden while the list has nothing to
// show or the families' faces are not known yet.
import { computed, watchEffect } from 'vue';
import { collectionColorVar } from '../api/collectionColors';
import { t } from '../i18n';
import { useCollectionsStore } from '../stores/collections';
import { useFontListStore } from '../stores/fontList';
import { useLibraryStore } from '../stores/library';
import CardLiveText from './CardLiveText.vue';

/** Families the strip shows: what fits a narrow window, and few faces to hold. */
const STRIP_FAMILIES = 6;
const STRIP_PX = 18;

const collections = useCollectionsStore();
const fontList = useFontListStore();
const library = useLibraryStore();

const families = computed(() =>
  collections.open ? fontList.families.slice(0, STRIP_FAMILIES) : [],
);

// `previewFace` answers from the faces already fetched: ask for those of the strip's families.
watchEffect(() => {
  for (const family of families.value) void library.facesOf(family.id).catch(() => undefined);
});

const specimens = computed(() =>
  families.value.flatMap((family) => {
    const face = fontList.previewFace(family.id);
    return face ? [{ id: family.id, name: family.name, face: face.id }] : [];
  }),
);
const color = computed(() => collectionColorVar(collections.open?.color));
</script>

<template>
  <ul
    v-if="specimens.length"
    class="strip"
    :class="{ tinted: color }"
    :style="{ '--strip-color': color }"
    :aria-label="t('collections.specimens', { name: collections.open?.name ?? '' })"
    data-testid="collection-strip"
  >
    <li v-for="specimen in specimens" :key="specimen.id" class="specimen">
      <CardLiveText :face="specimen.face" :text="specimen.name" :size-px="STRIP_PX" />
    </li>
  </ul>
</template>

<style scoped>
.strip {
  display: flex;
  flex: none;
  gap: var(--fm-space-5);
  margin: var(--fm-space-2) var(--fm-space-3) 0;
  padding: var(--fm-space-2) var(--fm-space-3);
  overflow: hidden;
  border-inline-start: 3px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-surface);
  list-style: none;
}
/* The words stay --fm-fg: the band carries the colour, at 10 % so the text keeps its contrast. */
.tinted {
  border-inline-start-color: var(--strip-color);
  background-color: color-mix(in srgb, var(--strip-color) 10%, var(--fm-bg));
}
.specimen {
  display: flex;
  flex: none;
  max-width: 220px;
  overflow: hidden;
}
</style>
