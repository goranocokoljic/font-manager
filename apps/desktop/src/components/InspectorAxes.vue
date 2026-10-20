<script setup lang="ts">
// The Axes tab: one slider and one number field per variation axis of the
// selected face (the font's axis name and tag, min to max, the default marked),
// its named instances as presets, and "Pin this instance", which saves the
// current coordinates as a virtual style of a manual collection or a set
// (E10.2). Moving an axis changes the face's preview rows live through
// font-variation-settings (the axes store); the font is not reloaded.
import { computed, onMounted, ref, useId, watch } from 'vue';
import type { ApiError, Axis, NamedInstance } from '../api/types';
import { axisStep, useAxesStore } from '../stores/axes';
import { useCollectionsStore } from '../stores/collections';
import { useSelectionStore } from '../stores/selection';
import { useSetsStore } from '../stores/sets';
import { useToastStore } from '../stores/toast';
import { useVirtualStylesStore } from '../stores/virtualStyles';

const selection = useSelectionStore();
const axesSet = useAxesStore();
const collections = useCollectionsStore();
const sets = useSetsStore();
const pinned = useVirtualStylesStore();
const toast = useToastStore();
const uid = useId();

const d = computed(() => selection.detail);
const faceId = computed(() => d.value?.face.id ?? null);
const axes = computed(() => d.value?.axes ?? []);

/** "Weight (wght)". */
const label = (a: Axis) => `${a.name} (${a.tag})`;
const value = (a: Axis) =>
  faceId.value === null ? (a.default ?? 0) : axesSet.value(faceId.value, a);

/** Tags whose number field holds text that is not a number. */
const invalid = ref(new Set<string>());
watch(faceId, () => (invalid.value = new Set()));

function onSlide(a: Axis, e: Event): void {
  if (faceId.value === null) return;
  axesSet.set(faceId.value, a, (e.target as HTMLInputElement).valueAsNumber);
  clearInvalid(a);
}

function onType(a: Axis, e: Event): void {
  const el = e.target as HTMLInputElement;
  if (faceId.value === null) return;
  if (axesSet.setTyped(faceId.value, a, el.value)) {
    clearInvalid(a);
    // A clamped value shows the clamped number, not what was typed.
    el.value = String(value(a));
  } else {
    invalid.value = new Set(invalid.value).add(a.tag);
  }
}

function clearInvalid(a: Axis): void {
  if (!invalid.value.has(a.tag)) return;
  const next = new Set(invalid.value);
  next.delete(a.tag);
  invalid.value = next;
}

function reset(): void {
  if (faceId.value === null) return;
  axesSet.reset(faceId.value);
  invalid.value = new Set();
}

const isAt = (i: NamedInstance) =>
  faceId.value !== null && axesSet.isAt(faceId.value, axes.value, i);
function choose(i: NamedInstance): void {
  if (faceId.value === null) return;
  axesSet.applyInstance(faceId.value, axes.value, i);
  invalid.value = new Set();
}

// Pinning: a name and a holder (a manual collection or a project set).
interface Holder {
  key: string;
  label: string;
  collection: number | null;
  set: number | null;
}
const holders = computed<Holder[]>(() => [
  ...collections.collections
    .filter((c) => !c.rules)
    .map((c) => ({ key: `c${c.id}`, label: c.name, collection: c.id, set: null })),
  ...sets.sets.map((s) => ({
    key: `s${s.id}`,
    label: `Set: ${s.name}`,
    collection: null,
    set: s.id,
  })),
]);
const holderKey = ref('');
const pinName = ref('');
const pinError = ref<string | null>(null);
const pinning = ref(false);

// The open manual collection is the first choice, else the first holder.
watch(
  [holders, () => collections.openId],
  ([list, open]) => {
    if (list.some((h) => h.key === holderKey.value)) return;
    holderKey.value = list.find((h) => h.collection === open)?.key ?? list[0]?.key ?? '';
  },
  { immediate: true },
);

/** The named instance the axes are at, if any: the name a pin starts with. */
const atInstance = computed(() => d.value?.instances.find((i) => isAt(i))?.name ?? '');
watch(atInstance, (name) => (pinName.value = name), { immediate: true });
watch(faceId, () => (pinError.value = null));

const canPin = computed(
  () =>
    faceId.value !== null &&
    pinName.value.trim() !== '' &&
    holderKey.value !== '' &&
    !pinning.value,
);

async function pin(): Promise<void> {
  const holder = holders.value.find((h) => h.key === holderKey.value);
  if (faceId.value === null || !holder || !canPin.value) return;
  pinning.value = true;
  try {
    const style = await pinned.pin({
      face: faceId.value,
      name: pinName.value.trim(),
      coordinates: axesSet.coordinates(faceId.value, axes.value),
      collection: holder.collection,
      set: holder.set,
    });
    pinError.value = null;
    toast.show(`Pinned “${style.name}” to ${holder.label}.`);
    if (holder.collection !== null) void collections.refresh();
  } catch (e) {
    pinError.value = (e as ApiError).message ?? 'The instance could not be pinned.';
  } finally {
    pinning.value = false;
  }
}

