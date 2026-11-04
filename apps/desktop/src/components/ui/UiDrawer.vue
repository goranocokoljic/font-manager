<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { t } from '../../i18n';

/**
 * A panel that slides in from the inline-start edge. It does not dim the page:
 * what it controls (a filter) changes the list behind it, live. Esc or a press
 * outside closes it, and focus goes back to what opened it.
 */
const props = defineProps<{
  open: boolean;
  label: string;
  /** Presses on this element do not count as "outside" (the button that toggles it). */
  anchor?: HTMLElement | null;
}>();
const emit = defineEmits<{ close: [] }>();

const panel = ref<HTMLElement | null>(null);
let returnTo: HTMLElement | null = null;

function onPointerDown(e: PointerEvent): void {
  const target = e.target as Node | null;
  if (!target || panel.value?.contains(target) || props.anchor?.contains(target)) return;
  // A popover (a select's menu) is teleported out of the panel but belongs to it.
  if ((target as HTMLElement).closest?.('[data-ui-popover]')) return;
  emit('close');
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      returnTo = document.activeElement as HTMLElement | null;
      document.addEventListener('pointerdown', onPointerDown, true);
      await nextTick();
      panel.value?.focus();
    } else {
      document.removeEventListener('pointerdown', onPointerDown, true);
      returnTo?.focus?.();
      returnTo = null;
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => document.removeEventListener('pointerdown', onPointerDown, true));
</script>

<template>
  <Teleport to="body">
    <Transition name="drawer">
      <aside
        v-if="open"
        ref="panel"
        class="drawer"
        role="dialog"
        :aria-label="label"
        tabindex="-1"
        @keydown.esc.stop.prevent="emit('close')"
      >
        <header class="head">
          <h2>{{ label }}</h2>
          <slot name="actions" />
          <button
            type="button"
            class="close"
            :aria-label="t('common.close')"
            @click="emit('close')"
          >
            <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
              <path
                d="M4 4l8 8M12 4l-8 8"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
              />
            </svg>
          </button>
        </header>
        <div class="body"><slot /></div>
      </aside>
    </Transition>
  </Teleport>
</template>

<style scoped>
.drawer {
  position: fixed;
  inset-block: 0;
  inset-inline-start: 0;
  z-index: var(--fm-z-popover);
  display: flex;
  flex-direction: column;
  width: 320px;
  max-width: 90vw;
  border-inline-end: 1px solid var(--fm-border-strong);
  background-color: var(--fm-surface-2);
  box-shadow: var(--fm-shadow-2);
  outline: none;
}
.head {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  padding: var(--fm-space-3) var(--fm-space-4);
  border-bottom: 1px solid var(--fm-border);
}
.head h2 {
  flex: 1;
  margin: 0;
  font-size: var(--fm-text-lg);
  font-weight: 600;
}
.close {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  padding: 0;
  border-color: transparent;
}
.body {
  flex: 1;
  padding: var(--fm-space-3) var(--fm-space-4) var(--fm-space-5);
  overflow-y: auto;
}
.drawer-enter-active,
.drawer-leave-active {
  transition:
    transform var(--fm-motion-move) var(--fm-ease),
    opacity var(--fm-motion-move) var(--fm-ease);
}
.drawer-enter-from,
.drawer-leave-to {
  transform: translateX(-24px);
  opacity: 0;
}
[dir='rtl'] .drawer-enter-from,
[dir='rtl'] .drawer-leave-to {
  transform: translateX(24px);
}
@media (prefers-reduced-motion: reduce) {
  .drawer-enter-active,
  .drawer-leave-active {
    transition: none;
  }
}
</style>
