<script setup lang="ts">
// The Tags group of the sidebar (E13.4): every tag with its face count. Drop fonts from
// the list or the table on a tag to put it on every dragged font, whole families
// included. While fonts are dragged the group unfolds and marks the tags the dragged
// fonts already carry; after the drop it folds back if it was folded.
import { onBeforeUnmount, onMounted, ref } from 'vue';
import type { TagId } from '../api/types';
import { ACTIONS, chordOf, formatChord } from '../keyboard/keymap';
import { isFontDrag, readFontDrop } from '../list/drag';
import { useTaggingStore } from '../stores/tagging';
import { useTagsStore } from '../stores/tags';

const tags = useTagsStore();
const tagging = useTaggingStore();
const target = ref<TagId | null>(null);
const tagChord = formatChord(chordOf(ACTIONS.tag)!);

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
}

function onDrop(name: string, e: DragEvent): void {
  e.preventDefault();
  target.value = null;
  const fonts = readFontDrop(e);
  if (fonts) void tagging.dropOn(name, fonts);
}

function countLabel(n: number): string {
  return `${n} ${n === 1 ? 'font' : 'fonts'}`;
}
</script>

<template>
  <section class="tags" aria-labelledby="fm-tags-heading">
    <header class="head">
      <h2 id="fm-tags-heading" class="heading">
        <button
          type="button"
          class="fold"
          :aria-expanded="!tagging.folded"
          aria-controls="fm-tags-list"
          @click="tagging.folded = !tagging.folded"
        >
          <span class="chevron" aria-hidden="true">{{ tagging.folded ? '▸' : '▾' }}</span>
          Tags
        </button>
      </h2>
      <span class="count" :aria-label="`${tags.tags.length} tags`">{{ tags.tags.length }}</span>
    </header>
    <p v-if="tags.error" role="alert" class="error">{{ tags.error.message }}</p>
    <ul v-show="!tagging.folded" id="fm-tags-list" class="list" aria-labelledby="fm-tags-heading">
      <li
        v-for="tag in tags.tags"
        :key="tag.id"
        class="item"
        :class="{
          carried: tagging.dragging && tagging.dragCarried.has(tag.id),
          drop: target === tag.id,
        }"
        :data-tag="tag.name"
        :data-carried="tagging.dragging && tagging.dragCarried.has(tag.id) ? 'true' : undefined"
        @dragover="onDragOver(tag.id, $event)"
        @dragleave="target = null"
        @drop="onDrop(tag.name, $event)"
      >
        <span class="name">{{ tag.name }}</span>
        <span class="count" :aria-label="countLabel(tag.faceCount)">{{ tag.faceCount }}</span>
      </li>
      <li v-if="!tags.tags.length" class="empty">
        No tags yet. Press {{ tagChord }} to tag the selected fonts.
      </li>
    </ul>
  </section>
</template>

<style scoped>
.tags {
  padding: 8px;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
}
.heading {
  margin: 0;
  padding: 8px 0;
  font-size: 11.5px;
  font-weight: 600;
}
.fold {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--fm-muted);
  font: inherit;
}
.chevron {
  width: 10px;
}
.list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border-radius: 6px;
}
.item:hover {
  background: var(--fm-bg);
}
/* A tag the dragged fonts already carry. */
.carried {
  box-shadow: inset 2px 0 0 var(--fm-accent);
  color: var(--fm-accent);
  font-weight: 600;
}
.drop {
  outline: 2px solid var(--fm-accent);
}
.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.count {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
}
.empty {
  padding: 4px 8px;
  color: var(--fm-muted);
}
.error {
  margin: 4px 8px;
  color: var(--fm-danger);
}
</style>
