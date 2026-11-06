<script setup lang="ts">
// The family page's axis row: one slider and one number field per variation axis of the
// selected face (the font's axis name and tag, min to max, the default marked), ticks and a
// "= Medium" label for the named instances on the weight axis, the instances as presets,
// "Pin 537 as my instance", which saves the current coordinates as a virtual style of a manual
// collection or a set (E10.2), and "Copy CSS" for the moved axes (E10.5). Moving an axis changes
// the face's previews live through font-variation-settings (the axes store); the font is not
// reloaded.
import { computed, onMounted, ref, useId, watch } from 'vue';
import type { ApiError, Axis, NamedInstance } from '../api/types';
import { errorMessage, t } from '../i18n';
import { axisStep, instanceNear, useAxesStore, weightMarks } from '../stores/axes';
import CopyCss from './CopyCss.vue';
import UiButton from './ui/UiButton.vue';
import UiSegmentedControl from './ui/UiSegmentedControl.vue';
import UiSelect from './ui/UiSelect.vue';
import { useCollectionsStore } from '../stores/collections';
import { useFamilyPageStore } from '../stores/familyPage';
import { useSelectionStore } from '../stores/selection';
import { useSetsStore } from '../stores/sets';
import { useToastStore } from '../stores/toast';
import { useVirtualStylesStore } from '../stores/virtualStyles';

const selection = useSelectionStore();
const axesSet = useAxesStore();
const collections = useCollectionsStore();
const sets = useSetsStore();
const pinned = useVirtualStylesStore();
const page = useFamilyPageStore();
const toast = useToastStore();
const uid = useId();

const d = computed(() => selection.detail);
const faceId = computed(() => d.value?.face.id ?? null);
const axes = computed(() => d.value?.axes ?? []);

const wght = computed(() => axes.value.find((a) => a.tag === 'wght') ?? null);
const marks = computed(() => weightMarks(d.value?.instances ?? []));
/** Where a mark sits on the slider's run, 0 to 1. */
function markAt(a: Axis, value: number): number {
  const [min, max] = [a.min ?? 0, a.max ?? 0];
  return max > min ? (value - min) / (max - min) : 0;
}
/** "= Medium" at a named instance of the weight axis, else "between instances". */
const instanceLabel = computed(() => {
  if (!wght.value || !marks.value.length) return null;
  const name = instanceNear(marks.value, value(wght.value));
  return name ? t('axes.atInstance', { name }) : t('axes.betweenInstances');
});

/** "Weight (wght) value". */
const valueLabel = (a: Axis) => t('axes.valueLabel', { name: a.name, tag: a.tag });
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
function chooseByName(name: string): void {
  const i = d.value?.instances.find((x) => x.name === name);
  if (i) choose(i);
}
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
    label: t('axes.setHolder', { name: s.name }),
    collection: null,
    set: s.id,
  })),
]);
const holderOptions = computed(() => holders.value.map((h) => ({ value: h.key, label: h.label })));
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

/** What the pin is called when the name field is empty: there is one click to pin the weight. */
const pinFallback = computed(() =>
  wght.value ? t('axes.pinDefaultName', { value: value(wght.value) }) : '',
);
const pinStyleName = computed(() => pinName.value.trim() || pinFallback.value);
const pinLabel = computed(() =>
  wght.value ? t('axes.pinValue', { value: value(wght.value) }) : t('axes.pinPlain'),
);

const canPin = computed(
  () =>
    faceId.value !== null && pinStyleName.value !== '' && holderKey.value !== '' && !pinning.value,
);

