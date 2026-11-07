<script setup lang="ts">
// The line above the wall: how many families there are, which filters gave them, the one
// selected row with its state, and how they are sorted, with a menu to change that. The API
// does the sorting. With your own text past Basic Latin on the wall it says how many of the
// families render it completely (E43.1; the API checks the coverage), and the reading rows
// add their body size and line height.
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue';
import { errorMessage, t, type MessageKey } from '../i18n';
import { columnById, SORT_COLUMNS, type ColumnId } from '../list/columns';
import { rowStateLabel, styleName } from '../list/rows';
import { useActiveFilters } from '../search/activeFilters';
import { beyondBasicLatin } from '../preview/sampleText';
import { useFontListStore } from '../stores/fontList';
import { usePreviewStore } from '../stores/preview';
import { useSearchStore } from '../stores/search';
import { useViewsStore } from '../stores/views';

const props = defineProps<{
  view: string;
  /** The wall shows its reading rows (E43.1): the line names their body size. */
  rows?: boolean;
}>();

const search = useSearchStore();
const fontList = useFontListStore();
const views = useViewsStore();
const { labels } = useActiveFilters();
const uid = useId();

const selecting = computed(() => fontList.selectedCount > 1);
const selectedRow = computed(() =>
  fontList.selectedRows.length === 1 ? fontList.selectedRows[0]! : null,
);
const selectedName = computed(() => {
  const row = selectedRow.value;
  if (!row) return '';
  return row.kind === 'family' ? row.family.name : `${row.family.name} ${styleName(row)}`;
});
const familyCount = computed(() => (search.hits ? search.total : fontList.families.length));
/** Nothing to count before the first answer, or in an empty library (its empty state says so). */
const shown = computed(() => search.hits !== null || fontList.families.length > 0);

const preview = usePreviewStore();
const settings = computed(() => preview.settingsFor(props.view));
/** Your own text, when it has characters some faces may lack; else empty. */
const coverText = computed(() => {
  const s = settings.value;
  const text = s.sample === 'custom' ? s.customText.trim() : '';
  return beyondBasicLatin(text) ? text : '';
});
watch(coverText, (text) => search.setCoverText(text), { immediate: true });
onBeforeUnmount(() => search.setCoverText(''));
/** How many of the families render your text completely; null when not counted. */
const covering = computed(() =>
  coverText.value && search.coverText === coverText.value ? search.coverTotal : null,
);
const bodyText = computed(() =>
  t('home.bodySize', {
    size: settings.value.bodySizePx,
    lineHeight: settings.value.bodyLineHeight,
  }),
);

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
      <template v-else-if="shown && covering !== null">
        <strong data-testid="renders-text">{{
          t('home.rendersText.count', { covered: covering, count: familyCount })
        }}</strong>
        {{ t('home.rendersText.tail', { count: familyCount }) }}
      </template>
      <template v-else-if="shown">
        <strong>{{ t('home.families', { count: familyCount }) }}</strong>
        <template v-for="label in labels" :key="label"> · {{ label }}</template>
      </template>
    </p>
    <p v-if="selectedRow" class="selected-row" data-testid="selected-row">
      <bdi class="selected-name">{{ selectedName }}</bdi>
      <span aria-hidden="true">·</span>
      <span data-testid="selected-row-state">{{ rowStateLabel(selectedRow) }}</span>
    </p>
    <p v-if="rows" class="body" data-testid="body-size">{{ bodyText }}</p>
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
.selected-row {
  display: flex;
  flex: 0 1 auto;
  gap: var(--fm-space-1);
  min-width: 0;
  max-width: 50%;
  margin: 0;
  white-space: nowrap;
}
.selected-name {
  overflow: hidden;
  color: var(--fm-fg);
  font-weight: 500;
  text-overflow: ellipsis;
}
.body {
  flex: none;
  margin: 0;
  white-space: nowrap;
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
