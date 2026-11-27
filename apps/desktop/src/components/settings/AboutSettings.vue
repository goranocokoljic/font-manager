<script setup lang="ts">
// Settings › About (board 15): one quiet panel with the logotype, one line on what Font Fu
// is and the version; then, where this copy updates itself, the state of updates as the
// title of one setting row, the update levels as choice cards, and the platform notes in a
// quiet panel.
import { computed } from 'vue';
import { formatApiVersion } from '../../api/version';
import type { UpdateLevel } from '../../api/types';
import { errorMessage, formatDate, t, type MessageKey } from '../../i18n';
import { useAppStore } from '../../stores/app';
import { useSettingsStore } from '../../stores/settings';
import { useUpdatesStore } from '../../stores/updates';
import AppLogo from '../AppLogo.vue';
import UiButton from '../ui/UiButton.vue';
import UiChoiceCards, { type ChoiceCard } from '../ui/UiChoiceCards.vue';
import UiSettingGroup from '../ui/UiSettingGroup.vue';
import UiSettingRow from '../ui/UiSettingRow.vue';
import SettingsSection from './SettingsSection.vue';

const store = useSettingsStore();
const app = useAppStore();
const updates = useUpdatesStore();
const s = computed(() => store.settings);

/** The line of each level's card; "install when I quit" has its two notes as its line. */
const levelLine = (level: UpdateLevel): string =>
  level === 'installOnQuit'
    ? `${t('updates.level.majorAsks')} ${t('updates.level.sessionEnds')}`
    : t(`updates.level.${level}Line` as MessageKey);

/**
 * The update levels this copy offers (E55.10), as cards: only those fm-api lists, so no
 * combination of the two settings it refuses is shown; none where it does not update itself.
 */
const levels = computed<ChoiceCard<UpdateLevel>[]>(() =>
  (updates.overview?.levels ?? []).map((level) => ({
    value: level,
    title: t(`updates.level.${level}` as MessageKey),
    line: levelLine(level),
  })),
);

/** Saves the level (both settings at once), then reads the status under the control again. */
async function chooseLevel(level: UpdateLevel): Promise<void> {
  await store.update({ updateLevel: level });
  await updates.loadOverview();
}

/** The title of the status row: where "Check for updates" is, else what the last check found. */
const stateTitle = computed(() => {
  if (updates.status === 'checking') return t('updates.checking');
  if (updates.status === 'upToDate') return t('updates.upToDate');
  if ((updates.status === 'available' || updates.status === 'installing') && updates.update)
    return t('updates.available', { version: updates.update.version });
  const outcome = updates.overview?.lastCheck?.outcome;
  if (outcome?.type === 'found') return t('updates.available', { version: outcome.version });
  if (outcome?.type === 'upToDate') return t('updates.upToDate');
  if (outcome?.type === 'failed') return t('updates.lastCheckFailed');
  return t('updates.lastCheck.never');
});

/** The row's line: when the last check ran and what it found; none before the first check. */
const lastCheckText = computed(() => {
  const last = updates.overview?.lastCheck;
  if (!last) return undefined;
  const ms = Date.parse(last.at);
  const when = Number.isFinite(ms) ? formatDate(ms) : last.at;
  const outcome = last.outcome;
  if (outcome.type === 'found')
    return t('updates.lastCheck.found', { when, version: outcome.version });
  if (outcome.type === 'failed')
    return t('updates.lastCheck.failed', { when, message: outcome.message });
  return t('updates.lastCheck.upToDate', { when });
});

/** A failed check, said on the status row; the dialog says its own errors. */
const checkError = computed(() =>
  updates.error && !updates.open ? errorMessage(updates.error) : undefined,
);
</script>

<template>
  <SettingsSection id="about" :heading="t('settings.sections.about')">
    <template v-if="app.info">
      <!-- The board's wordmark only stands in for the logotype, at its size. -->
      <div class="hero" data-testid="app-info">
        <div class="identity">
          <AppLogo class="brand" data-testid="app-logo" />
          <p class="tagline">{{ t('settings.about.tagline') }}</p>
        </div>
        <p class="version">
          <span class="number"
            ><span class="sr-only">{{ app.info.name }} </span
            ><bdi>{{ app.info.version }}</bdi></span
          >
          <span class="meta"
            >{{ t('settings.about.api', { version: formatApiVersion(app.info.apiVersion) }) }} ·
            <span class="sr-only">{{ t('home.info.platform') }} </span
            ><bdi>{{ app.info.platform }}</bdi></span
          >
        </p>
      </div>
      <!-- Hidden where Flatpak or a package manager installs the updates; the
           platform notes below say so (ADR 0012). -->
      <section v-if="updates.support?.offered" class="updates" aria-labelledby="updates-title">
        <h3 id="updates-title" class="heading">{{ t('updates.title') }}</h3>
        <UiSettingGroup>
          <UiSettingRow live :title="stateTitle" :line="lastCheckText" :error="checkError">
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
          </UiSettingRow>
          <UiSettingRow
            v-if="updates.overview?.waiting"
            :title="t('updates.waiting', { version: updates.overview.waiting })"
          >
            <UiButton @click="updates.openDialog()">{{ t('updates.installNow') }}</UiButton>
          </UiSettingRow>
        </UiSettingGroup>
        <!-- E55.10: one control over "Check automatically" and "Install updates when I
             quit"; fm-api maps each level to the two settings. -->
        <!-- Not a region of its own: the cards' radio group carries the name, and a second
             region named "…updates" would sit inside "Updates". -->
        <div v-if="s && levels.length" class="levels">
          <h3 id="update-level-heading" class="heading">{{ t('updates.level.legend') }}</h3>
          <UiChoiceCards
            name="update-level"
            :label="t('updates.level.legend')"
            :columns="1"
            :model-value="updates.overview?.level ?? null"
            :options="levels"
            @update:model-value="chooseLevel"
          />
        </div>
      </section>
      <section aria-labelledby="platform-notes-title">
        <h3 id="platform-notes-title" class="heading">
          {{ t('settings.about.platformNotes') }}
        </h3>
        <div class="panel notes-panel">
          <ul
            v-if="app.info.platformNotes?.length"
            class="notes"
            aria-labelledby="platform-notes-title"
          >
            <li v-for="note in app.info.platformNotes" :key="note">{{ note }}</li>
          </ul>
          <p v-else class="empty">{{ t('settings.about.noPlatformNotes') }}</p>
        </div>
      </section>
    </template>
  </SettingsSection>
</template>

<style scoped>
/* The quiet panel: the surface tone, no border. */
.hero,
.panel {
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface);
}
.hero {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--fm-space-3) var(--fm-space-5);
  margin: var(--fm-space-4) 0 var(--fm-space-5);
  padding: var(--fm-space-6) var(--fm-space-5);
}
.identity {
  min-width: 0;
}
.brand {
  block-size: 72px;
  max-inline-size: 100%;
  margin-block-end: var(--fm-space-3);
}
.tagline {
  margin: 0;
  color: var(--fm-muted);
}
.version {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
  margin: 0;
}
.number {
  font-size: var(--fm-text-xl);
  font-weight: 600;
}
.meta {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.levels {
  margin: 0 0 var(--fm-space-5);
}
/* The small-caps heading of UiSettingGroup, over the groups of this section. */
.heading {
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.notes-panel {
  padding: var(--fm-space-3) var(--fm-space-4);
  color: var(--fm-muted);
}
.notes {
  margin: 0;
  padding-inline-start: 20px;
}
.notes li + li {
  margin-top: 6px;
}
.empty {
  margin: 0;
}
</style>
