<script setup lang="ts">
// The Developer tab (E20.4): the font-variation-settings and
// font-feature-settings declarations for the selected face's current Axes and
// Features state, as fm-core writes them (PreviewService::css_values). It
// follows the sliders and switches live; "Copy" copies the whole snippet.
import { computed, ref, watch } from 'vue';
import { useClient } from '../api/client';
import { errorMessage, t } from '../i18n';
import { useAxesStore } from '../stores/axes';
import { useFeaturesStore } from '../stores/features';
import { useSelectionStore } from '../stores/selection';
import { useCopy } from './useCopy';

const selection = useSelectionStore();
const axesSet = useAxesStore();
const features = useFeaturesStore();
const { status, copy } = useCopy();

const faceId = computed(() => selection.detail?.face.id ?? null);
/** What fm-core is asked for: the face, its moved axes and its switched features. */
const request = computed(() =>
  faceId.value === null
    ? null
    : {
        face: faceId.value,
        variations: axesSet.values(faceId.value) ?? [],
        features: features.switches(faceId.value),
      },
);

const snippet = ref('');
const error = ref<string | null>(null);
// Only the answer to the latest request is shown; a slow earlier one is dropped.
let latest = 0;

watch(
  request,
  async (req) => {
    const ticket = ++latest;
    if (!req) {
      snippet.value = '';
      error.value = null;
      return;
    }
    try {
      const css = await useClient().cssValues(req);
      if (ticket !== latest) return;
      snippet.value = `${css.variationSettings}\n${css.featureSettings}`;
      error.value = null;
    } catch (e) {
      if (ticket !== latest) return;
      snippet.value = '';
      error.value = errorMessage(e);
    }
  },
  { immediate: true, deep: true },
);
</script>

<template>
  <div class="developer">
    <p v-if="faceId === null" class="placeholder">{{ t('developer.noFace') }}</p>
    <template v-else>
      <div class="head">
        <p class="hint">{{ t('developer.hint') }}</p>
        <span class="actions">
          <span class="status" role="status">
            <template v-if="status === 'copied'">{{ t('common.copied') }}</template>
            <template v-else-if="status === 'failed'">{{ t('copy.failed') }}</template>
          </span>
          <button
            type="button"
            class="button"
            :aria-label="t('developer.copyLabel')"
            :disabled="!snippet"
            @click="copy(snippet)"
          >
            {{ t('common.copy') }}
          </button>
        </span>
      </div>
      <p v-if="error" class="error" role="alert">{{ error }}</p>
      <pre v-else class="snippet" data-testid="css-snippet"><code>{{ snippet }}</code></pre>
    </template>
  </div>
</template>

<style scoped>
.developer {
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
.actions {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 8px;
}
.button {
  padding: 2px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button:disabled {
  border-color: var(--fm-border);
  color: var(--fm-muted);
}
.status {
  color: var(--fm-muted);
  font-size: 11.5px;
}
.snippet {
  margin: 0;
  padding: 8px;
  border: 1px solid var(--fm-border);
  border-radius: 4px;
  font-family: ui-monospace, monospace;
  font-size: 12px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  user-select: text;
}
.error {
  margin: 0;
  color: var(--fm-warning);
}
</style>
