<script setup lang="ts">
// Settings › Online catalogs (board 15, E57.5). Each catalog of `list_catalogs` is a card like
// a source card: a mark, the name, one line, the number of families while it is on, and its
// switch. A catalog being turned on says so on its card and its switch waits; a failed switch
// says it on that card. Adobe Fonts is a card without a switch: its "How it works" opens the
// note. Under "Installed fonts": the folder for installed fonts as a setting row, the tree of
// where each catalog that is on installs (the paths `installPath` computes), and "Download
// fonts a document needs". The facts come from fm-api through the stores; this file lays them out.
import { computed, ref } from 'vue';
import type { Catalog } from '../../api/types';
import { errorMessage, t } from '../../i18n';
import { installPath, useCatalogsStore } from '../../stores/catalogs';
import { useSettingsStore } from '../../stores/settings';
import AdobeFontsNote from '../AdobeFontsNote.vue';
import UiButton from '../ui/UiButton.vue';
import UiSettingGroup from '../ui/UiSettingGroup.vue';
import UiSettingRow from '../ui/UiSettingRow.vue';
import UiSwitch from '../ui/UiSwitch.vue';
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
/** The Adobe Fonts card shows its note when "How it works" opens it. */
const adobeFontsOpen = ref(false);

/** The card's mark: the first letter of the catalog's name. */
const mark = (name: string) => Array.from(name)[0] ?? '';

/** The card's one line: connecting while its switch works, else on or off. */
function line(c: Catalog): string {
  if (catalogs.busy === c.id) return t('settings.catalogs.connecting');
  return c.enabled ? t('settings.catalogs.lineOn') : t('settings.catalogs.lineOff');
}

/**
 * A failed switch of this catalog, said on its card. A catalog that is also open in the
 * browser keeps a failed fetch of its list in `refreshError`: that is this card's too.
 */
function cardError(c: Catalog): string | undefined {
  if (catalogs.error && catalogs.errorCatalog === c.id) return errorMessage(catalogs.error);
  if (catalogs.refreshError && catalogs.open === c.id) return errorMessage(catalogs.refreshError);
  return undefined;
}

/**
 * The tree under the folder: for each catalog that is on, where an Install puts a family,
 * written from the folder down (`Google Fonts\` and the family placeholder, muted).
 */
const tree = computed(() => {
  const folder = s.value?.downloadFolder;
  if (!folder) return [];
  const family = t('setup.catalogs.familyPlaceholder');
  return catalogs.catalogs
    .filter((c) => c.enabled)
    .map((c) => {
      const full = installPath(folder, c, family);
      // `installPath` writes `<folder><sep><catalog folder><sep><family>`.
      const rest = full.slice(folder.length).replace(/^[\\/]/, '');
      return { id: c.id, catalog: rest.slice(0, rest.length - family.length), family };
    });
});

/** The row's title, then its line: the description of a button its own text names. */
function describedByRow(control: {
  'aria-labelledby': string;
  'aria-describedby': string | undefined;
}): string {
  return [control['aria-labelledby'], control['aria-describedby']].filter(Boolean).join(' ');
}

function toggleFetchMissing(): void {
  void store.update({ fetchMissingFonts: !s.value?.fetchMissingFonts });
}
</script>

