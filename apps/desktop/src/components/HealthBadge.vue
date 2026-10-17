<script setup lang="ts">
// Whether a face can be used, as fm-api reports it; the reason shows beside it.
import { computed } from 'vue';
import type { Health } from '../api/types';

const props = defineProps<{ health: Health }>();

const label = computed(
  () => ({ ok: 'OK', broken: 'Broken', unsupported: 'Unsupported' })[props.health.kind],
);
const reason = computed(() => (props.health.kind === 'ok' ? null : props.health.reason));
</script>

<template>
  <p class="health" data-testid="health-badge">
    <span class="badge" :class="health.kind">{{ label }}</span>
    <span v-if="reason" class="reason">{{ reason }}</span>
  </p>
</template>

<style scoped>
.health {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px;
  margin: 0;
}
.badge {
  padding: 0 8px;
  border: 1px solid currentColor;
  border-radius: 8px;
  font-size: 11.5px;
  font-weight: 600;
  line-height: 18px;
}
.ok {
  color: var(--fm-success);
}
.broken {
  color: var(--fm-danger);
}
.unsupported {
  color: var(--fm-warning);
}
.reason {
  color: var(--fm-muted);
}
</style>
