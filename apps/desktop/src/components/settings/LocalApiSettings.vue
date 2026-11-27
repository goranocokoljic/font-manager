<script setup lang="ts">
// Settings › Local API (E25.1, ADR 0015; board 15, E57.6): under "Connection" the endpoint's
// address (a dot before it while the API runs, "Copy") and its token, dots until Show, with
// Copy; "Regenerate the token" is a row of its own. While the token is hidden only the dots
// are rendered, never the token. "Copied" and "regenerated" are said in the section's status.
import { computed, onMounted, ref } from 'vue';
import { t } from '../../i18n';
import { useSettingsStore } from '../../stores/settings';
import CopyValue from '../CopyValue.vue';
import UiButton from '../ui/UiButton.vue';
import UiSettingGroup from '../ui/UiSettingGroup.vue';
import UiSettingRow from '../ui/UiSettingRow.vue';
import { useCopy } from '../useCopy';
import SettingsSection from './SettingsSection.vue';

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
const tokenText = computed(() => (shown.value ? (info.value?.token ?? '') : masked.value));

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
  <SettingsSection
    id="local-api"
    :heading="t('settings.sections.localApi')"
    :lead="t('settings.localApi.note')"
  >
    <template v-if="info">
      <UiSettingGroup :heading="t('settings.localApi.connection')">
        <UiSettingRow
          :title="t('settings.localApi.address')"
          :empty="info.url ? undefined : t('settings.localApi.notRunning')"
          data-testid="local-api-address"
        >
          <template v-if="info.url" #value>
            <span class="dot" aria-hidden="true" />{{ info.url }}
          </template>
          <CopyValue
            v-if="info.url"
            :value="info.url"
            :label="t('settings.localApi.addressLabel')"
            :text="t('settings.localApi.copy')"
          />
        </UiSettingRow>
        <UiSettingRow
          :title="t('settings.localApi.token')"
          :line="t('settings.localApi.tokenHint')"
          data-testid="local-api-token"
        >
          <template #value>
            <span id="local-api-token-value" class="token">{{ tokenText }}</span>
          </template>
          <template #default="{ control }">
            <UiButton
              aria-controls="local-api-token-value"
              :aria-pressed="shown"
              :aria-describedby="control['aria-describedby']"
              @click="shown = !shown"
            >
              {{ shown ? t('settings.localApi.hide') : t('settings.localApi.show') }}
            </UiButton>
            <UiButton :aria-label="t('settings.localApi.copyToken')" @click="copyToken">
              {{ t('settings.localApi.copy') }}
            </UiButton>
          </template>
        </UiSettingRow>
      </UiSettingGroup>
      <UiSettingGroup>
        <UiSettingRow
          v-slot="{ control }"
          :title="t('settings.localApi.regenerateTitle')"
          :line="t('settings.localApi.regenerateHint')"
        >
          <UiButton
            :aria-describedby="control['aria-describedby']"
            :disabled="busy"
            @click="regenerate"
          >
            {{ t('settings.localApi.regenerate') }}
          </UiButton>
        </UiSettingRow>
      </UiSettingGroup>
      <p role="status" class="status">
        <template v-if="copyStatus === 'copied'">{{ t('common.copied') }}</template>
        <template v-else-if="copyStatus === 'failed'">{{ t('copy.failed') }}</template>
        <template v-else-if="regenerated">{{ t('settings.localApi.regenerated') }}</template>
      </p>
    </template>
  </SettingsSection>
</template>

<style scoped>
/* The running dot before the address: the text colour, like the board's. */
.dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  margin-inline-end: var(--fm-space-2);
  border-radius: 50%;
  background-color: var(--fm-fg);
  vertical-align: middle;
}
.token {
  letter-spacing: 0.02em;
}
.status {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.status:empty {
  display: none;
}
</style>
