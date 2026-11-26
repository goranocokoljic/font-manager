<script setup lang="ts">
// Settings › Online catalogs: each catalog's switch, the folder installed fonts go to, and
// "Download fonts a document needs". Moved out of SettingsView in E57.3 as it was; E57.5
// draws it with catalog cards and setting rows (board 15).
import { computed, ref } from 'vue';
import type { SettingsUpdate } from '../../api/types';
import { errorMessage, t } from '../../i18n';
import { installPath, useCatalogsStore } from '../../stores/catalogs';
import { useSettingsStore } from '../../stores/settings';
import AdobeFontsNote from '../AdobeFontsNote.vue';
import UiButton from '../ui/UiButton.vue';
import SettingsSection from './SettingsSection.vue';

const store = useSettingsStore();
const catalogs = useCatalogsStore();
const s = computed(() => store.settings);

/**
 * "Download fonts a document needs" (E29.5) has nothing to do without the download
 * folder and a catalog that is on.
 */
const canFetchMissing = computed(
  () => !!s.value?.downloadFolder && catalogs.catalogs.some((c) => c.enabled),
);
/** The Adobe Fonts entry shows its note when opened. */
const adobeFontsOpen = ref(false);

function choose(patch: SettingsUpdate): void {
  void store.update(patch);
}
</script>

<template>
  <SettingsSection
    id="catalogs"
    :heading="t('settings.sections.catalogs')"
    :lead="t('settings.catalogs.note')"
  >
    <template v-for="c in catalogs.catalogs" :key="c.id">
      <label class="choice">
        <input
          type="checkbox"
          :name="`catalog-${c.id}`"
          :aria-describedby="`catalog-hint-${c.id}`"
          :checked="c.enabled"
          :disabled="catalogs.busy === c.id"
          @change="catalogs.setEnabled(c.id, ($event.target as HTMLInputElement).checked)"
        />
        <bdi>{{ c.name }}</bdi>
      </label>
      <p :id="`catalog-hint-${c.id}`" class="note hint">
        {{ t('settings.catalogs.connectsHint', { name: c.name }) }}
      </p>
    </template>
    <p v-if="catalogs.error" role="alert" class="note">
      {{ errorMessage(catalogs.error) }}
    </p>
    <UiButton
      aria-controls="settings-adobe-fonts"
      :aria-expanded="adobeFontsOpen"
      @click="adobeFontsOpen = !adobeFontsOpen"
    >
      {{ t('catalogs.adobeFonts.name') }}
    </UiButton>
    <AdobeFontsNote v-if="adobeFontsOpen" id="settings-adobe-fonts" :level="3" />
    <div class="field" data-testid="settings-download-folder">
      <span id="download-folder-label">{{ t('settings.catalogs.downloadFolder') }}</span>
      <span class="folder-row">
        <bdi v-if="s?.downloadFolder" class="folder" data-testid="settings-download-folder-path">{{
          s.downloadFolder
        }}</bdi>
        <span v-else class="note">{{ t('settings.catalogs.downloadFolderNone') }}</span>
        <UiButton
          aria-describedby="download-folder-label download-folder-hint"
          data-testid="settings-download-folder-change"
          @click="catalogs.changeDownloadFolder()"
        >
          {{ t('settings.catalogs.change') }}
        </UiButton>
      </span>
    </div>
    <p id="download-folder-hint" class="note hint">
      {{ t('settings.catalogs.downloadFolderHint') }}
    </p>
    <ul
      v-if="s?.downloadFolder"
      class="catalog-paths"
      :aria-label="t('settings.catalogs.pathsLabel')"
      data-testid="settings-catalog-paths"
    >
      <li v-for="c in catalogs.catalogs" :key="c.id" :data-catalog="c.id">
        <bdi>{{ c.name }}</bdi
        >:
        <bdi class="folder">{{
          installPath(s.downloadFolder, c, t('setup.catalogs.familyPlaceholder'))
        }}</bdi>
      </li>
    </ul>
    <label class="choice">
      <input
        type="checkbox"
        name="fetch-missing-fonts"
        aria-describedby="fetch-missing-fonts-hint"
        :checked="s?.fetchMissingFonts ?? false"
        :disabled="!canFetchMissing"
        @change="choose({ fetchMissingFonts: ($event.target as HTMLInputElement).checked })"
      />
      {{ t('settings.catalogs.fetchMissing') }}
    </label>
    <p id="fetch-missing-fonts-hint" class="note hint" data-testid="fetch-missing-fonts-hint">
      {{
        canFetchMissing
          ? t('settings.catalogs.fetchMissingHint')
          : t('settings.catalogs.fetchMissingNeeds')
      }}
    </p>
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
.choice {
  display: flex;
  align-items: center;
  gap: 6px;
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
.folder-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.folder {
  overflow-wrap: anywhere;
}
.catalog-paths {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  list-style: none;
}
</style>
