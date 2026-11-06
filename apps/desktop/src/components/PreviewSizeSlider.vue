<script setup lang="ts">
// The size slider between a small and a large A (board `1-wall-word.png`). Every change
// goes to the preview store for `view`; the wall scales it by its preset (`list/grid.ts`).
import { computed } from 'vue';
import { t } from '../i18n';
import { SIZE, usePreviewStore } from '../stores/preview';

const props = defineProps<{
  view: string;
  /** The slider's range, when the view mode has its own (the wall's 24–120). */
  sizeRange?: { readonly min: number; readonly max: number; readonly step: number };
}>();

const store = usePreviewStore();
const range = computed(() => props.sizeRange ?? SIZE);
/** A saved size outside the view mode's range shows at its end. */
const shown = computed(() =>
  Math.min(range.value.max, Math.max(range.value.min, store.settingsFor(props.view).sizePx)),
);

function slid(e: Event): void {
  store.update(props.view, { sizePx: (e.target as HTMLInputElement).valueAsNumber });
}
</script>

<template>
  <div class="size">
    <span class="small" aria-hidden="true">{{ t('preview.sizeGlyph') }}</span>
    <input
      type="range"
      :aria-label="t('preview.sizeSlider')"
      :min="range.min"
      :max="range.max"
      :step="range.step"
      :value="shown"
      @input="slid"
    />
    <span class="large" aria-hidden="true">{{ t('preview.sizeGlyph') }}</span>
  </div>
</template>

<style scoped>
.size {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--fm-space-2);
  color: var(--fm-muted);
}
.small {
  font-size: var(--fm-text-xs);
}
.large {
  font-size: var(--fm-text-lg);
}
input {
  width: 96px;
  accent-color: var(--fm-accent);
}
input:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
