<script setup lang="ts">
import { onMounted } from 'vue';
import { formatApiVersion } from '../api/version';
import MainLayout from '../components/MainLayout.vue';
import { useAppStore } from '../stores/app';
import { useLayoutStore } from '../stores/layout';

const app = useAppStore();
const layout = useLayoutStore();
onMounted(() => app.load());

// Sidebar groups from the spec's information architecture; E5.7 and later fill them.
const groups = [
  'Sources',
  'Collections',
  'Smart collections',
  'Project sets',
  'Tags',
  'Problems',
  'Online catalogs',
];
</script>

<template>
  <MainLayout>
    <template #sidebar>
      <ul class="groups">
        <li v-for="group in groups" :key="group" class="group">{{ group }}</li>
      </ul>
    </template>

    <template #list>
      <header class="toolbar">
        <h1 class="title">All fonts</h1>
        <button
          type="button"
          class="toggle"
          aria-controls="fm-inspector"
          aria-keyshortcuts="Control+I Meta+I"
          :aria-pressed="layout.inspectorVisible"
          title="Show or hide the inspector (Ctrl+I / ⌘I)"
          @click="layout.toggleInspector"
        >
          Inspector
        </button>
      </header>
      <section class="empty" aria-label="Font list">
        <p>No fonts yet. Add a folder to start your library.</p>
      </section>
    </template>

    <template #inspector>
      <p class="placeholder">Nothing selected</p>
    </template>

    <template #status>
      <p v-if="app.error" role="alert" class="error">{{ app.error.message }}</p>
      <dl v-else-if="app.info" class="info" data-testid="app-info">
        <dt>App</dt>
        <dd>{{ app.info.name }} {{ app.info.version }}</dd>
        <dt>API</dt>
        <dd>{{ formatApiVersion(app.info.apiVersion) }}</dd>
        <dt>Platform</dt>
        <dd>{{ app.info.platform }}</dd>
      </dl>
    </template>
  </MainLayout>
</template>

<style scoped>
.groups {
  margin: 0;
  padding: 8px;
  list-style: none;
}
.group {
  padding: 8px;
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
}
.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  height: 48px;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.toggle {
  padding: 4px 8px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.toggle[aria-pressed='true'] {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.empty {
  display: grid;
  flex: 1;
  place-items: center;
  color: var(--fm-muted);
}
.placeholder {
  margin: 0;
  padding: 16px;
  color: var(--fm-muted);
}
.info {
  display: flex;
  gap: 4px 8px;
  margin: 0;
}
.info dt::after {
  content: ':';
}
.info dd {
  margin: 0 8px 0 0;
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
</style>
