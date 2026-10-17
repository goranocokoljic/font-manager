<script setup lang="ts">
// The inspector's tabs. Info is the first; later stories add more tabs here.
import InspectorInfo from './InspectorInfo.vue';
</script>

<template>
  <div class="panel">
    <div class="tabs" role="tablist" aria-label="Inspector tabs">
      <button
        id="fm-tab-info"
        type="button"
        role="tab"
        class="tab"
        aria-selected="true"
        aria-controls="fm-tabpanel-info"
      >
        Info
      </button>
    </div>
    <div id="fm-tabpanel-info" role="tabpanel" aria-labelledby="fm-tab-info" tabindex="0">
      <InspectorInfo />
    </div>
  </div>
</template>

<style scoped>
.tabs {
  display: flex;
  gap: 8px;
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
