<script setup lang="ts">
// The Problems view: broken files, legacy formats, sidecar and activation conflicts
// in one list, filtered by group. The filter is in the URL (`#/problems?kind=legacy`),
// so the sidebar links open it filtered. Duplicates are the Duplicates module's
// (E56.15): one line counts the fonts with more than one copy and opens the module,
// and the old link `#/problems?kind=duplicates` opens the module (router.ts). Sidecar
// conflicts (E28.1) are resolved here: each shows both values with "Keep mine" and
// "Use theirs".
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import type { SidecarField } from '../api/types';
import { errorMessage, t } from '../i18n';
import UiButton from '../components/ui/UiButton.vue';
import { useCommandsStore } from '../stores/commands';
import { useDuplicateSummaryStore } from '../stores/duplicateSummary';
import {
  PROBLEM_GROUPS,
  groupOf,
  problemGroup,
  useProblemsStore,
  type ProblemGroupId,
} from '../stores/problems';

const store = useProblemsStore();
const dupes = useDuplicateSummaryStore();
const commands = useCommandsStore();
const route = useRoute();
const router = useRouter();

const filter = computed(() => problemGroup(route.query.kind)?.id);
const rows = computed(() => store.inGroup(filter.value));
const showSidecar = computed(
  () =>
    (filter.value === undefined || filter.value === 'sidecar') && store.sidecarConflicts.length > 0,
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
    close();
  }
}

// The view takes the focus, so Escape closes it at once.
const title = ref<HTMLHeadingElement | null>(null);
onMounted(() => {
  title.value?.focus();
  void dupes.start();
  return store.load();
});
onBeforeUnmount(() => store.stop());

/** The name of a sidecar conflict's field, in the UI's language. */
function fieldName(field: SidecarField): string {
  return t(`problems.sidecar.fields.${field}`);
}
</script>

<template>
  <div class="problems" @keydown="onKey">
    <header class="bar" data-top-bar>
      <h1 id="problems-title" ref="title" class="title" tabindex="-1">
        {{ t('problems.title') }}
      </h1>
      <UiButton aria-keyshortcuts="Escape" @click="close">
        {{ t('common.close') }}
      </UiButton>
    </header>
    <main class="panel">
      <div class="filters" role="group" :aria-label="t('problems.filters')">
        <UiButton :aria-pressed="filter === undefined" @click="setFilter(undefined)">
          {{ t('problems.all') }} <span class="n">{{ store.total }}</span>
        </UiButton>
        <UiButton
          v-for="g in PROBLEM_GROUPS"
          :key="g.id"
          :aria-pressed="filter === g.id"
          @click="setFilter(g.id)"
        >
          {{ g.label }} <span class="n">{{ store.counts[g.id] }}</span>
        </UiButton>
      </div>
      <p v-if="dupes.fonts > 0" class="duplicates" data-testid="problems-duplicates">
        {{ t('problems.duplicatesLine', { count: dupes.fonts }) }}
        <RouterLink :to="{ name: 'duplicates' }" class="link">{{
          t('problems.openDuplicates')
        }}</RouterLink>
      </p>
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

      <section v-if="showSidecar" aria-labelledby="sidecar-title" class="section">
        <h2 id="sidecar-title" class="h2">{{ t('problems.sidecar.title') }}</h2>
        <p class="note">{{ t('problems.sidecar.note') }}</p>
        <p v-if="store.actionError" role="alert" class="error">
          {{ errorMessage(store.actionError) }}
        </p>
        <div
          v-for="c in store.sidecarConflicts"
          :key="c.id"
          role="group"
          :aria-label="`${c.family}: ${fieldName(c.field)}`"
          class="group"
          data-testid="sidecar-conflict"
        >
          <h3 class="h3">
            {{ c.family }} <span class="muted">{{ fieldName(c.field) }}</span>
          </h3>
          <dl class="values">
            <dt>{{ t('problems.sidecar.mine') }}</dt>
            <dd>{{ c.mine }}</dd>
            <dt>{{ t('problems.sidecar.theirs') }}</dt>
            <dd>{{ c.theirs }}</dd>
          </dl>
          <p class="path">{{ c.sidecarPath }}</p>
          <div class="actions start">
            <UiButton
              :aria-label="
                t('problems.sidecar.keepMineLabel', {
                  field: fieldName(c.field),
                  family: c.family,
                  value: c.mine,
                })
              "
              @click="store.resolveConflict(c.id, 'mine')"
            >
              {{ t('problems.sidecar.keepMine') }}
            </UiButton>
            <UiButton
              :aria-label="
                t('problems.sidecar.useTheirsLabel', {
                  field: fieldName(c.field),
                  family: c.family,
                  value: c.theirs,
                })
              "
              @click="store.resolveConflict(c.id, 'theirs')"
            >
              {{ t('problems.sidecar.useTheirs') }}
            </UiButton>
          </div>
        </div>
      </section>
    </main>
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
  font-size: var(--fm-text-md);
  font-weight: 600;
}
.panel {
  flex: 1;
  padding: 16px 24px;
  overflow-y: auto;
}
.duplicates {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px;
  margin: 0 0 12px;
}
.link {
  color: var(--fm-fg);
  text-decoration: underline;
  text-underline-offset: 2px;
}
.filters {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin: 0 0 12px;
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
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-xs);
}
.section {
  margin-top: 24px;
}
.h2 {
  margin: 0 0 8px;
  font-size: var(--fm-text-md);
  font-weight: 600;
}
.h3 {
  margin: 0 0 4px;
  font-size: var(--fm-text-sm);
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
.dialog {
  position: fixed;
  inset-block-start: 20%;
  inset-inline: 0;
  width: min(560px, calc(100vw - 32px));
  margin: 0 auto;
  padding: 16px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
  box-shadow: var(--fm-shadow-2);
  overflow-wrap: anywhere;
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
.actions.start {
  justify-content: flex-start;
}
.values {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: 2px 12px;
  margin: 0 0 4px;
}
.values dt {
  color: var(--fm-muted);
}
.values dd {
  margin: 0;
}
.button:disabled {
  opacity: 0.5;
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
.button.primary {
  border-color: var(--fm-accent);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
}
</style>
