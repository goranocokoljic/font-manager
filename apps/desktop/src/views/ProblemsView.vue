<script setup lang="ts">
// The Problems view: duplicates, broken files, legacy formats, sidecar and
// activation conflicts in one list, filtered by group. The filter is in the URL
// (`#/problems?kind=duplicates`), so the sidebar links open it filtered.
// Duplicates are resolved here (E8.3): "Keep this one" shows the dry run, asks
// once, moves the other copies to the trash and offers undo in the app toast; the
// history below the list undoes any earlier action.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { errorMessage, formatDate, t } from '../i18n';
import { useCommandsStore } from '../stores/commands';
import {
  PROBLEM_GROUPS,
  groupOf,
  problemGroup,
  resolutionSummary as summary,
  useProblemsStore,
  type ProblemGroupId,
} from '../stores/problems';

const store = useProblemsStore();
const commands = useCommandsStore();
const route = useRoute();
const router = useRouter();

const filter = computed(() => problemGroup(route.query.kind)?.id);
const rows = computed(() => store.inGroup(filter.value));
const showDuplicates = computed(
  () =>
    (filter.value === undefined || filter.value === 'duplicates') && store.duplicates.length > 0,
);
const planned = computed(() => store.plan?.moves.filter((m) => m.status === 'planned') ?? []);
const notMoving = computed(() => store.plan?.moves.filter((m) => m.status !== 'planned') ?? []);

// The confirm dialog takes the focus, so Enter confirms and Escape cancels.
const confirmButton = ref<HTMLButtonElement | null>(null);
watch(
  () => store.plan,
  async (p) => {
    if (!p) return;
    await nextTick();
    confirmButton.value?.focus();
  },
);

function setFilter(group: ProblemGroupId | undefined): void {
  void router.replace({ query: group ? { kind: group } : {} });
}

function close(): void {
  void router.push({ name: 'home' });
}

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' && !commands.open && !e.defaultPrevented) {
    e.preventDefault();
    if (store.plan) store.cancelPlan();
    else close();
  }
}

// The view takes the focus, so Escape closes it at once.
const title = ref<HTMLHeadingElement | null>(null);
onMounted(() => {
  title.value?.focus();
  return store.load();
});
onBeforeUnmount(() => store.stop());

/** When an action happened, in the UI's locale; the API's text if it is not a date. */
function when(at: string): string {
  const d = new Date(at);
  return Number.isNaN(d.getTime()) ? at : formatDate(d);
}
</script>

