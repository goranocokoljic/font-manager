<script setup lang="ts">
// Settings › Appearance: the colour theme, light or dark, and the language. Moved out of
// SettingsView in E57.3 as it was; E57.4 draws it with choice cards and setting rows.
import { computed } from 'vue';
import type { Language, SettingsUpdate } from '../../api/types';
import { CATALOGS, LOCALES, resolveLocale, t, type Catalog } from '../../i18n';
import { useSettingsStore } from '../../stores/settings';
import { useThemesStore } from '../../stores/themes';
import ThemeCard from '../ThemeCard.vue';
import UiSelect from '../ui/UiSelect.vue';
import SettingsSection from './SettingsSection.vue';

const store = useSettingsStore();
const themes = useThemesStore();
const s = computed(() => store.settings);

/** Radio choices; each change is saved at once. */
function choose(patch: SettingsUpdate): void {
  void store.update(patch);
}

/** A locale's own name for itself (`Deutsch`), from its catalog. */
function localeName(tag: string): string {
  const name = (CATALOGS[tag]?.meta as Catalog | undefined)?.name;
  return typeof name === 'string' ? name : tag;
}

/** The locale "System" resolves to on this computer, by its own name. */
const systemLocaleName = computed(() => {
  const preferred = navigator.languages?.length ? navigator.languages : [navigator.language];
  return localeName(resolveLocale('system', preferred));
});

const languageOptions = computed(() => [
  { value: 'system', label: t('language.system', { name: systemLocaleName.value }) },
  ...LOCALES.map((tag) => ({ value: tag, label: localeName(tag) })),
]);
function chooseLanguage(value: string): void {
  choose({ language: value as Language });
}
</script>

<template>
  <SettingsSection
    id="appearance"
    :heading="t('settings.sections.appearance')"
    :lead="t('settings.lead.appearance')"
  >
    <template v-if="s">
      <fieldset class="group">
        <legend>{{ t('settings.appearance.colorTheme') }}</legend>
        <div class="theme-cards">
          <ThemeCard
            v-for="theme in themes.themes"
            :key="theme.id"
            :theme="theme"
            :scheme="themes.scheme"
            :checked="themes.active?.id === theme.id"
            @choose="themes.choose"
          />
        </div>
      </fieldset>
      <fieldset class="group">
        <legend>{{ t('settings.appearance.theme') }}</legend>
        <label v-for="value in ['system', 'light', 'dark'] as const" :key="value" class="choice">
          <input
            type="radio"
            name="theme"
            :checked="s.theme === value"
            @change="choose({ theme: value })"
          />
          {{ t(`settings.appearance.themes.${value}`) }}
        </label>
      </fieldset>
      <label class="field">
        <span>{{ t('language.label') }}</span>
        <UiSelect
          name="language"
          class="select"
          aria-describedby="language-hint"
          :model-value="s.language ?? 'system'"
          :options="languageOptions"
          @update:model-value="chooseLanguage"
        />
      </label>
      <p id="language-hint" class="note">{{ t('language.hint') }}</p>
    </template>
  </SettingsSection>
</template>

<style scoped>
.note {
  margin: 0 0 12px;
  color: var(--fm-muted);
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
}
/* Group legends and field labels share one quiet caption style. */
.group legend,
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
.select {
  align-self: flex-start;
}
.theme-cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--fm-space-3);
}
</style>
