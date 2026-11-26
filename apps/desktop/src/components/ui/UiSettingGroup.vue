<script setup lang="ts">
// A group of setting rows (E57.3, board 15): one card of `UiSettingRow`s, with an optional
// small-caps heading and one note above it. The heading names the group for assistive tech.
// A local copy of a setting group is a bug (DESIGN.md, "Settings").
import { useId } from 'vue';

defineProps<{ heading?: string; note?: string }>();

const uid = useId();
</script>

<template>
  <section
    class="ui-setting-group"
    :aria-labelledby="heading ? `${uid}-heading` : undefined"
    :aria-describedby="note ? `${uid}-note` : undefined"
  >
    <h3 v-if="heading" :id="`${uid}-heading`" class="heading">{{ heading }}</h3>
    <p v-if="note" :id="`${uid}-note`" class="note">{{ note }}</p>
    <div class="card"><slot /></div>
  </section>
</template>

<style scoped>
.ui-setting-group {
  margin: 0 0 var(--fm-space-5);
}
.heading {
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.note {
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.card {
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
}
/* One hairline between rows, none above the first. */
.card > :deep(* + *) {
  border-top: 1px solid var(--fm-border);
}
</style>
