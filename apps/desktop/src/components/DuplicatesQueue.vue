<script setup lang="ts">
// The queue of duplicate sets (E56.11, board 14 "Sets"): the sets still to decide on
// the left (320 px, virtualized, paged through the API), one set on the right with a
// card per copy. The level chips, the filters and the order go to the API; a decision
// goes into the draft plan through the API and the queue moves on to the set that took
// its place. Nothing moves on disk here.
//
// Keys (anywhere in the queue but a text field): ↑/↓ the previous and next set; 1–9 keep
// that copy and send the others that may leave to the trash; T, H, K the outcome of the
// copy with the focus (else the last one chosen by number); B keep all; N not
// duplicates; C compare two copies; Enter add to the plan and go on. When a set leaves
// the queue the focus goes to the row now in its place, never to the page. The
// comparison (E56.13) takes the cards' place; Escape closes it and gives the focus back
// to the Compare button.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type {
  CopyChoice,
  DuplicateSet,
  DuplicateSetSort,
  FontFormat,
  LanguagePreset,
  SetDecision,
} from '../api/types';
import { errorMessage, t } from '../i18n';
import { poolSize, scrollToShow, windowRange } from '../list/virtual';
import { fadeIn } from '../motion/fadeIn';
import { useDuplicatesStore, type QueueChip } from '../stores/duplicates';
import { useSourcesStore } from '../stores/sources';
import DuplicateCompare from './DuplicateCompare.vue';
import DuplicateCopyCard from './DuplicateCopyCard.vue';
import UiButton from './ui/UiButton.vue';
import UiEmptyState from './ui/UiEmptyState.vue';
import UiSelect from './ui/UiSelect.vue';

const props = defineProps<{
  languages: LanguagePreset[];
  trash: string;
}>();

const route = useRoute();
const router = useRouter();
const store = useDuplicatesStore();
const sources = useSourcesStore();

const ROW_PX = 64;
const OVERSCAN = 4;
const viewport = ref<HTMLElement | null>(null);
const panel = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(600);
let observer: ResizeObserver | undefined;

function measure(): void {
  const el = viewport.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  if (el.clientHeight > 0) height.value = el.clientHeight;
}

const count = computed(() => store.total ?? 0);
const range = computed(() =>
  windowRange(scrollTop.value, height.value, ROW_PX, OVERSCAN, count.value),
);
const pool = computed(() => poolSize(height.value, ROW_PX, OVERSCAN));
const rows = computed(() => {
  const out: { index: number; slot: number; set: DuplicateSet | undefined }[] = [];
  for (let i = range.value.start; i < range.value.end; i++)
    out.push({ index: i, slot: i % pool.value, set: store.at(i) });
  return out;
});
watch(range, (r) => {
  if (r.end > r.start) {
    void store.ensure(r.start);
    void store.ensure(r.end - 1);
  }
});

const levelName = (s: DuplicateSet) =>
  s.level === 'sameVersion' && s.formatsDiffer
    ? t('duplicatesModule.queue.level.formatsDiffer', {
        formats: [...new Set(s.copies.map((c) => c.format).filter((f) => f != null))]
          .map((f) => f!.toUpperCase())
          .join(' · '),
      })
    : t(`duplicatesModule.queue.level.${s.level}`);
const setName = (s: DuplicateSet) =>
  s.postscriptName ?? s.copies[0]?.path.split(/[\\/]/).pop() ?? s.id;

