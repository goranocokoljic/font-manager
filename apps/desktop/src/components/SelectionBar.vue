<script setup lang="ts">
// The selection bar (E44.1, board `4-drag.png`): a dark bar at the bottom centre of the
// library while rows are selected. It names the selection ('3 families') and acts on all of
// it through the calls the rest of the app already makes: Activate (the A key's bulk toggle),
// Collection (the family page's menu, with the whole selection), Tag (the tagging panel),
// Compare (pins the selection's faces, up to the sheet's four), Studio (the first face, and
// the second as the headline) and × (clears the selection). The parent shows it in an
// `<FmSlide from="bottom" distance="16px">`, the motion table's rise. A toolbar of plain
// buttons: each one is a Tab stop; Esc in it clears the selection like Esc in the list.
import { computed } from 'vue';
import { useRouter } from 'vue-router';
import { t } from '../i18n';
import { rowActivation } from '../list/activation';
import { pickOf } from '../list/drag';
import { useActivationStore } from '../stores/activation';
import { MAX_PINS, useCompareStore } from '../stores/compare';
import { useFontListStore } from '../stores/fontList';
import { useTaggingStore } from '../stores/tagging';
import FamilyCollectionMenu from './FamilyCollectionMenu.vue';
import UiIcon from './ui/UiIcon.vue';

/** The bar's × or Esc cleared the selection; the bar goes, so the focus must go somewhere. */
const emit = defineEmits<{ cleared: [] }>();

const list = useFontListStore();
const activation = useActivationStore();
const compare = useCompareStore();
const tagging = useTaggingStore();
const router = useRouter();

const rows = computed(() => list.selectedRows);
const onlyFamilies = computed(() => rows.value.every((r) => r.kind === 'family'));
const name = computed(() =>
  t(onlyFamilies.value ? 'selection.bar.families' : 'selection.bar.fonts', {
    count: rows.value.length,
  }),
);
/** Every selected row fully active: the toggle deactivates them (as the A key does). */
const allActive = computed(
  () => rows.value.length > 0 && rows.value.every((r) => rowActivation(r) === 'active'),
);
const pick = computed(() => pickOf(list.selection.selected));

function activate(): void {
  void activation.toggleRows(rows.value);
}

function tag(): void {
  void tagging.openFor(pick.value);
}

async function compareSelection(): Promise<void> {
  const free = MAX_PINS - compare.pins.length;
  for (const face of await list.selectedFaces(Math.max(1, free))) compare.pin(face);
  compare.openSheet();
}

async function studio(): Promise<void> {
  const [face, headline] = await list.selectedFaces(2);
  if (!face) return;
  const query: Record<string, string> = { face: String(face.id) };
  if (headline) query.headline = String(headline.id);
  await router.push({ name: 'studio', query });
}

function clear(): void {
  list.clearSelection();
  emit('cleared');
}

function onKey(e: KeyboardEvent): void {
  if (e.key !== 'Escape' || e.defaultPrevented) return;
  e.preventDefault();
  clear();
}
</script>

<template>
  <div class="dock">
    <div
      class="bar"
      role="toolbar"
      :aria-label="t('selection.bar.label')"
      data-testid="selection-bar"
      @keydown="onKey"
    >
      <span class="name" data-testid="selection-bar-name">{{ name }}</span>
      <button
        type="button"
        class="action"
        data-testid="bar-activate"
        :disabled="activation.busy"
        @click="activate"
      >
        <UiIcon name="power" />
        {{ allActive ? t('selection.bar.deactivate') : t('selection.bar.activate') }}
      </button>
      <FamilyCollectionMenu
        class="action"
        data-testid="bar-collection"
        :pick="pick"
        :text="t('selection.bar.collection')"
      />
      <button
        type="button"
        class="action"
        data-testid="bar-tag"
        :disabled="tagging.open"
        @click="tag"
      >
        <UiIcon name="tag" />
        {{ t('selection.bar.tag') }}
      </button>
      <button type="button" class="action" data-testid="bar-compare" @click="compareSelection">
        <UiIcon name="compare" />
        {{ t('selection.bar.compare') }}
      </button>
      <button type="button" class="action" data-testid="bar-studio" @click="studio">
        <UiIcon name="layout" />
        {{ t('selection.bar.studio') }}
      </button>
      <span class="divider" aria-hidden="true" />
      <button
        type="button"
        class="action clear"
        data-testid="bar-clear"
        :aria-label="t('selection.bar.clear')"
        :title="t('selection.bar.clear')"
        @click="clear"
      >
        <UiIcon name="close" />
      </button>
    </div>
  </div>
</template>

<style scoped>
/* The full width of the library, so the bar centres; only the bar takes the pointer. */
.dock {
  position: absolute;
  inset-inline: 0;
  bottom: var(--fm-space-6);
  z-index: 2;
  display: flex;
  justify-content: center;
  pointer-events: none;
}
.bar {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  padding: var(--fm-space-2) var(--fm-space-3);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bar-bg);
  box-shadow: var(--fm-shadow-2);
  color: var(--fm-bar-fg);
  font-size: var(--fm-text-sm);
  pointer-events: auto;
}
.name {
  padding-inline: var(--fm-space-2);
  font-weight: 600;
  white-space: nowrap;
}
.action,
.bar :deep(.action) {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-2);
  height: 32px;
  padding: 0 var(--fm-space-3);
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-bar-fg);
  font: inherit;
  white-space: nowrap;
}
.action:not(:disabled):hover,
.bar :deep(.action:not(:disabled):hover),
.bar :deep(.action[aria-expanded='true']) {
  background-color: rgb(255 255 255 / 12%);
  color: var(--fm-bar-fg);
}
.action:focus-visible,
.bar :deep(.action:focus-visible) {
  outline: 2px solid var(--fm-bar-fg);
  outline-offset: -2px;
}
.action:disabled {
  opacity: 0.55;
}
.clear {
  padding: 0 var(--fm-space-2);
}
.divider {
  align-self: stretch;
  width: 1px;
  margin-block: var(--fm-space-1);
  background-color: rgb(255 255 255 / 20%);
}
</style>
