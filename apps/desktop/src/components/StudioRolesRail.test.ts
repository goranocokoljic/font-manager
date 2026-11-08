import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { nextTick } from 'vue';
import { setClient } from '../api/client';
import { MOCK_STUDIO_CONTEXTS, createMockClient, mockFaces } from '../api/mock';
import type { BlockRole } from '../api/types';
import { useStudioStore } from '../stores/studio';
import StudioRolesRail from './StudioRolesRail.vue';

const inter = mockFaces[0]!.face;
const mono = mockFaces[1]!.face;
const magazine = MOCK_STUDIO_CONTEXTS.find((c) => c.id === 'studio-magazine')!.layout;

beforeEach(() => {
  setActivePinia(createPinia());
  setClient(createMockClient());
});

afterEach(() => {
  setClient(undefined);
  document.body.innerHTML = '';
});

function rail(selected: BlockRole | null = null) {
  return mount(StudioRolesRail, {
    props: { scope: 'studio-magazine', layout: magazine, face: inter, selected },
    attachTo: document.body,
  });
}

const menu = () => document.body.querySelector<HTMLElement>('.ui-popover [role="listbox"]');

describe('StudioRolesRail', () => {
  it('has one card per role in the core order: name, lock, picker, style and size', () => {
    const w = rail();
    const cards = w.findAll('[data-testid="studio-role"]');
    expect(cards.map((c) => c.get('h3').text())).toEqual(['Headline', 'Deck', 'Body', 'Caption']);
    const headline = cards[0]!;
    expect(headline.get('[data-testid="studio-role-lock"]').attributes('aria-label')).toBe(
      'Lock Headline',
    );
    const picker = headline.get('[role="combobox"]');
    expect(picker.attributes('aria-label')).toBe('Font for Headline');
    // Without a face of its own, a role shows the panel's.
    expect(picker.text()).toBe(inter.familyName);
    expect(headline.get('[data-testid="studio-role-detail"]').text()).toBe(
      `${inter.style} · 40 pt · 1.05`,
    );
  });

  it('a lock toggles its own role only', async () => {
    const w = rail();
    const locks = w.findAll('[data-testid="studio-role-lock"]');
    await locks[0]!.trigger('click');
    expect(locks[0]!.attributes('aria-pressed')).toBe('true');
    expect(locks[2]!.attributes('aria-pressed')).toBe('false');
    expect(useStudioStore().isLocked('studio-magazine', 'headline')).toBe(true);
    await locks[0]!.trigger('click');
    expect(locks[0]!.attributes('aria-pressed')).toBe('false');
  });

  it('the selected role has the ring', async () => {
    const w = rail('body');
    const cards = w.findAll('[data-testid="studio-role"]');
    expect(cards[2]!.attributes('aria-current')).toBe('true');
    expect(cards[2]!.classes()).toContain('selected');
    expect(cards[0]!.attributes('aria-current')).toBeUndefined();
    await w.setProps({ selected: 'caption' });
    expect(cards[3]!.attributes('aria-current')).toBe('true');
    expect(cards[2]!.attributes('aria-current')).toBeUndefined();
  });

  it('the picker searches the library, lists each family in itself and sets the whole role', async () => {
    const w = rail();
    const picker = w.findAll('[role="combobox"]')[3]!;
    await picker.trigger('click');
    await flushPromises();
    expect(useStudioStore().searchFor).toBe('studio-magazine@caption');
    const labels = [...menu()!.querySelectorAll<HTMLElement>('[role="option"] .label')];
    expect(labels.length).toBeGreaterThan(1);
    for (const l of labels) expect(l.style.fontFamily).toMatch(/fm-face-\d+/);
    for (const key of [...mono.familyName.slice(0, 4)]) await picker.trigger('keydown', { key });
    await flushPromises();
    const option = [...menu()!.querySelectorAll<HTMLElement>('[role="option"]')].find(
      (o) => o.textContent?.trim() === mono.familyName,
    )!;
    option.click();
    await flushPromises();
    await nextTick();
    const studio = useStudioStore();
    // Both blocks of Caption, the kicker and the caption, take the family.
    expect(studio.faceOf('studio-magazine', 0)?.family).toBe(mono.family);
    expect(studio.faceOf('studio-magazine', 4)?.family).toBe(mono.family);
    expect(studio.faceOf('studio-magazine', 3)).toBeNull();
    expect(picker.text()).toBe(mono.familyName);
  });
});
