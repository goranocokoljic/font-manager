<script setup lang="ts">
// The line above the wall: how many families there are, which filters gave them, and
// how they are sorted, with a menu to change that. The API does the sorting.
import { computed, nextTick, ref, useId } from 'vue';
import { errorMessage, t, type MessageKey } from '../i18n';
import { columnById, SORT_COLUMNS, type ColumnId } from '../list/columns';
import { useActiveFilters } from '../search/activeFilters';
import { useFontListStore } from '../stores/fontList';
import { useSearchStore } from '../stores/search';
import { useViewsStore } from '../stores/views';

const props = defineProps<{ view: string }>();

const search = useSearchStore();
const fontList = useFontListStore();
const views = useViewsStore();
const { labels } = useActiveFilters();
const uid = useId();

const selecting = computed(() => fontList.selectedCount > 1);
const familyCount = computed(() => (search.hits ? search.total : fontList.families.length));
/** Nothing to count before the first answer, or in an empty library (its empty state says so). */
const shown = computed(() => search.hits !== null || fontList.families.length > 0);

const sort = computed(() => views.tableFor(props.view).sort);
const typing = computed(() => search.query.trim() !== '');
/** The API's default: best match for a text search, else A to Z. */
const column = computed<ColumnId | null>(
  () => sort.value?.column ?? (typing.value ? null : 'name'),
);
const sortText = computed(() => {
  const key = t(`home.sort.keys.${column.value ?? 'bestMatch'}` as MessageKey);
  return t(sort.value?.descending ? 'home.sort.byDescending' : 'home.sort.by', { key });
});
/** A menu row stands for its API key: the table's Style column is the Weight row here. */
const isCurrent = (id: ColumnId) =>
  column.value !== null && columnById(column.value).sortKey === columnById(id).sortKey;

function choose(id: ColumnId | null): void {
  views.setSort(props.view, id, id !== null && !!sort.value?.descending);
}
function setDescending(on: boolean): void {
  views.setSort(props.view, column.value, on);
}

const open = ref(false);
const button = ref<HTMLButtonElement | null>(null);
const panel = ref<HTMLElement | null>(null);

async function toggle(): Promise<void> {
  open.value = !open.value;
  if (!open.value) return;
  await nextTick();
  const inputs = panel.value;
  (
    inputs?.querySelector<HTMLElement>('input:checked') ??
    inputs?.querySelector<HTMLElement>('input')
  )?.focus();
}
function close(): void {
  open.value = false;
  button.value?.focus();
}
function onFocusOut(e: FocusEvent): void {
  const next = e.relatedTarget as Node | null;
  if (next && (panel.value?.contains(next) || button.value?.contains(next))) return;
  open.value = false;
}
</script>

<template>
  <div v-show="shown || search.error" class="line" data-testid="count-line">
    <p v-if="search.error" role="alert" class="error">{{ errorMessage(search.error) }}</p>
    <p class="count" role="status" :title="labels.join(' · ') || undefined">
      <template v-if="selecting">{{
        t('home.selected', { count: fontList.selectedCount })
      }}</template>
      <template v-else-if="shown">
        <strong>{{ t('home.families', { count: familyCount }) }}</strong>
        <template v-for="label in labels" :key="label"> · {{ label }}</template>
      </template>
    </p>
    <div class="sort" @keydown.esc.stop="close">
      <span>{{ sortText }}</span> ·
      <button
        ref="button"
        type="button"
        class="change"
        :aria-label="t('home.sort.changeLabel')"
        :aria-expanded="open"
        :aria-controls="`${uid}-menu`"
        @click="toggle"
      >
        {{ t('home.sort.change') }}
      </button>
      <!-- tabindex: in WebKit a click on a radio does not focus it; the focus stays in here. -->
      <div
        v-if="open"
        :id="`${uid}-menu`"
        ref="panel"
        class="menu"
        role="group"
        tabindex="-1"
        :aria-label="t('home.sort.menu')"
        @focusout="onFocusOut"
      >
        <label v-if="typing">
          <input type="radio" :name="uid" :checked="column === null" @change="choose(null)" />
          {{ t('home.sort.bestMatch') }}
        </label>
        <label v-for="id in SORT_COLUMNS" :key="id">
          <input type="radio" :name="uid" :checked="isCurrent(id)" @change="choose(id)" />
          {{ columnById(id).label }}
        </label>
        <label class="direction">
          <input
            type="checkbox"
            :checked="!!sort?.descending"
            :disabled="column === null"
            @change="setDescending(($event.target as HTMLInputElement).checked)"
          />
          {{ t('home.sort.descending') }}
        </label>
      </div>
    </div>
  </div>
</template>

<style scoped>
.line {
  display: flex;
  flex: none;
  align-items: baseline;
  gap: var(--fm-space-4);
  padding: var(--fm-space-1) var(--fm-space-4);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.count {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.count strong {
  color: var(--fm-fg);
  font-weight: 500;
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
.sort {
  position: relative;
  flex: none;
  white-space: nowrap;
}
.change {
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  text-decoration: underline;
  cursor: pointer;
}
.change:hover {
  color: var(--fm-fg);
}
.menu {
  position: absolute;
  z-index: var(--fm-z-popover);
  top: calc(100% + 4px);
  inset-inline-end: 0;
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-1);
  min-width: 180px;
  padding: var(--fm-space-2);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  outline: none;
  background-color: var(--fm-surface-2);
  box-shadow: var(--fm-shadow-2);
  color: var(--fm-fg);
}
.menu label {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
}
.direction {
  margin-top: var(--fm-space-1);
  padding-top: var(--fm-space-2);
  border-top: 1px solid var(--fm-border);
}
</style>
