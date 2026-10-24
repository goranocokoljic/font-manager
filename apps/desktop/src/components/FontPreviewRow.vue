<script setup lang="ts">
// One preview row: the face's name in the UI font, and the sample text in the
// face itself, rendered by the webview from the face's file (an `@font-face`
// registered by the shared face loader). Variable faces render at `variations`
// (a virtual style's coordinates) when given, else at the coordinates set in the
// inspector's Axes tab (the axes store), else at their defaults, through
// `font-variation-settings`; moving an axis restyles the row, the font stays loaded.
// Features switched in the inspector's Features tab apply through
// `font-feature-settings`. A color palette chosen in the inspector for the face's
// family in `view` applies through `font-palette` (E16.3).
// The list passes the view's preview settings (size, spacing, alignment, color).
// When the face lacks characters of the sample, the list passes the fallback text
// the API chose and `fallback`; the row then shows a small "Fallback" hint.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { Axis, AxisValue, FaceId, FaceSummary, SampleFallback } from '../api/types';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import { useAxesStore } from '../stores/axes';
import { useFeaturesStore } from '../stores/features';
import { usePalettesStore } from '../stores/palettes';
import { ALL_FONTS_VIEW } from '../stores/preview';
import { faceAxes, variationSettings } from '../preview/variations';
import { t } from '../i18n';

const props = withDefaults(
  defineProps<{
    face: FaceSummary;
    /** Name shown above the sample; "Family Style" when absent. */
    label?: string;
    text?: string;
    sizePx?: number;
    lineHeight?: number;
    /** Letter spacing in em. */
    trackingEm?: number;
    align?: 'start' | 'center' | 'end';
    /** Sample text color; inherits the list's when absent. */
    color?: string | null;
    variations?: AxisValue[];
    /** Set when `text` is a fallback from the face's own coverage, not the sample. */
    fallback?: SampleFallback | null;
    /** Characters of the sample the face lacks. */
    missing?: string[];
    /** The view whose chosen color palettes apply. */
    view?: string;
  }>(),
  {
    label: undefined,
    text: 'The quick brown fox jumps over the lazy dog',
    sizePx: 28,
    lineHeight: 1.2,
    trackingEm: 0,
    align: 'start',
    color: null,
    variations: () => [],
    fallback: null,
    missing: () => [],
    view: ALL_FONTS_VIEW,
  },
);

type LoadState = 'loading' | 'ready' | 'failed';

const loader = useFaceLoader();
const features = useFeaturesStore();
const axesSet = useAxesStore();
const palettes = usePalettesStore();
const state = ref<LoadState>('loading');
const axes = ref<Axis[]>([]);
let held: FaceId | null = null;

function hold(face: FaceId): void {
  if (held === face) return;
  if (held !== null) loader.release(held);
  held = face;
  state.value = 'loading';
  loader.acquire(face).then(
    () => {
      if (held === face) state.value = 'ready';
    },
    () => {
      if (held === face) state.value = 'failed';
    },
  );
}

watch(
  () => props.face.id,
  (id) => hold(id),
  { immediate: true },
);

watch(
  () => [props.face.id, props.face.isVariable] as const,
  ([id, isVariable]) => {
    axes.value = [];
    if (!isVariable) return;
    faceAxes(id).then(
      (list) => {
        if (props.face.id === id) axes.value = list;
      },
      () => undefined,
    );
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  if (held !== null) loader.release(held);
  held = null;
});

const sampleStyle = computed(() => ({
  fontFamily: `"${faceFamily(props.face.id)}", var(--fm-preview-fallback, system-ui), sans-serif`,
  fontSize: `${props.sizePx}px`,
  lineHeight: String(props.lineHeight),
  letterSpacing: `${props.trackingEm}em`,
  textAlign: props.align,
  color: props.color ?? undefined,
  fontVariationSettings: variationSettings(
    axes.value,
    props.variations.length ? props.variations : (axesSet.values(props.face.id) ?? []),
  ),
  fontFeatureSettings: features.settings(props.face.id),
  fontPalette: palettes.fontPaletteOf(props.view, props.face),
}));

const fallbackTitle = computed(() => {
  if (!props.fallback) return undefined;
  const letters = props.fallback.kind === 'script';
  if (!props.missing.length)
    return t(letters ? 'list.fallbackTitle.letters' : 'list.fallbackTitle.characters');
  const missing = `${props.missing.slice(0, 8).join(' ')}${props.missing.length > 8 ? ' …' : ''}`;
  return t(letters ? 'list.fallbackTitle.missingLetters' : 'list.fallbackTitle.missingCharacters', {
    missing,
  });
});

const name = computed(() => props.label ?? `${props.face.familyName} ${props.face.style}`);
</script>

<template>
  <div class="row" :data-face="face.id" :data-state="state">
    <span class="label">
      <slot name="lead" />
      <bdi>{{ name }}</bdi>
      <slot name="detail" />
      <span v-if="face.isVariable" class="badge">{{ t('list.variable') }}</span>
      <span
        v-if="fallback"
        class="fallback"
        data-testid="fallback-hint"
        :title="fallbackTitle"
        :aria-label="fallbackTitle"
        >{{ t('list.fallback') }}</span
      >
      <span v-if="state === 'failed'" class="note">{{ t('list.previewUnavailable') }}</span>
    </span>
    <span class="sample" dir="auto" :style="sampleStyle" :data-fallback="fallback?.kind">{{
      text
    }}</span>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 2px;
  box-sizing: border-box;
  height: 100%;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
  overflow: hidden;
}
.label {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.badge {
  color: var(--fm-accent);
}
.note {
  color: var(--fm-warning);
}
.fallback {
  padding: 0 4px;
  border: 1px solid var(--fm-border);
  border-radius: 3px;
  color: var(--fm-muted);
  font-size: 10.5px;
  line-height: 14px;
}
.sample {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
