<script setup lang="ts">
// The Tags group of the sidebar (E13.4): every tag as a chip, its face count in the tooltip
// and for screen readers. Drop fonts from
// the list or the table on a tag to put it on every dragged font, whole families
// included. While fonts are dragged the group unfolds and marks the tags the dragged
// fonts already carry; after the drop it folds back if it was folded. The tag under the
// pointer shows the accent ring and its count now and after the drop ("31 → 34", E44.2);
// the drop flies the ghost into it.
import { onBeforeUnmount, onMounted, ref } from 'vue';
import type { TagId } from '../api/types';
import { errorMessage, formatNumber, t } from '../i18n';
import { isFontDrag, readFontDrop } from '../list/drag';
import { FmFade } from '../motion';
import { useFontDragStore, type DropTarget } from '../stores/fontDrag';
import { useTaggingStore } from '../stores/tagging';
import { useTagsStore } from '../stores/tags';
import SidebarHead from './SidebarHead.vue';

const tags = useTagsStore();
const tagging = useTaggingStore();
const target = ref<TagId | null>(null);
const fontDrag = useFontDragStore();
const dropTarget = (id: TagId): DropTarget => ({ kind: 'tag', id });

/** "31 → 34" while a font drag is over the tag and fm-api has answered. */
function dropCount(id: TagId): { text: string; label: string } | null {
  const c = fontDrag.isTarget(dropTarget(id)) ? fontDrag.countOf(dropTarget(id)) : null;
  if (!c) return null;
  return {
    text: t('drag.count', { before: formatNumber(c.before), after: formatNumber(c.after) }),
    label: t('drag.countLabel', { before: countLabel(c.before), after: countLabel(c.after) }),
  };
}

// Any font drag in the window: the payload can be read while the drag starts.
function onWindowDragStart(e: DragEvent): void {
  if (!isFontDrag(e)) return;
  const pick = readFontDrop(e);
  if (pick) void tagging.startDrag(pick);
}
function onWindowDragEnd(): void {
  target.value = null;
  tagging.endDrag();
}
onMounted(() => {
  void tags.refresh();
  window.addEventListener('dragstart', onWindowDragStart);
  window.addEventListener('dragend', onWindowDragEnd);
  window.addEventListener('drop', onWindowDragEnd);
});
onBeforeUnmount(() => {
  window.removeEventListener('dragstart', onWindowDragStart);
  window.removeEventListener('dragend', onWindowDragEnd);
  window.removeEventListener('drop', onWindowDragEnd);
});

function onDragOver(id: TagId, e: DragEvent): void {
  if (!isFontDrag(e)) return;
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy';
  target.value = id;
  fontDrag.enter(dropTarget(id));
}

function onDragLeave(id: TagId): void {
  target.value = null;
  fontDrag.leave(dropTarget(id));
}

function onDrop(name: string, e: DragEvent): void {
  e.preventDefault();
  target.value = null;
  const fonts = readFontDrop(e);
  if (fonts && fontDrag.land(e.currentTarget as Element | null, e))
    void tagging.dropOn(name, fonts);
}

function countLabel(n: number): string {
  return t('tags.fontCount', { count: n });
}
</script>

<template>
  <section class="side-section" aria-labelledby="fm-tags-heading">
    <SidebarHead
      id="fm-tags-heading"
      :folded="tagging.folded"
      controls="fm-tags-list"
      @toggle="tagging.folded = !tagging.folded"
    >
      {{ t('tags.heading') }}
    </SidebarHead>
    <p v-if="tags.error" role="alert" class="error">{{ errorMessage(tags.error) }}</p>
    <ul v-show="!tagging.folded" id="fm-tags-list" class="list" aria-labelledby="fm-tags-heading">
      <li
        v-for="tag in tags.tags"
        :key="tag.id"
        class="chip"
        :title="countLabel(tag.faceCount)"
        :class="{
          carried: tagging.dragging && tagging.dragCarried.has(tag.id),
          drop: target === tag.id,
        }"
        :data-tag="tag.name"
        :data-carried="tagging.dragging && tagging.dragCarried.has(tag.id) ? 'true' : undefined"
        @dragover="onDragOver(tag.id, $event)"
        @dragleave="onDragLeave(tag.id)"
        @drop="onDrop(tag.name, $event)"
      >
        <span class="name">{{ tag.name }}</span>
        <FmFade speed="fast">
          <span
            v-if="dropCount(tag.id)"
            class="drop-count"
            data-testid="tag-drop-count"
            :aria-label="dropCount(tag.id)!.label"
            >{{ dropCount(tag.id)!.text }}</span
          >
        </FmFade>
        <span class="sr-only count" :aria-label="countLabel(tag.faceCount)">{{
          formatNumber(tag.faceCount)
        }}</span>
      </li>
      <li v-if="!tags.tags.length" class="side-quiet">{{ t('tags.empty') }}</li>
    </ul>
  </section>
</template>

<style scoped>
.list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1);
  margin: 0;
  padding: 0 var(--fm-space-2);
  list-style: none;
}
.chip {
  max-width: 100%;
  padding: 2px var(--fm-space-2);
  border-radius: 999px;
  background-color: var(--fm-hover);
  font-size: var(--fm-text-sm);
}
.chip:hover {
  background-color: var(--fm-active);
}
/* A tag the dragged fonts already carry. */
.carried {
  box-shadow: inset 0 0 0 1px var(--fm-accent);
  color: var(--fm-fg);
  font-weight: 600;
}
.drop {
  outline: 2px solid var(--fm-accent);
}
.chip:has(.drop-count) {
  display: flex;
  gap: var(--fm-space-1);
}
.drop-count {
  font-weight: 600;
  white-space: nowrap;
}
.name {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.side-quiet {
  width: 100%;
  padding-inline: 0;
}
.error {
  margin: 4px 8px;
  color: var(--fm-danger);
}
</style>
