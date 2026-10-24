import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient } from '../api/client';
import { createMockClient, mockFaces } from '../api/mock';
import { useStudioStore } from '../stores/studio';
import BlockFontPicker from './BlockFontPicker.vue';

const mono = mockFaces[1]!.face;

beforeEach(() => {
  setActivePinia(createPinia());
  setClient(createMockClient());
});

afterEach(() => setClient(undefined));

async function picker(face = useStudioStore().faceOf('book', 0)) {
  const w = mount(BlockFontPicker, {
    props: { layoutId: 'book', index: 0, label: 'Block 1', face },
    attachTo: document.body,
  });
  await flushPromises();
  return w;
}

describe('BlockFontPicker', () => {
  it('is a labelled combobox; focus lists the library, typing narrows it, Enter picks', async () => {
    const w = await picker();
    const input = w.get('input');
    expect(input.attributes('role')).toBe('combobox');
    expect(input.attributes('aria-label')).toBe('Font for Block 1');
    expect(input.attributes('aria-expanded')).toBe('false');
    await input.trigger('focus');
    await flushPromises();
    expect(input.attributes('aria-expanded')).toBe('true');
    expect(w.findAll('[role="option"]').length).toBeGreaterThan(1);
    await input.setValue(mono.familyName);
    await flushPromises();
    const options = w.findAll('[role="option"]');
    expect(options[0]!.text()).toBe(mono.familyName);
    expect(input.attributes('aria-activedescendant')).toBe(options[0]!.attributes('id'));
    await input.trigger('keydown', { key: 'Enter' });
    await flushPromises();
    expect(useStudioStore().faceOf('book', 0)?.family).toBe(mono.family);
    expect(input.attributes('aria-expanded')).toBe('false');
  });

  it('Escape closes an open list without reaching the studio; a closed list lets it through', async () => {
    const w = await picker();
    const input = w.get('input');
    const outside = vi.fn();
    document.body.addEventListener('keydown', outside);
    await input.trigger('focus');
    await flushPromises();
    await input.trigger('keydown', { key: 'Escape' });
    expect(input.attributes('aria-expanded')).toBe('false');
    expect(outside).not.toHaveBeenCalled();
    await input.trigger('keydown', { key: 'Escape' });
    expect(outside).toHaveBeenCalledOnce();
    document.body.removeEventListener('keydown', outside);
    w.unmount();
  });

  it('shows the block font; "Use panel font" resets the block', async () => {
    const studio = useStudioStore();
    await studio.setBlockFace('book', 0, mono.id);
    const w = await picker();
    expect((w.get('input').element as HTMLInputElement).value).toBe(
      `${mono.familyName} ${mono.style}`,
    );
    const reset = w.get('[data-testid="block-font-reset"]');
    expect(reset.attributes('aria-label')).toBe('Use panel font for Block 1');
    await reset.trigger('click');
    expect(studio.faceOf('book', 0)).toBeNull();
  });

  it('without a block font there is no reset button and the box shows the placeholder', async () => {
    const w = await picker(null);
    expect(w.find('[data-testid="block-font-reset"]').exists()).toBe(false);
    expect(w.get('input').attributes('placeholder')).toBe('Panel font');
  });
});
