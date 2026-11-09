// Going from the library to a family page and back (E42.1). The specimen of the row or card
// and the page's hero share a `view-transition-name`, so the browser moves one into the other
// (MOTION.md, "Open a family"); a webview without view transitions gets the page's own slide.
// Back reverses it on the exit curve; the arrows crossfade the page content only (E46.1).
// Under reduced motion each of them is a 120 ms fade of the new page.
import { nextTick } from 'vue';
import { useRouter } from 'vue-router';
import type { FaceId, FamilyId } from '../api/types';
import { familyKey } from '../list/rows';
import { fadeIn, viewTransitionsRun, withViewTransition } from '../motion';
import { prefersReducedMotion } from '../motion/useReducedMotion';
import { useFamilyPageStore } from '../stores/familyPage';
import { useFamilyRailStore, withLanguage } from '../stores/familyRail';
import { useFontListStore } from '../stores/fontList';
import { ALL_FONTS_VIEW, usePreviewStore } from '../stores/preview';
import { useSearchStore } from '../stores/search';

/** The name FamilyView's hero carries in its stylesheet. */
export const SHARED_SPECIMEN = 'fm-family-specimen';
const OPENING = 'fm-family-opening';
const CLOSING = 'fm-family-closing';
const STEPPING = 'fm-family-stepping';

/** The page the router shows: the root element of the route's view. */
function routeView(): Element | null | undefined {
  return document.getElementById('app')?.firstElementChild;
}

/** The family page's content under its top bar, which the arrows change. */
function familyContent(): Element | null {
  return document.querySelector('.family-page > .scroll');
}

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

let latest = 0;
/** Where the arrow presses in flight are going; null when none is. */
let stepTarget: FamilyId | null = null;

/**
 * Runs a page change with the root class that picks the timing, and cleans up after it. A
 * change that starts while one runs skips that one; the skipped one leaves the class and the
 * name alone, as they belong to the newer change now.
 */
function transition(rootClass: string, change: () => Promise<void>): Promise<void> {
  const root = document.documentElement;
  const mine = ++latest;
  root.classList.remove(OPENING, CLOSING, STEPPING);
  root.classList.add(rootClass);
  return withViewTransition(change, () => {
    if (mine !== latest) return;
    root.classList.remove(rootClass);
    share(null);
  });
}

export function useFamilyNavigation() {
  const router = useRouter();
  const page = useFamilyPageStore();
  const list = useFontListStore();
  const preview = usePreviewStore();
  const rail = useFamilyRailStore();
  const search = useSearchStore();

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
    // Reduced motion: no shared element and no slide, the new page fades in.
    if (prefersReducedMotion()) fadeIn(routeView());
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
   * With the rail's language filter chosen, the wall shows only the fonts that cover it.
   */
  async function back(): Promise<void> {
    const family = page.id;
    // "Show only fonts that can do …" in the rail (E42.9): the wall comes back filtered.
    const language = rail.takeWallLanguage();
    await transition(CLOSING, async () => {
      if (cameFromLibrary()) await pop();
      else await router.replace({ name: 'home' });
      if (language) search.setFilters(withLanguage(search.filters, language));
      await nextTick();
      if (family === null) return;
      const specimen = specimenOf(family);
      share(specimen);
      // The list takes the keyboard again, as it had it before the page opened.
      specimen?.closest<HTMLElement>('[tabindex="0"]')?.focus({ preventScroll: true });
    });
    if (prefersReducedMotion()) fadeIn(routeView());
  }

  /**
   * The arrows: the family `by` places on, on the same history entry, so Back still goes to the
   * library. Only the content under the top bar crossfades (200 ms); the bar stays where it is.
   * The new snapshot waits for the family's read, or the crossfade would be to an empty page.
   * Without view transitions, and under reduced motion, the new content fades in.
   *
   * The route changes only once the old page is captured, a frame or more later. A press in
   * that time counts from the family the step before it goes to, not from the page on screen,
   * so two quick presses move two places.
   */
  async function step(by: -1 | 1): Promise<void> {
    const from = stepTarget ?? page.id;
    const family = from === null ? null : page.neighbor(from, by);
    if (family === null) return;
    stepTarget = family;
    const animated = viewTransitionsRun();
    try {
      await transition(STEPPING, async () => {
        await router.replace({ name: 'family', params: { id: String(family) } });
        await nextTick();
        if (animated) await page.settled();
      });
    } finally {
      if (stepTarget === family) stepTarget = null;
    }
    if (!animated) fadeIn(familyContent());
  }

  return { open, back, step };
}
