<script setup lang="ts">
// The compare icon (E48.4, board 7): one glyph with two states, on every place that adds a
// font to Compare (wall cards, list and table rows, the family page, the sheet's faces, the
// suggestions). Outline: not in the set, a click adds it. Filled in the accent: in the set,
// a click removes it. The accessible name says which ("Add … to Compare" / "Remove … from
// Compare"); there is no aria-pressed, as the name already changes with the state.
//
// The 28 px UiIconButton (E50.6) with the accent focus ring. `scope="family"` stands for the face's whole
// family: a family's card or row is in the set while any of its faces is. By default a click
// goes to the compare store's `toggle`; with `manual` it only emits `toggle`, for a parent
// that must act around it (the sheet keeps the focus on the next face). With `fresh` a click
// outside the set starts a new comparison with the face as its base (the family page's
// Compare, E48.5) instead of adding to the set. `disabledReason` disables the icon and puts the
// reason in its title (a full set, E48.3).
import { computed } from 'vue';
import type { FaceSummary } from '../api/types';
import { t } from '../i18n';
import { fontNameSample } from '../preview/sampleText';
import { useCompareStore, type CompareScope } from '../stores/compare';
import UiIconButton from './ui/UiIconButton.vue';

const props = withDefaults(
  defineProps<{
    face: FaceSummary;
    scope?: CompareScope;
    manual?: boolean;
    fresh?: boolean;
    shortcut?: string;
    disabledReason?: string;
  }>(),
  { scope: 'face', manual: false, fresh: false, shortcut: undefined, disabledReason: undefined },
);
const emit = defineEmits<{ toggle: [inSet: boolean] }>();

const compare = useCompareStore();
const inSet = computed(() => compare.inSet(props.face, props.scope));
const label = computed(() => {
  const name = props.scope === 'family' ? props.face.familyName : fontNameSample(props.face);
  return t(inSet.value ? 'compare.removeLabel' : 'compare.addLabel', { name });
});

const title = computed(
  () =>
    props.disabledReason ?? (props.shortcut ? `${label.value} (${props.shortcut})` : label.value),
);

function onClick(): void {
  if (props.disabledReason) return;
  const was = inSet.value;
  if (props.manual) {
    // The parent acts.
  } else if (props.fresh && !was) compare.compareFresh(props.face);
  else compare.toggle(props.face, props.scope);
  emit('toggle', was);
}
</script>

<template>
  <UiIconButton
    class="compare-toggle"
    :class="{ 'in-set': inSet }"
    :data-in-set="inSet"
    :icon="inSet ? 'compareFilled' : 'compare'"
    :label="label"
    :title="title"
    :disabled="disabledReason ? true : undefined"
    @click.stop="onClick"
  />
</template>

<style scoped>
/* The box, the glyph and the hover come from UiIconButton; a square 28 px, as before. */
.compare-toggle {
  inline-size: 28px;
  padding: 0;
}
/* In the set: the shapes fill in the accent. A fill and a stroke, never a text color: the
   accent is not a text color (styles/contrast.test.ts). */
.compare-toggle.in-set :deep(rect) {
  fill: var(--fm-accent);
  stroke: var(--fm-accent);
}
.compare-toggle:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.compare-toggle:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
