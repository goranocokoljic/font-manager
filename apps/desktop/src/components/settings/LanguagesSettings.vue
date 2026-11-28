<script setup lang="ts">
// Settings › Languages (board 15, E58.2, ADR 0028): "Languages I work in" as rows to reorder
// (grip, place, name, the language's distinctive letters, x), with "Add a language"; under it
// "Then the rest", what the API answers after them: the languages that complete the first
// ten (the ten the filter drawer shows) and what is behind "Show more". Every change saves
// at once through `updateSettings`; the core then reorders `list_languages` and says so
// (`languageOrderChanged`), and the search store reads the new answer. Nothing here sorts.
import { computed, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import type { LanguagePreset } from '../../api/types';
import { t } from '../../i18n';
import { languageRest } from '../../search/languages';
import { useSearchStore } from '../../stores/search';
import { useSettingsStore } from '../../stores/settings';
import LanguagePicker from '../LanguagePicker.vue';
import UiButton from '../ui/UiButton.vue';
import UiIconButton from '../ui/UiIconButton.vue';
import UiReorderList from '../ui/UiReorderList.vue';
import UiSettingGroup from '../ui/UiSettingGroup.vue';
import UiSettingRow from '../ui/UiSettingRow.vue';
import SettingsSection from './SettingsSection.vue';

const settings = useSettingsStore();
const search = useSearchStore();
const router = useRouter();

onMounted(() => void search.loadLanguages());

/** The user's languages, in the user's order; set at once on a change, then saved. */
const mine = ref<string[]>([]);
watch(
  () => settings.settings?.myLanguages,
  (codes) => {
    mine.value = [...(codes ?? [])];
  },
  { immediate: true },
);

const presets = computed(() => search.languages);
const byCode = computed(() => new Map(presets.value.map((p) => [p.code, p])));
/** A code the presets do not know (yet) stands as its code. */
const presetOf = (code: string): LanguagePreset =>
  byCode.value.get(code) ?? { code, name: code, script: '' };

function save(codes: string[]): void {
  mine.value = codes;
  void settings.update({ myLanguages: codes });
}
const add = (code: string) => save([...mine.value, code]);
const remove = (code: string) => save(mine.value.filter((c) => c !== code));

/** While none is set: who leads now, as the API answers it (the system's language). */
const leadNow = computed(() =>
  mine.value.length ? [] : presets.value.filter((p) => p.group === 'mine'),
);
const emptyLine = computed(() =>
  leadNow.value.length
    ? t('settings.languages.emptySystem', {
        names: leadNow.value.map((p) => p.name).join(', '),
      })
    : t('settings.languages.emptyNone'),
);

const rest = computed(() => languageRest(presets.value));
/** The first names of the A-to-Z part, as the board shows them. */
const OTHERS_NAMED = 4;
const othersLine = computed(() => {
  const others = rest.value.others;
  if (!others.length) return '';
  const names = others.slice(0, OTHERS_NAMED).map((p) => p.name);
  return t(
    others.length > OTHERS_NAMED ? 'settings.languages.othersMore' : 'settings.languages.others',
    { count: others.length, names: names.join(', ') },
  );
});

function openRules(): void {
  void router.push({ name: 'duplicates', query: { section: 'rules' } });
}
</script>

<template>
  <SettingsSection
    id="languages"
    :heading="t('settings.sections.languages')"
    :lead="t('settings.lead.languages')"
  >
    <UiSettingGroup
      class="mine"
      :heading="t('settings.languages.mineHeading')"
      :note="t('settings.languages.mineNote')"
    >
      <UiReorderList
        v-if="mine.length"
        :items="mine"
        :key-of="(code) => code"
        :label-of="(code) => presetOf(code).name"
        :label="t('settings.languages.mineHeading')"
        data-testid="my-languages"
        @update:items="save"
      >
        <template #default="{ item }">
          <span class="name">{{ presetOf(item).name }}</span>
          <span
            v-if="presetOf(item).sampleGlyphs"
            class="glyphs"
            :lang="item"
            data-testid="my-language-glyphs"
            >{{ presetOf(item).sampleGlyphs }}</span
          >
          <span class="spacer" />
          <UiIconButton
            icon="close"
            size="sm"
            :label="t('settings.languages.remove', { name: presetOf(item).name })"
            @click="remove(item)"
          />
        </template>
      </UiReorderList>
      <p v-else class="empty" data-testid="my-languages-empty">{{ emptyLine }}</p>
      <div class="add">
        <LanguagePicker :presets="presets" :chosen="mine" @add="add" />
      </div>
    </UiSettingGroup>

    <UiSettingGroup
      class="rest-group"
      :heading="t('settings.languages.restHeading')"
      :note="t('settings.languages.restNote')"
    >
      <div class="rest" data-testid="languages-rest">
        <ul class="chips" :aria-label="t('settings.languages.firstLabel')">
          <li v-for="r in rest.first" :key="r.preset.code" class="chip">
            <span class="place">{{ r.place }}</span>
            {{ r.preset.name }}
          </li>
        </ul>
        <div
          v-if="rest.more.length || rest.others.length"
          class="behind"
          data-testid="languages-behind"
        >
          <span id="languages-behind-label" class="behind-label">{{
            t('settings.languages.behind')
          }}</span>
          <ul v-if="rest.more.length" class="chips" aria-labelledby="languages-behind-label">
            <li v-for="p in rest.more" :key="p.code" class="chip">{{ p.name }}</li>
          </ul>
          <span v-if="othersLine" class="others">{{ othersLine }}</span>
        </div>
      </div>
    </UiSettingGroup>

    <UiSettingGroup>
      <UiSettingRow
        v-slot="{ control }"
        :title="t('settings.languages.duplicatesTitle')"
        :line="t('settings.languages.duplicatesLine')"
      >
        <UiButton
          :aria-describedby="control['aria-describedby']"
          data-testid="languages-open-rules"
          @click="openRules"
        >
          {{ t('settings.languages.openRules') }}
        </UiButton>
      </UiSettingRow>
    </UiSettingGroup>
  </SettingsSection>
</template>

<style scoped>
.mine :deep(.ui-reorder-list) {
  border: 0;
}
.name {
  font-weight: 500;
}
.glyphs {
  color: var(--fm-muted);
  letter-spacing: 0.04em;
}
.spacer {
  flex: 1;
}
.empty {
  margin: 0;
  padding: var(--fm-space-3) var(--fm-space-4);
  color: var(--fm-muted);
}
.add {
  padding: var(--fm-space-2) var(--fm-space-3);
}
.rest-group :deep(.card) {
  border-color: transparent;
  background-color: var(--fm-surface);
}
.rest {
  padding: var(--fm-space-3) var(--fm-space-4);
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
.chip {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
  height: 28px;
  padding: 0 var(--fm-space-3);
  border: 1px solid var(--fm-control-border);
  border-radius: 999px;
  background-color: var(--fm-bg);
}
.place {
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-variant-numeric: tabular-nums;
}
.behind {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2) var(--fm-space-3);
  margin-top: var(--fm-space-3);
  padding-top: var(--fm-space-3);
  border-top: 1px solid var(--fm-border);
}
.behind-label {
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.others {
  color: var(--fm-muted);
}
</style>
