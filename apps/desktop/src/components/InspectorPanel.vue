<script setup lang="ts">
// The inspector's tabs: Info, Axes, Features, then Glyphs. Arrow keys move between tabs
// (WAI-ARIA tabs pattern); only the selected tab is in the Tab order.
import { nextTick, ref } from 'vue';
import InspectorAxes from './InspectorAxes.vue';
import InspectorFeatures from './InspectorFeatures.vue';
import InspectorGlyphs from './InspectorGlyphs.vue';
import InspectorInfo from './InspectorInfo.vue';

const TABS = [
  { id: 'info', title: 'Info' },
  { id: 'axes', title: 'Axes' },
  { id: 'features', title: 'Features' },
  { id: 'glyphs', title: 'Glyphs' },
] as const;
type TabId = (typeof TABS)[number]['id'];

const current = ref<TabId>('info');

async function onKey(e: KeyboardEvent): Promise<void> {
  const i = TABS.findIndex((t) => t.id === current.value);
  let next: number;
  if (e.key === 'ArrowRight') next = (i + 1) % TABS.length;
  else if (e.key === 'ArrowLeft') next = (i - 1 + TABS.length) % TABS.length;
  else if (e.key === 'Home') next = 0;
  else if (e.key === 'End') next = TABS.length - 1;
  else return;
  e.preventDefault();
  current.value = TABS[next]!.id;
  await nextTick();
  document.getElementById(`fm-tab-${current.value}`)?.focus();
}
</script>

<template>
  <div class="panel">
    <div class="tabs" role="tablist" aria-label="Inspector tabs" @keydown="onKey">
      <button
        v-for="t in TABS"
        :id="`fm-tab-${t.id}`"
        :key="t.id"
        type="button"
        role="tab"
        class="tab"
        :aria-selected="current === t.id"
        :aria-controls="`fm-tabpanel-${t.id}`"
        :tabindex="current === t.id ? 0 : -1"
        @click="current = t.id"
      >
        {{ t.title }}
      </button>
    </div>
    <div
      v-show="current === 'info'"
      id="fm-tabpanel-info"
      role="tabpanel"
      aria-labelledby="fm-tab-info"
      tabindex="0"
    >
      <InspectorInfo />
    </div>
    <div
      v-show="current === 'axes'"
      id="fm-tabpanel-axes"
      role="tabpanel"
      aria-labelledby="fm-tab-axes"
    >
      <InspectorAxes />
    </div>
    <div
      v-show="current === 'features'"
      id="fm-tabpanel-features"
      role="tabpanel"
      aria-labelledby="fm-tab-features"
      tabindex="0"
    >
      <InspectorFeatures />
    </div>
    <div
      v-show="current === 'glyphs'"
      id="fm-tabpanel-glyphs"
      role="tabpanel"
      aria-labelledby="fm-tab-glyphs"
    >
      <!-- Mounted only while shown: it reads the face's file and loads its font. -->
      <InspectorGlyphs v-if="current === 'glyphs'" />
    </div>
  </div>
</template>

<style scoped>
.tabs {
  display: flex;
  gap: 16px;
  height: 48px;
  align-items: flex-end;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.tab {
  padding: 8px 0;
  border: 0;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--fm-muted);
  font: inherit;
  font-weight: 600;
}
.tab[aria-selected='true'] {
  border-bottom-color: var(--fm-accent);
  color: var(--fm-fg);
}
</style>
