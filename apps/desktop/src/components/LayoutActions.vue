<script setup lang="ts">
// The actions on the open layout template file: Save (the editor's draft), Revert (drop
// unsaved changes), Duplicate (an independent copy, which opens), Reset (a preset back to
// its shipped original) and History (the kept versions, each with Restore). Every action
// goes through the text layouts store and the generated client; the core keeps the files.
import { computed, ref } from 'vue';
import { useTextLayoutsStore } from '../stores/textLayouts';

const emit = defineEmits<{
  /** A new file was opened (a duplicate); the parent shows it from now on. */
  opened: [id: string];
}>();

const layouts = useTextLayoutsStore();
const historyOpen = ref(false);
const status = ref('');

const canReset = computed(() => layouts.saved?.layout.preset != null);

async function save(): Promise<void> {
  if (await layouts.save()) status.value = 'Saved.';
}

async function revert(): Promise<void> {
  if (await layouts.revert()) status.value = 'Unsaved changes dropped.';
}

async function duplicate(): Promise<void> {
  const copy = await layouts.duplicate();
  if (!copy) return;
  status.value = `Duplicated as “${copy.layout.name}”.`;
  emit('opened', copy.id);
}

async function reset(): Promise<void> {
  if (await layouts.resetToOriginal()) status.value = 'Reset to the original preset.';
}

async function toggleHistory(): Promise<void> {
  historyOpen.value = !historyOpen.value;
  if (historyOpen.value) await layouts.loadHistory();
}

async function restore(number: number): Promise<void> {
  if (await layouts.restore(number)) status.value = `Version ${number} restored.`;
}

/** `2026-10-16T09:00:00Z` as `2026-10-16 09:00 UTC`. */
const when = (iso: string) => `${iso.slice(0, 10)} ${iso.slice(11, 16)} UTC`;
</script>

<template>
  <div class="actions" role="toolbar" aria-label="Layout file">
    <button
      type="button"
      class="toggle"
      :disabled="!layouts.dirty || layouts.busy"
      title="Save the edited text to the layout file"
      @click="save"
    >
      Save
    </button>
    <button
      type="button"
      class="toggle"
      :disabled="!layouts.dirty || layouts.busy"
      title="Drop the unsaved changes"
      @click="revert"
    >
      Revert
    </button>
    <button
      type="button"
      class="toggle"
      :disabled="!layouts.saved || layouts.busy"
      title="Copy this layout as saved into a new layout"
      @click="duplicate"
    >
      Duplicate
    </button>
    <button
      v-if="canReset"
      type="button"
      class="toggle"
      :disabled="layouts.busy"
      title="Put back the preset this layout started from"
      @click="reset"
    >
      Reset to original
    </button>
    <button
      type="button"
      class="toggle"
      aria-controls="fm-layout-history"
      :aria-expanded="historyOpen"
      :disabled="!layouts.saved"
      @click="toggleHistory"
    >
      History
    </button>
    <span v-if="layouts.dirty" class="note">Unsaved changes</span>
    <span class="note" role="status">{{ layouts.error ? layouts.error.message : status }}</span>
  </div>
  <section v-if="historyOpen" id="fm-layout-history" class="history" aria-label="Layout history">
    <p v-if="!layouts.history.length" class="note">No earlier versions yet.</p>
    <ol v-else>
      <li v-for="v in layouts.history" :key="v.number">
        <span class="version">Version {{ v.number }}</span>
        <span>{{ v.name ?? 'Unreadable' }}</span>
        <span class="note">{{ when(v.savedAt) }}</span>
        <button
          type="button"
          class="toggle"
          :disabled="v.broken !== null || layouts.busy"
          :title="v.broken ?? 'Make this version current; the current one is kept'"
          :aria-label="`Restore version ${v.number}`"
          @click="restore(v.number)"
        >
          Restore
        </button>
      </li>
    </ol>
  </section>
</template>

<style scoped>
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 4px 16px;
  border-bottom: 1px solid var(--fm-border);
  font-size: 12px;
}
.toggle {
  padding: 2px 8px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.toggle:disabled {
  opacity: 0.5;
}
.toggle[aria-expanded='true'] {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.note {
  color: var(--fm-muted);
}
.history {
  max-height: 30vh;
  overflow: auto;
  padding: 4px 16px;
  border-bottom: 1px solid var(--fm-border);
  font-size: 12px;
}
.history ol {
  margin: 0;
  padding: 0;
  list-style: none;
}
.history li {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 0;
}
.version {
  font-weight: 600;
}
</style>