async function pin(): Promise<void> {
  const holder = holders.value.find((h) => h.key === holderKey.value);
  if (faceId.value === null || !holder || !canPin.value) return;
  pinning.value = true;
  try {
    const style = await pinned.pin({
      face: faceId.value,
      name: pinStyleName.value,
      coordinates: axesSet.coordinates(faceId.value, axes.value),
      collection: holder.collection,
      set: holder.set,
    });
    pinError.value = null;
    toast.show(t('axes.pinned', { name: style.name, holder: holder.label }));
    if (holder.collection !== null) void collections.refresh();
    await page.readPins();
  } catch (e) {
    pinError.value = (e as ApiError).message ? errorMessage(e) : t('axes.pinFailed');
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
    <p v-if="!d" class="placeholder">{{ t('axes.noFace') }}</p>
    <p v-else-if="!axes.length" class="placeholder">{{ t('axes.none') }}</p>
    <template v-else>
      <ul class="plain list" :aria-label="t('axes.list')">
        <li v-for="a in axes" :key="a.tag" class="axis" :data-tag="a.tag">
          <label class="name" :for="`${uid}-${a.tag}-range`">
            {{ a.name }} <span class="tag">{{ a.tag }}</span>
          </label>
          <div class="controls">
            <div class="track">
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
              <div v-if="a.tag === 'wght' && marks.length" class="ticks" aria-hidden="true">
                <span
                  v-for="m in marks"
                  :key="m.name"
                  class="tick"
                  :data-name="m.name"
                  :style="{ '--at': markAt(a, m.value) }"
                />
              </div>
              <datalist :id="`${uid}-${a.tag}-default`">
                <option :value="a.default ?? undefined" :label="t('axes.default')" />
              </datalist>
            </div>
            <input
              type="text"
              inputmode="decimal"
              class="number"
              :value="value(a)"
              :aria-label="valueLabel(a)"
              :aria-invalid="invalid.has(a.tag)"
              :aria-describedby="`${uid}-${a.tag}-range-text`"
              @change="onType(a, $event)"
            />
            <span v-if="a.tag === 'wght' && instanceLabel" class="at" data-testid="axis-instance">
              {{ instanceLabel }}
            </span>
          </div>
          <p :id="`${uid}-${a.tag}-range-text`" class="range">
            {{ t('axes.range', { min: a.min ?? '', max: a.max ?? '' }) }}
            <span class="default">{{ a.default }}</span>
            <span v-if="invalid.has(a.tag)" class="error" role="alert">
              — {{ t('axes.typeNumber') }}</span
            >
          </p>
        </li>
      </ul>
      <UiSegmentedControl
        v-if="d.instances.length"
        class="instances"
        wrap
        :label="t('axes.instances')"
        :model-value="atInstance"
        :options="d.instances.map((i) => ({ value: i.name, label: i.name }))"
        @update:model-value="chooseByName"
      />
      <div class="actions">
        <p v-if="wght" class="hint">{{ t('axes.scrubHint') }}</p>
        <div class="pin">
          <p v-if="!holders.length" class="hint">{{ t('axes.noHolder') }}</p>
          <template v-else>
            <label :for="`${uid}-pin-name`">{{ t('axes.styleName') }}</label>
            <input
              :id="`${uid}-pin-name`"
              v-model="pinName"
              type="text"
              class="field"
              :placeholder="pinFallback || t('axes.styleNamePlaceholder')"
            />
            <label :for="`${uid}-pin-holder`">{{ t('axes.saveIn') }}</label>
            <UiSelect :id="`${uid}-pin-holder`" v-model="holderKey" :options="holderOptions" />
            <UiButton class="pin-button" :disabled="!canPin" @click="pin">
              {{ pinLabel }}
            </UiButton>
          </template>
        </div>
        <CopyCss
          v-if="faceId !== null"
          :label="t('axes.cssLabel')"
          :css="() => axesSet.css(faceId!)"
        />
        <UiButton
          class="reset"
          :disabled="faceId === null || !axesSet.isChanged(faceId)"
          @click="reset"
        >
          {{ t('common.reset') }}
        </UiButton>
      </div>
      <p v-if="pinError" class="error pin-error" role="alert">{{ pinError }}</p>
    </template>
  </div>
</template>

<style scoped>
.placeholder,
.hint {
  margin: 0;
  color: var(--fm-muted);
}
.plain {
  margin: 0;
  padding: 0;
  list-style: none;
}
.list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-2) var(--fm-space-5);
}
.axis {
  flex: 1 1 320px;
  min-width: 0;
  max-width: 560px;
}
.name {
  display: block;
  overflow-wrap: anywhere;
}
.tag {
  margin-inline-start: 4px;
  color: var(--fm-muted);
  font-family: var(--fm-font-mono);
}
.controls {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}
.track {
  position: relative;
  flex: 1;
  min-width: 0;
}
.slider {
  display: block;
  width: 100%;
}
/* A tick sits under the thumb's centre, which runs from 7 px in to 7 px short of the end. */
.ticks {
  position: relative;
  height: 6px;
}
.tick {
  position: absolute;
  inset-inline-start: calc(7px + var(--at) * (100% - 14px));
  width: 1px;
  height: 6px;
  background-color: var(--fm-border-strong);
}
.at {
  flex: none;
  min-width: 9em;
  color: var(--fm-fg);
  line-height: 24px;
  font-variant-numeric: tabular-nums;
}
.number,
.field {
  box-sizing: border-box;
  padding: 2px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
.number {
  flex: none;
  width: 72px;
  font-variant-numeric: tabular-nums;
}
.number[aria-invalid='true'] {
  border-color: var(--fm-warning);
}
.range {
  margin: 2px 0 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.default {
  font-weight: 600;
}
.error {
  color: var(--fm-warning);
}
.instances {
  margin-block-start: var(--fm-space-2);
}
.actions,
.pin {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2);
}
.actions {
  justify-content: flex-end;
  margin-block-start: var(--fm-space-2);
}
.field {
  width: 160px;
}
.pin-error {
  margin: var(--fm-space-1) 0 0;
  text-align: end;
}
</style>