// Level chips: All, then each level the queue holds, then "Keep both".
type Chip = QueueChip;
const chip = computed(() => store.chip);
const chips = computed(() => {
  const o = store.overview;
  return [
    { id: 'all' as Chip, label: t('duplicatesModule.queue.all') },
    {
      id: 'otherVersion' as Chip,
      label: t('duplicatesModule.queue.chip', {
        name: t('duplicatesModule.queue.level.otherVersion'),
        count: o?.otherVersion.sets ?? 0,
      }),
    },
    {
      id: 'sameVersion' as Chip,
      label: t('duplicatesModule.queue.chip', {
        name: t('duplicatesModule.queue.level.sameVersion'),
        count: o?.sameVersion.sets ?? 0,
      }),
    },
    {
      id: 'keepBoth' as Chip,
      label: t('duplicatesModule.queue.chip', {
        name: t('duplicatesModule.overview.keepBoth'),
        count: o?.keepBoth ?? 0,
      }),
    },
  ];
});
function pickChip(id: Chip): void {
  void store.showChip(id);
}

const sortOptions = computed(() => [
  { value: 'name' as DuplicateSetSort, label: t('duplicatesModule.queue.byName') },
  { value: 'frees' as DuplicateSetSort, label: t('duplicatesModule.queue.byFrees') },
  { value: 'copies' as DuplicateSetSort, label: t('duplicatesModule.queue.byCopies') },
]);
const FORMATS: FontFormat[] = ['otf', 'ttf', 'ttc', 'woff2', 'woff', 'type1'];
const formatOptions = computed(() => [
  { value: 'any', label: t('duplicatesModule.queue.anyFormat') },
  ...FORMATS.map((f) => ({ value: f, label: t(`info.formats.${f}`) })),
]);
const sourceOptions = computed(() => [
  { value: 'any', label: t('duplicatesModule.queue.anySource') },
  ...sources.sources.map((s) => ({ value: String(s.id), label: s.path })),
]);
const scopeSource = computed(() =>
  store.filter.scope.kind === 'source' ? String(store.filter.scope.source) : 'any',
);
const folderText = ref(store.filter.scope.kind === 'folder' ? store.filter.scope.path : '');
function pickSource(v: string): void {
  folderText.value = '';
  void store.setFilter({
    scope: v === 'any' ? { kind: 'library' } : { kind: 'source', source: Number(v) },
  });
}
function applyFolder(): void {
  const path = folderText.value.trim();
  void store.setFilter({ scope: path ? { kind: 'folder', path } : { kind: 'library' } });
}
/** Leaves one family's sets (E56.15): the whole queue again, and `?family=` off the URL. */
async function showAll(): Promise<void> {
  const query = { ...route.query };
  delete query.family;
  await router.replace({ query });
  await store.setFilter({ ids: null });
}
const filtersOpen = ref(false);
const filtered = computed(
  () =>
    store.filter.format != null ||
    store.filter.scope.kind !== 'library' ||
    store.filter.recommendation === 'hasRecommendation',
);

// The set in view.
const set = computed(() => store.current);
const keptCount = computed(() => {
  const s = set.value;
  if (!s) return 0;
  return s.copies.filter((c) => (store.choices[c.path] ?? 'keep') === 'keep').length;
});
const leaving = computed(() => {
  const s = set.value;
  if (!s) return { trash: 0, hide: 0 };
  const of = (c: CopyChoice) => s.copies.filter((x) => store.choices[x.path] === c).length;
  return { trash: of('trash'), hide: of('hide') };
});
const sample = computed(() => {
  const lines = props.languages
    .map((l) => l.pangram ?? l.sampleGlyphs)
    .filter((x): x is string => !!x)
    .slice(0, 2);
  return lines.length ? lines.join(' · ') : 'Hamburgefonstiv 0123';
});
/** The copy keys T, H and K act on when the focus is on no card. */
const focusedCopy = ref(0);

watch(
  () => store.current?.id,
  async () => {
    focusedCopy.value = 0;
    await nextTick();
    fadeIn(panel.value, 'fast');
  },
);

async function focusRow(index: number): Promise<void> {
  const el = viewport.value;
  if (el) {
    el.scrollTop = scrollToShow(index, ROW_PX, el.scrollTop, el.clientHeight || height.value);
    measure();
  }
  await nextTick();
  const row = el?.querySelector<HTMLElement>(`[data-index="${index}"]`);
  if (row) row.focus();
  else (document.getElementById('queue-heading') as HTMLElement | null)?.focus();
}

