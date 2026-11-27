<script setup lang="ts">
// The head of one Settings section (E57.3, board 15): the section's heading and one line
// under it, then the section's groups. The heading takes focus when the section changes
// (SettingsView), so it carries `tabindex="-1"` and `data-section-heading`; the shell draws
// its focus ring for the keyboard only.
defineProps<{ id: string; heading: string; lead?: string }>();
</script>

<template>
  <section class="settings-section" :aria-labelledby="`${id}-title`">
    <h2 :id="`${id}-title`" class="heading" tabindex="-1" data-section-heading>{{ heading }}</h2>
    <p v-if="lead" class="lead">{{ lead }}</p>
    <slot />
  </section>
</template>

<style scoped>
.heading {
  margin: 0 0 var(--fm-space-1);
}
.lead {
  max-width: 60ch;
  margin: 0 0 var(--fm-space-5);
  color: var(--fm-muted);
}
</style>
