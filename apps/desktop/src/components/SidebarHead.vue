<script setup lang="ts">
// The heading of a sidebar group: 11 px uppercase. The label folds the group; the chevron
// and the group's actions (icon buttons, the `actions` slot) appear on hover and focus.
// The parent owns the body (`v-show` with `controls` as its id) and the folded state.
import UiFoldButton from './ui/UiFoldButton.vue';

defineProps<{ id: string; folded: boolean; controls: string }>();
defineEmits<{ toggle: [] }>();
</script>

<template>
  <header class="side-head">
    <h2 :id="id" class="heading">
      <UiFoldButton :folded="folded" :controls="controls" @toggle="$emit('toggle')">
        <slot />
      </UiFoldButton>
    </h2>
    <span v-if="$slots.actions" class="head-actions" data-testid="section-actions">
      <slot name="actions" />
    </span>
  </header>
</template>

<style scoped>
.side-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: var(--fm-side-row);
  padding-inline: var(--fm-space-2) var(--fm-space-1);
}
.heading {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
/* The label starts where the rows' icons start; the chevron follows it. */
.heading :deep(.ui-fold) {
  flex-direction: row-reverse;
  text-transform: inherit;
}
.heading :deep(.chevron),
.head-actions {
  opacity: 0;
  transition: opacity var(--fm-motion-fast) var(--fm-ease);
}
.side-head:hover :deep(.chevron),
.side-head:focus-within :deep(.chevron),
.heading :deep([aria-expanded='false'] .chevron),
.side-head:hover .head-actions,
.side-head:focus-within .head-actions {
  opacity: 1;
}
.head-actions {
  display: flex;
  gap: var(--fm-space-1);
}
</style>
