<script setup lang="ts">
// The Tags group of the sidebar (E13.4): every tag as a chip, its face count in the tooltip
// and for screen readers. Drop fonts from
// the list or the table on a tag to put it on every dragged font, whole families
// included. While fonts are dragged the group unfolds and marks the tags the dragged
// fonts already carry; after the drop it folds back if it was folded. The tag under the
// pointer shows the accent ring and its count now and after the drop ("31 → 34", E44.2);
// the drop flies the ghost into it.
// Each tag is a button (E54.3): a click filters the list by it and marks it pressed, a second
// click takes it off; several combine. It is the search store's `filters.tags`, the one the
// Filters popover and its chips set. Tab and the arrow keys move between the tags. A drop
// only tags: it is no click, so it never toggles the filter.
import { onBeforeUnmount, onMounted, ref } from 'vue';
import type { TagId } from '../api/types';
import { errorMessage, formatNumber, t } from '../i18n';
import { isFontDrag, readFontDrop } from '../list/drag';
import { FmFade } from '../motion';
import { useFontDragStore, type DropTarget } from '../stores/fontDrag';
import { useSearchStore } from '../stores/search';
import { useTaggingStore } from '../stores/tagging';
import { useTagsStore } from '../stores/tags';
import SidebarHead from './SidebarHead.vue';

const tags = useTagsStore();
const tagging = useTaggingStore();
const search = useSearchStore();
const list = ref<HTMLElement | null>(null);
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

/** The arrow keys, Home and End move the focus between the tags, in the reading direction. */
function onKey(e: KeyboardEvent): void {
  if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
  const buttons = [...(list.value?.querySelectorAll<HTMLElement>('button.chip') ?? [])];
  const at = buttons.indexOf(e.target as HTMLElement);
  if (at < 0) return;
  const rtl = getComputedStyle(e.target as HTMLElement).direction === 'rtl';
  const step: Record<string, number> = {
    ArrowDown: 1,
    ArrowUp: -1,
    ArrowRight: rtl ? -1 : 1,
    ArrowLeft: rtl ? 1 : -1,
  };
  let next: number;
  if (e.key in step) next = Math.min(buttons.length - 1, Math.max(0, at + step[e.key]!));
  else if (e.key === 'Home') next = 0;
  else if (e.key === 'End') next = buttons.length - 1;
  else return;
  e.preventDefault();
  buttons[next]?.focus();
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
    <ul
      v-show="!tagging.folded"
      id="fm-tags-list"
      ref="list"
      class="list"
      aria-labelledby="fm-tags-heading"
      @keydown="onKey"
    >
      <li
        v-for="tag in tags.tags"
        :key="tag.id"
        class="tag"
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
        <button
          type="button"
          class="chip"
          :title="countLabel(tag.faceCount)"
          :aria-pressed="search.isTag(tag.id)"
          @click="search.toggleTag(tag.id)"
        >
          <span class="name">{{ tag.name }}</span>
          <span class="sr-only count" :aria-label="countLabel(tag.faceCount)">{{
            countLabel(tag.faceCount)
          }}</span>
        </button>
        <FmFade speed="fast">
          <span
            v-if="dropCount(tag.id)"
            class="drop-count"
            data-testid="tag-drop-count"
            :aria-label="dropCount(tag.id)!.label"
            >{{ dropCount(tag.id)!.text }}</span
          >
        </FmFade>
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
.tag {
  /* The count floats above the chip, so the chip under the pointer keeps its size and place:
     a wider chip could wrap to the next line, leave the pointer and miss the drop. */
  position: relative;
  max-width: 100%;
  border-radius: 999px;
}
.chip {
  display: block;
  max-width: 100%;
  padding: 2px var(--fm-space-2);
  border: none;
  border-radius: 999px;
  background-color: var(--fm-hover);
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
.chip:hover {
  background-color: var(--fm-active);
  background-image: none;
}
/* The tag filters the list (E54.3): filled with the accent, as a pressed filter chip is, so
   the mark reaches 3:1 against an unpressed tag. */
.chip[aria-pressed='true'] {
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
}
/* A tag the dragged fonts already carry: a ring, not bold type, so no chip changes width and
   moves another under the pointer during the drag. */
.carried .chip {
  box-shadow: inset 0 0 0 1px var(--fm-accent);
}
.carried .chip[aria-pressed='true'] {
  box-shadow: inset 0 0 0 1px var(--fm-on-accent);
}
.drop {
  outline: 2px solid var(--fm-accent);
}
.drop-count {
  position: absolute;
  inset-block-end: calc(100% + var(--fm-space-1));
  inset-inline-start: 50%;
  z-index: var(--fm-z-popover);
  padding: 2px var(--fm-space-2);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font-weight: 600;
  white-space: nowrap;
  pointer-events: none;
  translate: -50% 0;
}
:dir(rtl) .drop-count {
  translate: 50% 0;
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
