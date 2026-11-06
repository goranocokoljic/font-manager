<script setup lang="ts">
// The main window: sidebar | font list, with a status bar below. The sidebar's width and
// visibility come from the layout store; this component offers the layout actions (sidebar
// toggle, go to a column). Each column is a tab stop, so Tab moves between columns.
import { ref } from 'vue';
import { t } from '../i18n';
import { ACTIONS } from '../keyboard/keymap';
import { useActions } from '../stores/commands';
import { SIDEBAR, useLayoutStore } from '../stores/layout';
import PaneDivider from './PaneDivider.vue';

const layout = useLayoutStore();
const sidebar = ref<HTMLElement | null>(null);
const list = ref<HTMLElement | null>(null);

/** The font view in the list column: list, table, grid or waterfall. */
const FONT_VIEW = '[role="tree"], [role="treegrid"], [role="listbox"]';

/**
 * Up and Down on the list column itself move the focus into its font view, so the
 * keyboard reaches the rows without tabbing through the toolbar and the filters.
 */
function onListKey(e: KeyboardEvent): void {
  if (e.target !== list.value || (e.key !== 'ArrowDown' && e.key !== 'ArrowUp')) return;
  const view = list.value?.querySelector<HTMLElement>(FONT_VIEW);
  if (!view) return;
  e.preventDefault();
  view.focus();
}

// Getters: the palette shows the titles in the language chosen now.
useActions([
  {
    id: ACTIONS.sidebar,
    get title() {
      return t('layout.actions.toggleSidebar');
    },
    get group() {
      return t('palette.groups.view');
    },
    run: layout.toggleSidebar,
  },
  {
    id: 'focus.sidebar',
    get title() {
      return t('layout.actions.goSidebar');
    },
    get group() {
      return t('palette.groups.go');
    },
    run: () => sidebar.value?.focus(),
  },
  {
    id: 'focus.list',
    get title() {
      return t('layout.actions.goList');
    },
    get group() {
      return t('palette.groups.go');
    },
    run: () => list.value?.focus(),
  },
]);
</script>

<template>
  <div class="window">
    <div class="columns">
      <nav
        v-if="layout.sidebarOpen"
        id="fm-sidebar"
        ref="sidebar"
        class="sidebar column"
        tabindex="0"
        :aria-label="t('layout.library')"
        :style="{ width: `${layout.sidebarWidth}px` }"
      >
        <slot name="sidebar" />
      </nav>
      <PaneDivider
        v-if="layout.sidebarOpen"
        :label="t('layout.resizeSidebar')"
        controls="fm-sidebar"
        :value="layout.sidebarWidth"
        :min="SIDEBAR.min"
        :max="SIDEBAR.max"
        :direction="1"
        @resize="layout.setSidebarWidth"
      />
      <main
        ref="list"
        class="list column"
        tabindex="0"
        :aria-label="t('layout.fonts')"
        @keydown="onListKey"
      >
        <slot name="list" />
      </main>
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
/* A column, so the sidebar's foot can sit at its end. */
.sidebar {
  display: flex;
  flex: none;
  flex-direction: column;
  overflow: auto;
  background-color: var(--fm-surface);
}
.list {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 320px;
  overflow: hidden;
}
.column:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.status {
  display: flex;
  align-items: center;
  gap: 16px;
  height: 24px;
  padding: 0 8px;
  border-top: 1px solid var(--fm-border);
  background-color: var(--fm-surface);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
</style>
