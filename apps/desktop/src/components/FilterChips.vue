<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { ApiError } from '../api/types';
import { errorMessage, formatNumber, t } from '../i18n';
import { useActiveFilters } from '../search/activeFilters';
import {
  chipsThatFit,
  hasChosenFilters,
  hasFilters,
  isCommonFeature,
  languageName,
  toggleChip,
  toggleLanguage,
  type Chip,
} from '../search/chips';
import { useCollectionsStore } from '../stores/collections';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useSearchStore } from '../stores/search';
import UiBadge from './ui/UiBadge.vue';
import UiChevron from './ui/UiChevron.vue';
import UiDrawer from './ui/UiDrawer.vue';

const search = useSearchStore();
const collections = useCollectionsStore();
const preview = usePreviewStore();
const { groups, languages, chips: active, labels } = useActiveFilters();

/** The first languages sit in the drawer; the rest hide behind an expander. */
const LANGUAGES_SHOWN = 10;
const allLanguages = ref(false);
const shownLanguages = computed(() => {
  const all = search.languages;
  if (allLanguages.value) return all;
  return all.filter((l, i) => i < LANGUAGES_SHOWN || languages.value.includes(l.code));
});
const filtering = computed(() => hasFilters(search.filters));

// The facets live in a drawer; the bar shows only what is chosen, as chips to remove.
const drawerOpen = ref(false);
const bar = ref<HTMLElement | null>(null);
const opener = ref<HTMLButtonElement | null>(null);
const allFeatures = ref(false);
/** Raw OpenType tags are developer data: they sit behind an expander. */
const facetGroups = computed(() => groups.value.filter((g) => g.facet !== 'features'));
/** Language sits right after Class, not at the bottom of a long drawer. */
const LANGUAGE_SLOT = { facet: 'language' as const };
const drawerFacets = computed(() => {
  const list: ((typeof facetGroups.value)[number] | typeof LANGUAGE_SLOT)[] = [
    ...facetGroups.value,
  ];
  const i = list.findIndex((g) => g.facet === 'classification');
  list.splice(i < 0 ? list.length : i + 1, 0, LANGUAGE_SLOT);
  return list;
});
const featureGroup = computed(() => groups.value.find((g) => g.facet === 'features') ?? null);
/** Named features show in the drawer; stylistic sets, variants and the rest sit behind an expander. */
const commonFeatures = computed(
  () => featureGroup.value?.chips.filter((c) => isCommonFeature(String(c.value))) ?? [],
);
const otherFeatures = computed(
  () => featureGroup.value?.chips.filter((c) => !isCommonFeature(String(c.value))) ?? [],
);
/** The named chips, then the expander and the rest; an empty set draws nothing. */
const featureSets = computed(() =>
  [
    { key: 'common', chips: commonFeatures.value },
    { key: 'other', chips: otherFeatures.value },
  ].filter((set) => set.chips.length),
);
const activeCount = computed(() => labels.value.length);
/** A smart collection needs a chosen filter; the open collection alone is none. */
const savable = computed(() => hasChosenFilters(search.filters));

/** A chip's accessible name: its label and the families it gives. */
const chipLabel = (name: string, n: number) =>
  t('chips.chipLabel', { name, families: t('chips.families', { count: n }) });

function toggle(chip: Chip): void {
  search.setFilters(toggleChip(search.filters, chip.facet, chip.value));
}

/** Choosing a language also puts its pangram in the All fonts previews. */
function onLanguage(code: string): void {
  const adding = !languages.value.includes(code);
  search.setFilters(toggleLanguage(search.filters, code));
  if (adding && search.languages.find((l) => l.code === code)?.pangram) {
    preview.update(ALL_FONTS_VIEW, { language: code });
  }
}

/** Takes a filter off from its chip; the chip goes away, so the focus goes to the button. */
async function remove(off: () => void): Promise<void> {
  off();
  await nextTick();
  if (!drawerOpen.value) opener.value?.focus();
}

// The chips stay on one row: those that do not fit stay in the page, unseen, to be
// measured again, and "+N" stands for them and opens the drawer.
const row = ref<HTMLElement | null>(null);
const more = ref<HTMLButtonElement | null>(null);
const fit = ref(Infinity);
const spilled = computed(() => Math.max(0, activeCount.value - fit.value));

function measure(): void {
  const el = row.value;
  if (!el) return;
  const widths = [...el.querySelectorAll<HTMLElement>('.chip.active')].map((c) => c.offsetWidth);
  const gap = parseFloat(getComputedStyle(el).columnGap) || 0;
  fit.value = chipsThatFit(widths, el.clientWidth, more.value?.offsetWidth ?? 0, gap);
}
watch(labels, measure, { flush: 'post' });
let observer: ResizeObserver | undefined;
onMounted(() => {
  measure();
  if (typeof ResizeObserver === 'undefined' || !row.value) return;
  observer = new ResizeObserver(measure);
  observer.observe(row.value);
});
onBeforeUnmount(() => observer?.disconnect());

