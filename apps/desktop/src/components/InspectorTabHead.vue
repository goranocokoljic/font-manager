<script setup lang="ts">
// The head every inspector tab shares: a quiet hint on its own line, the
// tab's actions right-aligned underneath (default slot).
defineProps<{ hint: string }>();
</script>

<template>
  <div class="tab-head">
    <p class="hint">{{ hint }}</p>
    <div class="actions"><slot /></div>
  </div>
</template>

<style scoped>
.tab-head {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: var(--fm-space-2);
  padding-block-end: var(--fm-space-3);
}
.hint {
  margin: 0;
  color: var(--fm-muted);
}
.actions {
  display: flex;
  flex: none;
  justify-content: flex-end;
  align-items: center;
  gap: var(--fm-space-2);
}
</style>
