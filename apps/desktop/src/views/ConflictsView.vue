<script setup lang="ts">
// The Conflicts view: every font the app has active that shares its
// PostScript name with another font, and which of the two wins.
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import type { Clash } from '../api/types';
import { useCommandsStore } from '../stores/commands';
import { useConflictsStore } from '../stores/conflicts';

const store = useConflictsStore();
const commands = useCommandsStore();
const router = useRouter();

function close(): void {
  void router.push({ name: 'home' });
}

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' && !commands.open && !e.defaultPrevented) {
    e.preventDefault();
    close();
  }
}

// The view takes the focus, so Escape closes it at once.
const title = ref<HTMLHeadingElement | null>(null);
onMounted(() => {
  title.value?.focus();
  return store.load();
});
onBeforeUnmount(() => store.stop());

function other(c: Clash): string {
  return c.with.kind === 'system' ? `Installed font: ${c.with.path}` : store.nameOf(c.with.face);
}

function winner(c: Clash): string {
  return c.winner === 'face' ? store.nameOf(c.face) : other(c);
}
</script>

<template>
  <div class="conflicts" @keydown="onKey">
    <header class="bar">
      <h1 id="conflicts-title" ref="title" class="title" tabindex="-1">Font conflicts</h1>
      <button type="button" class="button" aria-keyshortcuts="Escape" @click="close">Close</button>
    </header>
    <main class="panel">
      <p class="note">
        Two active fonts with the same PostScript name are a conflict: apps ask for fonts by that
        name. The winner is the font the app puts first: a font you chose to replace the other one
        with for this session, or the later of two fonts the app activated.
      </p>
      <p v-if="store.error" role="alert" class="error">{{ store.error.message }}</p>
      <p v-else-if="store.loaded && !store.clashes.length" class="note">No conflicts.</p>
      <table v-else-if="store.clashes.length" aria-labelledby="conflicts-title" class="table">
        <thead>
          <tr>
            <th scope="col">PostScript name</th>
            <th scope="col">Active font</th>
            <th scope="col">Conflicts with</th>
            <th scope="col">Winner</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in store.clashes" :key="`${c.face}-${other(c)}`">
            <td>{{ c.postscriptName }}</td>
            <td>{{ store.nameOf(c.face) }}</td>
            <td>{{ other(c) }}</td>
            <td>{{ winner(c) }}</td>
          </tr>
        </tbody>
      </table>
    </main>
  </div>
</template>

<style scoped>
.conflicts {
  display: flex;
  flex-direction: column;
  height: 100vh;
}
.bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 48px;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.panel {
  flex: 1;
  padding: 16px 24px;
  overflow-y: auto;
}
.note {
  margin: 0 0 12px;
  color: var(--fm-muted);
}
.error {
  color: var(--fm-danger);
}
.table {
  width: 100%;
  border-collapse: collapse;
}
.table th,
.table td {
  padding: 6px 8px;
  border-bottom: 1px solid var(--fm-border);
  text-align: start;
  overflow-wrap: anywhere;
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
</style>