<template>
  <div class="problems" @keydown="onKey">
    <header class="bar">
      <h1 id="problems-title" ref="title" class="title" tabindex="-1">
        {{ t('problems.title') }}
      </h1>
      <button type="button" class="button" aria-keyshortcuts="Escape" @click="close">
        {{ t('common.close') }}
      </button>
    </header>
    <main class="panel">
      <div class="filters" role="group" :aria-label="t('problems.filters')">
        <button
          type="button"
          class="toggle"
          :aria-pressed="filter === undefined"
          @click="setFilter(undefined)"
        >
          {{ t('problems.all') }} <span class="n">{{ store.total }}</span>
        </button>
        <button
          v-for="g in PROBLEM_GROUPS"
          :key="g.id"
          type="button"
          class="toggle"
          :aria-pressed="filter === g.id"
          @click="setFilter(g.id)"
        >
          {{ g.label }} <span class="n">{{ store.counts[g.id] }}</span>
        </button>
      </div>
      <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
      <p v-else-if="store.loaded && !rows.length" class="note">
        {{ filter ? t('problems.noneOfKind') : t('problems.none') }}
      </p>
      <table v-else-if="rows.length" aria-labelledby="problems-title" class="table">
        <thead>
          <tr>
            <th scope="col">{{ t('problems.columns.problem') }}</th>
            <th scope="col">{{ t('problems.columns.file') }}</th>
            <th scope="col">{{ t('problems.columns.with') }}</th>
            <th scope="col">{{ t('problems.columns.details') }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(p, i) in rows" :key="`${p.kind}-${p.path}-${i}`">
            <td>{{ groupOf(p.kind)?.label }}</td>
            <td class="path">{{ p.path }}</td>
            <td class="path">{{ p.other ?? '' }}</td>
            <td>{{ p.message }}</td>
          </tr>
        </tbody>
      </table>

      <section v-if="showDuplicates" aria-labelledby="duplicates-title" class="section">
        <h2 id="duplicates-title" class="h2">{{ t('problems.duplicates.title') }}</h2>
        <p class="note">{{ t('problems.duplicates.note') }}</p>
        <p v-if="store.actionError" role="alert" class="error">
          {{ errorMessage(store.actionError) }}
        </p>
        <div
          v-for="(g, gi) in store.duplicates"
          :key="`${gi}-${g.copies[0]?.path}`"
          role="group"
          :aria-label="g.postscriptName ?? t('problems.duplicates.identicalFiles')"
          class="group"
        >
          <h3 class="h3">
            {{ g.postscriptName ?? t('problems.duplicates.identicalFiles') }}
            <span class="muted">{{
              t(g.kind === 'exact' ? 'problems.duplicates.exact' : 'problems.duplicates.similar')
            }}</span>
          </h3>
          <ul class="copies">
            <li v-for="c in g.copies" :key="c.path" class="copy">
              <span class="path">{{ c.path }}</span>
              <span v-if="c.version" class="muted">{{ c.version }}</span>
              <span v-if="c.keep" class="badge">{{ t('problems.duplicates.recommended') }}</span>
              <button
                type="button"
                class="button"
                :aria-label="t('problems.duplicates.keepLabel', { path: c.path })"
                @click="store.planKeep(g, c.path)"
              >
                {{ t('problems.duplicates.keep') }}
              </button>
            </li>
          </ul>
        </div>
      </section>

      <section v-if="store.history.length" aria-labelledby="history-title" class="section">
        <h2 id="history-title" class="h2">{{ t('problems.history.title') }}</h2>
        <table aria-labelledby="history-title" class="table">
          <thead>
            <tr>
              <th scope="col">{{ t('problems.history.when') }}</th>
              <th scope="col">{{ t('problems.history.kept') }}</th>
              <th scope="col">{{ t('problems.history.what') }}</th>
              <th scope="col">
                <span class="sr-only">{{ t('common.undo') }}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in store.history" :key="r.id ?? r.at">
              <td>{{ when(r.at) }}</td>
              <td class="path">{{ r.kept }}</td>
              <td>{{ summary(r) }}</td>
              <td>
                <button
                  v-if="!r.undone && r.id !== null"
                  type="button"
                  class="button"
                  :aria-label="t('problems.history.undoLabel', { summary: summary(r) })"
                  @click="store.undo(r.id)"
                >
                  {{ t('common.undo') }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </section>
    </main>

    <div
      v-if="store.plan"
      role="dialog"
      aria-modal="true"
      aria-labelledby="plan-title"
      class="dialog"
    >
      <h2 id="plan-title" class="h2">
        {{ t('problems.plan.title', { count: planned.length }) }}
      </h2>
      <p class="note">
        {{ t('problems.plan.keeping') }} <span class="path">{{ store.plan.kept }}</span>
      </p>
      <ul v-if="planned.length" :aria-label="t('problems.plan.willMove')" class="copies">
        <li v-for="m in planned" :key="m.path" class="path">{{ m.path }}</li>
      </ul>
      <ul v-if="notMoving.length" :aria-label="t('problems.plan.willStay')" class="copies">
        <li v-for="m in notMoving" :key="m.path">
          <span class="path">{{ m.path }}</span> — {{ m.message }}
        </li>
      </ul>
      <div class="actions">
        <button type="button" class="button" @click="store.cancelPlan()">
          {{ t('common.cancel') }}
        </button>
        <button
          ref="confirmButton"
          type="button"
          class="button primary"
          :disabled="!planned.length"
          @click="store.confirmPlan()"
        >
          {{ t('problems.plan.confirm') }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.problems {
  display: flex;
  flex-direction: column;
  height: 100vh;
}
.bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 48px;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.panel {
  flex: 1;
  padding: 16px 24px;
  overflow-y: auto;
}
.filters {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin: 0 0 12px;
}
.toggle {
  padding: 4px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.toggle[aria-pressed='true'] {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.n {
  margin-inline-start: 4px;
  font-variant-numeric: tabular-nums;
}
.note {
  margin: 0 0 12px;
  color: var(--fm-muted);
}
.error {
  color: var(--fm-danger);
}
.table {
  width: 100%;
  border-collapse: collapse;
}
.table th,
.table td {
  padding: 6px 8px;
  border-bottom: 1px solid var(--fm-border);
  text-align: start;
  vertical-align: top;
  overflow-wrap: anywhere;
}
.path {
  font-family: ui-monospace, monospace;
  font-size: 11.5px;
}
.section {
  margin-top: 24px;
}
.h2 {
  margin: 0 0 8px;
  font-size: 13px;
  font-weight: 600;
}
.h3 {
  margin: 0 0 4px;
  font-size: 12px;
  font-weight: 600;
}
.muted {
  margin-inline-start: 6px;
  color: var(--fm-muted);
  font-weight: normal;
}
.group {
  margin: 0 0 12px;
}
.copies {
  margin: 0 0 8px;
  padding: 0;
  list-style: none;
}
.copy {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
  border-bottom: 1px solid var(--fm-border);
}
.copy .path {
  flex: 1;
}
.badge {
  padding: 0 6px;
  border: 1px solid var(--fm-accent);
  border-radius: 8px;
  color: var(--fm-accent);
}
.dialog {
  position: fixed;
  inset-block-start: 20%;
  inset-inline: 0;
  width: min(560px, calc(100vw - 32px));
  margin: 0 auto;
  padding: 16px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: var(--fm-bg);
  box-shadow: 0 8px 24px rgb(0 0 0 / 25%);
  overflow-wrap: anywhere;
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.primary {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.button:disabled {
  opacity: 0.5;
}
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
</style>
