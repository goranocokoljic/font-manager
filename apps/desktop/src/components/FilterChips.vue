<script setup lang="ts">
import { computed, nextTick, ref } from 'vue';
import type { ApiError, SourceId } from '../api/types';
import {
  chipGroups,
  chosenLanguages,
  hasChosenFilters,
  hasFilters,
  languageName,
  toggleChip,
  toggleLanguage,
  type Chip,
} from '../search/chips';
import { useCollectionsStore } from '../stores/collections';
import { useSearchStore } from '../stores/search';
import { useSourcesStore } from '../stores/sources';
import { useTagsStore } from '../stores/tags';

const search = useSearchStore();
const collections = useCollectionsStore();
const sources = useSourcesStore();
const tags = useTagsStore();

/** A source chip shows the folder's name. */
function sourceName(id: SourceId): string {
  const path = sources.sources.find((s) => s.id === id)?.path;
  return path?.split(/[\\/]/).filter(Boolean).pop() ?? `Source ${id}`;
}

const groups = computed(() => chipGroups(search.filters, search.counts, sourceName, tags.nameOf));
const languages = computed(() => chosenLanguages(search.filters));
const addable = computed(() => search.languages.filter((l) => !languages.value.includes(l.code)));
const filtering = computed(() => hasFilters(search.filters));
/** A smart collection needs a chosen filter; the open collection alone is none. */
const savable = computed(() => hasChosenFilters(search.filters));

const countLabel = (n: number) => `${n} ${n === 1 ? 'family' : 'families'}`;

function toggle(chip: Chip): void {
  search.setFilters(toggleChip(search.filters, chip.facet, chip.value));
}

function onLanguage(e: Event): void {
  const select = e.target as HTMLSelectElement;
  if (select.value) search.setFilters(toggleLanguage(search.filters, select.value));
  select.value = '';
}

// Saving the chosen filters as a smart collection (E14.4): the rules are the
// filters as they are, so the collection lists the families the chips gave.
const naming = ref(false);
const name = ref('');
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
    saved.value = `Saved “${c.name}” as a smart collection.`;
    await endSave();
    // The sidebar lists it, with its count.
    await collections.refresh();
  } catch (e) {
    saveError.value = e as ApiError;
  }
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
    <div class="group actions">
      <button v-if="filtering" type="button" class="chip" @click="search.setFilters({})">
        Clear filters
      </button>
      <form
        v-if="naming"
        class="save"
        aria-label="Save as smart collection"
        @submit.prevent="save"
        @keydown.esc.prevent="endSave"
      >
        <input
          ref="nameInput"
          v-model="name"
          class="name"
          aria-label="Smart collection name"
          placeholder="Name"
          :aria-invalid="saveError ? 'true' : undefined"
          :aria-describedby="saveError ? 'fm-save-smart-error' : undefined"
          @input="saveError = null"
        />
        <button type="submit" class="chip">Save</button>
        <button type="button" class="chip" @click="endSave">Cancel</button>
        <p v-if="saveError" id="fm-save-smart-error" role="alert" class="error">
          {{ saveError.message }}
        </p>
      </form>
      <button
        v-else
        ref="saveButton"
        type="button"
        class="chip"
        :disabled="!savable"
        :title="savable ? undefined : 'Choose a filter to save it as a smart collection'"
        @click="startSave"
      >
        Save as smart collection
      </button>
    </div>
    <p v-if="search.error" role="alert" class="error">{{ search.error.message }}</p>
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
.chip:disabled {
  color: var(--fm-muted);
  cursor: default;
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
