<script setup lang="ts">
// Settings › Advanced: the preview cache, the rescan interval, the logs folder and running
// setup again. Moved out of SettingsView in E57.3 as it was; E57.5 draws it with setting
// rows (board 15).
import { computed, ref } from 'vue';
import type { FullSettings } from '../../api/client';
import { t } from '../../i18n';
import { useSettingsStore } from '../../stores/settings';
import { useSetupStore } from '../../stores/setup';
import CopyValue from '../CopyValue.vue';
import UiButton from '../ui/UiButton.vue';
import UiNumberField from '../ui/UiNumberField.vue';
import SettingsSection from './SettingsSection.vue';

const store = useSettingsStore();
const setup = useSetupStore();
const s = computed(() => store.settings);

type NumberKey = keyof Pick<FullSettings, 'cacheLimitMb' | 'rescanIntervalMinutes'>;

/**
 * A number field saves when the user commits it (Enter, leaving the field, − / +), not on
 * every key, so "1" on the way to "1024" is never sent. A refused or empty value re-keys the
 * field, which shows the saved number again.
 */
const numberReset = ref<Record<NumberKey, number>>({ cacheLimitMb: 0, rescanIntervalMinutes: 0 });
async function saveNumber(key: NumberKey, value: number | ''): Promise<void> {
  const valid = value !== '' && value >= 0 && Number.isInteger(value);
  const ok = valid && (await store.update({ [key]: value }));
  if (!ok) numberReset.value[key] += 1;
}

// Identifiers the template uses, kept here: the catalog check (src/i18n/templates.test.ts)
// reads a camelCase literal in a template as text.
const saveCacheLimit = (v: number | '') => saveNumber('cacheLimitMb', v);
const saveRescanInterval = (v: number | '') => saveNumber('rescanIntervalMinutes', v);
</script>

<template>
  <SettingsSection
    id="advanced"
    :heading="t('settings.sections.advanced')"
    :lead="t('settings.lead.advanced')"
  >
    <template v-if="s">
      <label class="field">
        <span>{{ t('settings.advanced.cacheSize') }}</span>
        <span class="number">
          <UiNumberField
            :key="`cache-${numberReset.cacheLimitMb}`"
            :model-value="s.cacheLimitMb"
            :min="0"
            @commit="saveCacheLimit"
          />
        </span>
      </label>
      <label class="field">
        <span>{{ t('settings.advanced.rescanInterval') }}</span>
        <span class="number">
          <UiNumberField
            :key="`rescan-${numberReset.rescanIntervalMinutes}`"
            :model-value="s.rescanIntervalMinutes"
            :min="0"
            aria-describedby="rescan-hint"
            @commit="saveRescanInterval"
          />
        </span>
      </label>
      <p id="rescan-hint" class="note">{{ t('settings.advanced.rescanHint') }}</p>
      <div v-if="store.folders" class="field">
        <span>{{ t('settings.advanced.logsFolder') }}</span>
        <span>
          <CopyValue :value="store.folders.logs" :label="t('settings.advanced.logsFolderLabel')" />
        </span>
      </div>
      <div class="field">
        <span>{{ t('settings.advanced.setup') }}</span>
        <span>
          <UiButton
            aria-describedby="run-setup-hint"
            data-testid="settings-run-setup"
            @click="setup.runAgain()"
          >
            {{ t('settings.advanced.runSetup') }}
          </UiButton>
        </span>
      </div>
      <p id="run-setup-hint" class="note">{{ t('settings.advanced.runSetupHint') }}</p>
    </template>
  </SettingsSection>
</template>

<style scoped>
.note {
  margin: 0 0 12px;
  color: var(--fm-muted);
}
.field > span:first-child {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  font-weight: 600;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0 0 12px;
}
.number {
  width: 120px;
}
</style>
