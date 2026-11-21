<script setup lang="ts" generic="T">
// A short ordered list the user reorders (E56.14): by dragging a row onto another, or from
// the row's grip button with the keyboard — Up and Down move the item one place, Home and
// End move it to the top or the end. The focus stays on the moved item's grip, and a live
// region says where it went. The parent owns the order (`v-model:items`); each row's
// content is the default slot.
import { nextTick, ref, useId } from 'vue';
import { t } from '../../i18n';
import UiIconButton from './UiIconButton.vue';

// Attributes (`data-testid`, `class`) go on the list, not on the live regions beside it.
defineOptions({ inheritAttrs: false });

const props = defineProps<{
  items: readonly T[];
  /** A stable key of an item, unique in the list. */
  keyOf: (item: T) => string;
  /** The item's name, for the grip's label and the announcement. */
  labelOf: (item: T) => string;
  /** The list's accessible name. */
  label: string;
}>();
const emit = defineEmits<{ 'update:items': [items: T[]] }>();

const uid = useId();
const hintId = `${uid}-hint`;
const gripId = (item: T) => `${uid}-grip-${props.keyOf(item)}`;
const announcement = ref('');

async function move(from: number, to: number, refocus: boolean): Promise<void> {
  const last = props.items.length - 1;
  const target = Math.max(0, Math.min(last, to));
  if (from === target || from < 0 || from > last) return;
  const next = [...props.items];
  const [item] = next.splice(from, 1) as [T];
  next.splice(target, 0, item);
  emit('update:items', next);
  announcement.value = t('reorder.moved', {
    name: props.labelOf(item),
    position: target + 1,
    count: next.length,
  });
  if (!refocus) return;
  await nextTick();
  document.getElementById(gripId(item))?.focus();
}

function onGripKey(index: number, e: KeyboardEvent): void {
  const to =
    e.key === 'ArrowUp'
      ? index - 1
      : e.key === 'ArrowDown'
        ? index + 1
        : e.key === 'Home'
          ? 0
          : e.key === 'End'
            ? props.items.length - 1
            : null;
  if (to === null) return;
  e.preventDefault();
  void move(index, to, true);
}

const dragging = ref<number | null>(null);
const dropTarget = ref<number | null>(null);
// A list may hold another one (a rule's own short list): each handles its own rows only.
function onDragStart(index: number, e: DragEvent): void {
  e.stopPropagation();
  dragging.value = index;
  e.dataTransfer?.setData('text/plain', String(index));
  if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
}
function onDragOver(index: number, e: DragEvent): void {
  if (dragging.value === null) return;
  e.preventDefault();
  e.stopPropagation();
  dropTarget.value = index;
}
function onDrop(index: number, e: DragEvent): void {
  if (dragging.value === null) return;
  e.preventDefault();
  e.stopPropagation();
  void move(dragging.value, index, false);
  onDragEnd();
}
function onDragEnd(): void {
  dragging.value = null;
  dropTarget.value = null;
}
</script>

<template>
  <ol v-bind="$attrs" class="ui-reorder-list" :aria-label="label">
    <li
      v-for="(item, index) in items"
      :key="keyOf(item)"
      class="row"
      :class="{ dragging: dragging === index, target: dropTarget === index && dragging !== index }"
      :data-key="keyOf(item)"
      draggable="true"
      @dragstart="onDragStart(index, $event)"
      @dragover="onDragOver(index, $event)"
      @drop="onDrop(index, $event)"
      @dragend="onDragEnd"
    >
      <UiIconButton
        :id="gripId(item)"
        class="grip"
        icon="grip"
        size="sm"
        :label="t('reorder.move', { name: labelOf(item) })"
        :title="t('reorder.hint')"
        :aria-describedby="hintId"
        aria-keyshortcuts="ArrowUp ArrowDown Home End"
        @keydown="onGripKey(index, $event)"
      />
      <span class="position" aria-hidden="true">{{ index + 1 }}</span>
      <slot :item="item" :index="index" />
    </li>
  </ol>
  <span :id="hintId" class="sr-only">{{ t('reorder.hint') }}</span>
  <span class="sr-only" aria-live="polite">{{ announcement }}</span>
</template>

<style scoped>
.ui-reorder-list {
  margin: 0;
  padding: 0;
  list-style: none;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
}
.row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2);
  min-height: 40px;
  padding: var(--fm-space-1) var(--fm-space-3) var(--fm-space-1) var(--fm-space-1);
}
.row + .row {
  border-top: 1px solid var(--fm-border);
}
.row.dragging {
  opacity: 0.5;
}
.row.target {
  box-shadow: inset 0 2px 0 var(--fm-accent);
}
.grip {
  cursor: grab;
}
.position {
  min-width: 1.5em;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  font-variant-numeric: tabular-nums;
}
</style>
