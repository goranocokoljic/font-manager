<script setup lang="ts">
// The font picker of one Pairing Studio block (E24.2): a combobox that searches the
// library (fm-core's search, through the studio store) and sets the block in the chosen
// family's best face. While the box is not being typed in, it shows the block's own
// font, or "Panel font". "Use panel font" puts the block back on the panel's face.
// Escape closes an open list without closing the studio.
import { computed, ref, useId, watch } from 'vue';
import type { FaceSummary, FamilyHit } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useStudioStore } from '../stores/studio';

const props = defineProps<{
  /** The layout the block belongs to, as its panel's block-font scope (`blockScope`). */
  layoutId: string;
  /** The block's index in the layout. */
  index: number;
  /** The block's accessible name ("Block 1"). */
  label: string;
  /** The block's own face; null when it uses the panel's. */
  face: FaceSummary | null;
}>();

const studio = useStudioStore();
const id = useId();
const listId = `${id}-list`;
const optionId = (i: number) => `${id}-option-${i}`;

const listOpen = ref(false);
const active = ref(0);
const mine = computed(() => studio.searchFor === `${props.layoutId}#${props.index}`);
const results = computed(() => (mine.value ? studio.results : []));
const expanded = computed(() => listOpen.value && mine.value);
const faceName = computed(() => (props.face ? `${props.face.familyName} ${props.face.style}` : ''));
const shown = computed(() => (expanded.value ? studio.query : faceName.value));

watch(results, () => (active.value = 0));

function search(text: string): void {
  listOpen.value = true;
  void studio.search(props.layoutId, props.index, text);
}

function onFocus(): void {
  if (!expanded.value) search('');
}

function close(): void {
  listOpen.value = false;
}

async function choose(hit: FamilyHit | undefined): Promise<void> {
  if (!hit) return;
  close();
  await studio.pickFamily(props.layoutId, props.index, hit);
}

function onKey(e: KeyboardEvent): void {
  const n = results.value.length;
  switch (e.key) {
    case 'ArrowDown':
      if (!expanded.value) search(studio.query);
      else if (n) active.value = (active.value + 1) % n;
      break;
    case 'ArrowUp':
      if (!expanded.value) search(studio.query);
      else if (n) active.value = (active.value - 1 + n) % n;
      break;
    case 'Enter':
      if (!expanded.value) return;
      void choose(results.value[active.value]);
      break;
    case 'Escape':
      if (!expanded.value) return;
      e.stopPropagation();
      close();
      break;
    default:
      return;
  }
  e.preventDefault();
}
</script>

<template>
  <div class="block-font" data-testid="block-font">
    <input
      :value="shown"
      class="field"
      type="text"
      role="combobox"
      :aria-label="t('studio.blockFont', { label })"
      aria-autocomplete="list"
      :aria-expanded="expanded"
      :aria-controls="listId"
      :aria-activedescendant="expanded && results.length ? optionId(active) : undefined"
      :placeholder="t('studio.panelFont')"
      :title="face ? faceName : t('studio.panelFont')"
      autocomplete="off"
      spellcheck="false"
      @input="search(($event.target as HTMLInputElement).value)"
      @focus="onFocus"
      @blur="close"
      @keydown="onKey"
    />
    <!-- mousedown.prevent keeps focus in the input while an option is clicked. -->
    <ul
      v-show="expanded"
      :id="listId"
      class="list"
      role="listbox"
      :aria-label="t('studio.fonts')"
      @mousedown.prevent
    >
      <li
        v-for="(hit, i) in results"
        :id="optionId(i)"
        :key="hit.family.id"
        role="option"
        class="option"
        :class="{ active: i === active }"
        :aria-selected="i === active"
        @mousemove="active = i"
        @click="choose(hit)"
      >
        {{ hit.family.name }}
      </li>
      <li v-if="!results.length" class="muted" role="presentation">
        {{ studio.error ? errorMessage(studio.error) : t('studio.noMatch') }}
      </li>
    </ul>
    <button
      v-if="face"
      type="button"
      class="reset"
      data-testid="block-font-reset"
      :aria-label="t('studio.usePanelFontFor', { label })"
      @click="studio.resetBlock(layoutId, index)"
    >
      {{ t('studio.usePanelFont') }}
    </button>
  </div>
</template>

<style scoped>
.block-font {
  position: relative;
  align-self: stretch;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 4px;
  min-width: 0;
  font:
    11px/1.4 system-ui,
    sans-serif;
}
.field {
  box-sizing: border-box;
  width: 100%;
  padding: 1px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 3px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
}
.list {
  position: absolute;
  top: 100%;
  inset-inline-start: 0;
  z-index: 2;
  box-sizing: border-box;
  min-width: 100%;
  max-width: 20em;
  max-height: 16em;
  margin: 2px 0 0;
  padding: 2px 0;
  overflow: auto;
  list-style: none;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: var(--fm-bg);
  color: var(--fm-fg);
}
.option,
.muted {
  padding: 2px 8px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.option {
  cursor: pointer;
}
.option.active {
  background: var(--fm-surface);
  box-shadow: inset 2px 0 0 var(--fm-accent);
}
.muted {
  color: var(--fm-muted);
}
.reset {
  padding: 1px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 3px;
  background: transparent;
  color: var(--fm-fg);
  font: inherit;
  white-space: nowrap;
  cursor: pointer;
}
.field:focus-visible,
.reset:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
