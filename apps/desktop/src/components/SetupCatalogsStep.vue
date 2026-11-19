<script setup lang="ts">
// Setup step two, "Use the online catalogs?" (board 13, E55.5). One switch per catalog of
// `list_catalogs` (Google Fonts, Fontshare; Adobe Fonts is not a catalog the app can download
// from, ADR 0026), both off, each saying that turning it on fetches the list now. With a
// catalog on the step asks where downloaded fonts go: inside a font folder of step one, or
// another folder (the native picker; `<Documents>/Font Fu Fonts` proposed). It writes out
// where an install lands, `<folder>/<catalog>/<Family>`, for the catalogs that are on. The
// facts (folder names, refusals, the cloud mark, the license link) come from fm-api through
// the setup and catalogs stores; this file only lays them out.
import { computed, nextTick, onMounted } from 'vue';
import type { SourceId } from '../api/types';
import { errorMessage, t } from '../i18n';
import { installPath, useCatalogsStore } from '../stores/catalogs';
import { useSetupStore } from '../stores/setup';
import UiButton from './ui/UiButton.vue';
import UiSelect from './ui/UiSelect.vue';
import UiSwitch from './ui/UiSwitch.vue';

const setup = useSetupStore();
const catalogs = useCatalogsStore();

onMounted(async () => {
  await nextTick();
  document.getElementById('setup-step-heading')?.focus();
});

const folderOptions = computed(() => setup.folders.map((s) => ({ value: s.id, label: s.path })));

/** The lines "Google Fonts → <folder>/Google Fonts/<Family>", for the catalogs that are on. */
const landing = computed(() => {
  const folder = setup.downloadFolder;
  if (!folder) return [];
  return setup.catalogsOn.map((c) => ({
    id: c.id,
    name: c.name,
    path: installPath(folder, c, t('setup.catalogs.familyPlaceholder')),
  }));
});
</script>

<template>
  <div class="catalogs-step">
    <h1 id="setup-step-heading" class="heading" tabindex="-1">
      {{ t('setup.catalogs.heading') }}
    </h1>
    <p class="lead">{{ t('setup.catalogs.lead') }}</p>

    <ul class="cards" :aria-label="t('setup.catalogs.listLabel')" data-testid="setup-catalogs">
      <li v-for="c in catalogs.catalogs" :key="c.id" class="card" :data-catalog="c.id">
        <div class="text">
          <bdi :id="`setup-catalog-name-${c.id}`" class="name">{{ c.name }}</bdi>
          <span :id="`setup-catalog-hint-${c.id}`" class="hint">
            {{ t('settings.catalogs.connectsHint', { name: c.name }) }}
          </span>
        </div>
        <UiSwitch
          :checked="c.enabled"
          :aria-labelledby="`setup-catalog-name-${c.id}`"
          :aria-describedby="`setup-catalog-hint-${c.id}`"
          :disabled="catalogs.busy === c.id"
          :data-testid="`setup-catalog-${c.id}`"
          @click="setup.setCatalog(c.id, !c.enabled)"
        />
      </li>
    </ul>
    <p v-if="catalogs.error" class="refusal" role="alert">{{ errorMessage(catalogs.error) }}</p>

    <fieldset v-if="setup.catalogsOn.length" class="place" data-testid="setup-download-place">
      <legend class="legend">{{ t('setup.catalogs.placeLegend') }}</legend>
      <div v-if="setup.canPlaceInFontFolder" class="option">
        <label class="choice">
          <input
            type="radio"
            name="setup-download-place"
            :checked="setup.place === 'fontFolder'"
            @change="setup.choosePlace('fontFolder')"
          />
          {{ t('setup.catalogs.inside') }}
        </label>
        <template v-if="setup.place === 'fontFolder'">
          <UiSelect
            v-if="setup.folders.length > 1"
            class="folder-select"
            :model-value="setup.placeFolder?.id"
            :options="folderOptions"
            :aria-label="t('setup.catalogs.insideSelect')"
            data-testid="setup-font-folder-select"
            @update:model-value="(id: SourceId) => setup.chooseFontFolder(id)"
          />
          <bdi v-else class="path">{{ setup.placeFolder?.path }}</bdi>
        </template>
      </div>
      <div class="option">
        <label class="choice">
          <input
            type="radio"
            name="setup-download-place"
            :checked="setup.place === 'other'"
            @change="setup.choosePlace('other')"
          />
          {{ t('setup.catalogs.another') }}
        </label>
        <span v-if="setup.place === 'other'" class="other">
          <bdi class="path" data-testid="setup-other-folder">{{ setup.otherFolder }}</bdi>
          <UiButton data-testid="setup-other-choose" @click="setup.pickOtherFolder()">
            {{ t('setup.catalogs.choose') }}
          </UiButton>
        </span>
      </div>
    </fieldset>
    <p v-if="setup.folderError" class="refusal" role="alert" data-testid="setup-folder-error">
      {{ setup.folderError }}
    </p>

    <div v-if="landing.length" class="landing" data-testid="setup-landing">
      <p class="landing-title">{{ t('setup.catalogs.landingTitle') }}</p>
      <ul class="lines">
        <li v-for="line in landing" :key="line.id" :data-catalog="line.id">
          <bdi class="path">{{ line.path }}</bdi>
        </li>
      </ul>
    </div>

    <p v-if="setup.placeFolder?.cloud" class="cloud" data-testid="setup-cloud-line">
      {{ t('setup.catalogs.cloud') }}
      <template v-for="c in setup.catalogsOn" :key="c.id">
        <button
          v-if="c.licenseUrl"
          type="button"
          class="link"
          :data-testid="`setup-license-${c.id}`"
          @click="setup.openLicense(c)"
        >
          {{ t('setup.catalogs.license', { name: c.name }) }}
        </button>
      </template>
    </p>
  </div>
