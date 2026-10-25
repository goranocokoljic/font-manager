<script setup lang="ts">
// The question an AI assistant's destructive call waits on (E27.6): "An AI assistant
// wants to deactivate 3 fonts", Allow / Deny. Not modal: the user can keep working,
// and no answer within the timeout counts as Deny in fm-core. The region is always in
// the page, so a screen reader announces each new question.
import { computed, onMounted } from 'vue';
import { t } from '../i18n';
import { useMcpConfirmStore } from '../stores/mcpConfirm';

const store = useMcpConfirmStore();

onMounted(() => {
  void store.start();
});

/** What the call would do, in words: deactivate_fonts names its count. */
const message = computed(() => {
  const c = store.current;
  if (!c) return '';
  if (c.tool === 'deactivate_fonts') {
    return c.faces === null
      ? t('mcpConfirm.deactivateUnknown')
      : t('mcpConfirm.deactivate', { count: c.faces });
  }
  return t('mcpConfirm.generic', { tool: c.tool });
});
</script>

<template>
  <div class="region" aria-live="assertive">
    <section
      v-if="store.current"
      :key="store.current.id"
      class="confirm"
      data-testid="mcp-confirm"
      :aria-label="t('mcpConfirm.label')"
      aria-describedby="mcp-confirm-message mcp-confirm-timeout"
    >
      <p id="mcp-confirm-message" class="message">{{ message }}</p>
      <p id="mcp-confirm-timeout" class="hint">
        {{ t('mcpConfirm.timeout', { seconds: store.current.timeoutSecs }) }}
      </p>
      <div class="actions">
        <button
          type="button"
          class="allow"
          :disabled="store.busy"
          @click="store.answer(store.current.id, true)"
        >
          {{ t('mcpConfirm.allow') }}
        </button>
        <button
          type="button"
          class="deny"
          :disabled="store.busy"
          @click="store.answer(store.current.id, false)"
        >
          {{ t('mcpConfirm.deny') }}
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.region {
  position: fixed;
  bottom: 96px;
  inset-inline: 0;
  z-index: 10;
  width: fit-content;
  margin-inline: auto;
}
.confirm {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-width: min(480px, 90vw);
  padding: 12px 16px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
}
.message {
  margin: 0;
  font-weight: 600;
}
.hint {
  margin: 0;
  color: var(--fm-muted);
}
.actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
button {
  padding: 4px 14px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
button:disabled {
  opacity: 0.5;
}
.allow {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
  font-weight: 600;
}
@media (prefers-reduced-motion: no-preference) {
  .confirm {
    animation: rise 120ms ease-out;
  }
}
@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(4px);
  }
}
</style>
