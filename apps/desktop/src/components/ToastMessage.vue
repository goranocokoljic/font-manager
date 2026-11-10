<script setup lang="ts">
// The toasts at the top right, under the toolbar (E48.6), e.g. after a bulk activation,
// with its Undo button. Up to three stack, newest on top. They are click-through: they never
// block a control underneath, and only an action button (Undo) takes the pointer. They stay
// clear of the selection bar and the Compare sheet's handle, which sit lower and in the
// middle. The live region is always in the page, so a screen reader reads each new message;
// a toast stays while the pointer is on its button or the focus is in it, and Escape there
// takes it away. `data-testid="toast"` is the newest; the ones under it are `toast-stacked`.
// E48.8: the timer starts again only when the pointer and the focus have both left; after
// Undo or Escape the focus goes back to where it was before the toast took it.
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { useToastStore } from '../stores/toast';
import { FmSlide } from '../motion';
import { retireToast } from './retireToast';

const toasts = useToastStore();

/** The gap between the page's top bar and the first toast, in px. */
const GAP = 8;
/** Where the stack starts when the page has no top bar, in px. */
const FALLBACK_TOP = 56;

/** The stack's top edge: just under the page's top bar (`[data-top-bar]`), however tall it is. */
const top = ref(FALLBACK_TOP);

function measure(): void {
  const bar = document.querySelector<HTMLElement>('[data-top-bar]');
  const rect = bar?.getBoundingClientRect();
  top.value = rect && rect.height > 0 ? Math.max(GAP, Math.round(rect.bottom) + GAP) : FALLBACK_TOP;
  follow(bar ?? undefined);
}

// The bar can change its height while a toast is up: a control in it comes or goes, a font
// loads, the window narrows and the bar wraps. So the stack follows the bar's size while a
// toast is on screen, and lets go of it when the last toast is gone.
let observer: ResizeObserver | undefined;
let observed: HTMLElement | undefined;
function follow(bar: HTMLElement | undefined): void {
  if (bar === observed) return;
  observer?.disconnect();
  observed = bar;
  if (!bar || typeof ResizeObserver === 'undefined') return;
  observer ??= new ResizeObserver(() => measure());
  observer.observe(bar);
}

// Measured as a toast comes: the bar wraps to two rows in a narrow window. In the next frame,
// before it paints: a read in the middle of the change would make the browser lay out early
// and start transitions the change never meant (a button disabled and enabled again in it).
watch(
  () => toasts.toast?.id,
  (id) => {
    if (id !== undefined) requestAnimationFrame(measure);
    else follow(undefined);
  },
);
onBeforeUnmount(() => observer?.disconnect());

// Hold and release, paired (E48.8): a toast is held while the pointer is on its button or
// the focus is in it, and starts its time again only when both have left.
type Holder = 'pointer' | 'focus';
const holders = new Map<number, Set<Holder>>();
function enter(id: number, by: Holder): void {
  const on = holders.get(id) ?? new Set<Holder>();
  on.add(by);
  holders.set(id, on);
  toasts.hold(id);
}
function leave(id: number, by: Holder): void {
  const on = holders.get(id);
  if (!on?.delete(by) || on.size) return;
  holders.delete(id);
  toasts.release(id);
}
function onFocusOut(id: number, e: FocusEvent): void {
  const into = e.relatedTarget;
  if (into instanceof Node && (e.currentTarget as Element).contains(into)) return;
  leave(id, 'focus');
}

// Where the focus goes back to (E48.8): the element that had it before the toast took it,
// by the keyboard (focusin) or by the pointer (pointerdown, before the button takes it).
const returnTo = new Map<number, HTMLElement>();
function remember(id: number, from: EventTarget | null, toast: Element): void {
  if (returnTo.has(id)) return;
  if (from instanceof HTMLElement && from !== document.body && !toast.contains(from)) {
    returnTo.set(id, from);
  }
}
function onFocusIn(id: number, e: FocusEvent): void {
  remember(id, e.relatedTarget, e.currentTarget as Element);
  enter(id, 'focus');
}
function onPointerDown(id: number, e: PointerEvent): void {
  remember(id, document.activeElement, (e.currentTarget as Element).closest('.toast')!);
}