</template>

<style scoped>
.heading {
  margin: 0;
  font-size: var(--fm-text-xl);
  font-weight: 600;
  letter-spacing: -0.01em;
}
/* The heading takes the focus by script (tabindex -1, never a Tab stop); no ring. */
.heading:focus {
  outline: none;
}
.lead {
  margin: var(--fm-space-3) 0 var(--fm-space-6);
  color: var(--fm-muted);
  line-height: 1.55;
}
.cards {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
.card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--fm-space-3);
  padding: var(--fm-space-3) var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface-2);
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
.hint {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  line-height: 1.45;
}
.place {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
  margin: var(--fm-space-6) 0 0;
  padding: 0;
  border: 0;
}
.legend {
  margin-bottom: var(--fm-space-2);
  padding: 0;
  font-weight: 600;
}
.option {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
}
.choice {
  display: flex;
  align-items: center;
  gap: 6px;
}
.folder-select,
.other {
  margin-inline-start: 22px;
}
.other {
  display: flex;
  align-items: center;
  gap: var(--fm-space-3);
  min-width: 0;
}
.option > .path {
  margin-inline-start: 22px;
}
.path {
  overflow: hidden;
  color: var(--fm-muted);
  font-family: var(--fm-font-mono);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.refusal {
  margin: var(--fm-space-3) 0 0;
  color: var(--fm-danger);
  font-size: var(--fm-text-sm);
}
.landing {
  margin-top: var(--fm-space-6);
}
.landing-title {
  margin: 0 0 var(--fm-space-2);
  font-size: var(--fm-text-sm);
}
.lines {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}
.lines .path {
  display: block;
  color: var(--fm-fg);
}
.cloud {
  margin: var(--fm-space-4) 0 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  line-height: 1.55;
}
.link {
  margin-inline-start: var(--fm-space-2);
  padding: 0;
  border: 0;
  background: none;
  color: var(--fm-fg);
  font: inherit;
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}
</style>