<template>
  <SettingsSection
    id="catalogs"
    :heading="t('settings.sections.catalogs')"
    :lead="t('settings.catalogs.note')"
  >
    <section class="group" aria-labelledby="catalogs-heading">
      <h3 id="catalogs-heading" class="heading">{{ t('settings.catalogs.catalogsHeading') }}</h3>
      <ul class="cards" aria-labelledby="catalogs-heading">
        <li
          v-for="c in catalogs.catalogs"
          :key="c.id"
          class="card"
          :data-catalog="c.id"
          :aria-busy="catalogs.busy === c.id"
        >
          <span class="mark" aria-hidden="true">{{ mark(c.name) }}</span>
          <div class="text">
            <bdi :id="`catalog-name-${c.id}`" class="name">{{ c.name }}</bdi>
            <span :id="`catalog-hint-${c.id}`" class="line" data-testid="catalog-card-line">{{
              line(c)
            }}</span>
            <span
              v-if="cardError(c)"
              :id="`catalog-error-${c.id}`"
              role="alert"
              class="error"
              data-testid="catalog-card-error"
              >{{ cardError(c) }}</span
            >
          </div>
          <span class="count" data-testid="catalog-card-count">{{
            c.enabled
              ? t('settings.catalogs.families', { count: c.familyCount })
              : t('settings.catalogs.off')
          }}</span>
          <UiSwitch
            :checked="c.enabled"
            :name="`catalog-${c.id}`"
            :aria-labelledby="`catalog-name-${c.id}`"
            :aria-describedby="
              cardError(c) ? `catalog-hint-${c.id} catalog-error-${c.id}` : `catalog-hint-${c.id}`
            "
            :disabled="catalogs.busy === c.id"
            @click="catalogs.setEnabled(c.id, !c.enabled)"
          />
        </li>
        <li class="card" data-catalog="adobeFonts">
          <span class="mark" aria-hidden="true">{{ mark(t('catalogs.adobeFonts.name')) }}</span>
          <div class="text">
            <bdi id="catalog-name-adobeFonts" class="name">{{ t('catalogs.adobeFonts.name') }}</bdi>
            <span id="catalog-hint-adobeFonts" class="line">{{
              t('settings.catalogs.adobeLine')
            }}</span>
          </div>
          <UiButton
            class="how"
            aria-controls="settings-adobe-fonts"
            aria-describedby="catalog-name-adobeFonts"
            :aria-expanded="adobeFontsOpen"
            @click="adobeFontsOpen = !adobeFontsOpen"
          >
            {{ t('settings.catalogs.howItWorks') }}
          </UiButton>
          <AdobeFontsNote
            v-if="adobeFontsOpen"
            id="settings-adobe-fonts"
            class="adobe-note"
            :level="3"
          />
        </li>
      </ul>
      <!-- A failure that is not one catalog's (the list of catalogs) is the section's. -->
      <p v-if="catalogs.error && !catalogs.errorCatalog" role="alert" class="error">
        {{ errorMessage(catalogs.error) }}
      </p>
    </section>

    <UiSettingGroup :heading="t('settings.catalogs.installedHeading')">
      <UiSettingRow
        :title="t('settings.catalogs.downloadFolder')"
        :line="t('settings.catalogs.downloadFolderHint')"
        :empty="t('settings.catalogs.downloadFolderNone')"
        data-testid="settings-download-folder"
      >
        <!-- The path, in the mono face, as the row's value. -->
        <template v-if="s?.downloadFolder" #value>
          <span data-testid="settings-download-folder-path">{{ s.downloadFolder }}</span>
        </template>
        <!-- Named by its text ("Change…", label in name); the row's title and line describe it. -->
        <template #default="{ control }">
          <UiButton
            :aria-describedby="describedByRow(control)"
            data-testid="settings-download-folder-change"
            @click="catalogs.changeDownloadFolder()"
          >
            {{ t('settings.catalogs.change') }}
          </UiButton>
        </template>
      </UiSettingRow>
    </UiSettingGroup>
    <ul
      v-if="s?.downloadFolder && tree.length"
      class="tree"
      :aria-label="t('settings.catalogs.pathsLabel')"
      data-testid="settings-catalog-paths"
    >
      <li class="root">
        <bdi>{{ s.downloadFolder }}</bdi>
      </li>
      <li v-for="branch in tree" :key="branch.id" class="branch" :data-catalog="branch.id">
        <bdi
          >{{ branch.catalog }}<span class="placeholder">{{ branch.family }}</span></bdi
        >
      </li>
    </ul>
    <UiSettingGroup>
      <UiSettingRow
        v-slot="{ control }"
        :title="t('settings.catalogs.fetchMissing')"
        :line="
          canFetchMissing
            ? t('settings.catalogs.fetchMissingHint')
            : t('settings.catalogs.fetchMissingNeeds')
        "
        data-testid="fetch-missing-fonts"
      >
        <UiSwitch
          v-bind="control"
          name="fetch-missing-fonts"
          :checked="s?.fetchMissingFonts ?? false"
          :disabled="!canFetchMissing"
          @click="toggleFetchMissing"
        />
      </UiSettingRow>
    </UiSettingGroup>
  </SettingsSection>
</template>

<style scoped>
.group {
  margin: 0 0 var(--fm-space-5);
}
/* The small-caps heading of UiSettingGroup, over the catalog cards. */
.heading {
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.cards {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
/* A source card (SourcesSettings): mark, text, count, control. */
.card {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto auto;
  align-items: center;
  gap: var(--fm-space-3);
  padding: var(--fm-space-3) var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
}
.card[data-catalog='adobeFonts'] {
  grid-template-columns: auto minmax(0, 1fr) auto;
}
.mark {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-hover);
  color: var(--fm-fg);
  font-family: Georgia, 'Times New Roman', serif;
  font-size: 18px;
}
.text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.name {
  font-weight: 500;
}
.line,
.count {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.count {
  white-space: nowrap;
}
.error {
  margin: 0;
  color: var(--fm-danger);
  font-size: var(--fm-text-sm);
}
.how {
  text-decoration: underline;
}
.adobe-note {
  grid-column: 1 / -1;
}
/* The folder tree: the folder, then one line per catalog that is on, in the mono face. */
.tree {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  margin: calc(-1 * var(--fm-space-2)) 0 var(--fm-space-5);
  padding: var(--fm-space-4);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface);
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
  list-style: none;
  overflow-wrap: anywhere;
}
.branch {
  padding-inline-start: var(--fm-space-4);
}
.placeholder {
  color: var(--fm-muted);
}
</style>
