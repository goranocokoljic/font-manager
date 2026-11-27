<script setup lang="ts">
// The Problems view (board 16, E57.9): the shell of Settings and Duplicates. A 232 px
// navigation with the title, "All" and the groups of `PROBLEM_GROUPS` with their counts (a
// vertical tab list; a group with nothing is muted and can still be opened), and beside it
// the panel: the count in words, then the problems grouped under small-caps headings, each a
// row in its group's card with one action. The filter is in the URL
// (`#/problems?kind=legacy`), so the sidebar's link and old links open it filtered; the old
// link `?kind=duplicates` opens the Duplicates module (router.ts). Duplicates are not
// problems: one line at the foot of the panel counts the fonts with copies and opens it.
// The rows come ready from the problems store; nothing about a problem is computed here.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import type { ConflictSide } from '../api/types';
import { copyText } from '../api/clipboard';
import UiIcon from '../components/ui/UiIcon.vue';
import UiIconButton from '../components/ui/UiIconButton.vue';
import { errorMessage, formatNumber, t } from '../i18n';
import { scrollToShow, windowRange } from '../list/virtual';
import { useCommandsStore } from '../stores/commands';
import { useDuplicateSummaryStore } from '../stores/duplicateSummary';
import {
  PROBLEM_GROUPS,
  problemGroup,
  useProblemsStore,
  type ProblemAction,
  type ProblemGroupId,
  type ProblemListRow,
  type ProblemRow,
  type ProblemSection,
} from '../stores/problems';
import { useToastStore } from '../stores/toast';

const store = useProblemsStore();
const dupes = useDuplicateSummaryStore();
const commands = useCommandsStore();
const toasts = useToastStore();
const route = useRoute();
const router = useRouter();

/** The rows of a group "All" shows; "N more" opens the group (board 16). */
const PREVIEW = 2;
/** A group's own panel windows its rows from this many on (fixed-height rows). */
const VIRTUAL_FROM = 50;
const ROW_PX = 84;
const OVERSCAN = 4;

type TabId = ProblemGroupId | 'all';

const filter = computed(() => problemGroup(route.query.kind)?.id);
const current = computed<TabId>(() => filter.value ?? 'all');
const tabs = computed(() => [
  { id: 'all' as TabId, label: t('problems.all'), count: store.total },
  ...PROBLEM_GROUPS.map((g) => ({ id: g.id as TabId, label: g.label, count: store.counts[g.id] })),
]);

const sections = computed(() => store.sections(filter.value));
const empty = computed(() => store.loaded && !store.error && store.total === 0);
/** What the heading counts: every problem, or the open group's. */
const shownCount = computed(() => (filter.value ? store.counts[filter.value] : store.total));
const headingLine = computed(() =>
  shownCount.value > 0
    ? t('problems.headingLine')
    : store.total === 0
      ? t('problems.emptyLine')
      : t('problems.groupNone'),
);

/** The rows a section shows here: a preview in "All", every row in the group's own panel. */
function shown(s: ProblemSection): ProblemListRow[] {
  return filter.value ? s.rows : s.rows.slice(0, PREVIEW);
}

/** A group's own panel with many plain rows: only the rows in view exist. */
function windowed(s: ProblemSection): boolean {
  return (
    !!filter.value && s.rows.length >= VIRTUAL_FROM && s.rows.every((r) => r.type === 'problem')
  );
}

// --- The tab list: as in Settings, arrows move between groups, Home and End jump. ---

const focusTab = (id: TabId) => document.getElementById(`problems-tab-${id}`)?.focus();
const focusHeading = () => document.getElementById('problems-heading')?.focus();
let focusAfter: 'tab' | 'heading' | null = null;

async function select(id: TabId, focus: 'tab' | 'heading'): Promise<void> {
  if (id === current.value) {
    if (focus === 'heading') focusHeading();
    return;
  }
  focusAfter = focus;
  await router.replace({ query: id === 'all' ? {} : { kind: id } });
}

watch(current, async (id) => {
  const how = focusAfter;
  focusAfter = null;
  scrollTop.value = 0;
  await nextTick();
  if (how === 'tab') focusTab(id);
  else if (how === 'heading') focusHeading();
});

function onTabKey(e: KeyboardEvent): void {
  const list = tabs.value;
  const i = list.findIndex((s) => s.id === current.value);
  const last = list.length - 1;
  const to =
    e.key === 'ArrowDown'
      ? (i + 1) % list.length
      : e.key === 'ArrowUp'
        ? (i + last) % list.length
        : e.key === 'Home'
          ? 0
          : e.key === 'End'
            ? last
            : -1;
  if (to < 0) return;
  e.preventDefault();
  void select(list[to]!.id, 'tab');
}

