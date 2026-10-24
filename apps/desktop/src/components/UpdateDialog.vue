<script setup lang="ts">
// The update dialog (E18.2): the version on offer, its date and release notes, and
// Install. Nothing installs without that click; a major update asks a second time.
// Like SmartRuleEditor, the dialog keeps focus and plain keys to itself while it is
// open: Tab cycles inside it, and the list's shortcuts behind it never see a key.
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { errorMessage, formatDate, t } from '../i18n';
import { useUpdatesStore } from '../stores/updates';

const updates = useUpdatesStore();
const panel = ref<HTMLElement | null>(null);
let returnFocus: HTMLElement | null = null;

const u = computed(() => updates.update);
const installing = computed(() => updates.status === 'installing');
const published = computed(() => {
  const d = u.value?.date ? new Date(u.value.date) : null;
  return d && !Number.isNaN(d.getTime())
    ? formatDate(d, { year: 'numeric', month: 'numeric', day: 'numeric' })
    : null;
});

function listen(on: boolean): void {
  if (on) {
    window.addEventListener('keydown', onOutsideKey, true);
    document.addEventListener('focusin', onFocusIn);
  } else {
    window.removeEventListener('keydown', onOutsideKey, true);
    document.removeEventListener('focusin', onFocusIn);
  }
}
onBeforeUnmount(() => listen(false));

async function focusPrimary(): Promise<void> {
  await nextTick();
  panel.value?.querySelector<HTMLElement>('[data-primary]')?.focus();
}

watch(
  () => updates.open,
  async (open) => {
    listen(open);
    if (open) {
      returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      await focusPrimary();
    } else {
      const target = returnFocus;
      returnFocus = null;
      await nextTick();
      if (target?.isConnected) target.focus();
    }
  },
  { immediate: true },
);

// The second question replaces the first: its button takes the focus.
watch(
  () => updates.confirmingMajor,
  (confirming) => {
    if (confirming) void focusPrimary();
  },
);

/** Tab cycles inside the dialog; Escape closes it; plain keys stay out of the list's shortcuts. */
function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    updates.dismiss();
    return;
  }
  if (!e.ctrlKey && !e.metaKey) e.stopPropagation();
  if (e.key !== 'Tab' || !panel.value) return;
  const stops = [...panel.value.querySelectorAll<HTMLElement>('button, [tabindex="0"]')].filter(
    (el) => !el.hasAttribute('disabled'),
  );
  const first = stops[0];
  const last = stops[stops.length - 1];
  if (!first || !last) return;
  if (document.activeElement === panel.value) {
    e.preventDefault();
    (e.shiftKey ? last : first).focus();
  } else if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}

/** A key whose target is outside the dialog is handled as if it were in the dialog. */
function onOutsideKey(e: KeyboardEvent): void {
  if (!panel.value || (e.target instanceof Node && panel.value.contains(e.target))) return;
  // The command palette may open over the dialog; it owns its keys.
  if (e.target instanceof Element && e.target.closest('[role="dialog"]')) return;
  if (e.key === 'Tab') {
    e.preventDefault();
    panel.value.focus();
  }
  onKeyDown(e);
}

/** Focus that moves to the page behind the dialog is taken back to it. */
function onFocusIn(e: FocusEvent): void {
  const target = e.target;
  if (!panel.value || !(target instanceof Element) || panel.value.contains(target)) return;
  if (target.closest('[role="dialog"]')) return;
  panel.value.focus();
}
</script>

<template>
  <div v-if="updates.open && u" class="backdrop" @mousedown.self="updates.dismiss">
    <div
      ref="panel"
      class="panel"
      role="dialog"
      aria-modal="true"
      aria-labelledby="fm-update-title"
      aria-describedby="fm-update-lead"
      tabindex="-1"
      @keydown="onKeyDown"
    >
      <template v-if="!updates.confirmingMajor">
        <h2 id="fm-update-title" class="title">
          {{ t('updates.dialog.title', { version: u.version }) }}
        </h2>
        <p id="fm-update-lead" class="lead">
          {{ t('updates.dialog.currentVersion', { version: u.currentVersion })
          }}<template v-if="published">
            {{ t('updates.dialog.released', { date: published }) }}</template
          >
          {{ t('updates.dialog.installQuits') }}
        </p>
        <h3 class="subheading">{{ t('updates.dialog.releaseNotes') }}</h3>
        <div class="notes" tabindex="0" :aria-label="t('updates.dialog.releaseNotes')">
          {{ u.notes || t('updates.dialog.noNotes') }}
        </div>
      </template>
      <template v-else>
        <h2 id="fm-update-title" class="title">{{ t('updates.dialog.majorTitle') }}</h2>
        <p id="fm-update-lead" class="lead">
          {{ t('updates.dialog.majorLead', { version: u.version, current: u.currentVersion }) }}
        </p>
      </template>

      <p v-if="updates.error" role="alert" class="error">{{ errorMessage(updates.error) }}</p>
      <p v-if="installing" role="status" class="lead">
        {{ t('updates.dialog.installing') }}
      </p>

      <div class="actions">
        <button type="button" class="button" :disabled="installing" @click="updates.dismiss">
          {{ t(updates.confirmingMajor ? 'common.cancel' : 'updates.dialog.later') }}
        </button>
        <button
          type="button"
          class="button primary"
          data-primary
          :disabled="installing"
          @click="updates.install"
        >
          {{
            updates.confirmingMajor
              ? t('updates.dialog.installVersion', { version: u.version })
              : t('updates.dialog.installAndRestart')
          }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 10;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 12vh;
  background: rgb(0 0 0 / 20%);
}
.panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: min(520px, 92vw);
  max-height: 76vh;
  padding: 16px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: var(--fm-bg);
  box-shadow: 0 8px 32px rgb(0 0 0 / 24%);
}
.panel:focus {
  outline: none;
}
.title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}
.subheading {
  margin: 4px 0 0;
  font-size: 13px;
}
.lead {
  margin: 0;
  color: var(--fm-muted);
}
.notes {
  min-height: 48px;
  max-height: 240px;
  padding: 8px;
  overflow: auto;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
  white-space: pre-wrap;
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding-top: 4px;
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button.primary {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.button:disabled {
  opacity: 0.5;
}
</style>
