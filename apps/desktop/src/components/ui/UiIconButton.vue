<script setup lang="ts">
// The one icon button of the app (E50.6): a 28 px target around a 16 px glyph. The label is
// the accessible name. `size="sm"` (24 px, same glyph) is only for a button inside a short
// row: a sidebar row (styles/sidebar.css) and the remove button of a tag chip. `size="lg"`
// (32 px around a 20 px glyph, E54.9) is the row actions' size: the actions a hovered or
// focused list row, card or table row shows (DESIGN.md, "Row actions"). Everything else keeps
// the default. `aria-pressed` (a toggle) keeps a tint; `aria-expanded` turns it accent, like
// UiButton. The hover/press tint and the disabled fade come from the global `button` baseline
// in theme.css (do not add a second tint here). The default slot adds a small trailing count.
// No component draws an icon-only button of its own (iconButtons.test.ts).
//
// The tooltip (E54.9) is the app's own `UiTooltip`, never a native `title`: `title` (or the
// label) is its text and `shortcut` its key. It comes up after a short rest of the pointer
// (tooltip.ts) and at once on a keyboard focus; a press, Esc, leaving or blurring hides it.
// The tooltip is the button's accessible description (`aria-describedby`).
import { computed, onBeforeUnmount, ref, useAttrs } from 'vue';
import UiIcon from './UiIcon.vue';
import UiTooltip from './UiTooltip.vue';
import type { IconName } from './icons';
import { claim, hoverDelay, keyboardFocus, nextTooltipId, release, watchInput } from './tooltip';

const props = withDefaults(
  defineProps<{
    icon: IconName;
    label: string;
    /** The tooltip's text when it says more than the label (a reason the button is off). */
    title?: string;
    /** The key that does the same, as the user reads it ("S", "Ctrl+K"). */
    shortcut?: string;
    size?: 'md' | 'sm' | 'lg';
  }>(),
  { title: undefined, shortcut: undefined, size: 'md' },
);

// The attributes go on the button by hand, so a caller's own `aria-describedby` (a hint such
// as "Connects to the internet") joins the tooltip's instead of replacing it.
defineOptions({ inheritAttrs: false });
const attrs = useAttrs();

watchInput();

const button = ref<HTMLButtonElement | null>(null);
const open = ref(false);
const tipId = nextTooltipId();
const tip = computed(() => props.title ?? props.label);
const describedBy = computed(() =>
  [attrs['aria-describedby'], tipId].filter((v) => typeof v === 'string' && v).join(' '),
);
let timer: ReturnType<typeof setTimeout> | undefined;

function listen(on: boolean): void {
  if (on) {
    window.addEventListener('keydown', onWindowKey, true);
    window.addEventListener('scroll', hide, true);
  } else {
    window.removeEventListener('keydown', onWindowKey, true);
    window.removeEventListener('scroll', hide, true);
  }
}

function show(): void {
  clearTimeout(timer);
  if (open.value) return;
  claim(hide);
  open.value = true;
  listen(true);
}

function hide(): void {
  clearTimeout(timer);
  if (!open.value) return;
  open.value = false;
  listen(false);
  release(hide);
}

function onPointerEnter(e: PointerEvent): void {
  if (e.pointerType === 'touch') return;
  clearTimeout(timer);
  const wait = hoverDelay();
  if (wait === 0) show();
  else timer = setTimeout(show, wait);
}

function onFocus(): void {
  if (keyboardFocus()) show();
}

function onKeyDown(e: KeyboardEvent): void {
  // A press by key hides it, like a press by pointer.
  if (e.key === 'Enter' || e.key === ' ') hide();
}

function onWindowKey(e: KeyboardEvent): void {
  if (e.key === 'Escape') hide();
}

onBeforeUnmount(hide);
</script>

<template>
  <button
    ref="button"
    v-bind="attrs"
    type="button"
    class="ui-icon-button"
    :class="size"
    :data-size="size"
    :aria-label="label"
    :aria-describedby="describedBy"
    @pointerenter="onPointerEnter"
    @pointerleave="hide"
    @pointerdown="hide"
    @focus="onFocus"
    @blur="hide"
    @keydown="onKeyDown"
  >
    <UiIcon :name="icon" :size="size === 'lg' ? 20 : 16" />
    <slot />
    <!-- Teleported to <body>; inside the button only so the button stays the one root. -->
    <UiTooltip :id="tipId" :open="open" :anchor="button" :text="tip" :shortcut="shortcut" />
  </button>
</template>

<style scoped>
.ui-icon-button {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  min-width: 28px;
  height: 28px;
  padding: 0 var(--fm-space-1);
  gap: var(--fm-space-1);
  border: 0;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: var(--fm-muted);
}
.ui-icon-button.sm {
  min-width: 24px;
  height: 24px;
}
.ui-icon-button.lg {
  min-width: 32px;
  height: 32px;
}
.ui-icon-button[aria-disabled='true'] {
  opacity: 0.55;
}
.ui-icon-button:not(:disabled):hover,
.ui-icon-button[aria-pressed='true'] {
  color: var(--fm-fg);
}
.ui-icon-button[aria-pressed='true'] {
  background-color: var(--fm-hover);
}
.ui-icon-button[aria-expanded='true'] {
  background-color: color-mix(in srgb, var(--fm-accent) 16%, transparent);
  color: var(--fm-fg);
}
</style>
