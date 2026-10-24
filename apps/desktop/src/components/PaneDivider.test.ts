import { mount } from '@vue/test-utils';
import { afterEach, describe, expect, it } from 'vitest';
import { FALLBACK_LOCALE, PSEUDO_RTL, setLocale } from '../i18n';
import PaneDivider from './PaneDivider.vue';

afterEach(() => setLocale(FALLBACK_LOCALE));

/** A pointer event at `clientX` (jsdom has no PointerEvent; the handler reads only these). */
const pointer = (el: Element, type: string, clientX: number) =>
  el.dispatchEvent(new MouseEvent(type, { clientX, button: 0 }));

/** The divider of a column at the inline start (the sidebar), 240 px wide. */
const sidebarDivider = () =>
  mount(PaneDivider, {
    props: { label: 'Resize', controls: 'x', value: 240, min: 160, max: 400, direction: 1 },
  });

describe('PaneDivider', () => {
  it('in ltr chrome, ArrowRight and a drag to the right widen the sidebar', async () => {
    const w = sidebarDivider();
    await w.trigger('keydown', { key: 'ArrowRight' });
    await w.trigger('keydown', { key: 'ArrowLeft' });
    pointer(w.element, 'pointerdown', 100);
    pointer(w.element, 'pointermove', 130);
    expect(w.emitted('resize')).toEqual([[248], [232], [270]]);
  });

  it('in rtl chrome the sidebar is on the right: ArrowLeft and a drag to the left widen it', async () => {
    setLocale(PSEUDO_RTL);
    const w = sidebarDivider();
    await w.trigger('keydown', { key: 'ArrowLeft' });
    await w.trigger('keydown', { key: 'ArrowRight' });
    pointer(w.element, 'pointerdown', 100);
    pointer(w.element, 'pointermove', 70);
    expect(w.emitted('resize')).toEqual([[248], [232], [270]]);
  });
});
