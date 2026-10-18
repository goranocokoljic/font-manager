<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import type { ApiError, SourceId } from '../api/types';
import {
  chipGroups,
  chosenLanguages,
  hasFilters,
  languageName,
  toggleChip,
  toggleLanguage,
  type Chip,
} from '../search/chips';
import { useSearchStore } from '../stores/search';
import { useSourcesStore } from '../stores/sources';

const search = useSearchStore();
const sources = useSourcesStore();

/** A source chip shows the folder's name. */
function sourceName(id: SourceId): string {
  const path = sources.sources.find((s) => s.id === id)?.path;
  return path?.split(/[\\/]/).filter(Boolean).pop() ?? `Source ${id}`;
}

const groups = computed(() => chipGroups(search.filters, search.counts, sourceName));
const languages = computed(() => chosenLanguages(search.filters));
const addable = computed(() => search.languages.filter((l) => !languages.value.includes(l.code)));
const filtering = computed(() => hasFilters(search.filters));

const countLabel = (n: number) => `${n} ${n === 1 ? 'family' : 'families'}`;

function toggle(chip: Chip): void {
  search.setFilters(toggleChip(search.filters, chip.facet, chip.value));
}

function onLanguage(e: Event): void {
  const select = e.target as HTMLSelectElement;
  if (select.value) search.setFilters(toggleLanguage(search.filters, select.value));
  select.value = '';
}

// Saving the chip set as a smart collection; the collections sidebar comes with E14.
const naming = ref(false);
const name = ref('');
const saved = ref('');
const saveError = ref<ApiError | null>(null);
const nameInput = ref<HTMLInputElement | null>(null);

async function startSave(): Promise<void> {
  naming.value = true;
  saved.value = '';
  saveError.value = null;
  await nextTick();
  nameInput.value?.focus();
}

async function save(): Promise<void> {
  try {
    const c = await search.saveAsSmartCollection(name.value);
    saved.value = `Saved “${c.name}” as a smart collection.`;
    saveError.value = null;
    naming.value = false;
    name.value = '';
  } catch (e) {
    saveError.value = e as ApiError;
  }
}

function cancelSave(): void {
  naming.value = false;
  saveError.value = null;
}
</script>

<template>
  <section class="chips" aria-label="Filters">
    <div v-for="g in groups" :key="g.facet" class="group" role="group" :aria-label="g.label">
      <span class="label" aria-hidden="true">{{ g.label }}</span>
      <button
        v-for="chip in g.chips"
        :key="String(chip.value)"
        type="button"
        class="chip"
        :aria-pressed="chip.selected"
        :aria-label="`${chip.label}, ${countLabel(chip.count)}`"
        @click="toggle(chip)"
      >
        {{ chip.label }} <span class="n">{{ chip.count }}</span>
      </button>
    </div>
    <div class="group" role="group" aria-label="Languages">
      <span class="label" aria-hidden="true">Language</span>
      <button
        v-for="code in languages"
        :key="code"
        type="button"
        class="chip"
        aria-pressed="true"
        :aria-label="`${languageName(search.languages, code)}, ${countLabel(search.total)}`"
        @click="search.setFilters(toggleLanguage(search.filters, code))"
      >
        {{ languageName(search.languages, code) }} <span class="n">{{ search.total }}</span>
      </button>
      <select v-if="addable.length" class="add" aria-label="Add a language" @change="onLanguage">
        <option value="">Add…</option>
        <option v-for="l in addable" :key="l.code" :value="l.code">{{ l.name }}</option>
      </select>
    </div>
    <div v-if="filtering" class="group actions">
      <button type="button" class="chip" @click="search.setFilters({})">Clear filters</button>
      <form v-if="naming" class="save" @submit.prevent="save" @keydown.esc.prevent="cancelSave">
        <input
          ref="nameInput"
          v-model="name"
          class="name"
          aria-label="Smart collection name"
          placeholder="Name"
          required
        />
        <button type="submit" class="chip">Save</button>
        <button type="button" class="chip" @click="cancelSave">Cancel</button>
      </form>
      <button v-else type="button" class="chip" @click="startSave">Save as smart collection</button>
    </div>
    <p v-if="search.error" role="alert" class="error">{{ search.error.message }}</p>
    <p v-if="saveError" role="alert" class="error">{{ saveError.message }}</p>
    <p class="note" role="status">{{ saved }}</p>
  </section>
</template>

<style scoped>
.chips {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 12px;
  max-height: 96px;
  padding: 8px 16px;
  overflow-y: auto;
  border-bottom: 1px solid var(--fm-border);
}
.group {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
}
.label {
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
}
.chip,
.add,
.name {
  padding: 2px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 12px;
  background: transparent;
  color: inherit;
  font: inherit;
  font-size: 12px;
}
.name {
  width: 140px;
  border-radius: 8px;
}
.chip[aria-pressed='true'] {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
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
  gap: 4px;
  margin: 0;
}
.note,
.error {
  margin: 0;
  font-size: 12px;
}
.note {
  color: var(--fm-muted);
}
.error {
  color: var(--fm-danger);
}
</style>
