<script setup lang="ts">
// Settings › Activation (board 15): how the Activate button activates and what happens on a
// name clash, as choice cards; what the app does while it runs, as setting rows with switches.
// Every choice saves at once.
import { computed, ref } from 'vue';
import type { FullSettings } from '../../api/client';
import type { SettingsUpdate } from '../../api/types';
import { errorMessage, t } from '../../i18n';
import { useSettingsStore } from '../../stores/settings';
import UiChoiceCards, { type ChoiceCard } from '../ui/UiChoiceCards.vue';
import UiSettingGroup from '../ui/UiSettingGroup.vue';
import UiSettingRow from '../ui/UiSettingRow.vue';
import UiSwitch from '../ui/UiSwitch.vue';
import SettingsSection from './SettingsSection.vue';

type Mode = FullSettings['activationMode'];
type Conflict = FullSettings['onConflict'];

const store = useSettingsStore();
const s = computed(() => store.settings);

function choose(patch: SettingsUpdate): void {
  void store.update(patch);
}

const modes = computed<ChoiceCard<Mode>[]>(() => [
  {
    value: 'session',
    title: t('settings.activation.modeSession'),
    line: t('settings.activation.modeSessionLine'),
  },
  {
    value: 'permanent',
    title: t('settings.activation.modePermanent'),
    line: t('settings.activation.modePermanentLine'),
  },
]);

const conflicts = computed<ChoiceCard<Conflict>[]>(() => [
  {
    value: 'keepExisting',
    title: t('settings.activation.conflictAsk'),
    line: t('settings.activation.conflictAskLine'),
  },
  {
    value: 'replace',
    title: t('settings.activation.conflictReplace'),
    line: t('settings.activation.conflictReplaceLine'),
  },
]);

/** The message of a failed "Start at login" change, said on its row until the next change. */
const startAtLoginError = ref<string>();

/**
 * "Start at login": the OS may refuse the login entry; then the row says so (in place of the
 * toast it was before E57.4) and the switch shows the state fm-api reports again: it follows
 * the store, which changes only on fm-api's answer.
 */
async function toggleStartAtLogin(): Promise<void> {
  startAtLoginError.value = undefined;
  if (await store.update({ startAtLogin: !s.value?.startAtLogin })) return;
  startAtLoginError.value = store.error
    ? errorMessage(store.error)
    : t('settings.activation.startAtLoginFailed');
  // The row says it; the panel's alert above the section would say it a second time.
  store.error = null;
}
</script>

<template>
  <SettingsSection
    id="activation"
    :heading="t('settings.sections.activation')"
    :lead="t('settings.lead.activation')"
  >
    <template v-if="s">
      <section class="choices" aria-labelledby="activation-mode-heading">
        <h3 id="activation-mode-heading" class="heading">
          {{ t('settings.activation.modeLegend') }}
        </h3>
        <UiChoiceCards
          name="activation-mode"
          :label="t('settings.activation.modeLegend')"
          :model-value="s.activationMode"
          :options="modes"
          @update:model-value="choose({ activationMode: $event })"
        >
          <template #figure="{ option }">
            <!-- A bar that ends at the quit, a bar that goes on. -->
            <span class="bar" :class="option.value" data-testid="activation-figure" />
          </template>
        </UiChoiceCards>
      </section>
      <section class="choices" aria-labelledby="on-conflict-heading">
        <h3 id="on-conflict-heading" class="heading">
          {{ t('settings.activation.conflictLegend') }}
        </h3>
        <UiChoiceCards
          name="on-conflict"
          :label="t('settings.activation.conflictLegend')"
          :model-value="s.onConflict"
          :options="conflicts"
          @update:model-value="choose({ onConflict: $event })"
        />
      </section>
      <UiSettingGroup :heading="t('settings.activation.whileRunning')">
        <UiSettingRow
          v-slot="{ control }"
          :title="t('settings.activation.reactivateOnLaunch')"
          :line="t('settings.activation.reactivateOnLaunchLine')"
        >
          <UiSwitch
            v-bind="control"
            name="reactivate-on-launch"
            :checked="s.reactivateOnLaunch"
            @click="choose({ reactivateOnLaunch: !s.reactivateOnLaunch })"
          />
        </UiSettingRow>
        <UiSettingRow
          v-slot="{ control }"
          :title="t('settings.activation.keepInTray')"
          :line="t('settings.activation.keepInTrayHint')"
        >
          <UiSwitch
            v-bind="control"
            name="keep-in-tray"
            :checked="s.keepInTray"
            @click="choose({ keepInTray: !s.keepInTray })"
          />
        </UiSettingRow>
        <!-- fm-api reports null where the system does not offer it (ADR 0011). -->
        <UiSettingRow
          v-if="s.startAtLogin != null"
          v-slot="{ control }"
          :title="t('settings.activation.startAtLogin')"
          :line="t('settings.activation.startAtLoginHint')"
          :error="startAtLoginError"
        >
          <UiSwitch
            v-bind="control"
            name="start-at-login"
            :checked="s.startAtLogin"
            @click="toggleStartAtLogin"
          />
        </UiSettingRow>
      </UiSettingGroup>
    </template>
  </SettingsSection>
</template>

<style scoped>
.choices {
  margin: 0 0 var(--fm-space-5);
}
/* The small-caps heading of UiSettingGroup, over a set of choice cards. */
.heading {
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.bar {
  box-sizing: border-box;
  display: block;
  width: 36px;
  height: 20px;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-sm);
}
/* For this session: the accent stops halfway, where the app quits. */
.bar.session {
  background: linear-gradient(to right, var(--fm-accent) 0 50%, var(--fm-surface-2) 50% 100%);
}
.bar.permanent {
  border-color: var(--fm-accent);
  background: var(--fm-accent);
}
[dir='rtl'] .bar.session {
  background: linear-gradient(to left, var(--fm-accent) 0 50%, var(--fm-surface-2) 50% 100%);
}
</style>
