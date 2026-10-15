<script setup lang="ts">
import { formatApiVersion } from './api/version';

const apiVersion = formatApiVersion();
</script>

<template>
  <main class="shell">
    <h1>Font Manager</h1>
    <p>API {{ apiVersion }}</p>
  </main>
</template>

<style>
:root {
  color-scheme: light dark;
  font-family: system-ui, sans-serif;
}
.shell {
  padding: 2rem;
}
</style>