/** Puts the focus back where it was, or on the main view when that is gone; never on body. */
function giveBack(id: number, toast: Element): void {
  const from = returnTo.get(id);
  returnTo.delete(id);
  if (!toast.contains(document.activeElement) && document.activeElement !== document.body) return;
  void nextTick(() => {
    if (from?.isConnected && !from.closest('[inert]')) from.focus();
    if (from && document.activeElement === from) return;
    const main =
      document.querySelector<HTMLElement>('main[tabindex]') ?? document.querySelector('main');
    if (!main) return;
    if (!main.hasAttribute('tabindex')) main.tabIndex = -1;
    main.focus();
  });
}
function onAct(id: number, e: MouseEvent): void {
  const toast = (e.currentTarget as Element).closest('.toast')!;
  toasts.act(id);
  giveBack(id, toast);
}
function onEscape(id: number, e: KeyboardEvent): void {
  const toast = e.currentTarget as Element;
  toasts.dismiss(id);
  giveBack(id, toast);
}

// A toast gone: what it held and where it would give the focus back go with it.
watch(
  () => toasts.toasts,
  (list) => {
    const live = new Set(list.map((t) => t.id));
    for (const id of holders.keys()) if (!live.has(id)) holders.delete(id);
    for (const id of returnTo.keys()) if (!live.has(id)) returnTo.delete(id);
  },
  { flush: 'post' },
);
</script>

<template>
  <div class="region" role="status" aria-live="polite" :style="{ top: `${top}px` }">
    <FmSlide
      group
      tag="div"
      class="stack"
      from="bottom"
      distance="8px"
      speed="fast"
      reduced="fade"
      @before-leave="retireToast"
    >
      <div
        v-for="(item, index) in toasts.toasts"
        :key="item.id"
        class="toast"
        :data-testid="index === 0 ? 'toast' : 'toast-stacked'"
        @focusin="onFocusIn(item.id, $event)"
        @focusout="onFocusOut(item.id, $event)"
        @keydown.esc.stop="onEscape(item.id, $event)"
      >
        <span class="message">{{ item.message }}</span>
        <button
          v-if="item.action"
          type="button"
          class="action"
          @pointerenter="enter(item.id, 'pointer')"
          @pointerleave="leave(item.id, 'pointer')"
          @pointerdown="onPointerDown(item.id, $event)"
          @click="onAct(item.id, $event)"
        >
          {{ item.action.label }}
        </button>
      </div>
    </FmSlide>
  </div>
</template>

<style scoped>
.region {
  position: fixed;
  inset-inline-end: var(--fm-space-4);
  z-index: var(--fm-z-toast);
  /* Click-through (E48.6): the toasts never block a control underneath. */
  pointer-events: none;
}
.stack {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: var(--fm-space-2);
}
.toast {
  display: flex;
  align-items: center;
  gap: 12px;
  /* Off the middle of the window, where the Compare sheet's handle is. */
  max-width: min(420px, calc(50vw - 48px));
  min-height: 36px;
  padding-block: 6px;
  padding-inline: 16px;
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-fg);
  color: var(--fm-bg);
  box-shadow: var(--fm-shadow-2);
  pointer-events: none;
}
.toast:has(.action) {
  padding-inline-end: 6px;
}
.message {
  flex: 1;
}
button {
  border: 1px solid transparent;
  border-radius: var(--fm-radius-sm);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.action {
  padding: 2px 12px;
  border-color: var(--fm-accent);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font-weight: 600;
  /* The one exception (E48.6): the action button takes the pointer. */
  pointer-events: auto;
}
</style>
