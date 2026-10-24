<script setup lang="ts">
// The actions on the open layout template file: Save (the editor's draft), Revert (drop
// unsaved changes), Duplicate (an independent copy, which opens), Reset (a preset back to
// its shipped original) and History (the kept versions, each with Restore). Every action
// goes through the text layouts store and the generated client; the core keeps the files.
// The default slot adds the parent's own controls at the end of the bar. A Pairing Studio
// panel passes its own store (E24.3); without one, the library's store is used.
import { computed, ref, useId } from 'vue';
import { errorMessage, formatDate, t } from '../i18n';
import { type TextLayoutsStore, useTextLayoutsStore } from '../stores/textLayouts';

const props = defineProps<{
  /** The text layouts store the actions work on; the library's when not given. */
  store?: TextLayoutsStore;
}>();

const emit = defineEmits<{
  /** A new file was opened (a duplicate); the parent shows it from now on. */
  opened: [id: string];
}>();

const layouts = props.store ?? useTextLayoutsStore();
const historyId = `fm-layout-history-${useId()}`;
const historyOpen = ref(false);
/** The last action's outcome, read in the language shown now. */
const status = ref<(() => string) | null>(null);

const canReset = computed(() => layouts.saved?.layout.preset != null);

async function save(): Promise<void> {
  if (await layouts.save()) status.value = () => t('textLayout.file.saved');
}

async function revert(): Promise<void> {
  if (await layouts.revert()) status.value = () => t('textLayout.file.reverted');
}

async function duplicate(): Promise<void> {
  const copy = await layouts.duplicate();
  if (!copy) return;
  const name = copy.layout.name;
  status.value = () => t('textLayout.file.duplicated', { name });
  emit('opened', copy.id);
}

async function reset(): Promise<void> {
  if (await layouts.resetToOriginal()) status.value = () => t('textLayout.file.wasReset');
}

async function toggleHistory(): Promise<void> {
  historyOpen.value = !historyOpen.value;
  if (historyOpen.value) await layouts.loadHistory();
}

async function restore(number: number): Promise<void> {
  if (await layouts.restore(number))
    status.value = () => t('textLayout.file.restored', { number: String(number) });
}

/** `2026-10-16T09:00:00Z` as a date and time in the UI's locale; an unreadable one as it is. */
const when = (iso: string) => {
  const ms = Date.parse(iso);
  return Number.isNaN(ms) ? iso : formatDate(ms);
};
</script>

<template>
  <div class="actions" role="toolbar" :aria-label="t('textLayout.file.toolbar')">
    <button
      type="button"
      class="toggle"
      :disabled="!layouts.dirty || layouts.busy"
      :title="t('textLayout.file.saveTitle')"
      @click="save"
    >
      {{ t('common.save') }}
    </button>
    <button
      type="button"
      class="toggle"
      :disabled="!layouts.dirty || layouts.busy"
      :title="t('textLayout.file.revertTitle')"
      @click="revert"
    >
      {{ t('textLayout.file.revert') }}
    </button>
    <button
      type="button"
      class="toggle"
      :disabled="!layouts.saved || layouts.busy"
      :title="t('textLayout.file.duplicateTitle')"
      @click="duplicate"
    >
      {{ t('textLayout.file.duplicate') }}
    </button>
    <button
      v-if="canReset"
      type="button"
      class="toggle"
      :disabled="layouts.busy"
      :title="t('textLayout.file.resetTitle')"
      @click="reset"
    >
      {{ t('textLayout.file.resetOriginal') }}
    </button>
    <button
      type="button"
      class="toggle"
      :aria-controls="historyId"
      :aria-expanded="historyOpen"
      :disabled="!layouts.saved"
      @click="toggleHistory"
    >
      {{ t('textLayout.file.history') }}
    </button>
    <span v-if="layouts.dirty" class="note">{{ t('textLayout.file.unsaved') }}</span>
    <span class="note" role="status">{{
      layouts.error ? errorMessage(layouts.error) : (status?.() ?? '')
    }}</span>
    <slot />
  </div>
  <section
    v-if="historyOpen"
    :id="historyId"
    class="history"
    :aria-label="t('textLayout.file.historyRegion')"
  >
    <p v-if="!layouts.history.length" class="note">{{ t('textLayout.file.noHistory') }}</p>
    <ol v-else>
      <li v-for="v in layouts.history" :key="v.number">
        <span class="version">{{
          t('textLayout.file.version', { number: String(v.number) })
        }}</span>
        <span>{{ v.name ?? t('textLayout.file.unreadable') }}</span>
        <span class="note">{{ when(v.savedAt) }}</span>
        <button
          type="button"
          class="toggle"
          :disabled="v.broken !== null || layouts.busy"
          :title="v.broken ?? t('textLayout.file.restoreTitle')"
          :aria-label="t('textLayout.file.restoreLabel', { number: String(v.number) })"
          @click="restore(v.number)"
        >
          {{ t('textLayout.file.restore') }}
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
