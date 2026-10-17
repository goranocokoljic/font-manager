<script setup lang="ts">
// One preview row: the face's name in the UI font, and the sample text in the
// face itself, rendered by the webview from the face's file (an `@font-face`
// registered by the shared face loader). Variable faces render at their
// default coordinates, or at `variations`, through `font-variation-settings`.
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { Axis, AxisValue, FaceId, FaceSummary } from '../api/types';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import { faceAxes, variationSettings } from '../preview/variations';

const props = withDefaults(
  defineProps<{
    face: FaceSummary;
    text?: string;
    sizePx?: number;
    variations?: AxisValue[];
  }>(),
  {
    text: 'The quick brown fox jumps over the lazy dog',
    sizePx: 28,
    variations: () => [],
  },
);

type LoadState = 'loading' | 'ready' | 'failed';

const loader = useFaceLoader();
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
  fontVariationSettings: variationSettings(axes.value, props.variations),
}));

const label = computed(() => `${props.face.familyName} ${props.face.style}`);
</script>

<template>
  <div class="row" :data-face="face.id" :data-state="state">
    <span class="label">
      {{ label }}
      <span v-if="face.isVariable" class="badge">Variable</span>
      <span v-if="state === 'failed'" class="note">Preview not available</span>
    </span>
    <span class="sample" :style="sampleStyle">{{ text }}</span>
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
.sample {
  overflow: hidden;
  line-height: 1.2;
  white-space: nowrap;
  text-overflow: ellipsis;
}
</style>
