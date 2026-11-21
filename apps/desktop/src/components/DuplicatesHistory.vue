<script setup lang="ts">
// History of the Duplicates module (E56.15): the cleanup sessions and "Keep this one"
// actions, newest first, as `listCleanupSessions` lists them — each with its date, what
// it did and Undo for the whole session; it opens to its files, each with Undo of its own.
// A file that cannot come back (the trash was emptied, another file is at its path) says
// so on its row, in the API's words.
import { computed, onMounted, ref } from 'vue';
import type { CleanupSession, SessionFile } from '../api/types';
import { formatDate, t } from '../i18n';
import { useCleanupStore } from '../stores/cleanup';
import UiButton from './ui/UiButton.vue';
import UiChevron from './ui/UiChevron.vue';
import UiEmptyState from './ui/UiEmptyState.vue';

const props = defineProps<{ trash: string; sizeText: (bytes: number) => string }>();

const cleanup = useCleanupStore();
onMounted(() => cleanup.loadHistory());

const open = ref<Record<number, boolean>>({});
const toggle = (id: number) => (open.value = { ...open.value, [id]: !open.value[id] });
const name = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** "2 fonts · 3 in the Recycle Bin · 1 hidden · frees 1.2 MB". */
const what = (s: CleanupSession) => {
  const parts = [t('duplicatesModule.history.sets', { count: s.sets })];
  parts.push(t('duplicatesModule.history.trashed', { count: s.trashed, trash: props.trash }));
  if (s.hidden) parts.push(t('duplicatesModule.history.hidden', { count: s.hidden }));
  if (s.frees) parts.push(t('duplicatesModule.plan.frees', { size: props.sizeText(s.frees) }));
  return parts.join(' · ');
};
const state = (s: CleanupSession) =>
  s.running
    ? t('duplicatesModule.history.running')
    : s.undone
      ? t('duplicatesModule.history.undone')
      : s.cancelled
        ? t('duplicatesModule.history.cancelled')
        : null;
const status = (f: SessionFile) =>
  f.removedAs === 'hide' && (f.status === 'trashed' || f.status === 'restored')
    ? t(`duplicatesModule.history.hiddenStatus.${f.status}`)
    : t(`duplicatesModule.history.status.${f.status}`, { trash: props.trash });
const canUndo = (s: CleanupSession) => !s.running && s.files.some((f) => f.status === 'trashed');
const list = computed(() => cleanup.sessions ?? []);
</script>

<template>
  <div class="page" data-testid="duplicates-history">
    <h2 class="heading">{{ t('duplicatesModule.nav.history') }}</h2>
    <ul v-if="list.length" class="rows">
      <li v-for="s in list" :key="s.id" class="session" :data-session="s.id">
        <div class="head">
          <button
            type="button"
            class="toggle"
            :aria-expanded="!!open[s.id]"
            :aria-controls="`dup-session-${s.id}`"
            @click="toggle(s.id)"
          >
            <UiChevron :open="!!open[s.id]" />
            <span class="title">
              <span>{{ s.name }}</span>
              <span class="muted small">
                {{ formatDate(Date.parse(s.at)) }} · {{ what(s) }}
                <template v-if="state(s)"> · {{ state(s) }}</template>
              </span>
            </span>
          </button>
          <UiButton
            v-if="canUndo(s)"
            class="outlined"
            :aria-label="t('duplicatesModule.history.undoSessionLabel', { name: s.name })"
            @click="cleanup.undo(s.id)"
          >
            {{ t('duplicatesModule.history.undoSession') }}
          </UiButton>
        </div>
        <ul v-if="open[s.id]" :id="`dup-session-${s.id}`" class="files">
          <li v-for="f in s.files" :key="f.path" class="file" :data-status="f.status">
            <span class="file-text">
              <code class="path"
                ><bdi>{{ f.path }}</bdi></code
              >
              <span class="muted small">
                {{ status(f) }}<template v-if="f.message"> — {{ f.message }}</template>
              </span>
            </span>
            <UiButton
              v-if="f.status === 'trashed' || f.status === 'failed'"
              class="outlined"
              :aria-label="t('duplicatesModule.history.undoFileLabel', { file: name(f.path) })"
              @click="cleanup.undo(s.id, [f.path])"
            >
              {{
                f.status === 'failed'
                  ? t('duplicatesModule.history.retry')
                  : t('duplicatesModule.history.undoFile')
              }}
            </UiButton>
          </li>
        </ul>
      </li>
    </ul>
    <UiEmptyState v-else-if="cleanup.sessions" :title="t('duplicatesModule.history.empty')" />
  </div>
</template>

<style scoped>
.page {
  max-width: 960px;
  padding: var(--fm-space-6) var(--fm-space-8);
}
.heading {
  margin: 0 0 var(--fm-space-4);
  font-size: var(--fm-text-xl);
}
.muted {
  color: var(--fm-muted);
}
.small {
  font-size: var(--fm-text-sm);
}
.rows {
  margin: 0;
  padding: 0;
  list-style: none;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
}
.rows > li + li {
  border-top: 1px solid var(--fm-border);
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-4);
  padding-block: var(--fm-space-2);
  padding-inline: var(--fm-space-2) var(--fm-space-4);
}
.toggle {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--fm-space-2);
  min-width: 0;
  padding: var(--fm-space-1) var(--fm-space-2);
  border: 0;
  border-radius: var(--fm-radius-md);
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  text-align: start;
}
.toggle:hover {
  background-color: var(--fm-hover);
}
.title {
  display: grid;
  gap: 2px;
  min-width: 0;
}
.files {
  margin: 0;
  padding-block: 0 var(--fm-space-3);
  padding-inline: var(--fm-space-8) var(--fm-space-4);
  list-style: none;
}
.file {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-4);
  padding: var(--fm-space-2) 0;
}
.file-text {
  display: grid;
  gap: 2px;
  min-width: 0;
}
.path {
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
  overflow-wrap: anywhere;
}
.outlined {
  height: 32px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  color: var(--fm-fg);
}
</style>