async function go(index: number, focus = true): Promise<void> {
  if (!count.value) return;
  await store.select(Math.max(0, Math.min(index, count.value - 1)));
  if (focus) await focusRow(store.cursor);
}

async function decide(kind: SetDecision['kind']): Promise<void> {
  if (!set.value) return;
  if (kind === 'copies' && keptCount.value === 0) return;
  const done = await store.decide(kind);
  if (done) await focusRow(store.cursor);
}

/** The copy index of the card that holds the focus, else the last one chosen by number. */
function copyIndexOfFocus(): number {
  const card = (document.activeElement as HTMLElement | null)?.closest('[data-copy-index]');
  return card ? Number(card.getAttribute('data-copy-index')) : focusedCopy.value;
}

function isTextField(el: EventTarget | null): boolean {
  if (!(el instanceof HTMLElement)) return false;
  return (
    el.isContentEditable ||
    el instanceof HTMLInputElement ||
    el instanceof HTMLTextAreaElement ||
    !!el.closest('[role="listbox"]:not(.rows), [role="menu"], [role="dialog"]')
  );
}

function onKey(e: KeyboardEvent): void {
  if (e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey || isTextField(e.target)) return;
  const s = set.value;
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  const inSegments = (e.target as HTMLElement | null)?.closest('[data-testid="copy-outcome"]');
  if ((key === 'ArrowDown' || key === 'ArrowUp') && !inSegments) {
    e.preventDefault();
    void go(store.cursor + (key === 'ArrowDown' ? 1 : -1));
    return;
  }
  if (!s) return;
  if (/^[1-9]$/.test(key)) {
    const i = Number(key) - 1;
    const copy = s.copies[i];
    if (!copy) return;
    e.preventDefault();
    focusedCopy.value = i;
    store.keepOnly(copy.path);
    return;
  }
  const outcome: Record<string, CopyChoice> = { t: 'trash', h: 'hide', k: 'keep' };
  if (outcome[key]) {
    const copy = s.copies[copyIndexOfFocus()];
    if (!copy) return;
    e.preventDefault();
    store.choose(copy.path, outcome[key]);
    return;
  }
  if (key === 'b') {
    e.preventDefault();
    void decide('keepAll');
  } else if (key === 'n') {
    e.preventDefault();
    void decide('notDuplicates');
  } else if (key === 'c') {
    e.preventDefault();
    void store.compare(s.copies[copyIndexOfFocus()]?.path);
  } else if (key === 'Enter') {
    const target = e.target as HTMLElement | null;
    if (target?.closest('button, a')) return;
    e.preventDefault();
    void decide('copies');
  }
}

function onRowKey(e: KeyboardEvent, index: number): void {
  // Enter is "add to the plan" (the queue's key); Space shows the row's set.
  if (e.key === ' ') {
    e.preventDefault();
    void go(index, false);
  } else if (e.key === 'Home' || e.key === 'End') {
    e.preventDefault();
    void go(e.key === 'Home' ? 0 : count.value - 1);
  }
}

const compareButton = ref<InstanceType<typeof UiButton> | null>(null);
async function closeComparison(): Promise<void> {
  store.comparison = null;
  await nextTick();
  (compareButton.value?.$el as HTMLElement | undefined)?.focus();
}

onMounted(() => {
  measure();
  if (typeof ResizeObserver !== 'undefined' && viewport.value) {
    observer = new ResizeObserver(measure);
    observer.observe(viewport.value);
  }
  // The module view loads the queue when it opens; the queue only shows it.
  void sources.load();
});
onBeforeUnmount(() => observer?.disconnect());
</script>

