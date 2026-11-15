<script setup lang="ts">
// The activation dot of a row, a card, the Family page hero or a catalog item (E50.5): a
// 24 px button around a 10 px dot in one of three states, told apart by shape — off (a
// hollow ring at 35 % ink), session (filled with the accent), permanent (filled, then a 2 px
// gap and a 1.5 px accent ring). A click on an off dot activates its target for the session,
// on a session dot deactivates it, both through the activation store (a toast with Undo);
// on a permanent dot it opens the dot's menu, since removing a permanent activation is the
// heavier action. A catalog item's dot (`catalog`) has no library target: it asks its owner
// to install the family (`activate`). `decorative` draws the dot alone (the Table's header).
// `pointer` (a Wall card): a listbox option holds no button (E48.4), so the dot is a span
// hidden from assistive technology that only the pointer uses, like the card's chevron; the
// card's name says the state and A toggles it from the keyboard.
// Activation fills it: the fill scales 0 → 1 over `--fm-motion-fast` (E46.4, motion table
// "Activate"); under reduced motion it changes at once. `still` (the Table view) changes at
// once always: nothing moves on the Table.
import { computed, ref } from 'vue';
import type { Target } from '../api/types';
import { t } from '../i18n';
import type { DotState } from '../list/activation';
import { useActivationStore } from '../stores/activation';
import UiPopover from './ui/UiPopover.vue';

const props = defineProps<{
  state: DotState;
  /** The font's name, for the label. */
  name?: string;
  /** What a click acts on; none for a catalog item. */
  target?: Target | null;
  catalog?: boolean;
  decorative?: boolean;
  pointer?: boolean;
  still?: boolean;
}>();
const emit = defineEmits<{ activate: [] }>();
// Two roots (the button and its menu): the owner's attributes (`tabindex`, a class) go on the button.
defineOptions({ inheritAttrs: false });

const activation = useActivationStore();

const label = computed(() => {
  const name = props.name ?? '';
  if (props.catalog) return t('activation.dot.catalogLabel', { name });
  if (props.state === 'permanent') return t('activation.dot.permanentLabel', { name });
  if (props.state === 'session') return t('activation.dot.sessionLabel', { name });
  return t('activation.dot.offLabel', { name });
});

const button = ref<HTMLElement | null>(null);
const menuOpen = ref(false);
const menuEl = ref<HTMLElement | null>(null);

function onClick(): void {
  if (props.catalog) {
    emit('activate');
    return;
  }
  const target = props.target;
  if (!target) return;
  if (props.state === 'permanent') {
    if (!menuOpen.value) returnTo = focusBack();
    menuOpen.value = !menuOpen.value;
  } else void activation.toggleSession(target, props.state === 'session');
}

/**
 * Where the focus goes when the menu closes: what had it when the menu opened (the dot
 * itself, or the list that holds a Wall card's pointer-only dot), else the dot.
 */
let returnTo: HTMLElement | null = null;
function focusBack(): HTMLElement | null {
  const active = document.activeElement;
  return active instanceof HTMLElement && active !== document.body ? active : button.value;
}

function closeMenu(): void {
  menuOpen.value = false;
}

/** Closes the menu and gives the focus back, so the keyboard goes on from the dot. */
function closeMenuAndRefocus(): void {
  closeMenu();
  (returnTo ?? button.value)?.focus();
}

function focusMenu(): void {
  menuEl.value?.querySelector('button')?.focus();
}

// The layer lives in <body>, so its keys never reach the row and must not reach the page.
function onMenuKey(e: KeyboardEvent): void {
  e.stopPropagation();
  if (e.key === 'Tab') {
    closeMenuAndRefocus();
  } else if (e.key === 'Escape') {
    e.preventDefault();
    closeMenuAndRefocus();
  }
}

function deactivate(): void {
  closeMenuAndRefocus();
  if (props.target) void activation.deactivateTarget(props.target);
}
</script>

<template>
  <span
    v-if="decorative"
    class="dot"
    :class="[state, { still }]"
    :data-activation="state"
    aria-hidden="true"
  />
  <component
    :is="pointer ? 'span' : 'button'"
    v-else
    v-bind="$attrs"
    ref="button"
    :type="pointer ? undefined : 'button'"
    class="activation-dot"
    data-testid="activation-dot"
    :data-activation="state"
    :aria-hidden="pointer ? 'true' : undefined"
    :aria-label="pointer ? undefined : label"
    :title="label"
    :aria-haspopup="state === 'permanent' && !catalog && !pointer ? 'menu' : undefined"
    :aria-expanded="state === 'permanent' && !catalog && !pointer ? menuOpen : undefined"
    draggable="false"
    @click.stop="onClick"
    @dblclick.stop
    @pointerdown.stop
    @keydown.enter.stop
    @keydown.space.stop
  >
    <span class="dot" :class="[state, { still }]" aria-hidden="true" />
  </component>
  <UiPopover
    v-if="!decorative && !catalog"
    :open="menuOpen"
    :anchor="button"
    @close="closeMenu"
    @placed="focusMenu"
  >
    <div
      v-if="menuOpen"
      ref="menuEl"
      class="activation-menu"
      role="menu"
      data-testid="activation-dot-menu"
      :aria-label="t('activation.dot.menu', { name: name ?? '' })"
      @keydown="onMenuKey"
    >
      <button type="button" role="menuitem" @click="deactivate">
        {{ t('activation.dot.deactivate') }}
      </button>
    </div>
  </UiPopover>
</template>

<style scoped>
/* The 24 px target; the 6 % hover ground; the focus ring is the global :focus-visible one. */
.activation-dot {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background-color: transparent;
  color: inherit;
}
.activation-dot:not(:disabled):hover {
  background-color: color-mix(in srgb, var(--fm-fg) 6%, transparent);
  background-image: none;
}
.dot {
  position: relative;
  display: inline-block;
  flex: none;
  width: 10px;
  height: 10px;
  box-sizing: border-box;
  border: 1.5px solid color-mix(in srgb, var(--fm-fg) 35%, transparent);
  border-radius: 50%;
  outline: 0 solid transparent;
  outline-offset: 2px;
  transition:
    border-color var(--fm-motion-fast) var(--fm-ease),
    outline-color var(--fm-motion-fast) var(--fm-ease);
}
/* The fill, over the ring: hidden at scale 0, grown to 1 when the face is active. */
.dot::before {
  position: absolute;
  inset: -1.5px;
  border-radius: 50%;
  background: var(--fm-accent);
  transform: scale(0);
  transition: transform var(--fm-motion-fast) var(--fm-ease);
  content: '';
}
.dot.session,
.dot.permanent {
  border-color: var(--fm-accent);
}
.dot.session::before,
.dot.permanent::before {
  transform: scale(1);
}
/* Permanent: a 2 px gap (the ground shows through), then a 1.5 px accent ring. */
.dot.permanent {
  outline: 1.5px solid var(--fm-accent);
}
.dot.still,
.dot.still::before {
  transition: none;
}
.activation-menu button {
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
</style>
