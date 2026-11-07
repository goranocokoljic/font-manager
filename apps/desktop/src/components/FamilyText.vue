<script setup lang="ts">
// The Text section of the family page (E42.4): the same passage at three sizes, at the user's line
// length and leading, on paper or ink, with the body size on the opposite ground below. The columns
// share one width, the measure at the caption size, as on the board: at the default all three show
// side by side, and a longer measure scrolls the row sideways instead of leaving the slider dead.
import { computed, useId } from 'vue';
import type { FaceDetail } from '../api/types';
import { t } from '../i18n';
import {
  BODY_PX,
  LEADING,
  MEASURE,
  READING_SIZES,
  readingPassage,
  readingVariation,
} from '../preview/readingTest';
import { useAxesStore } from '../stores/axes';
import { useFamilyPageStore, type TextGround } from '../stores/familyPage';
import UiSegmentedControl from './ui/UiSegmentedControl.vue';

const props = defineProps<{
  /** The detail of the hero's face, once it has loaded. */
  detail: FaceDetail | null;
  /** The preview text, as the hero shows it. */
  text: string;
  /** The hero's font: family, features and palette. Undefined until the face has loaded. */
  faceStyle: Record<string, string | undefined> | undefined;
}>();

const page = useFamilyPageStore();
const axesSet = useAxesStore();
const uid = useId();

const passage = computed(() => readingPassage(props.text));
const moved = computed(() => (page.face && axesSet.values(page.face.id)) ?? []);

/** The font and line settings of the passage at `px`. Never a fallback font: see `.loading`. */
function textStyle(px: number) {
  return {
    ...props.faceStyle,
    fontSize: `${px}px`,
    lineHeight: String(page.textLeading),
    fontVariationSettings: readingVariation(props.detail?.axes ?? [], moved.value, px),
  };
}

/** Sets `ch` for the columns inside: the face at the caption size. */
const rowStyle = computed(() => ({ ...props.faceStyle, fontSize: `${READING_SIZES[0].px}px` }));
const columnWidth = computed(() => `${page.textMeasure}ch`);

const measureLabel = computed(() => t('family.textMeasureValue', { value: page.textMeasure }));
const leadingLabel = computed(() => page.textLeading.toFixed(2));
const grounds = computed<{ value: TextGround; label: string }[]>(() => [
  { value: 'paper', label: t('family.textPaper') },
  { value: 'ink', label: t('family.textInk') },
]);

const slid = (e: Event) => Number((e.target as HTMLInputElement).value);
</script>

<template>
  <div
    class="family-text"
    data-testid="family-text"
    :class="page.textGround === 'ink' ? 'surface-dark' : 'surface-light'"
  >
    <div class="controls">
      <p class="hint">{{ t('family.textHint') }}</p>
      <label :for="`${uid}-measure`">{{ t('family.textMeasure') }}</label>
      <input
        :id="`${uid}-measure`"
        type="range"
        :min="MEASURE.min"
        :max="MEASURE.max"
        :step="MEASURE.step"
        :value="page.textMeasure"
        :aria-valuetext="measureLabel"
        @input="page.textMeasure = slid($event)"
      />
      <output :for="`${uid}-measure`">{{ measureLabel }}</output>
      <label :for="`${uid}-leading`">{{ t('family.textLeading') }}</label>
      <input
        :id="`${uid}-leading`"
        type="range"
        :min="LEADING.min"
        :max="LEADING.max"
        :step="LEADING.step"
        :value="page.textLeading"
        :aria-valuetext="leadingLabel"
        @input="page.textLeading = slid($event)"
      />
      <output :for="`${uid}-leading`">{{ leadingLabel }}</output>
      <UiSegmentedControl
        v-model="page.textGround"
        class="ground"
        :options="grounds"
        :label="t('family.textGround')"
      />
    </div>
    <!-- Focusable so the keyboard can scroll it; FamilyView leaves the arrow keys to it. -->
    <div
      class="row"
      role="group"
      tabindex="0"
      data-keeps-arrows
      data-testid="family-text-row"
      :aria-label="t('family.textColumns')"
      :style="rowStyle"
    >
      <div
        v-for="size in READING_SIZES"
        :key="size.px"
        class="column"
        :style="{ width: columnWidth }"
      >
        <p class="label">
          <span>{{ t('family.textSize', { size: size.px }) }}</span>
          <span>{{ t(`family.textRoles.${size.role}`) }}</span>
        </p>
        <p
          class="passage"
          dir="auto"
          data-testid="family-text-passage"
          :data-px="size.px"
          :class="{ loading: !faceStyle }"
          :style="textStyle(size.px)"
        >
          {{ passage }}
        </p>
      </div>
    </div>
    <div class="ink" data-testid="family-text-ink">
      <p class="ink-label">{{ t('family.textInverted') }}</p>
      <p
        class="passage"
        dir="auto"
        data-testid="family-text-ink-passage"
        :class="{ loading: !faceStyle }"
        :style="[textStyle(BODY_PX), { width: columnWidth }]"
      >
        {{ passage }}
      </p>
    </div>
  </div>
</template>

<style scoped>
/* The sheet sticks out by its padding, so on a ground like the page's its text lines up with the
   switch above. */
.family-text {
  display: grid;
  gap: var(--fm-space-4);
  margin-inline: calc(var(--fm-space-3) * -1);
  padding: var(--fm-space-3);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  min-width: 0;
}
.controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2) var(--fm-space-3);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.hint {
  margin: 0;
}
.controls input {
  width: 120px;
}
.controls output {
  min-width: 4ch;
  color: var(--fm-fg);
  font-variant-numeric: tabular-nums;
}
.ground {
  margin-inline-start: auto;
}
.row {
  display: flex;
  gap: var(--fm-space-5);
  overflow-x: auto;
  min-width: 0;
  padding-block-end: var(--fm-space-2);
}
.column {
  flex: none;
}
/* The chrome's font: the row carries the face only to size the columns. */
.label {
  display: flex;
  justify-content: space-between;
  gap: var(--fm-space-3);
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-family: var(--fm-font-ui);
  font-size: var(--fm-text-sm);
  font-feature-settings: normal;
  font-variation-settings: normal;
  font-variant-numeric: tabular-nums;
}
.passage {
  margin: 0;
}
.ink {
  padding: var(--fm-space-4);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-fg);
  color: var(--fm-bg);
}
.ink-label {
  margin: 0 0 var(--fm-space-2);
  font-size: var(--fm-text-sm);
}
.ink .passage {
  max-width: 100%;
}
/* Never a fallback font: nothing shows until the face has loaded. */
.loading {
  visibility: hidden;
}
</style>