<template>
  <div class="queue" data-testid="duplicates-queue" @keydown="onKey">
    <section class="list" :aria-labelledby="'queue-heading'">
      <div class="list-head">
        <h2 id="queue-heading" class="heading" tabindex="-1">
          {{ t('duplicatesModule.queue.heading', { count }) }}
        </h2>
        <UiSelect
          class="sort"
          :model-value="store.sort"
          :options="sortOptions"
          :aria-label="t('duplicatesModule.queue.sort')"
          @update:model-value="store.setSort($event as DuplicateSetSort)"
        />
      </div>
      <div class="chips" role="group" :aria-label="t('duplicatesModule.queue.levels')">
        <button
          v-for="c in chips"
          :key="c.id"
          type="button"
          class="chip"
          :aria-pressed="chip === c.id"
          @click="pickChip(c.id)"
        >
          {{ c.label }}
        </button>
        <button
          type="button"
          class="chip"
          :aria-expanded="filtersOpen"
          aria-controls="queue-filters"
          :aria-pressed="filtered"
          @click="filtersOpen = !filtersOpen"
        >
          {{ t('duplicatesModule.queue.filters') }}
        </button>
      </div>
      <p v-if="store.filter.ids" class="family-filter" data-testid="queue-family">
        {{ t('duplicatesModule.queue.family', { count: store.filter.ids.length }) }}
        <button type="button" class="link-button" @click="showAll">
          {{ t('duplicatesModule.queue.allSets') }}
        </button>
      </p>
      <div v-if="filtersOpen" id="queue-filters" class="filters">
        <label class="field">
          <span>{{ t('duplicatesModule.queue.source') }}</span>
          <UiSelect
            :model-value="scopeSource"
            :options="sourceOptions"
            :aria-label="t('duplicatesModule.queue.source')"
            @update:model-value="pickSource($event as string)"
          />
        </label>
        <label class="field">
          <span>{{ t('duplicatesModule.queue.folder') }}</span>
          <input
            v-model="folderText"
            class="text"
            type="text"
            :placeholder="t('duplicatesModule.queue.folderPlaceholder')"
            @keydown.enter.prevent="applyFolder"
            @change="applyFolder"
          />
        </label>
        <label class="field">
          <span>{{ t('duplicatesModule.queue.format') }}</span>
          <UiSelect
            :model-value="store.filter.format ?? 'any'"
            :options="formatOptions"
            :aria-label="t('duplicatesModule.queue.format')"
            @update:model-value="
              store.setFilter({ format: $event === 'any' ? null : ($event as FontFormat) })
            "
          />
        </label>
        <label class="check">
          <input
            type="checkbox"
            :checked="store.filter.recommendation === 'hasRecommendation'"
            @change="
              store.setFilter({
                recommendation: ($event.target as HTMLInputElement).checked
                  ? 'hasRecommendation'
                  : null,
                level: store.filter.level,
              })
            "
          />
          {{ t('duplicatesModule.queue.recommended') }}
        </label>
      </div>
      <div
        ref="viewport"
        class="rows"
        role="listbox"
        :aria-label="t('duplicatesModule.queue.label')"
        @scroll="measure"
      >
        <div class="spacer" :style="{ height: `${count * ROW_PX}px` }">
          <div
            v-for="row in rows"
            :key="row.slot"
            role="option"
            class="row"
            :data-index="row.index"
            :aria-selected="row.index === store.cursor"
            :aria-posinset="row.index + 1"
            :aria-setsize="count"
            :tabindex="row.index === store.cursor ? 0 : -1"
            :style="{ transform: `translateY(${row.index * ROW_PX}px)` }"
            @click="go(row.index, false)"
            @keydown="onRowKey($event, row.index)"
          >
            <template v-if="row.set">
              <span class="row-text">
                <span class="row-name">{{ setName(row.set) }}</span>
                <span class="row-level">{{ levelName(row.set) }}</span>
              </span>
              <span class="row-count">
                {{ t('duplicatesModule.queue.copies', { count: row.set.copies.length }) }}
              </span>
            </template>
            <span v-else class="row-level">{{ t('duplicatesModule.queue.loading') }}</span>
          </div>
        </div>
      </div>
    </section>

    <section
      v-if="set"
      ref="panel"
      class="set"
      :aria-label="t('duplicatesModule.set.region', { name: setName(set) })"
      data-testid="duplicate-set"
    >
      <header class="set-head">
        <div>
          <h2 class="set-title">{{ setName(set) }}</h2>
          <p class="set-sub">
            {{ t('duplicatesModule.set.copies', { count: set.copies.length }) }} ·
            {{ levelName(set) }} ·
            {{ t('duplicatesModule.set.position', { n: store.cursor + 1, total: count }) }}
          </p>
        </div>
        <UiButton
          ref="compareButton"
          class="outlined"
          aria-keyshortcuts="C"
          @click="store.compare()"
        >
          {{ t('duplicatesModule.set.compare') }}
          <kbd>{{ t('duplicatesModule.set.compareKey') }}</kbd>
        </UiButton>
      </header>
      <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
      <DuplicateCompare
        v-if="store.comparison"
        :key="`${store.comparison.a.path}|${store.comparison.b.path}`"
        :set="set"
        :comparison="store.comparison"
        :languages="languages"
        :presets="store.presets"
        :sample="sample"
        :choices="store.choices"
        :keeper="store.keeper"
        :fixed="store.fixed"
        :trash="trash"
        @pick="(a, b) => store.compare(b, a)"
        @keep-only="store.keepOnly"
        @choose="store.choose"
        @decide="decide"
        @close="closeComparison"
      />
      <div v-else class="cards">
        <div
          v-for="(copy, i) in set.copies"
          :key="copy.path"
          :data-copy-index="i"
          @focusin="focusedCopy = i"
        >
          <DuplicateCopyCard
            :copy="copy"
            :index="i + 1"
            :file="store.fileOf(copy.path)"
            :choice="store.choices[copy.path] ?? 'keep'"
            :recommended="copy.path === store.keeper"
            :reason="store.reason"
            :fixed="store.fixed(copy.path)"
            :languages="languages"
            :sample="sample"
            :trash="trash"
            @choose="store.choose(copy.path, $event)"
          />
        </div>
      </div>
      <footer class="actions">
        <!-- The comparison offers these two itself, as Keep both and Not duplicates. -->
        <template v-if="!store.comparison">
          <UiButton aria-keyshortcuts="N" @click="decide('notDuplicates')">
            {{ t('duplicatesModule.set.notDuplicates') }}
            <kbd>{{ t('duplicatesModule.set.notDuplicatesKey') }}</kbd>
          </UiButton>
          <UiButton aria-keyshortcuts="B" @click="decide('keepAll')">
            {{ t('duplicatesModule.set.keepAll') }}
            <kbd>{{ t('duplicatesModule.set.keepAllKey') }}</kbd>
          </UiButton>
        </template>
        <span class="summary" aria-live="polite">
          <template v-if="keptCount === 0">{{ t('duplicatesModule.set.noKeeper') }}</template>
          <template v-else>
            {{ t('duplicatesModule.set.leaving', { count: leaving.trash, trash }) }}
            <template v-if="leaving.hide">
              · {{ t('duplicatesModule.set.hiding', { count: leaving.hide }) }}
            </template>
          </template>
        </span>
        <button
          type="button"
          class="primary"
          aria-keyshortcuts="Enter"
          :aria-disabled="keptCount === 0"
          @click="decide('copies')"
        >
          {{ t('duplicatesModule.set.add') }} <kbd>{{ t('duplicatesModule.set.addKey') }}</kbd>
        </button>
      </footer>
      <p class="keys">{{ t('duplicatesModule.set.keys') }}</p>
    </section>
    <div v-else class="set empty">
      <UiEmptyState
        :title="
          filtered || chip !== 'all'
            ? t('duplicatesModule.queue.emptyFiltered')
            : t('duplicatesModule.queue.empty')
        "
        :body="filtered || chip !== 'all' ? undefined : t('duplicatesModule.queue.emptyBody')"
      />
    </div>
  </div>
