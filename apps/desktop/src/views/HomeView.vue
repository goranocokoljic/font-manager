<script setup lang="ts">
import { onMounted } from 'vue';
import { formatApiVersion } from '../api/version';
import { useAppStore } from '../stores/app';

const app = useAppStore();
onMounted(() => app.load());
</script>

<template>
  <section class="home" aria-labelledby="home-title">
    <h1 id="home-title">Font Manager</h1>
    <p v-if="app.error" role="alert" class="error">{{ app.error.message }}</p>
    <dl v-else-if="app.info" class="info" data-testid="app-info">
      <dt>App</dt>
      <dd>{{ app.info.name }} {{ app.info.version }}</dd>
      <dt>API</dt>
      <dd>{{ formatApiVersion(app.info.apiVersion) }}</dd>
      <dt>Platform</dt>
      <dd>{{ app.info.platform }}</dd>
    </dl>
  </section>
</template>

<style scoped>
.home {
  padding: 2rem;
}
.info {
  display: grid;
  grid-template-columns: max-content 1fr;
  gap: 0.25rem 1rem;
}
.info dt {
  color: var(--fm-muted);
}
.info dd {
  margin: 0;
}
.error {
  color: var(--fm-danger);
}
</style>
