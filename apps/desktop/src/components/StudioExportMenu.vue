<script setup lang="ts">
// The Studio toolbar's "Export" (E45.4): a menu under the button with PNG at 1× and 2× (the
// core renders the saved layout file, E45.5) and PDF (the print dialog's save-as-PDF, E24.4). As
// in FamilyCollectionMenu, the button keeps the focus and handles the keys: a combobox over a
// listbox. A pick closes the menu and gives the focus back to the button. While `enabled` is
// false the button stays on the Tab path, marked aria-disabled, and its title tells why.
import { computed, ref, useId } from 'vue';
import { t } from '../i18n';
import UiButton from './ui/UiButton.vue';
import UiIcon from './ui/UiIcon.vue';
import UiPopover from './ui/UiPopover.vue';

/** What the menu offers: a PNG at 1× or 2×, or a PDF. */
export type ExportChoice = 'png1' | 'png2' | 'pdf';

// Two roots (the button and the popover): a class or title goes to the button.
defineOptions({ inheritAttrs: false });

const props = defineProps<{
  /** Whether the menu opens; a saved layout file is needed. */
  enabled: boolean;
}>();

const emit = defineEmits<{ pick: [choice: ExportChoice] }>();

const choices = computed<{ value: ExportChoice; label: string }[]>(() => [
  { value: 'png1', label: t('studio.exportPng1x') },
  { value: 'png2', label: t('studio.exportPng2x') },
  { value: 'pdf', label: t('studio.exportPdf') },
]);

const open = ref(false);
const active = ref(0);
const id = useId();
const button = ref<InstanceType<typeof UiButton> | null>(null);
const anchor = computed(() => (button.value?.$el as HTMLElement | undefined) ?? null);

function toggle(): void {
  if (open.value || !props.enabled) {
    open.value = false;
    return;
  }
  active.value = 0;
  open.value = true;
}

function pick(choice: ExportChoice): void {
  open.value = false;
  anchor.value?.focus();
  emit('pick', choice);
}

/** While the menu is open it owns these keys: the Studio's Escape must not close the page. */
function onKey(e: KeyboardEvent): void {
  if (!open.value) {
    if (e.key === 'ArrowDown' && props.enabled) {
      e.preventDefault();
      toggle();
    }
    return;
  }
  const count = choices.value.length;
  if (e.key === 'Tab') {
    open.value = false;
    return;
  }
  if (e.key === 'Escape') open.value = false;
  else if (e.key === 'ArrowDown') active.value = (active.value + 1) % count;
  else if (e.key === 'ArrowUp') active.value = (active.value - 1 + count) % count;
  else if (e.key === 'Home') active.value = 0;
  else if (e.key === 'End') active.value = count - 1;
  else if (e.key === 'Enter' || e.key === ' ') {
    const chosen = choices.value[active.value];
    if (chosen) pick(chosen.value);
  } else return;
  e.preventDefault();
  e.stopPropagation();
}
</script>

<template>
  <UiButton
    v-bind="$attrs"
    ref="button"
    class="export"
    role="combobox"
    aria-haspopup="listbox"
    :aria-expanded="open"
    :aria-controls="open ? id : undefined"
    :aria-activedescendant="open ? `${id}-${active}` : undefined"
    :aria-label="t('studio.export')"
    :aria-disabled="!enabled"
    @click="toggle"
    @keydown="onKey"
  >
    <UiIcon name="export" />{{ t('studio.export') }}
  </UiButton>
  <UiPopover :open="open" :anchor="anchor" @close="open = false">
    <div :id="id" role="listbox" :aria-label="t('studio.exportMenu')">
      <button
        v-for="(c, i) in choices"
        :id="`${id}-${i}`"
        :key="c.value"
        type="button"
        role="option"
        :aria-selected="i === active"
        class="item"
        :class="{ active: i === active }"
        tabindex="-1"
        @click="pick(c.value)"
      >
        {{ c.label }}
      </button>
    </div>
  </UiPopover>
</template>

<style scoped>
.export {
  padding: 0 var(--fm-space-3);
  height: 32px;
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bar-bg);
  color: var(--fm-bar-fg);
  font-weight: 600;
}
.export:not(:disabled):hover,
.export[aria-expanded='true'] {
  background-color: var(--fm-bar-bg);
  color: var(--fm-bar-fg);
}
.item {
  display: block;
  width: 100%;
  height: 28px;
  padding: 0 var(--fm-space-2);
  border: 0;
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  text-align: start;
  white-space: nowrap;
}
.item.active {
  background-color: var(--fm-hover);
  box-shadow: inset 2px 0 0 var(--fm-accent);
}
</style>