</template>

<style scoped>
.queue {
  display: flex;
  flex: 1;
  min-width: 0;
  min-height: 0;
}
.list {
  display: flex;
  flex-direction: column;
  flex: none;
  width: 320px;
  border-inline-end: 1px solid var(--fm-border);
}
.list-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-2);
  padding: var(--fm-space-4) var(--fm-space-4) var(--fm-space-2);
}
.heading {
  margin: 0;
  font-size: var(--fm-text-md);
  font-weight: 600;
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1);
  padding: 0 var(--fm-space-4) var(--fm-space-3);
}
.family-filter {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--fm-space-2);
  margin: var(--fm-space-2) 0 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.link-button {
  padding: 0;
  border: 0;
  background: none;
  color: var(--fm-fg);
  font: inherit;
  text-decoration: underline;
  text-underline-offset: 2px;
}
.chip {
  height: 28px;
  padding: 0 var(--fm-space-3);
  border: 1px solid var(--fm-control-border);
  border-radius: 999px;
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
}
.chip[aria-pressed='true'] {
  border-color: var(--fm-accent);
  box-shadow: inset 0 0 0 1px var(--fm-accent);
  background-color: var(--fm-hover);
  font-weight: 600;
}
.filters {
  display: grid;
  gap: var(--fm-space-2);
  padding: 0 var(--fm-space-4) var(--fm-space-3);
}
.field {
  display: grid;
  gap: var(--fm-space-1);
  font-size: var(--fm-text-sm);
  color: var(--fm-muted);
}
.text {
  height: var(--fm-control-height);
  padding: 0 var(--fm-space-2);
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
}
.check {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  font-size: var(--fm-text-sm);
}
.rows {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  border-top: 1px solid var(--fm-border);
}
.spacer {
  position: relative;
}
.row {
  position: absolute;
  inset-inline: 0;
  top: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-2);
  height: 64px;
  padding: 0 var(--fm-space-4);
  border-inline-start: 3px solid transparent;
  cursor: default;
}
.row:hover {
  background-color: var(--fm-hover);
}
.row[aria-selected='true'] {
  border-inline-start-color: var(--fm-accent);
  background-color: var(--fm-hover);
}
.row:focus-visible {
  outline-offset: -2px;
}
.row-text {
  display: grid;
  min-width: 0;
}
.row-name {
  overflow: hidden;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.row-level,
.row-count {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.row-count {
  flex: none;
}
.set {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--fm-space-3);
  min-width: 0;
  padding: var(--fm-space-5) var(--fm-space-6);
  overflow-y: auto;
}
.set.empty {
  justify-content: center;
}
.set-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--fm-space-3);
}
.set-title {
  margin: 0;
  font-size: var(--fm-text-xl);
}
.set-sub {
  margin: var(--fm-space-1) 0 0;
  color: var(--fm-muted);
}
.outlined {
  height: 32px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  color: var(--fm-fg);
}
kbd {
  padding: 0 4px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-xs);
}
.cards {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
}
.actions {
  display: flex;
  align-items: center;
  gap: var(--fm-space-3);
  margin-top: auto;
  padding-top: var(--fm-space-3);
  border-top: 1px solid var(--fm-border);
}
.summary {
  margin-inline-start: auto;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.primary {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-2);
  height: 36px;
  padding: 0 var(--fm-space-4);
  border: 1px solid var(--fm-accent);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font: inherit;
  font-weight: 600;
}
.primary kbd {
  border-color: currentColor;
}
.primary[aria-disabled='true'] {
  opacity: 0.55;
}
.keys {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
</style>
