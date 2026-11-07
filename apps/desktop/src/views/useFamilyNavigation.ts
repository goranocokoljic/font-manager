// Going from the library to a family page and back (E42.1). The specimen of the row or card
// and the page's hero share a `view-transition-name`, so the browser moves one into the other
// (MOTION.md, "Open a family"); a webview without view transitions gets the page's own slide.
import { nextTick } from 'vue';
import { useRouter } from 'vue-router';
import type { FaceId, FamilyId } from '../api/types';
import { familyKey } from '../list/rows';
import { withViewTransition } from '../motion';
import { useFamilyPageStore } from '../stores/familyPage';
import { useFontListStore } from '../stores/fontList';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';

/** The name FamilyView's hero carries in its stylesheet. */
export const SHARED_SPECIMEN = 'fm-family-specimen';
const OPENING = 'fm-family-opening';
const CLOSING = 'fm-family-closing';

/** The first text block on screen of a Layout view that shows `family`. */
function layoutSpecimenOf(family: FamilyId): HTMLElement | null {
  const layout = document.querySelector<HTMLElement>(`.layout[data-family="${family}"]`);
  if (!layout) return null;
  const box = layout.getBoundingClientRect();
  return (
    [...layout.querySelectorAll<HTMLElement>('.cell')].find((cell) => {
      const r = cell.getBoundingClientRect();
      return r.bottom > box.top && r.top < box.bottom;
    }) ?? null
  );
}

/** The row or card of `family` when it is on screen, its specimen if it has a separate one. */
function specimenOf(family: FamilyId): HTMLElement | null {
  const row = document.querySelector<HTMLElement>(`[data-key="${familyKey(family)}"]`);
  if (!row) return layoutSpecimenOf(family);
  const box = row.getBoundingClientRect();
  if (box.bottom <= 0 || box.top >= window.innerHeight) return null;
  return row.querySelector<HTMLElement>('.sample, .thumb') ?? row;
}

let shared: HTMLElement | null = null;

function share(el: HTMLElement | null): void {
  if (shared) shared.style.removeProperty('view-transition-name');
  shared = el;
  el?.style.setProperty('view-transition-name', SHARED_SPECIMEN);
}

/** Runs a page change with the root class that picks the timing, and cleans up after it. */
function transition(rootClass: string, change: () => Promise<void>): Promise<void> {
  const root = document.documentElement;
  root.classList.add(rootClass);
  return withViewTransition(change, () => {
    root.classList.remove(rootClass);
    share(null);
  });
}

export function useFamilyNavigation() {
  const router = useRouter();
  const page = useFamilyPageStore();
  const list = useFontListStore();
  const preview = usePreviewStore();

  /** Pushes the family page from the library, on `face` when a style was the one chosen. */
  async function open(family: FamilyId, face?: FaceId): Promise<void> {
    share(specimenOf(family));
    // Paragraph asks how the family reads, and the Text section is the place that answers it.
    page.section = preview.settingsFor(ALL_FONTS_VIEW).preset === 'paragraph' ? 'text' : 'styles';
    page.pushed = true;
    await transition(OPENING, async () => {
      // The view records where it is as it leaves, and Back brings it to that place.
      list.holdScroll(true);
      // The row leaves with the library, so the only element left with the name is the hero.
      await router.push({
        name: 'family',
        params: { id: String(family) },
        query: face === undefined ? {} : { face: String(face) },
      });
      await nextTick();
      list.holdScroll(false);
    });
  }

  /** The entry to go back to, when the page was opened from the library. */
  function cameFromLibrary(): boolean {
    const back = router.options.history.state.back;
    return typeof back === 'string' && router.resolve(back).name === 'home';
  }

  function pop(): Promise<void> {
    return new Promise((resolve) => {
      const stop = router.afterEach(() => {
        stop();
        resolve();
      });
      router.back();
    });
  }

  /**
   * Back to the library. A page opened from there pops, so the list's URL and selection come
   * back as they were; a deep link has nothing to pop and replaces itself with the library.
   */
  async function back(): Promise<void> {
    const family = page.id;
    await transition(CLOSING, async () => {
      if (cameFromLibrary()) await pop();
      else await router.replace({ name: 'home' });
      await nextTick();
      if (family === null) return;
      const specimen = specimenOf(family);
      share(specimen);
      // The list takes the keyboard again, as it had it before the page opened.
      specimen?.closest<HTMLElement>('[tabindex="0"]')?.focus({ preventScroll: true });
    });
  }

  /** The arrows: another family on the same history entry, so Back still goes to the library. */
  function step(family: FamilyId | null): void {
    if (family !== null) void router.replace({ name: 'family', params: { id: String(family) } });
  }

  return { open, back, step };
}
