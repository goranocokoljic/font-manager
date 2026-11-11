<script setup lang="ts">
// The shape picker of one letter in the Letters preset (E49.6, board 8-letters-picker.png):
// under the letter, one radio per value of the letter's glyph trait, each with the catalog's
// drawn example, its label and how many families of the current result have it, plus Any.
// One value per letter. A radio chosen with the arrow keys applies at once and the picker
// stays; a click or Enter chooses and closes. Escape and a press outside close it; the owner
// puts the focus back on the letter. The counts are the API's (`search.countTraitValues`).
import { computed, nextTick, ref, useId, watch } from 'vue';
import type { GlyphTraitInfo } from '../api/types';
import { t } from '../i18n';
import { useSearchStore } from '../stores/search';
import UiPopover from './ui/UiPopover.vue';

const props = defineProps<{
  open: boolean;
  anchor: HTMLElement | null;
  trait: GlyphTraitInfo;
  /** The value chosen now; null for Any. */
  chosen: string | null;
}>();
const emit = defineEmits<{
  /** A value was chosen; null is Any. */
  choose: [value: string | null];
  close: [];
}>();

const search = useSearchStore();
const uid = useId();
const body = ref<HTMLElement | null>(null);
/** Families per value in the current result; a value not counted yet is absent. */
const counts = ref<Map<string, number>>(new Map());
let generation = 0;
/** The value last sent, so a click and the change it causes choose once. */
let picked: string | null = null;

async function recount(): Promise<void> {
  const mine = ++generation;
  const next = await search.countTraitValues(props.trait);
  if (mine === generation) counts.value = next;
}
watch(
  () => props.open,
  (open) => {
    if (open) {
      counts.value = new Map();
      picked = props.chosen;
      void recount();
    }
  },
  { immediate: true },
);

const options = computed(() => [
  ...props.trait.values.map((v) => ({
    id: v.id,
    label: v.label,
    example: v.example,
    count: counts.value.get(v.id),
  })),
  { id: null, label: t('letters.any'), example: '', count: undefined },
]);

/** The focus goes to the chosen radio (Any when none) once the layer shows. */
async function focusChosen(): Promise<void> {
  await nextTick();
  const radios = body.value?.querySelectorAll<HTMLInputElement>('input[type="radio"]');
  const list = [...(radios ?? [])];
  (list.find((r) => r.checked) ?? list[0])?.focus();
}

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    emit('close');
    return;
  }
  // The layer ends <body>, so Tab from it would leave the page: it closes the picker and
  // the focus goes back to the letter, to go on from there (as FamilyStyles does).
  if (e.key === 'Tab') {
    e.preventDefault();
    emit('close');
    return;
  }
  // Plain keys stay in the picker: the app's one-key shortcuts (A activates) must not
  // act on the list behind it.
  if (!e.ctrlKey && !e.metaKey && !e.altKey) e.stopPropagation();
}

function choose(value: string | null): void {
  if (value === picked) return;
  picked = value;
  emit('choose', value);
}

/** A click (or Enter) chooses and closes; `detail` 0 is the click arrow keys cause. */
function chooseAndClose(value: string | null, e?: MouseEvent): void {
  if (e && e.detail === 0) return;
  choose(value);
  emit('close');
}
</script>

<template>
  <UiPopover :open="open" :anchor="anchor" @close="emit('close')" @placed="focusChosen">
    <div
      ref="body"
      class="picker"
      role="dialog"
      :aria-label="t('letters.pickerLabel', { name: trait.name })"
      data-testid="letter-picker"
      @keydown="onKey"
    >
      <div class="values" role="radiogroup" :aria-label="trait.name">
        <label
          v-for="o in options"
          :key="o.id ?? '__any'"
          class="option"
          :class="{ any: o.id === null, chosen: chosen === o.id }"
          @click="chooseAndClose(o.id, $event)"
        >
          <input
            class="sr-only"
            type="radio"
            :name="`${uid}-value`"
            :value="o.id ?? ''"
            :checked="chosen === o.id"
            @change="choose(o.id)"
            @keydown.enter.prevent="chooseAndClose(o.id)"
          />
          <svg v-if="o.example" class="example" viewBox="0 0 56 56" aria-hidden="true">
            <path :d="o.example" />
          </svg>
          <span class="label">{{ o.label }}</span>
          <span v-if="o.id !== null" class="n">{{
            o.count === undefined ? '' : t('chips.families', { count: o.count })
          }}</span>
        </label>
      </div>
    </div>
  </UiPopover>
</template>

<style scoped>
.values {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
  gap: var(--fm-space-1);
  width: 340px;
  max-width: 100%;
}
.picker {
  padding: var(--fm-space-2);
}
.option {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 6px 4px;
  border: 1px solid transparent;
  border-radius: var(--fm-radius-md);
  text-align: center;
  cursor: pointer;
}
.option:hover {
  background-color: var(--fm-surface);
}
.option.chosen {
  border-color: var(--fm-accent);
}
.option:focus-within {
  outline: 2px solid var(--fm-accent);
  outline-offset: -1px;
}
.option.any {
  justify-content: center;
  color: var(--fm-muted);
}
.example {
  width: 44px;
  height: 44px;
  fill: none;
  stroke: currentColor;
  stroke-width: 4;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.label {
  font-size: var(--fm-text-sm);
  font-weight: 500;
}
.n {
  min-height: 1.2em;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs, 11px);
}
</style>
