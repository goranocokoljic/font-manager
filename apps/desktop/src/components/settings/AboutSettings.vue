<script setup lang="ts">
// Settings › About: the logotype, the version, updates and the platform notes. Moved out of
// SettingsView in E57.3 as it was; E57.7 draws it as board 15 shows.
import { computed } from 'vue';
import { formatApiVersion } from '../../api/version';
import type { UpdateLevel } from '../../api/types';
import { errorMessage, formatDate, t, type MessageKey } from '../../i18n';
import { useAppStore } from '../../stores/app';
import { useSettingsStore } from '../../stores/settings';
import { useUpdatesStore } from '../../stores/updates';
import AppLogo from '../AppLogo.vue';
import UiButton from '../ui/UiButton.vue';
import SettingsSection from './SettingsSection.vue';

const store = useSettingsStore();
const app = useAppStore();
const updates = useUpdatesStore();
const s = computed(() => store.settings);

const upToDate = computed(() => updates.status === 'upToDate');

/** The update levels this copy offers (E55.10); none where it does not update itself. */
const levels = computed(() => updates.overview?.levels ?? []);
const INSTALL_ON_QUIT = 'installOnQuit' satisfies UpdateLevel;
const levelLabel = (level: UpdateLevel) => t(`updates.level.${level}` as MessageKey);

/** Saves the level (both settings at once), then reads the status under the control again. */
async function chooseLevel(level: UpdateLevel): Promise<void> {
  await store.update({ updateLevel: level });
  await updates.loadOverview();
}

/** When the last check ran and what it found. */
const lastCheckText = computed(() => {
  const last = updates.overview?.lastCheck;
  if (!last) return t('updates.lastCheck.never');
  const ms = Date.parse(last.at);
  const when = Number.isFinite(ms) ? formatDate(ms) : last.at;
  const outcome = last.outcome;
  if (outcome.type === 'found')
    return t('updates.lastCheck.found', { when, version: outcome.version });
  if (outcome.type === 'failed')
    return t('updates.lastCheck.failed', { when, message: outcome.message });
  return t('updates.lastCheck.upToDate', { when });
});
</script>

<template>
  <SettingsSection id="about" :heading="t('settings.sections.about')">
    <template v-if="app.info">
      <AppLogo class="brand" data-testid="app-logo" />
      <dl class="info" data-testid="app-info">
        <dt>{{ t('home.info.app') }}</dt>
        <dd>
          <bdi>{{ app.info.name }} {{ app.info.version }}</bdi>
        </dd>
        <dt>{{ t('home.info.api') }}</dt>
        <dd>{{ formatApiVersion(app.info.apiVersion) }}</dd>
        <dt>{{ t('home.info.platform') }}</dt>
        <dd>
          <bdi>{{ app.info.platform }}</bdi>
        </dd>
      </dl>
      <!-- Hidden where Flatpak or a package manager installs the updates; the
           platform notes below say so (ADR 0012). -->
      <section v-if="updates.support?.offered" class="updates" aria-labelledby="updates-title">
        <h3 id="updates-title" class="subheading">{{ t('updates.title') }}</h3>
        <div class="add">
          <UiButton
            :disabled="updates.status === 'checking' || updates.status === 'installing'"
            @click="updates.check()"
          >
            {{ t('updates.check') }}
          </UiButton>
          <UiButton
            v-if="updates.status === 'available' && updates.update && !updates.open"
            @click="updates.open = true"
          >
            {{ t('updates.showVersion', { version: updates.update.version }) }}
          </UiButton>
        </div>
        <p role="status" class="note status">
          <template v-if="updates.status === 'checking'">{{ t('updates.checking') }}</template>
          <template v-else-if="upToDate">{{ t('updates.upToDate') }}</template>
          <template v-else-if="updates.status === 'available' && updates.update">{{
            t('updates.available', { version: updates.update.version })
          }}</template>
        </p>
        <p v-if="updates.error && !updates.open" role="alert" class="error">
          {{ errorMessage(updates.error) }}
        </p>
        <!-- E55.10: one control over "Check automatically" and "Install updates when I
             quit"; fm-api maps each level to the two settings, so no combination it
             refuses is shown. Native radios: arrow keys move between the levels. -->
        <fieldset v-if="s && levels.length" class="group">
          <legend>{{ t('updates.level.legend') }}</legend>
          <template v-for="level in levels" :key="level">
            <label class="choice">
              <input
                type="radio"
                name="update-level"
                :checked="updates.overview?.level === level"
                :aria-describedby="level === INSTALL_ON_QUIT ? 'install-on-quit-notes' : undefined"
                @change="chooseLevel(level)"
              />
              {{ levelLabel(level) }}
            </label>
            <ul
              v-if="level === INSTALL_ON_QUIT"
              id="install-on-quit-notes"
              class="note hint level-notes"
            >
              <li>{{ t('updates.level.majorAsks') }}</li>
              <li>{{ t('updates.level.sessionEnds') }}</li>
            </ul>
          </template>
        </fieldset>
        <p class="note level-hint">
          {{ t('updates.checkAutomaticallyHint') }}
        </p>
        <p v-if="updates.overview" class="note last-check">{{ lastCheckText }}</p>
        <div v-if="updates.overview?.waiting" class="add waiting">
          <p class="note">
            {{ t('updates.waiting', { version: updates.overview.waiting }) }}
          </p>
          <UiButton @click="updates.openDialog()">{{ t('updates.installNow') }}</UiButton>
        </div>
      </section>
      <h3 id="platform-notes-title" class="subheading">
        {{ t('settings.about.platformNotes') }}
      </h3>
      <ul
        v-if="app.info.platformNotes?.length"
        class="notes"
        aria-labelledby="platform-notes-title"
      >
        <li v-for="note in app.info.platformNotes" :key="note">{{ note }}</li>
      </ul>
      <p v-else class="note">{{ t('settings.about.noPlatformNotes') }}</p>
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
.level-notes {
  margin-block: 2px 4px;
  padding-inline-start: 18px;
}
.level-hint {
  margin: 4px 0 0;
}
.last-check {
  margin-top: 8px;
}
.waiting {
  align-items: center;
}
.waiting .note {
  margin: 0;
}
.updates {
  margin-bottom: 8px;
}
.status {
  margin-top: 8px;
}
.status:empty {
  display: none;
}
.subheading {
  margin: 16px 0 8px;
  font-size: var(--fm-text-md);
}
.notes {
  margin: 0;
  padding-inline-start: 20px;
}
.notes li + li {
  margin-top: 6px;
}
.error {
  margin: 0 0 12px;
  color: var(--fm-danger);
}
.choice {
  display: flex;
  align-items: center;
  gap: 6px;
}
.add {
  display: flex;
  gap: 8px;
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
.brand {
  block-size: 40px;
  margin-block: var(--fm-space-3) var(--fm-space-4);
}
.info {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: var(--fm-space-1) var(--fm-space-4);
  margin: 0 0 var(--fm-space-4);
  font-size: var(--fm-text-sm);
}
.info dt {
  color: var(--fm-muted);
}
.info dd {
  margin: 0;
}
</style>
