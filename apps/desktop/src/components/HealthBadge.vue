<script setup lang="ts">
// Whether a face can be used, as fm-api reports it; the reason shows beside it.
import { computed } from 'vue';
import type { Health } from '../api/types';
import { t } from '../i18n';
import UiBadge from './ui/UiBadge.vue';

const props = defineProps<{ health: Health }>();

const label = computed(() => t(`health.${props.health.kind}`));
const tone = computed(() =>
  props.health.kind === 'broken'
    ? 'danger'
    : props.health.kind === 'unsupported'
      ? 'warning'
      : 'quiet',
);
const reason = computed(() => (props.health.kind === 'ok' ? null : props.health.reason));
</script>

<template>
  <p class="health" data-testid="health-badge">
    <UiBadge class="badge" :class="health.kind" :tone="tone">{{ label }}</UiBadge>
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
.reason {
  color: var(--fm-muted);
}
</style>