// Saving the chosen filters as a smart collection (E14.4): the rules are the
// filters as they are, so the collection lists the families the chips gave.
const naming = ref(false);
const name = ref('');
/** The name of the smart collection just saved, for the status line. */
const saved = ref('');
const saveError = ref<ApiError | null>(null);
const nameInput = ref<HTMLInputElement | null>(null);
const saveButton = ref<HTMLButtonElement | null>(null);

async function startSave(): Promise<void> {
  naming.value = true;
  saved.value = '';
  saveError.value = null;
  await nextTick();
  nameInput.value?.focus();
}

/** Closes the name field and gives focus back to the button. */
async function endSave(): Promise<void> {
  naming.value = false;
  saveError.value = null;
  name.value = '';
  await nextTick();
  saveButton.value?.focus();
}

async function save(): Promise<void> {
  try {
    const c = await search.saveAsSmartCollection(name.value);
    saved.value = c.name;
    await endSave();
    // The sidebar lists it, with its count.
    await collections.refresh();
  } catch (e) {
    saveError.value = e as ApiError;
  }
}
</script>

<template>
  <section ref="bar" class="chips" :aria-label="t('chips.filters')">
    <button
      ref="opener"
      type="button"
      class="opener"
      aria-haspopup="dialog"
      :aria-expanded="drawerOpen"
      @click="drawerOpen = !drawerOpen"
    >
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
        <path
          d="M2 4h12M4.5 8h7M7 12h2"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
        />
      </svg>
      {{ t('chips.filters') }}
      <UiBadge v-if="activeCount" tone="accent" class="count">{{
        formatNumber(activeCount)
      }}</UiBadge>
    </button>
    <div ref="row" class="row">
      <button
        v-for="(chip, i) in active"
        :key="`${chip.facet}:${String(chip.value)}`"
        type="button"
        class="chip active"
        :class="{ spilled: i >= fit }"
        :aria-label="t('chips.remove', { name: chip.label })"
        @click="remove(() => toggle(chip))"
      >
        {{ chip.label }}
        <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
          <path d="M4 4l8 8M12 4l-8 8" fill="none" stroke="currentColor" stroke-width="1.5" />
        </svg>
      </button>
      <button
        v-for="(code, i) in languages"
        :key="`lang:${code}`"
        type="button"
        class="chip active"
        :class="{ spilled: active.length + i >= fit }"
        :aria-label="t('chips.remove', { name: languageName(search.languages, code) })"
        @click="remove(() => onLanguage(code))"
      >
        {{ languageName(search.languages, code) }}
        <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
          <path d="M4 4l8 8M12 4l-8 8" fill="none" stroke="currentColor" stroke-width="1.5" />
        </svg>
      </button>
      <button
        ref="more"
        type="button"
        class="chip more-chips"
        :class="{ spilled: spilled === 0 }"
        :aria-label="t('chips.more', { count: spilled })"
        @click="drawerOpen = true"
      >
        +{{ formatNumber(spilled) }}
      </button>
    </div>

    <UiDrawer
      :open="drawerOpen"
      :label="t('chips.options')"
      :anchor="bar"
      @close="drawerOpen = false"
    >
      <div class="actions">
        <button v-if="filtering" type="button" class="link" @click="search.setFilters({})">
          {{ t('chips.clear') }}
        </button>
        <form
          v-if="naming"
          class="save"
          :aria-label="t('chips.saveAsSmart')"
          @submit.prevent="save"
          @keydown.esc.prevent.stop="endSave"
        >
          <input
            ref="nameInput"
            v-model="name"
            class="name"
            :aria-label="t('chips.smartName')"
            :placeholder="t('chips.namePlaceholder')"
            :aria-invalid="saveError ? 'true' : undefined"
            :aria-describedby="saveError ? 'fm-save-smart-error' : undefined"
            @input="saveError = null"
          />
          <button type="submit" class="chip">{{ t('common.save') }}</button>
          <button type="button" class="chip" @click="endSave">{{ t('common.cancel') }}</button>
          <p v-if="saveError" id="fm-save-smart-error" role="alert" class="error">
            {{ errorMessage(saveError) }}
          </p>
        </form>
        <button
          v-else
          ref="saveButton"
          type="button"
          class="link"
          :disabled="!savable"
          :title="savable ? undefined : t('chips.saveHint')"
          @click="startSave"
        >
          {{ t('chips.saveAsSmart') }}
        </button>
        <p class="note" role="status">{{ saved ? t('chips.saved', { name: saved }) : '' }}</p>
      </div>
      <template v-for="g in drawerFacets" :key="g.facet">
        <div
          v-if="g.facet === 'language'"
          class="facet"
          role="group"
          :aria-label="t('chips.languages')"
        >
          <h3 class="label" aria-hidden="true">{{ t('chips.language') }}</h3>
          <div class="chipset">
            <button
              v-for="l in shownLanguages"
              :key="l.code"
              type="button"
              class="chip"
              :aria-pressed="languages.includes(l.code)"
              @click="onLanguage(l.code)"
            >
              {{ l.name }}
            </button>
            <button
              v-if="search.languages.length > LANGUAGES_SHOWN"
              type="button"
              class="chip more"
              :aria-expanded="allLanguages"
              @click="allLanguages = !allLanguages"
            >
              {{
                allLanguages
                  ? t('chips.fewerLanguages')
                  : t('chips.moreLanguages', { count: search.languages.length - LANGUAGES_SHOWN })
              }}
            </button>
          </div>
        </div>
        <div v-else class="facet" role="group" :aria-label="g.label">
          <h3 class="label" aria-hidden="true">{{ g.label }}</h3>
          <div class="chipset">
            <button
              v-for="chip in g.chips"
              :key="String(chip.value)"
              type="button"
              class="chip"
              :aria-pressed="chip.selected"
              :aria-label="chipLabel(chip.label, chip.count)"
              @click="toggle(chip)"
            >
              {{ chip.label }} <span class="n">{{ chip.count }}</span>
            </button>
          </div>
        </div>
      </template>
      <div v-if="featureGroup" class="facet" role="group" :aria-label="featureGroup.label">
        <h3 v-if="commonFeatures.length" class="label" aria-hidden="true">
          {{ featureGroup.label }}
        </h3>
        <template v-for="set in featureSets" :key="set.key">
          <button
            v-if="set.key === 'other'"
            type="button"
            class="expander"
            :aria-expanded="allFeatures"
            @click="allFeatures = !allFeatures"
          >
            <span class="fm-directional"><UiChevron :open="allFeatures" /></span>
            {{ t('chips.moreFeatures') }}
          </button>
          <div v-if="set.key === 'common' || allFeatures" class="chipset">
            <button
              v-for="chip in set.chips"
              :key="String(chip.value)"
              type="button"
              class="chip"
              :title="String(chip.value)"
              :aria-pressed="chip.selected"
              :aria-label="chipLabel(chip.label, chip.count)"
              @click="toggle(chip)"
            >
              {{ chip.label }} <span class="n">{{ chip.count }}</span>
            </button>
          </div>
        </template>
      </div>
    </UiDrawer>
  </section>
