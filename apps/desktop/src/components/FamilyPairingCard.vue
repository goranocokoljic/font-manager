<script setup lang="ts">
// One card of the family page's Pairings section (E42.7): a mini layout, the headline face at
// 30 px over the body face at 14 px, then the candidate's name and why it pairs. Never a
// fallback font: each part shows once its face has loaded. The whole card is one button.
import { computed, toRef } from 'vue';
import type { PairingCard } from '../stores/familyPairings';
import { useHeldFace } from '../preview/useHeldFace';

const props = defineProps<{
  card: PairingCard;
  /** The headline's words. */
  headlineText: string;
  /** The body's words. */
  bodyText: string;
}>();
const emit = defineEmits<{ open: [card: PairingCard] }>();

const headlineFamily = useHeldFace(toRef(() => props.card.headline));
const bodyFamily = useHeldFace(toRef(() => props.card.body));
const name = computed(() => props.card.suggestion.face.familyName);
const reasons = computed(() => props.card.suggestion.reasons.join(' · '));
</script>

<template>
  <button
    type="button"
    class="pairing-card"
    data-testid="pairing-card"
    :data-headline="card.headline"
    :data-body="card.body"
    @click="emit('open', card)"
  >
    <span
      class="headline"
      :class="{ loading: !headlineFamily }"
      :style="{ fontFamily: headlineFamily ? `'${headlineFamily}'` : undefined }"
      >{{ headlineText }}</span
    >
    <span
      class="body"
      :class="{ loading: !bodyFamily }"
      :style="{ fontFamily: bodyFamily ? `'${bodyFamily}'` : undefined }"
      >{{ bodyText }}</span
    >
    <span class="caption">
      <bdi class="name">{{ name }}</bdi>
      <span v-if="reasons" class="reasons">{{ reasons }}</span>
    </span>
  </button>
</template>

<style scoped>
.pairing-card {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  min-width: 0;
  padding: var(--fm-space-3) var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-card-bg, var(--fm-bg));
  color: var(--fm-fg);
  font: inherit;
  text-align: start;
  cursor: pointer;
  /* MOTION.md "Hover anything": the card lifts like a wall card, on the fast time. */
  transition:
    box-shadow var(--fm-motion-fast) var(--fm-ease),
    background-color var(--fm-motion-fast) var(--fm-ease);
}
.pairing-card:hover {
  background-color: var(--fm-surface);
  box-shadow: var(--fm-shadow-1);
}
.pairing-card:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.headline {
  overflow: hidden;
  font-size: 30px;
  line-height: 1.15;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.body {
  display: -webkit-box;
  overflow: hidden;
  font-size: 14px;
  line-height: 1.45;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  line-clamp: 3;
}
.loading {
  visibility: hidden;
}
.caption {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-block-start: var(--fm-space-2);
  border-block-start: 1px solid var(--fm-border);
  font-size: var(--fm-text-sm);
}
.name {
  font-weight: 500;
}
.reasons {
  color: var(--fm-muted);
}
</style>
