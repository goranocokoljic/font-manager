<script setup lang="ts">
// The Problems group of the sidebar: the total and one link per group that has
// entries, each opening the Problems view filtered to it. The parent view loads
// the problems store (the status bar shows the same total).
import { computed } from 'vue';
import { RouterLink } from 'vue-router';
import { t } from '../i18n';
import { PROBLEM_GROUPS, useProblemsStore } from '../stores/problems';

const store = useProblemsStore();
const groups = computed(() => PROBLEM_GROUPS.filter((g) => store.counts[g.id] > 0));
</script>

<template>
  <section class="problems" aria-labelledby="fm-problems-heading">
    <h2 id="fm-problems-heading" class="heading">
      <RouterLink :to="{ name: 'problems' }" class="link" data-testid="sidebar-problems">
        <span class="name">{{ t('problems.title') }}</span>
        <span class="count">{{ store.total }}</span>
      </RouterLink>
    </h2>
    <ul v-if="groups.length" class="list" aria-labelledby="fm-problems-heading">
      <li v-for="g in groups" :key="g.id">
        <RouterLink
          :to="{ name: 'problems', query: { kind: g.id } }"
          class="link"
          :data-group="g.id"
        >
          <span class="name">{{ g.label }}</span>
          <span class="count">{{ store.counts[g.id] }}</span>
        </RouterLink>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.problems {
  padding: 8px;
}
.heading {
  margin: 0;
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
}
.heading .link {
  padding: 8px;
}
.list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.link {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border-radius: 6px;
  color: inherit;
  text-decoration: none;
}
.link:hover,
.link:focus-visible {
  background: var(--fm-bg);
}
.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.count {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
}
</style>
