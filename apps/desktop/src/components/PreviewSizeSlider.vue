<script setup lang="ts">
// The size slider between a small and a large A (board `1-wall-word.png`). Every change
// goes to the preview store for `view`; the wall scales it by its preset (`list/grid.ts`).
// Where the size has no effect (`preview/applicability.ts`, E51.4) the slider stays in its
// place but is off: dimmed, `aria-disabled`, deaf to the pointer and to keys that would move
// it, still on the Tab path so its tooltip, which says why, can be read. The stored size is
// kept and applies again when the slider does.
import { computed, useId } from 'vue';
import { t } from '../i18n';
import { applicability, previewPreset, type PreviewSurface } from '../preview/applicability';
import { SIZE, usePreviewStore } from '../stores/preview';

const props = withDefaults(
  defineProps<{
    view: string;
    /** The slider's range, when the view mode has its own (the wall's 24–120). */
    sizeRange?: { readonly min: number; readonly max: number; readonly step: number };
    /** The view mode the slider sizes, which decides whether it has an effect. */
    mode?: PreviewSurface;
  }>(),
  { sizeRange: undefined, mode: 'list' },
);

const store = usePreviewStore();
const uid = useId();
const range = computed(() => props.sizeRange ?? SIZE);
const settings = computed(() => store.settingsFor(props.view));
/** A saved size outside the view mode's range shows at its end. */
const shown = computed(() =>
  Math.min(range.value.max, Math.max(range.value.min, settings.value.sizePx)),
);
const state = computed(() => applicability(props.mode, previewPreset(settings.value)).size);
/** The tooltip of a slider that is off: why, and what to do. */
const reason = computed(() => (state.value.applies ? undefined : t(state.value.reason)));

function slid(e: Event): void {
  const input = e.target as HTMLInputElement;
  // Off, nothing moves: the stored size stays as it is.
  if (!state.value.applies) {
    input.valueAsNumber = shown.value;
    return;
  }
  store.update(props.view, { sizePx: input.valueAsNumber });
}

/** A press on a slider that is off neither moves it nor focuses it. */
function onPointer(e: MouseEvent): void {
  if (!state.value.applies) e.preventDefault();
}

/** Tab and Shift+Tab pass; the keys that would move a slider that is off do nothing. */
function onKey(e: KeyboardEvent): void {
  if (!state.value.applies && e.key !== 'Tab') e.preventDefault();
}
</script>

<template>
  <div
    class="size"
    :class="{ off: !state.applies }"
    :title="reason"
    data-testid="size-slider"
    @pointerdown="onPointer"
    @mousedown="onPointer"
  >
    <span class="small" aria-hidden="true">{{ t('preview.sizeGlyph') }}</span>
    <input
      type="range"
      :aria-label="t('preview.sizeSlider')"
      :aria-disabled="state.applies ? undefined : 'true'"
      :aria-describedby="reason ? `${uid}-why` : undefined"
      :title="reason"
      :min="range.min"
      :max="range.max"
      :step="range.step"
      :value="shown"
      @input="slid"
      @keydown="onKey"
    />
    <span class="large" aria-hidden="true">{{ t('preview.sizeGlyph') }}</span>
    <span v-if="reason" :id="`${uid}-why`" class="sr-only">{{ reason }}</span>
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
/* Off (E51.4): the same place and size, at 40 %. */
.size.off {
  opacity: 0.4;
}
.size.off input {
  cursor: not-allowed;
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
