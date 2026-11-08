import { defineStore } from 'pinia';
import { computed, ref, shallowRef } from 'vue';
import { useClient } from '../api/client';
import type { CollectionId, DropCounts, TagId } from '../api/types';
import { leftWindow, pickOf } from '../list/drag';
import type { RowKey } from '../list/rows';
import type { FontPick } from './collections';

/** A place fonts can be dropped on (E44.2). */
export type DropTarget =
  { kind: 'collection'; id: CollectionId } | { kind: 'tag'; id: TagId } | { kind: 'studio' };

/** A target's face count now and after the drop. */
export interface DropCount {
  before: number;
  after: number;
}

/** A point of the window, in CSS pixels. */
export interface Point {
  x: number;
  y: number;
}

/** What the ghost shows: a copy of the card or row the drag started on, and how many go. */
export interface GhostSource {
  /** A copy of the dragged element (ids and focus removed); the ghost shows it. */
  el: HTMLElement;
  /** Its size on the page, so the copy keeps its layout. */
  width: number;
  height: number;
  /** Where the pointer took it, from its top-left corner. */
  grab: Point;
}

/** A 1×1 transparent picture: the webview draws no drag image of its own, the ghost is ours. */
let blank: HTMLImageElement | null = null;
function blankImage(): HTMLImageElement | null {
  if (typeof Image === 'undefined') return null;
  if (!blank) {
    blank = new Image(1, 1);
    blank.src = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7';
  }
  return blank;
}
// Load it before the first drag: a drag image must be ready while `dragstart` runs.
blankImage();

/** True when `a` and `b` name the same drop target. */
export function sameTarget(a: DropTarget | null, b: DropTarget | null): boolean {
  if (!a || !b || a.kind !== b.kind) return false;
  return a.kind === 'studio' || a.id === (b as { id: number }).id;
}

/**
 * A copy of `el` for the ghost: no ids (they would repeat the list's), not focusable, hidden
 * from assistive technology (the ghost is decoration; the drop targets speak). Parts marked
 * `data-ghost="hide"` (a card's style count, its twisty) are left out: the ghost names the
 * family and, for more than one, how many more go.
 */
export function ghostCopy(el: HTMLElement): HTMLElement {
  const copy = el.cloneNode(true) as HTMLElement;
  for (const part of copy.querySelectorAll('[data-ghost="hide"]')) part.remove();
  for (const node of [copy, ...copy.querySelectorAll<HTMLElement>('*')]) {
    node.removeAttribute('id');
    node.removeAttribute('tabindex');
    node.removeAttribute('aria-selected');
    node.removeAttribute('role');
  }
  copy.setAttribute('aria-hidden', 'true');
  copy.setAttribute('draggable', 'false');
  return copy;
}

/**
 * Dragging fonts onto collections, tags and the Studio (E44.2, board `4-drag.png`).
 *
 * The drag stays an HTML drag (`application/x-fm-fonts`, ADR 0017), so the drop targets read
 * its payload as before; this store adds what the drag looks like inside the window. The
 * webview's own drag image is a blank, and the ghost (`DragGhost.vue`) is a copy of the card
 * at 0.9 scale, tilted −3°, that follows the pointer from the document's `dragover`. The
 * dragged rows fade to 35 % (`isOrigin`). While the drag is on, `dropCounts` tells every
 * collection's and tag's count now and after the drop, so a target the pointer enters shows
 * "31 → 34". A drop flies the ghost into the target; Esc (the OS's own cancel, or the key
 * when the webview passes it on) ends the drag and nothing is dropped.
 */
