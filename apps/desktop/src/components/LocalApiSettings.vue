<script setup lang="ts">
// Settings → Local API (E25.1, ADR 0015): the endpoint's address and its token,
// masked until the user asks to see it, with Copy and Regenerate.
import { computed, onMounted, ref } from 'vue';
import { t } from '../i18n';
import { useSettingsStore } from '../stores/settings';
import CopyValue from './CopyValue.vue';
import { useCopy } from './useCopy';

const store = useSettingsStore();
const shown = ref(false);
const regenerated = ref(false);
const busy = ref(false);
const { status: copyStatus, copy } = useCopy();

onMounted(() => {
  void store.loadEndpoint();
});

const info = computed(() => store.endpoint);
/** Dots in place of the token until Show is pressed; the same length as the token. */
const masked = computed(() => '•'.repeat(info.value?.token.length ?? 0));

function copyToken(): void {
  if (info.value) void copy(info.value.token);
}

async function regenerate(): Promise<void> {
  busy.value = true;
  regenerated.value = false;
  regenerated.value = await store.regenerateEndpointToken();
  busy.value = false;
}
</script>

<template>
  <section class="local-api" aria-labelledby="local-api-title">
    <h2 id="local-api-title" class="heading">{{ t('settings.sections.localApi') }}</h2>
    <p class="note">{{ t('settings.localApi.note') }}</p>
    <template v-if="info">
      <div class="field">
        <span>{{ t('settings.localApi.address') }}</span>
        <span v-if="info.url">
          <CopyValue :value="info.url" :label="t('settings.localApi.addressLabel')" />
        </span>
        <span v-else class="note">{{ t('settings.localApi.notRunning') }}</span>
      </div>
      <div class="field">
        <label for="local-api-token">{{ t('settings.localApi.token') }}</label>
        <div class="row">
          <input
            id="local-api-token"
            class="input token"
            readonly
            spellcheck="false"
            autocomplete="off"
            aria-describedby="local-api-token-hint"
            :value="shown ? info.token : masked"
          />
          <button
            type="button"
            class="button"
            aria-controls="local-api-token"
            :aria-pressed="shown"
            @click="shown = !shown"
          >
            {{ shown ? t('settings.localApi.hide') : t('settings.localApi.show') }}
          </button>
          <button
            type="button"
            class="button"
            :aria-label="t('settings.localApi.copyToken')"
            @click="copyToken"
          >
            {{ t('settings.localApi.copy') }}
          </button>
          <button
            type="button"
            class="button"
            aria-describedby="local-api-regenerate-hint"
            :disabled="busy"
            @click="regenerate"
          >
            {{ t('settings.localApi.regenerate') }}
          </button>
        </div>
        <p id="local-api-token-hint" class="note hint">{{ t('settings.localApi.tokenHint') }}</p>
        <p id="local-api-regenerate-hint" class="note hint">
          {{ t('settings.localApi.regenerateHint') }}
        </p>
        <p role="status" class="note status">
          <template v-if="copyStatus === 'copied'">{{ t('common.copied') }}</template>
          <template v-else-if="copyStatus === 'failed'">{{ t('copy.failed') }}</template>
          <template v-else-if="regenerated">{{ t('settings.localApi.regenerated') }}</template>
        </p>
      </div>
    </template>
  </section>
</template>

<style scoped>
.heading {
  margin: 0 0 12px;
  font-size: 15px;
}
.note {
  margin: 0 0 12px;
  color: var(--fm-muted);
}
.hint {
  margin: 4px 0 0;
}
.status {
  margin-top: 8px;
}
.status:empty {
  display: none;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0 0 12px;
}
.field > span:first-child,
.field > label {
  font-weight: 600;
}
.row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}
.input {
  padding: 4px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
}
.token {
  flex: 1;
  min-width: 16em;
  font-family: ui-monospace, monospace;
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button:disabled {
  opacity: 0.5;
}
</style>
