<script setup lang="ts">
// The main window: sidebar | font list | inspector, with a status bar below.
// Widths and inspector visibility come from the layout store; this component
// only wires the window width and the shortcut into it.
import { onBeforeUnmount, onMounted } from 'vue';
import {
  INSPECTOR,
  NARROW_WIDTH,
  SIDEBAR,
  isInspectorShortcut,
  useLayoutStore,
} from '../stores/layout';
import PaneDivider from './PaneDivider.vue';

const layout = useLayoutStore();

let query: MediaQueryList | undefined;
const onWidthChange = (e: { matches: boolean }) => layout.setNarrow(e.matches);

function onKeyDown(e: KeyboardEvent): void {
  if (!isInspectorShortcut(e)) return;
  e.preventDefault();
  layout.toggleInspector();
}

onMounted(() => {
  query = window.matchMedia?.(`(max-width: ${NARROW_WIDTH - 0.02}px)`);
  if (query) {
    onWidthChange(query);
    query.addEventListener('change', onWidthChange);
  }
  window.addEventListener('keydown', onKeyDown);
});

onBeforeUnmount(() => {
  query?.removeEventListener('change', onWidthChange);
  window.removeEventListener('keydown', onKeyDown);
});
</script>

<template>
  <div class="window">
    <div class="columns">
      <nav
        id="fm-sidebar"
        class="sidebar"
        aria-label="Library"
        :style="{ width: `${layout.sidebarWidth}px` }"
      >
        <slot name="sidebar" />
      </nav>
      <PaneDivider
        label="Resize sidebar"
        controls="fm-sidebar"
        :value="layout.sidebarWidth"
        :min="SIDEBAR.min"
        :max="SIDEBAR.max"
        :direction="1"
        @resize="layout.setSidebarWidth"
      />
      <main class="list">
        <slot name="list" />
      </main>
      <template v-if="layout.inspectorVisible">
        <PaneDivider
          label="Resize inspector"
          controls="fm-inspector"
          :value="layout.inspectorWidth"
          :min="INSPECTOR.min"
          :max="INSPECTOR.max"
          :direction="-1"
          @resize="layout.setInspectorWidth"
        />
        <aside
          id="fm-inspector"
          class="inspector"
          aria-label="Inspector"
          :style="{ width: `${layout.inspectorWidth}px` }"
        >
          <slot name="inspector" />
        </aside>
      </template>
    </div>
    <footer class="status">
      <slot name="status" />
    </footer>
  </div>
</template>

<style scoped>
.window {
  display: flex;
  flex-direction: column;
  height: 100vh;
}
.columns {
  display: flex;
  flex: 1;
  min-height: 0;
}
.sidebar,
.inspector {
  flex: none;
  overflow: auto;
  background: var(--fm-surface);
}
.list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 320px;
  overflow: hidden;
}
.status {
  display: flex;
  align-items: center;
  gap: 16px;
  height: 24px;
  padding: 0 8px;
  border-top: 1px solid var(--fm-border);
  background: var(--fm-surface);
  color: var(--fm-muted);
  font-size: 11.5px;
}
</style>