export const useFontDragStore = defineStore('fontDrag', () => {
  const active = ref(false);
  /** The dragged rows: they fade while the drag is on. */
  const keys = shallowRef<ReadonlySet<RowKey>>(new Set());
  /** The dragged fonts as the drop targets get them. */
  const pick = shallowRef<FontPick>({ faces: [], families: [] });
  const source = shallowRef<GhostSource | null>(null);
  /** Where the pointer is; null until the first move and while it is outside the window. */
  const pointer = ref<Point | null>(null);
  const counts = shallowRef<DropCounts | null>(null);
  const target = shallowRef<DropTarget | null>(null);
  /** The ghost's last flight: from where it was dropped into the target's box. */
  const flight = shallowRef<{ from: Point; to: DOMRect; source: GhostSource } | null>(null);
  let token = 0;
  /** Esc ended this drag: its drop, when the pointer is let go, drops nothing. */
  let cancelled = false;

  /** How many rows go: the ghost's badge shows it when it is more than one. */
  const count = computed(() => keys.value.size);

  /** True while the drag is on and `key` is one of the dragged rows. */
  function isOrigin(key: RowKey): boolean {
    return active.value && keys.value.has(key);
  }

  /**
   * The pointer moved: `dragover` where it is, or `drag` on the source. Chromium does not send
   * `dragover` for every move while no element takes the drop; `drag` comes on each one. A
   * `drag` at (0, 0) carries no position (the last one, as the drag ends) and is skipped.
   */
  function onOver(e: DragEvent): void {
    if (!active.value) return;
    if (e.type === 'drag' && e.clientX === 0 && e.clientY === 0) return;
    pointer.value = { x: e.clientX, y: e.clientY };
  }

  function onLeave(e: DragEvent): void {
    if (active.value && leftWindow(e, window.innerWidth, window.innerHeight)) pointer.value = null;
  }

  function onKey(e: KeyboardEvent): void {
    if (active.value && e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      end();
      cancelled = true;
      // The drag's own end (after the drop that follows, if any) closes the cancel.
      window.addEventListener('dragend', () => setTimeout(() => (cancelled = false)), {
        once: true,
        capture: true,
      });
    }
  }

  /**
   * A pointer move with no button down: the drag is over. A virtual list can drop the row the
   * drag started on, and its `dragend` then never reaches the document.
   */
  function onPointer(e: PointerEvent): void {
    if (e.buttons === 0) end();
  }

  function listen(on: boolean): void {
    if (on) {
      document.addEventListener('dragover', onOver, true);
      document.addEventListener('drag', onOver, true);
      document.addEventListener('dragleave', onLeave);
      document.addEventListener('dragend', end);
      window.addEventListener('keydown', onKey, true);
      window.addEventListener('pointermove', onPointer, true);
    } else {
      document.removeEventListener('dragover', onOver, true);
      document.removeEventListener('drag', onOver, true);
      document.removeEventListener('dragleave', onLeave);
      document.removeEventListener('dragend', end);
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('pointermove', onPointer, true);
    }
  }

  /**
   * A font drag of `dragged` starts on `e`, from the element `from` (the card or row). Sets
   * the blank drag image and asks fm-api what a drop would do to each target.
   */
  function start(e: DragEvent, dragged: RowKey[], from: HTMLElement | null): void {
    end();
    cancelled = false;
    const image = blankImage();
    if (image && e.dataTransfer?.setDragImage) e.dataTransfer.setDragImage(image, 0, 0);
    active.value = true;
    keys.value = new Set(dragged);
    pick.value = pickOf(dragged);
    pointer.value = { x: e.clientX, y: e.clientY };
    if (from) {
      const box = from.getBoundingClientRect();
      source.value = {
        el: ghostCopy(from),
        width: box.width,
        height: box.height,
        grab: { x: e.clientX - box.left, y: e.clientY - box.top },
      };
    }
    listen(true);
    const mine = ++token;
    useClient()
      .dropCounts({ faces: pick.value.faces, families: pick.value.families })
      .then((got) => {
        if (mine === token && active.value) counts.value = got;
      })
      .catch(() => undefined);
  }

  /** The drag ended: dropped, cancelled with Esc, or released outside a target. */
  function end(): void {
    token++;
    if (active.value) listen(false);
    active.value = false;
    keys.value = new Set();
    target.value = null;
    pointer.value = null;
    counts.value = null;
    source.value = null;
  }

  /** The count `target` shows while the drag is over it, once fm-api has answered. */
  function countOf(t: DropTarget): DropCount | null {
    const got = counts.value;
    if (!got || t.kind === 'studio') return null;
    const hit =
      t.kind === 'collection'
        ? got.collections.find((c) => c.collection === t.id)
        : got.tags.find((x) => x.tag === t.id);
    return hit ? { before: hit.before, after: hit.after } : null;
  }

  /** The pointer is over `t` (a `dragover` there): it shows its ring and count. */
  function enter(t: DropTarget): void {
    if (active.value && !sameTarget(target.value, t)) target.value = t;
  }

  /** The pointer left `t`. */
  function leave(t: DropTarget): void {
    if (sameTarget(target.value, t)) target.value = null;
  }

  /** True when the pointer is over `t` during a drag. */
  function isTarget(t: DropTarget): boolean {
    return active.value && sameTarget(target.value, t);
  }

  /**
   * A drop on the element `el` of a target: the ghost flies into its box and the drag ends.
   * Returns false when Esc cancelled the drag: the caller drops nothing then. A drop of a drag
   * that did not start in this window lands without a flight.
   */
  function land(el: Element | null, e: DragEvent): boolean {
    if (cancelled) return false;
    if (!active.value) return true;
    const from = source.value;
    if (el && from)
      flight.value = {
        from: { x: e.clientX, y: e.clientY },
        to: el.getBoundingClientRect(),
        source: from,
      };
    end();
    return true;
  }

  /** The ghost has landed. */
  function landed(): void {
    flight.value = null;
  }

  return {
    active,
    keys,
    pick,
    source,
    pointer,
    counts,
    target,
    flight,
    count,
    isOrigin,
    start,
    end,
    countOf,
    enter,
    leave,
    isTarget,
    land,
    landed,
  };
});
