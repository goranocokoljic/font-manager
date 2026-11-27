<script setup lang="ts">
// Settings › Advanced (board 15, E57.5): under "Performance" the preview cache and the rescan
// interval, number fields with their unit after them; under "Troubleshooting" the logs folder
// (its path in the mono face, "Copy path") and running setup again. There is no "Open" for the
// logs folder: fm-api has no request that opens a folder in the file manager.
import { computed, ref } from 'vue';
import type { FullSettings } from '../../api/client';
import { t } from '../../i18n';
import { useSettingsStore } from '../../stores/settings';
import { useSetupStore } from '../../stores/setup';
import CopyValue from '../CopyValue.vue';
import UiButton from '../ui/UiButton.vue';
import UiNumberField from '../ui/UiNumberField.vue';
import UiSettingGroup from '../ui/UiSettingGroup.vue';
import UiSettingRow from '../ui/UiSettingRow.vue';
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
      <UiSettingGroup :heading="t('settings.advanced.performance')">
        <!-- The field's name says its unit too ("Preview cache size (MB)"); the unit after it
             is for the eye. -->
        <UiSettingRow
          v-slot="{ control }"
          :title="t('settings.advanced.cacheSizeTitle')"
          :line="t('settings.advanced.cacheSizeLine')"
        >
          <span class="number">
            <UiNumberField
              :key="`cache-${numberReset.cacheLimitMb}`"
              :model-value="s.cacheLimitMb"
              :min="0"
              :aria-label="t('settings.advanced.cacheSize')"
              :aria-describedby="control['aria-describedby']"
              @commit="saveCacheLimit"
            />
          </span>
          <span class="unit" aria-hidden="true">{{ t('settings.advanced.unitMb') }}</span>
        </UiSettingRow>
        <UiSettingRow
          v-slot="{ control }"
          :title="t('settings.advanced.rescanTitle')"
          :line="t('settings.advanced.rescanHint')"
        >
          <span class="number">
            <UiNumberField
              :key="`rescan-${numberReset.rescanIntervalMinutes}`"
              :model-value="s.rescanIntervalMinutes"
              :min="0"
              :aria-label="t('settings.advanced.rescanInterval')"
              :aria-describedby="control['aria-describedby']"
              @commit="saveRescanInterval"
            />
          </span>
          <span class="unit" aria-hidden="true">{{ t('settings.advanced.unitMinutes') }}</span>
        </UiSettingRow>
      </UiSettingGroup>
      <UiSettingGroup :heading="t('settings.advanced.troubleshooting')">
        <UiSettingRow
          v-if="store.folders"
          :title="t('settings.advanced.logsFolder')"
          :value="store.folders.logs"
          data-testid="settings-logs-folder"
        >
          <CopyValue
            :value="store.folders.logs"
            :label="t('settings.advanced.logsFolderLabel')"
            :text="t('settings.advanced.copyPath')"
          />
        </UiSettingRow>
        <UiSettingRow
          v-slot="{ control }"
          :title="t('settings.advanced.setup')"
          :line="t('settings.advanced.runSetupHint')"
        >
          <UiButton
            :aria-describedby="control['aria-describedby']"
            data-testid="settings-run-setup"
            @click="setup.runAgain()"
          >
            {{ t('settings.advanced.runSetup') }}
          </UiButton>
        </UiSettingRow>
      </UiSettingGroup>
    </template>
  </SettingsSection>
</template>

<style scoped>
.number {
  width: 120px;
}
/* The unit after the field, in a column wide enough for "minutes", so the fields line up. */
.unit {
  min-width: 4.5em;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
</style>
