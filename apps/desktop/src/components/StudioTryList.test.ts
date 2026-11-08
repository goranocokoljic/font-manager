import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { MOCK_STUDIO_CONTEXTS, createMockClient, mockFaces } from '../api/mock';
import type { BlockRole } from '../api/types';
import { TRY_COUNT, useStudioStore } from '../stores/studio';
import StudioRolesRail from './StudioRolesRail.vue';

const inter = mockFaces[0]!.face;
const magazine = MOCK_STUDIO_CONTEXTS.find((c) => c.id === 'studio-magazine')!.layout;
const SCOPE = 'studio-magazine';

let client: FmClient;

/** The mock with twenty sample families after its own; the odd ones pair with Inter. */
function setup(extraFaces = 20) {
  setActivePinia(createPinia());
  client = createMockClient({ extraFaces });
  setClient(client);
}

beforeEach(() => setup());

afterEach(() => {
  vi.restoreAllMocks();
  setClient(undefined);
  document.body.innerHTML = '';
});

function rail(selected: BlockRole | null = null) {
  return mount(StudioRolesRail, {
    props: { scope: SCOPE, layout: magazine, face: inter, selected },
    attachTo: document.body,
  });
}

describe('Try for <role> (E45.3)', () => {
  it('lists three candidates for the selected role, each with its reasons', async () => {
    const spy = vi.spyOn(client, 'suggestPairings');
    const w = rail('body');
    await flushPromises();
    const list = w.get('[data-testid="studio-try"]');
    expect(list.get('h3').text()).toBe('Try for Body');
    expect(spy).toHaveBeenLastCalledWith({
      face: inter.id,
      limit: TRY_COUNT,
      role: 'body',
      partnerOf: inter.id,
    });
    const candidates = list.findAll('[data-testid="studio-try-candidate"]');
    expect(candidates).toHaveLength(3);
    const answer = await client.suggestPairings({
      face: inter.id,
      limit: TRY_COUNT,
      role: 'body',
      partnerOf: inter.id,
    });
    expect(candidates.map((c) => c.get('.family').text())).toEqual(
      answer.map((s) => s.face.familyName),
    );
    expect(candidates[0]!.get('[data-testid="studio-try-reasons"]').text()).toBe(
      answer[0]!.reasons.join(' · '),
    );
    expect(answer[0]!.reasons.length).toBeGreaterThan(0);
    // Each candidate is set in itself and names what a click does.
    expect(candidates[0]!.get('.family').attributes('style')).toMatch(/fm-face-\d+/);
    expect(candidates[0]!.attributes('aria-label')).toBe(
      `Set Body in ${answer[0]!.face.familyName}: ${answer[0]!.reasons.join(', ')}`,
    );
  });

  it('follows the selected role; without one it suggests for the first', async () => {
    const w = rail();
    await flushPromises();
    expect(w.get('[data-testid="studio-try"] h3').text()).toBe('Try for Headline');
    await w.setProps({ selected: 'caption' });
    await flushPromises();
    expect(w.get('[data-testid="studio-try"] h3').text()).toBe('Try for Caption');
    expect(useStudioStore().tries?.role).toBe('caption');
  });

  it('a click sets that role alone, and the list moves on from it', async () => {
    const w = rail('deck');
    await flushPromises();
    const studio = useStudioStore();
    const first = studio.tries!.suggestions[0]!;
    const spy = vi.spyOn(client, 'suggestPairings');
    await w.get('[data-testid="studio-try-candidate"]').trigger('click');
    await flushPromises();
    // Deck is block 2; the other blocks keep the panel's face.
    expect(studio.faceOf(SCOPE, 2)?.id).toBe(first.face.id);
    for (const i of [0, 1, 3, 4]) expect(studio.faceOf(SCOPE, i)).toBeNull();
    expect(studio.canUndo(SCOPE)).toBe(true);
    // Asked again for the new face: its family is the one being replaced.
    expect(spy).toHaveBeenLastCalledWith(expect.objectContaining({ face: first.face.id }));
  });

  it('pairs with a locked role: its face is the partner', async () => {
    const studio = useStudioStore();
    const mono = mockFaces[1]!.face;
    await studio.setBlocksFace(SCOPE, [3], mono.id);
    studio.toggleLock(SCOPE, 'body');
    const spy = vi.spyOn(client, 'suggestPairings');
    rail('headline');
    await flushPromises();
    expect(spy).toHaveBeenLastCalledWith(
      expect.objectContaining({ role: 'headline', partnerOf: mono.id }),
    );
  });

  it('nothing in the library to suggest: says so and offers the catalogs', async () => {
    setup(0);
    const w = rail('body');
    await flushPromises();
    expect(w.findAll('[data-testid="studio-try-candidate"]')).toHaveLength(0);
    const empty = w.get('[data-testid="studio-try-empty"]');
    expect(empty.text()).toContain('Nothing in your library pairs with this for Body yet.');
    await empty.get('[data-testid="studio-try-browse"]').trigger('click');
    expect(w.emitted('browse')).toHaveLength(1);
  });

  it('shows the error of a failed ask', async () => {
    vi.spyOn(client, 'suggestPairings').mockRejectedValue({ code: 'io', message: 'gone' });
    const w = rail('body');
    await flushPromises();
    expect(w.get('[data-testid="studio-try"] [role="alert"]').text()).toContain('gone');
    expect(w.find('[data-testid="studio-try-empty"]').exists()).toBe(false);
  });
});