// --- The window of a long group. ---

const scroller = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewHeight = ref(600);
let observer: ResizeObserver | undefined;

function range(count: number) {
  return windowRange(scrollTop.value, viewHeight.value, ROW_PX, OVERSCAN, count);
}

function windowRows(s: ProblemSection): { row: ProblemListRow; index: number }[] {
  const { start, end } = range(s.rows.length);
  return s.rows.slice(start, end).map((row, i) => ({ row, index: start + i }));
}

function setScroller(el: unknown): void {
  scroller.value = (el as HTMLElement | null) ?? null;
}

function onScroll(): void {
  scrollTop.value = scroller.value?.scrollTop ?? 0;
}

watch(scroller, (el) => {
  observer?.disconnect();
  if (!el) return;
  viewHeight.value = el.clientHeight || viewHeight.value;
  if (typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(() => (viewHeight.value = el.clientHeight || viewHeight.value));
    observer.observe(el);
  }
});

// --- Rows: arrows move between the rows' actions, as Home and End do. ---

function rowAction(group: ProblemGroupId, index: number): HTMLElement | null {
  return document.querySelector<HTMLElement>(
    `[data-group="${group}"] [data-row-index="${index}"] [data-row-action]`,
  );
}

async function focusRow(s: ProblemSection, index: number): Promise<void> {
  const count = shown(s).length;
  const to = Math.max(0, Math.min(count - 1, index));
  if (windowed(s) && scroller.value) {
    const top = scrollToShow(to, ROW_PX, scroller.value.scrollTop, scroller.value.clientHeight);
    scroller.value.scrollTop = top;
    scrollTop.value = top;
    await nextTick();
  }
  rowAction(s.group.id, to)?.focus();
}

function onRowKey(e: KeyboardEvent, s: ProblemSection, index: number): void {
  const last = shown(s).length - 1;
  const to =
    e.key === 'ArrowDown'
      ? index + 1
      : e.key === 'ArrowUp'
        ? index - 1
        : e.key === 'Home'
          ? 0
          : e.key === 'End'
            ? last
            : null;
  if (to === null || to < 0 || to > last) return;
  e.preventDefault();
  void focusRow(s, to);
}

// --- Actions. ---

/** The visible text and the accessible name of a row's action. */
const actionText = (a: ProblemAction) => t(`problems.actions.${a}`);
const actionLabel = (row: ProblemRow) =>
  t(`problems.actions.${row.action}Label`, { name: row.name });

async function act(row: ProblemRow): Promise<void> {
  if (row.action === 'openSources') {
    await router.push({ name: 'settings', query: { section: 'sources' } });
  } else if (row.action === 'decide') {
    await router.push({ name: 'conflicts' });
  } else {
    try {
      await copyText(row.path);
      toasts.show(t('problems.copied'));
    } catch {
      toasts.show(t('problems.copyFailed'));
    }
  }
}

/** The first row's action of a group, else its heading. */
function groupStart(id: ProblemGroupId): HTMLElement | null {
  return rowAction(id, 0) ?? document.getElementById(`problems-group-${id}`);
}

/**
 * Keeps one value of a sync conflict. Its row goes; the focus moves to the row that takes
 * its place (or the row before it), else to the group's heading. When the group is gone
 * ("All" lists only groups with problems), it moves to the group that took its place, else
 * the one before, else the panel's heading.
 */
async function resolve(s: ProblemSection, index: number, keep: ConflictSide): Promise<void> {
  const row = shown(s)[index];
  if (row?.type !== 'conflict') return;
  const at = sections.value.findIndex((x) => x.group.id === s.group.id);
  await store.resolveConflict(row.conflict.id, keep);
  await nextTick();
  if (store.sidecarConflicts.some((c) => c.id === row.conflict.id)) return;
  const id = s.group.id;
  const inGroup =
    rowAction(id, index) ??
    rowAction(id, index - 1) ??
    document.getElementById(`problems-group-${id}`);
  const after = sections.value[at] ?? sections.value[at - 1];
  (
    inGroup ??
    (after && groupStart(after.group.id)) ??
    document.getElementById('problems-heading')
  )?.focus();
}

// --- Closing: × and Esc, as in Settings. ---

function close(): void {
  void router.push({ name: 'home' });
}

const root = ref<HTMLElement | null>(null);

function onKey(e: KeyboardEvent): void {
  if (e.key !== 'Escape' || commands.open || e.defaultPrevented) return;
  const target = e.target as Node | null;
  const ours = target === document.body || target === document.documentElement;
  if (!ours && !(target && root.value?.contains(target))) return;
  e.preventDefault();
  close();
}