</template>

<style scoped>
.chips {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
}
.opener {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--fm-space-2);
  font-size: var(--fm-text-sm);
}
.opener[aria-expanded='true'] {
  background-color: var(--fm-hover);
}
/* A pill, and the digits line up. */
.count {
  border-radius: var(--fm-radius-lg);
  padding-inline: var(--fm-space-2);
  font-variant-numeric: tabular-nums;
}
.row {
  position: relative;
  display: flex;
  flex: 1 1 0;
  flex-wrap: nowrap;
  align-items: center;
  gap: var(--fm-space-1);
  min-width: 0;
}
/* Out of the row and of the keyboard's way, but still measured. */
.spilled {
  position: absolute;
  visibility: hidden;
}
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2);
  margin-bottom: var(--fm-space-4);
}
.label {
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.facet {
  margin-bottom: var(--fm-space-4);
}
.chipset {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1);
}
.more {
  border-color: transparent;
  color: var(--fm-muted);
}
.chip,
.name {
  padding: 2px 8px;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: transparent;
  color: inherit;
  font: inherit;
  font-size: var(--fm-text-sm);
}
.name {
  width: 140px;
  border-color: var(--fm-control-border);
}
.chip[aria-pressed='true'] {
  border-color: var(--fm-accent);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
}
.chip.active,
.more-chips {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: var(--fm-space-1);
  border-radius: var(--fm-radius-lg);
  white-space: nowrap;
}
.more-chips {
  color: var(--fm-muted);
}
.link {
  padding: 2px 4px;
  border-color: transparent;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.link:hover:not(:disabled) {
  color: inherit;
}
.link:disabled {
  opacity: 0.5;
  cursor: default;
}
.expander {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-2);
  margin-bottom: var(--fm-space-2);
  padding: 2px 0;
  border-color: transparent;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.n {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
}
.chip[aria-pressed='true'] .n {
  color: inherit;
}
.save {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  margin: 0;
}
.note:empty {
  display: none;
}
.note,
.error {
  margin: 0;
  font-size: var(--fm-text-sm);
}
.note {
  color: var(--fm-muted);
}
.error {
  color: var(--fm-danger);
}
</style>
