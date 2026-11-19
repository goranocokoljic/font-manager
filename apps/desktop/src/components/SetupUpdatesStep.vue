<script setup lang="ts">
// Setup step three, "Keep Font Fu up to date?" (board 13, E55.6). One question, "Check for
// updates automatically", on at a first start; nothing is saved and nothing is asked of the
// release server until Finish. Shown only where this copy updates itself
// (`UpdateSupport.offered`; the setup store leaves it out elsewhere). "Install when I quit"
// (E55.9) is not offered here. The facts come from the setup store; this file lays them out.
import { nextTick, onMounted } from 'vue';
import { t } from '../i18n';
import { useSetupStore } from '../stores/setup';
import UiIcon from './ui/UiIcon.vue';
import UiSwitch from './ui/UiSwitch.vue';

const setup = useSetupStore();

onMounted(async () => {
  await nextTick();
  document.getElementById('setup-step-heading')?.focus();
});
</script>

<template>
  <div class="updates-step">
    <h1 id="setup-step-heading" class="heading" tabindex="-1">
      {{ t('setup.updates.heading') }}
    </h1>
    <p class="lead">{{ t('setup.updates.lead') }}</p>

    <div class="card" data-testid="setup-updates">
      <span class="icon" aria-hidden="true"><UiIcon name="updateCheck" /></span>
      <div class="text">
        <span id="setup-updates-name" class="name">{{ t('setup.updates.check') }}</span>
        <span id="setup-updates-hint" class="hint">{{ t('setup.updates.checkHint') }}</span>
      </div>
      <UiSwitch
        :checked="setup.checkAutomatically"
        aria-labelledby="setup-updates-name"
        aria-describedby="setup-updates-hint"
        data-testid="setup-updates-switch"
        @click="setup.checkAutomatically = !setup.checkAutomatically"
      />
    </div>
    <p class="later">{{ t('setup.updates.later') }}</p>
  </div>
</template>

<style scoped>
.heading {
  margin: 0;
  font-size: var(--fm-text-xl);
  font-weight: 600;
  letter-spacing: -0.01em;
}
/* The heading takes the focus by script (tabindex -1, never a Tab stop); no ring. */
.heading:focus {
  outline: none;
}
.lead {
  margin: var(--fm-space-3) 0 var(--fm-space-6);
  color: var(--fm-muted);
  line-height: 1.55;
}
.card {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--fm-space-3);
  padding: var(--fm-space-3) var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface-2);
}
.icon {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
}
.text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.name {
  font-weight: 500;
}
.hint {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  line-height: 1.45;
}
.later {
  margin: var(--fm-space-3) 0 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
</style>
