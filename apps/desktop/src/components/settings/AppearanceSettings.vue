<script setup lang="ts">
// Settings › Appearance (board 15): the colour themes as `ThemeCard`s in one row, light or dark
// as three choice cards with small figures, and the language as a setting row. Every choice
// saves at once.
import { computed } from 'vue';
import type { Language, SettingsUpdate, Theme } from '../../api/types';
import { CATALOGS, LOCALES, resolveLocale, t, type Catalog } from '../../i18n';
import { useSettingsStore } from '../../stores/settings';
import { useThemesStore } from '../../stores/themes';
import ThemeCard from '../ThemeCard.vue';
import UiChoiceCards, { type ChoiceCard } from '../ui/UiChoiceCards.vue';
import UiSelect from '../ui/UiSelect.vue';
import UiSettingGroup from '../ui/UiSettingGroup.vue';
import UiSettingRow from '../ui/UiSettingRow.vue';
import SettingsSection from './SettingsSection.vue';

const store = useSettingsStore();
const themes = useThemesStore();
const s = computed(() => store.settings);

/** Each change is saved at once. */
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
const schemes = computed<ChoiceCard<Theme>[]>(() =>
  (['system', 'light', 'dark'] as const).map((value) => ({
    value,
    title: t(`settings.appearance.themes.${value}`),
  })),
);

/**
 * The figures of the light and dark cards: the active theme's own paper in that mode (half
 * and half for "Same as the system"), so each shows what the choice gives in this theme.
 */
const paperStyle = computed(() => {
  const theme = themes.active;
  if (!theme) return {};
  return {
    '--light': theme.light.bg,
    '--light-border': theme.light.border,
    '--dark': theme.dark.bg,
    '--dark-border': theme.dark.border,
  };
});

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
        <legend class="heading">{{ t('settings.appearance.colorTheme') }}</legend>
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
      <section class="group" aria-labelledby="scheme-heading">
        <h3 id="scheme-heading" class="heading">{{ t('settings.appearance.theme') }}</h3>
        <UiChoiceCards
          name="theme"
          :columns="3"
          :label="t('settings.appearance.theme')"
          :model-value="s.theme"
          :options="schemes"
          @update:model-value="choose({ theme: $event })"
        >
          <template #figure="{ option }">
            <span
              class="paper"
              :class="option.value"
              :style="paperStyle"
              data-testid="scheme-figure"
            />
          </template>
        </UiChoiceCards>
      </section>
      <UiSettingGroup :heading="t('language.label')">
        <UiSettingRow v-slot="{ control }" :title="t('language.label')" :line="t('language.hint')">
          <UiSelect
            v-bind="control"
            name="language"
            :model-value="s.language ?? 'system'"
            :options="languageOptions"
            @update:model-value="chooseLanguage"
          />
        </UiSettingRow>
      </UiSettingGroup>
    </template>
  </SettingsSection>
</template>

<style scoped>
.group {
  margin: 0 0 var(--fm-space-5);
  padding: 0;
  border: 0;
}
/* The small-caps heading of UiSettingGroup, over cards that are not setting rows. */
.heading {
  margin: 0 0 var(--fm-space-2);
  padding: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
/* The themes in one row, however many there are. */
.theme-cards {
  display: grid;
  grid-auto-columns: minmax(0, 1fr);
  grid-auto-flow: column;
  gap: var(--fm-space-3);
}
/* Light or dark: a small sheet of the theme's paper in that mode. */
.paper {
  box-sizing: border-box;
  display: block;
  width: 36px;
  height: 20px;
  border-radius: var(--fm-radius-sm);
}
.paper.light {
  border: 1px solid var(--light-border, var(--fm-border-strong));
  background: var(--light, var(--fm-bg));
}
.paper.dark {
  border: 1px solid var(--dark-border, var(--fm-border-strong));
  background: var(--dark, var(--fm-fg));
}
.paper.system {
  border: 1px solid var(--fm-border-strong);
  background: linear-gradient(
    to right,
    var(--light, var(--fm-bg)) 0 50%,
    var(--dark, var(--fm-fg)) 50% 100%
  );
}
</style>