onMounted(() => {
  window.addEventListener('keydown', onKey);
  focusTab(current.value);
  void dupes.start();
  return store.load();
});
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey);
  observer?.disconnect();
  void store.stop();
});
</script>

<template>
  <div ref="root" class="problems">
    <nav class="nav" aria-labelledby="problems-title">
      <h1 id="problems-title" class="title">{{ t('problems.title') }}</h1>
      <div
        role="tablist"
        aria-orientation="vertical"
        aria-labelledby="problems-title"
        class="tabs"
        @keydown="onTabKey"
      >
        <button
          v-for="tab in tabs"
          :id="`problems-tab-${tab.id}`"
          :key="tab.id"
          type="button"
          role="tab"
          class="tab"
          :class="{ none: tab.id !== 'all' && tab.count === 0, all: tab.id === 'all' }"
          :aria-selected="current === tab.id"
          aria-controls="problems-panel"
          :tabindex="current === tab.id ? 0 : -1"
          :data-testid="`problems-tab-${tab.id}`"
          @click="select(tab.id, 'heading')"
        >
          <span class="label">{{ tab.label }}</span>
          <span v-if="tab.count > 0" class="n">{{ formatNumber(tab.count) }}</span>
        </button>
      </div>
    </nav>

    <div class="main">
      <UiIconButton
        class="close"
        icon="close"
        :label="t('problems.close')"
        shortcut="Esc"
        aria-keyshortcuts="Escape"
        data-testid="problems-close"
        @click="close"
      />
      <div
        id="problems-panel"
        role="tabpanel"
        :aria-labelledby="`problems-tab-${current}`"
        class="panel"
      >
        <header class="head">
          <h2 id="problems-heading" class="heading" tabindex="-1" data-testid="problems-heading">
            {{ t('problems.heading', { count: shownCount }) }}
          </h2>
          <p class="line">{{ headingLine }}</p>
        </header>

        <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
        <p v-if="store.actionError" role="alert" class="error">
          {{ errorMessage(store.actionError) }}
        </p>

        <div v-if="empty" class="good" data-testid="problems-empty">
          <p class="good-title">{{ t('problems.allGood') }}</p>
          <p class="good-line">{{ t('problems.allGoodLine') }}</p>
        </div>

        <template v-else-if="store.loaded">
          <section
            v-for="s in sections"
            :key="s.group.id"
            class="group"
            :data-group="s.group.id"
            :aria-labelledby="`problems-group-${s.group.id}`"
            data-testid="problems-group"
          >
            <h3
              :id="`problems-group-${s.group.id}`"
              class="group-title"
              tabindex="-1"
              :aria-label="t('problems.groupLabel', { group: s.group.label, count: s.count })"
            >
              {{ s.group.label }} · {{ formatNumber(s.count) }}
            </h3>
            <p class="group-line">{{ s.group.line }}</p>

            <div
              v-if="windowed(s)"
              :ref="setScroller"
              class="card windowed"
              data-testid="problems-window"
              @scroll="onScroll"
            >
              <ul
                class="rows"
                :aria-labelledby="`problems-group-${s.group.id}`"
                :style="{ height: `${s.rows.length * ROW_PX}px` }"
              >
                <li
                  v-for="{ row, index } in windowRows(s)"
                  :key="index"
                  class="row fixed"
                  :data-row-index="index"
                  :aria-setsize="s.rows.length"
                  :aria-posinset="index + 1"
                  :style="{ transform: `translateY(${index * ROW_PX}px)`, height: `${ROW_PX}px` }"
                  data-testid="problem-row"
                >
                  <template v-if="row.type === 'problem'">
                    <span class="tile" aria-hidden="true"><UiIcon :name="s.group.icon" /></span>
                    <div class="body">
                      <p class="name-line">
                        <span class="name">{{ row.name }}</span>
                        <span class="kind">{{ row.kindLabel }}</span>
                      </p>
                      <p class="message" :title="row.message">{{ row.message }}</p>
                      <p class="path" :title="row.path">{{ row.path }}</p>
                    </div>
                    <button
                      type="button"
                      class="pill action"
                      data-row-action
                      :aria-label="actionLabel(row)"
                      @click="act(row)"
                      @keydown="onRowKey($event, s, index)"
                    >
                      {{ actionText(row.action) }}
                    </button>
                  </template>
                </li>
              </ul>
            </div>

            <ul
              v-else-if="s.rows.length"
              class="card rows"
              :aria-labelledby="`problems-group-${s.group.id}`"
            >
              <li
                v-for="(row, index) in shown(s)"
                :key="row.key"
                class="row"
                :data-row-index="index"
                data-testid="problem-row"
              >
                <span class="tile" aria-hidden="true"><UiIcon :name="s.group.icon" /></span>
                <div
                  v-if="row.type === 'conflict'"
                  class="body"
                  role="group"
                  :aria-label="`${row.name}: ${row.kindLabel}`"
                  data-testid="sidecar-conflict"
                >
                  <p class="name-line">
                    <span class="name">{{ row.name }}</span>
                    <span class="kind">{{ row.kindLabel }}</span>
                  </p>
                  <p class="path">{{ row.path }}</p>
                  <div class="values">
                    <div class="value">
                      <div>
                        <p class="side">{{ t('problems.sidecar.mine') }}</p>
                        <p class="said">{{ row.conflict.mine }}</p>
                      </div>
                      <button
                        type="button"
                        class="pill"
                        data-row-action
                        :aria-label="
                          t('problems.sidecar.keepMineLabel', {
                            field: row.kindLabel,
                            family: row.name,
                            value: row.conflict.mine,
                          })
                        "
                        @click="resolve(s, index, 'mine')"
                        @keydown="onRowKey($event, s, index)"
                      >
                        {{ t('problems.sidecar.keepMine') }}
                      </button>
                    </div>
                    <div class="value">
                      <div>
                        <p class="side">{{ t('problems.sidecar.theirs') }}</p>
                        <p class="said">{{ row.conflict.theirs }}</p>
                      </div>
                      <button
                        type="button"
                        class="pill"
                        :aria-label="
                          t('problems.sidecar.useTheirsLabel', {
                            field: row.kindLabel,
                            family: row.name,
                            value: row.conflict.theirs,
                          })
                        "
                        @click="resolve(s, index, 'theirs')"
                        @keydown="onRowKey($event, s, index)"
                      >
                        {{ t('problems.sidecar.useTheirs') }}
                      </button>
                    </div>
                  </div>
                </div>
                <template v-else>
                  <div class="body">
                    <p class="name-line">
                      <span class="name">{{ row.name }}</span>
                      <span class="kind">{{ row.kindLabel }}</span>
                    </p>
                    <p class="message">{{ row.message }}</p>
                    <p class="path">{{ row.path }}</p>
                  </div>
                  <button
                    type="button"
                    class="pill action"
                    data-row-action
                    :aria-label="actionLabel(row)"
                    @click="act(row)"
                    @keydown="onRowKey($event, s, index)"
                  >
                    {{ actionText(row.action) }}
                  </button>
                </template>
              </li>
              <li v-if="shown(s).length < s.rows.length" class="more-row">
                <button
                  type="button"
                  class="more"
                  :aria-label="
                    t('problems.moreLabel', {
                      count: s.rows.length - shown(s).length,
                      group: s.group.label,
                    })
                  "
                  data-testid="problems-more"
                  @click="select(s.group.id, 'heading')"
                >
                  {{ t('problems.more', { count: s.rows.length - shown(s).length }) }}
                </button>
              </li>
            </ul>
          </section>
        </template>

        <div v-if="dupes.fonts > 0" class="duplicates" data-testid="problems-duplicates">
          <UiIcon name="layers" class="dup-icon" />
          <p class="dup-text">
            <strong>{{ t('problems.duplicatesLine', { count: dupes.fonts }) }}</strong
            >{{ ' ' }}<span class="quiet">{{ t('problems.duplicatesQuiet') }}</span>
          </p>
          <RouterLink :to="{ name: 'duplicates' }" class="pill">{{
            t('problems.openDuplicates')
          }}</RouterLink>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.problems {
  display: flex;
  height: 100vh;
  min-height: 0;
}
.nav {
  box-sizing: border-box;
  display: flex;
  flex: none;
  flex-direction: column;
  width: 232px;
  padding: var(--fm-space-4) var(--fm-space-3);
  border-inline-end: 1px solid var(--fm-border);
  background-color: var(--fm-surface);
}
.title {
  margin: var(--fm-space-2) var(--fm-space-2) var(--fm-space-4);
  font-size: var(--fm-text-lg);
  font-weight: 600;
}
.tabs {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.tab {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-2);
  height: 32px;
  padding: 0 var(--fm-space-2);
  border: 0;
  border-radius: var(--fm-radius-md);
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  text-align: start;
}
/* "All" stands apart from the groups, as on the board. */
.tab.all {
  margin-block-end: var(--fm-space-3);
}
.tab:hover {
  background-color: var(--fm-hover);
}
.tab:focus-visible {
  outline-offset: -2px;
}
.tab[aria-selected='true'] {
  background-color: var(--fm-side-active, var(--fm-hover));
  font-weight: 600;
}
/* A group with nothing in it: the muted colour, still a tab that opens. */
.tab.none {
  color: var(--fm-muted);
}
.n {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  font-variant-numeric: tabular-nums;
  font-weight: normal;
}
.main {
  position: relative;
  flex: 1;
  min-width: 0;
  overflow-y: auto;
}
.close {
  position: absolute;
  inset-block-start: var(--fm-space-4);
  inset-inline-end: var(--fm-space-4);
}
.panel {
  box-sizing: border-box;
  max-width: calc(840px + 2 * var(--fm-space-8));
  padding: var(--fm-space-8) var(--fm-space-8) var(--fm-space-6);
}
.head {
  margin-block-end: var(--fm-space-6);
}
.heading {
  margin: 0 0 var(--fm-space-1);
  font-size: var(--fm-text-xl);
  font-weight: 600;
}
.heading:focus:not(:focus-visible) {
  outline: none;
}
.line,
.group-line {
  max-width: 540px;
  margin: 0;
  color: var(--fm-muted);
  line-height: 1.5;
}
.error {
  margin: 0 0 var(--fm-space-3);
  color: var(--fm-danger);
}
.group {
  margin-block-end: var(--fm-space-6);
}
.group-title {
  margin: 0 0 var(--fm-space-1);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.group-title:focus:not(:focus-visible) {
  outline: none;
}
.group-line {
  margin-block-end: var(--fm-space-3);
}
.card {
  margin: 0;
  padding: 0;
  list-style: none;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface-2);
}
.card.windowed {
  max-height: calc(100vh - 240px);
  overflow-y: auto;
}
.windowed .rows {
  position: relative;
  margin: 0;
  padding: 0;
  list-style: none;
}
.row {
  display: flex;
  align-items: center;
  gap: var(--fm-space-4);
  padding: var(--fm-space-3) var(--fm-space-4);
  border-block-end: 1px solid var(--fm-border);
}
.rows > .row:last-child {
  border-block-end: 0;
}
.row.fixed {
  box-sizing: border-box;
  position: absolute;
  inset-inline: 0;
  top: 0;
}
.row.fixed .body p {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* The status colour marks the tile only (DESIGN.md). */
.tile {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: var(--fm-radius-md);
  background-color: color-mix(in srgb, var(--fm-warning) 12%, transparent);
  color: var(--fm-warning);
}
.body {
  flex: 1;
  min-width: 0;
}
.body p {
  margin: 0;
}
.name-line {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--fm-space-2);
}
.name {
  font-weight: 500;
}
.kind {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.message {
  overflow-wrap: anywhere;
}
.path {
  color: var(--fm-muted);
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-xs);
  overflow-wrap: anywhere;
}
.pill {
  display: inline-flex;
  flex: none;
  align-items: center;
  height: 28px;
  padding: 0 var(--fm-space-3);
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface-2);
  color: var(--fm-fg);
  font: inherit;
  text-decoration: none;
  white-space: nowrap;
}
/* One action, shown on hover and on focus; it stays in the tab order. */
.action {
  opacity: 0;
}
.row:hover .action,
.row:focus-within .action {
  opacity: 1;
}
.values {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-2);
  margin-block-start: var(--fm-space-2);
}
.value {
  display: flex;
  flex: 0 1 300px;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-3);
  padding: var(--fm-space-2) var(--fm-space-3);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
}
.side {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.more-row {
  padding: var(--fm-space-2) var(--fm-space-4);
}
.more {
  padding: 0;
  border: 0;
  background: none;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  text-decoration: underline;
  text-underline-offset: 2px;
}
/* The empty state is neutral, never green (DESIGN.md). */
.good {
  margin-block-end: var(--fm-space-6);
  padding: var(--fm-space-8);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface);
}
.good-title {
  margin: 0 0 var(--fm-space-2);
  font-family: Georgia, 'Times New Roman', serif;
  font-size: 64px;
  line-height: 1.1;
  letter-spacing: -0.02em;
}
.good-line {
  max-width: 400px;
  margin: 0;
  color: var(--fm-muted);
  line-height: 1.5;
}
.duplicates {
  display: flex;
  align-items: center;
  gap: var(--fm-space-3);
  padding: var(--fm-space-3) var(--fm-space-4);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface);
}
.dup-icon {
  flex: none;
  color: var(--fm-muted);
}
.dup-text {
  flex: 1;
  margin: 0;
}
.dup-text strong {
  font-weight: 500;
}
.quiet {
  color: var(--fm-muted);
}
</style>