onMounted(() => {
  if (!collections.collections.length) void collections.refresh();
  if (!sets.sets.length) void sets.load();
});
</script>

<template>
  <div class="axes">
    <p v-if="!d" class="placeholder">Select a face to see its axes.</p>
    <p v-else-if="!axes.length" class="placeholder">This font has no variation axes</p>
    <template v-else>
      <div class="head">
        <p class="hint">Axes change this face's previews.</p>
        <button
          type="button"
          class="reset"
          :disabled="faceId === null || !axesSet.isChanged(faceId)"
          @click="reset"
        >
          Reset
        </button>
      </div>
      <ul class="plain list" aria-label="Variation axes">
        <li v-for="a in axes" :key="a.tag" class="axis" :data-tag="a.tag">
          <label class="name" :for="`${uid}-${a.tag}-range`">
            {{ a.name }} <span class="tag">{{ a.tag }}</span>
          </label>
          <div class="controls">
            <input
              :id="`${uid}-${a.tag}-range`"
              type="range"
              class="slider"
              :min="a.min ?? undefined"
              :max="a.max ?? undefined"
              :step="axisStep(a)"
              :value="value(a)"
              :list="`${uid}-${a.tag}-default`"
              :aria-valuetext="`${value(a)}`"
              @input="onSlide(a, $event)"
            />
            <datalist :id="`${uid}-${a.tag}-default`">
              <option :value="a.default ?? undefined" label="Default" />
            </datalist>
            <input
              type="text"
              inputmode="decimal"
              class="number"
              :value="value(a)"
              :aria-label="`${label(a)} value`"
              :aria-invalid="invalid.has(a.tag)"
              :aria-describedby="`${uid}-${a.tag}-range-text`"
              @change="onType(a, $event)"
            />
          </div>
          <p :id="`${uid}-${a.tag}-range-text`" class="range">
            {{ a.min }} to {{ a.max }}, default <span class="default">{{ a.default }}</span>
            <span v-if="invalid.has(a.tag)" class="error" role="alert"> — type a number</span>
          </p>
        </li>
      </ul>
      <section v-if="d.instances.length" :aria-labelledby="`${uid}-instances`">
        <h3 :id="`${uid}-instances`">Named instances</h3>
        <ul class="plain presets" :aria-labelledby="`${uid}-instances`">
          <li v-for="i in d.instances" :key="i.name">
            <button
              type="button"
              class="preset"
              :aria-pressed="isAt(i)"
              :data-instance="i.name"
              @click="choose(i)"
            >
              {{ i.name }}
            </button>
          </li>
        </ul>
      </section>
      <section class="pin" :aria-labelledby="`${uid}-pin`">
        <h3 :id="`${uid}-pin`">Pin this instance</h3>
        <p v-if="!holders.length" class="hint">
          Make a manual collection or a project set to pin instances into.
        </p>
        <template v-else>
          <label :for="`${uid}-pin-name`">Style name</label>
          <input
            :id="`${uid}-pin-name`"
            v-model="pinName"
            type="text"
            class="field"
            placeholder="Condensed Semibold"
          />
          <label :for="`${uid}-pin-holder`">Save in</label>
          <select :id="`${uid}-pin-holder`" v-model="holderKey" class="field">
            <option v-for="h in holders" :key="h.key" :value="h.key">{{ h.label }}</option>
          </select>
          <button type="button" class="pin-button" :disabled="!canPin" @click="pin">
            Pin this instance
          </button>
          <p v-if="pinError" class="error" role="alert">{{ pinError }}</p>
        </template>
      </section>
    </template>
  </div>
</template>

<style scoped>
.axes {
  padding: 16px;
}
.placeholder,
.hint {
  margin: 0;
  color: var(--fm-muted);
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding-bottom: 8px;
}
.reset,
.preset,
.pin-button {
  padding: 2px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.reset:disabled,
.pin-button:disabled {
  border-color: var(--fm-border);
  color: var(--fm-muted);
}
.preset[aria-pressed='true'] {
  border-color: var(--fm-accent);
  background: var(--fm-accent);
  color: var(--fm-bg);
}
h3 {
  margin: 16px 0 8px;
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
  text-transform: uppercase;
}
.plain {
  margin: 0;
  padding: 0;
  list-style: none;
}
.axis {
  padding: 6px 0;
}
.name {
  display: block;
  overflow-wrap: anywhere;
}
.tag {
  margin-left: 4px;
  color: var(--fm-muted);
  font-family: ui-monospace, monospace;
}
.controls {
  display: flex;
  align-items: center;
  gap: 8px;
}
.slider {
  flex: 1;
  min-width: 0;
}
.number,
.field {
  box-sizing: border-box;
  padding: 2px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.number {
  width: 72px;
}
.number[aria-invalid='true'] {
  border-color: var(--fm-warning);
}
.range {
  margin: 2px 0 0;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.default {
  font-weight: 600;
}
.error {
  color: var(--fm-warning);
}
.presets {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.pin {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.pin-button {
  align-self: flex-start;
  margin-top: 4px;
}
</style>
