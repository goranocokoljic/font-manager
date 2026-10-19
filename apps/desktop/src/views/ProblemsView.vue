<script setup lang="ts">
// The Problems view: duplicates, broken files, legacy formats, sidecar and
// activation conflicts in one list, filtered by group. The filter is in the URL
// (`#/problems?kind=duplicates`), so the sidebar links open it filtered.
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useCommandsStore } from '../stores/commands';
import {
  PROBLEM_GROUPS,
  groupOf,
  problemGroup,
  useProblemsStore,
  type ProblemGroupId,
} from '../stores/problems';

const store = useProblemsStore();
const commands = useCommandsStore();
const route = useRoute();
const router = useRouter();

const filter = computed(() => problemGroup(route.query.kind)?.id);
const rows = computed(() => store.inGroup(filter.value));

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
  return store.load();
});
onBeforeUnmount(() => store.stop());
</script>

<template>
  <div class="problems" @keydown="onKey">
    <header class="bar">
      <h1 id="problems-title" ref="title" class="title" tabindex="-1">Problems</h1>
      <button type="button" class="button" aria-keyshortcuts="Escape" @click="close">Close</button>
    </header>
    <main class="panel">
      <div class="filters" role="group" aria-label="Show problems">
        <button
          type="button"
          class="toggle"
          :aria-pressed="filter === undefined"
          @click="setFilter(undefined)"
        >
          All <span class="n">{{ store.total }}</span>
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
      <p v-if="store.error" role="alert" class="error">{{ store.error.message }}</p>
      <p v-else-if="store.loaded && !rows.length" class="note">
        {{ filter ? 'No problems of this kind.' : 'No problems. Your library is in good shape.' }}
      </p>
      <table v-else-if="rows.length" aria-labelledby="problems-title" class="table">
        <thead>
          <tr>
            <th scope="col">Problem</th>
            <th scope="col">File</th>
            <th scope="col">With</th>
            <th scope="col">Details</th>
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
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
</style>
