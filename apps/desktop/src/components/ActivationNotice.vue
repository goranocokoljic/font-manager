<script setup lang="ts">
// What the last activation of the selected face needs the user to read or answer: why it did
// not happen, or a name clash to settle. Under the count line on the wall, in the Info section
// on the family page.
import { computed, nextTick, ref, watch } from 'vue';
import type { Conflict, OutcomeStatus } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useSelectionStore } from '../stores/selection';

const selection = useSelectionStore();

function message(o: OutcomeStatus | null): string | null {
  if (!o) return null;
  switch (o.status) {
    case 'done':
    case 'unchanged':
      return null;
    case 'conflict':
      return o.with.kind === 'system'
        ? t('info.outcome.system', { path: o.with.path })
        : t('info.outcome.active', { face: String(o.with.face) });
    case 'skipped':
      return o.reason;
    case 'failed':
      return errorMessage(o.error);
  }
}
const outcomeMessage = computed(() => message(selection.outcome));

// The question takes the focus, so the keyboard answers it at once.
const keepButton = ref<HTMLButtonElement | null>(null);
watch(
  () => selection.pending,
  async (p) => {
    if (!p) return;
    await nextTick();
    keepButton.value?.focus();
  },
);

function clashText(c: Conflict): string {
  return c.with.kind === 'system'
    ? t('info.clash.system', { name: c.postscriptName, path: c.with.path })
    : t('info.clash.active', { name: c.postscriptName, face: String(c.with.face) });
}
</script>

<template>
  <p v-if="outcomeMessage" role="alert" class="outcome">{{ outcomeMessage }}</p>
  <div
    v-if="selection.pending"
    role="alertdialog"
    aria-labelledby="fm-clash-title"
    aria-describedby="fm-clash-text"
    class="clash"
  >
    <p id="fm-clash-title" class="clash-title">{{ t('info.clash.title') }}</p>
    <p id="fm-clash-text">
      <template v-for="c in selection.pending.conflicts" :key="clashText(c)">
        {{ clashText(c) }}
      </template>
    </p>
    <div class="clash-actions">
      <button
        ref="keepButton"
        type="button"
        class="choice"
        @click="selection.resolveConflict('keep')"
      >
        {{ t('info.clash.keep') }}
      </button>
      <button type="button" class="choice" @click="selection.resolveConflict('replace')">
        {{ t('info.clash.replace') }}
      </button>
      <button type="button" class="choice" @click="selection.resolveConflict('cancel')">
        {{ t('common.cancel') }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.outcome {
  margin: 0;
  color: var(--fm-danger);
}
.clash {
  display: grid;
  gap: 8px;
  padding: 8px 12px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
}
.clash p {
  margin: 0;
}
.clash-title {
  font-weight: 600;
}
.clash-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.choice {
  padding: 4px 10px;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
</style>
