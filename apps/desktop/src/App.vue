<script setup lang="ts">
import { RouterView } from 'vue-router';
</script>

<template>
  <main class="shell">
    <RouterView />
  </main>
</template>

<style>
.shell {
  min-height: 100vh;
}
</style>
