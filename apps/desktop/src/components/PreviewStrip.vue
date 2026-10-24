<script setup lang="ts">
// The preview strip above the detail list: the family (or style) under the
// cursor in the view's sample text, at the toolbar size until the strip's own
// size slider moves; "Use toolbar size" follows the toolbar again. A face that
// lacks characters of the sample shows the API's fallback text, like the list.
import { computed, useId, watchEffect } from 'vue';
import { t } from '../i18n';
import { sampleText } from '../preview/sampleText';
import { useFontListStore } from '../stores/fontList';
import { SIZE, usePreviewStore, type PreviewSettings } from '../stores/preview';
import { useSamplesStore } from '../stores/samples';
import { useViewsStore } from '../stores/views';
import FontPreviewRow from './FontPreviewRow.vue';

const props = defineProps<{ view: string; preview: PreviewSettings }>();

const list = useFontListStore();
const views = useViewsStore();
const samples = useSamplesStore();
const previews = usePreviewStore();
const uid = useId();

const own = computed(() => views.tableFor(props.view).stripSizePx);
const sizePx = computed(() => own.value ?? props.preview.sizePx);
const face = computed(() => list.cursorFace);
const sample = computed(() =>
  face.value ? sampleText(props.preview, face.value, previews.pangramText) : '',
);
const answer = computed(() => (face.value ? samples.get(face.value.id, sample.value) : undefined));
watchEffect(() => {
  if (face.value) samples.want(face.value.id, sample.value);
});
const label = computed(() => {
  const row = list.cursorRow;
  if (!row) return '';
  return row.kind === 'face' ? `${row.family.name} ${row.face.style}` : row.family.name;
});

function onInput(e: Event): void {
  views.setStripSize(props.view, (e.target as HTMLInputElement).valueAsNumber);
}
</script>

<template>
  <section
    class="strip"
    :aria-label="t('preview.strip.region')"
    :style="{ color: preview.textColor ?? undefined }"
  >
    <div class="controls">
      <label :for="`${uid}-size`">{{ t('preview.strip.size') }}</label>
      <input
        :id="`${uid}-size`"
        type="range"
        :min="SIZE.min"
        :max="SIZE.max"
        :step="SIZE.step"
        :value="sizePx"
        :aria-valuetext="t('preview.strip.px', { size: sizePx })"
        @input="onInput"
      />
      <output :for="`${uid}-size`">{{ t('preview.strip.px', { size: sizePx }) }}</output>
      <button
        v-if="own !== null"
        type="button"
        class="reset"
        @click="views.setStripSize(view, null)"
      >
        {{ t('preview.strip.useToolbar') }}
      </button>
    </div>
    <FontPreviewRow
      v-if="face"
      class="sample"
      :face="face"
      :view="view"
      :label="label"
      :text="answer?.text ?? sample"
      :fallback="answer?.fallback ?? null"
      :missing="answer?.missing ?? []"
      :size-px="sizePx"
      :line-height="preview.lineHeight"
      :tracking-em="preview.trackingEm"
      :align="preview.align"
      :color="preview.textColor"
    />
    <p v-else class="hint">{{ t('preview.strip.empty') }}</p>
  </section>
</template>

<style scoped>
.strip {
  display: flex;
  flex: none;
  flex-direction: column;
  max-height: 45%;
  overflow: auto;
  border-bottom: 1px solid var(--fm-border);
}
.controls {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 16px 0;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.controls input {
  width: 160px;
}
.reset {
  padding: 2px 8px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.sample {
  border-bottom: 0;
}
.hint {
  margin: 0;
  padding: 12px 16px;
  color: var(--fm-muted);
}
</style>
