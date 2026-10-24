<script setup lang="ts">
// The Conflicts view: every font the app has active that shares its
// PostScript name with another font, and which of the two wins.
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import type { Clash } from '../api/types';
import { errorMessage, t } from '../i18n';
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
  return c.with.kind === 'system'
    ? t('conflicts.installedFont', { path: c.with.path })
    : store.nameOf(c.with.face);
}

function winner(c: Clash): string {
  return c.winner === 'face' ? store.nameOf(c.face) : other(c);
}
</script>

<template>
  <div class="conflicts" @keydown="onKey">
    <header class="bar">
      <h1 id="conflicts-title" ref="title" class="title" tabindex="-1">
        {{ t('conflicts.title') }}
      </h1>
      <button type="button" class="button" aria-keyshortcuts="Escape" @click="close">
        {{ t('common.close') }}
      </button>
    </header>
    <main class="panel">
      <p class="note">{{ t('conflicts.intro') }}</p>
      <p v-if="store.error" role="alert" class="error">{{ errorMessage(store.error) }}</p>
      <p v-else-if="store.loaded && !store.clashes.length" class="note">
        {{ t('conflicts.none') }}
      </p>
      <table v-else-if="store.clashes.length" aria-labelledby="conflicts-title" class="table">
        <thead>
          <tr>
            <th scope="col">{{ t('conflicts.columns.postscriptName') }}</th>
            <th scope="col">{{ t('conflicts.columns.activeFont') }}</th>
            <th scope="col">{{ t('conflicts.columns.conflictsWith') }}</th>
            <th scope="col">{{ t('conflicts.columns.winner') }}</th>
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
