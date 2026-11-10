<script setup lang="ts">
// The compare icon (E48.4, board 7): one glyph with two states, on every place that adds a
// font to Compare (wall cards, list and table rows, the family page, the sheet's faces, the
// suggestions). Outline: not in the set, a click adds it. Filled in the accent: in the set,
// a click removes it. The accessible name says which ("Add … to Compare" / "Remove … from
// Compare"); there is no aria-pressed, as the name already changes with the state.
//
// A 28 px target with the accent focus ring. `scope="family"` stands for the face's whole
// family: a family's card or row is in the set while any of its faces is. By default a click
// goes to the compare store's `toggle`; with `manual` it only emits `toggle`, for a parent
// that must act around it (the sheet keeps the focus on the next face).
import { computed } from 'vue';
import type { FaceSummary } from '../api/types';
import { t } from '../i18n';
import { fontNameSample } from '../preview/sampleText';
import { useCompareStore, type CompareScope } from '../stores/compare';
import UiIcon from './ui/UiIcon.vue';

const props = withDefaults(
  defineProps<{ face: FaceSummary; scope?: CompareScope; manual?: boolean; shortcut?: string }>(),
  { scope: 'face', manual: false, shortcut: undefined },
);
const emit = defineEmits<{ toggle: [inSet: boolean] }>();

const compare = useCompareStore();
const inSet = computed(() => compare.inSet(props.face, props.scope));
const label = computed(() => {
  const name = props.scope === 'family' ? props.face.familyName : fontNameSample(props.face);
  return t(inSet.value ? 'compare.removeLabel' : 'compare.addLabel', { name });
});

function onClick(): void {
  const was = inSet.value;
  if (!props.manual) compare.toggle(props.face, props.scope);
  emit('toggle', was);
}
</script>

<template>
  <button
    type="button"
    class="compare-toggle"
    :class="{ 'in-set': inSet }"
    :data-in-set="inSet"
    :aria-label="label"
    :title="shortcut ? `${label} (${shortcut})` : label"
    @click.stop="onClick"
  >
    <UiIcon :name="inSet ? 'compareFilled' : 'compare'" />
  </button>
</template>

<style scoped>
.compare-toggle {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  inline-size: 28px;
  block-size: 28px;
  padding: 0;
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-muted);
}
.compare-toggle:not(:disabled):hover {
  color: var(--fm-fg);
}
/* In the set: the shapes fill in the accent. A fill and a stroke, never a text color: the
   accent is not a text color (styles/contrast.test.ts). */
.compare-toggle.in-set :deep(rect) {
  fill: var(--fm-accent);
  stroke: var(--fm-accent);
}
.compare-toggle:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
