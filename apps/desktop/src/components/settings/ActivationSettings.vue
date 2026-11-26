<script setup lang="ts">
// Settings › Activation: how the Activate button activates, what happens on a name clash,
// and what the app does while it runs. Moved out of SettingsView in E57.3 as it was;
// E57.4 draws it with choice cards and setting rows (board 15).
import { computed } from 'vue';
import type { FullSettings } from '../../api/client';
import type { SettingsUpdate } from '../../api/types';
import { errorMessage, t } from '../../i18n';
import { useSettingsStore } from '../../stores/settings';
import { useToastStore } from '../../stores/toast';
import SettingsSection from './SettingsSection.vue';

const store = useSettingsStore();
const toast = useToastStore();
const s = computed(() => store.settings);

/** Radio choices; each change is saved at once. */
function choose(patch: SettingsUpdate): void {
  void store.update(patch);
}

/**
 * "Start at login": the OS may refuse the login entry; then a toast says so and
 * the switch shows the state fm-api reports again.
 */
async function saveStartAtLogin(e: Event): Promise<void> {
  const input = e.target as HTMLInputElement;
  if (await store.update({ startAtLogin: input.checked })) return;
  toast.show(store.error ? errorMessage(store.error) : t('settings.activation.startAtLoginFailed'));
  input.checked = store.settings?.startAtLogin ?? false;
}

// Kept here: the catalog check (src/i18n/templates.test.ts) reads a camelCase literal in a
// template as text.
const KEEP_EXISTING = 'keepExisting' satisfies FullSettings['onConflict'];
</script>

<template>
  <SettingsSection
    id="activation"
    :heading="t('settings.sections.activation')"
    :lead="t('settings.lead.activation')"
  >
    <template v-if="s">
      <fieldset class="group">
        <legend>{{ t('settings.activation.modeLegend') }}</legend>
        <label class="choice">
          <input
            type="radio"
            name="activation-mode"
            :checked="s.activationMode === 'session'"
            @change="choose({ activationMode: 'session' })"
          />
          {{ t('settings.activation.modeSession') }}
        </label>
        <label class="choice">
          <input
            type="radio"
            name="activation-mode"
            :checked="s.activationMode === 'permanent'"
            @change="choose({ activationMode: 'permanent' })"
          />
          {{ t('settings.activation.modePermanent') }}
        </label>
      </fieldset>
      <fieldset class="group">
        <legend>{{ t('settings.activation.conflictLegend') }}</legend>
        <label class="choice">
          <input
            type="radio"
            name="on-conflict"
            :checked="s.onConflict === KEEP_EXISTING"
            @change="choose({ onConflict: KEEP_EXISTING })"
          />
          {{ t('settings.activation.conflictAsk') }}
        </label>
        <label class="choice">
          <input
            type="radio"
            name="on-conflict"
            :checked="s.onConflict === 'replace'"
            @change="choose({ onConflict: 'replace' })"
          />
          {{ t('settings.activation.conflictReplace') }}
        </label>
      </fieldset>
      <label class="choice">
        <input
          type="checkbox"
          name="reactivate-on-launch"
          :checked="s.reactivateOnLaunch"
          @change="choose({ reactivateOnLaunch: ($event.target as HTMLInputElement).checked })"
        />
        {{ t('settings.activation.reactivateOnLaunch') }}
      </label>
      <label class="choice">
        <input
          type="checkbox"
          name="keep-in-tray"
          aria-describedby="keep-in-tray-hint"
          :checked="s.keepInTray"
          @change="choose({ keepInTray: ($event.target as HTMLInputElement).checked })"
        />
        {{ t('settings.activation.keepInTray') }}
      </label>
      <p id="keep-in-tray-hint" class="note hint">
        {{ t('settings.activation.keepInTrayHint') }}
      </p>
      <!-- fm-api reports null where the system does not offer it (ADR 0011). -->
      <template v-if="s.startAtLogin != null">
        <label class="choice start-at-login">
          <input
            type="checkbox"
            name="start-at-login"
            aria-describedby="start-at-login-hint"
            :checked="s.startAtLogin"
            @change="saveStartAtLogin"
          />
          {{ t('settings.activation.startAtLogin') }}
        </label>
        <p id="start-at-login-hint" class="note hint">
          {{ t('settings.activation.startAtLoginHint') }}
        </p>
      </template>
    </template>
  </SettingsSection>
</template>

<style scoped>
.note {
  margin: 0 0 12px;
  color: var(--fm-muted);
}
.hint {
  margin-block: 4px 0;
  margin-inline: 22px 0;
}
.start-at-login {
  margin-top: 12px;
}
.choice {
  display: flex;
  align-items: center;
  gap: 6px;
}
.group {
  display: flex;
  flex-direction: column;
  gap: var(--fm-gap);
  margin: 0 0 16px;
  padding: 0;
  border: 0;
}
.group legend {
  margin-bottom: 6px;
  padding: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  font-weight: 600;
}
</style>
