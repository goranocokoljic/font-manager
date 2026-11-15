import { flushPromises, mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setClient, type FmClient } from '../api/client';
import { createMockClient } from '../api/mock';
import type { FamilySummary, Target } from '../api/types';
import { setLocale } from '../i18n';
import { useActivationStore } from '../stores/activation';
import { useToastStore } from '../stores/toast';
import ActivationDot from './ActivationDot.vue';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => false }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

let client: FmClient;

beforeEach(() => {
  setActivePinia(createPinia());
  client = createMockClient();
  setClient(client);
});

afterEach(() => {
  setLocale('en');
  setClient(undefined);
  document.body.innerHTML = '';
});

const target: Target = { kind: 'faces', faces: [7] };

function dot(state: 'off' | 'session' | 'permanent', extra: Record<string, unknown> = {}) {
  return mount(ActivationDot, {
    props: { state, name: 'Lora Italic', target, ...extra },
    attachTo: document.body,
  });
}

describe('ActivationDot (E50.5)', () => {
  it('is a button in three states, each its own shape class', () => {
    for (const state of ['off', 'session', 'permanent'] as const) {
      const w = dot(state);
      const button = w.get('button[data-testid="activation-dot"]');
      expect(button.attributes('type')).toBe('button');
      expect(button.attributes('data-activation')).toBe(state);
      expect(w.get('.dot').classes()).toContain(state);
      w.unmount();
    }
  });

  it('says the state and what a click does (en, de)', () => {
    expect(dot('off').get('button').attributes('aria-label')).toBe(
      'Lora Italic: inactive. Click to activate it for this session.',
    );
    expect(dot('session').get('button').attributes('aria-label')).toBe(
      'Lora Italic: active for this session. Click to deactivate it.',
    );
    expect(dot('permanent').get('button').attributes('aria-label')).toBe(
      'Lora Italic: permanently active. Click for the activation menu.',
    );
    setLocale('de');
    expect(dot('session').get('button').attributes('aria-label')).toBe(
      'Lora Italic: für diese Sitzung aktiv. Klicken, um die Schrift zu deaktivieren.',
    );
    expect(dot('permanent').get('button').attributes('aria-label')).toBe(
      'Lora Italic: dauerhaft aktiv. Klicken für das Aktivierungsmenü.',
    );
  });

  it('a click on an off dot activates for the session, on a session dot deactivates', async () => {
    const store = useActivationStore();
    const toggle = vi.spyOn(store, 'toggleSession').mockResolvedValue();
    await dot('off').get('button').trigger('click');
    expect(toggle).toHaveBeenLastCalledWith(target, false);
    await dot('session').get('button').trigger('click');
    expect(toggle).toHaveBeenLastCalledWith(target, true);
  });

  it('a click on a permanent dot opens its menu instead; the menu deactivates', async () => {
    const store = useActivationStore();
    const toggle = vi.spyOn(store, 'toggleSession').mockResolvedValue();
    const remove = vi.spyOn(store, 'deactivateTarget').mockResolvedValue();
    const w = dot('permanent');
    const button = w.get('button');
    expect(button.attributes('aria-haspopup')).toBe('menu');
    await button.trigger('click');
    await flushPromises();
    expect(toggle).not.toHaveBeenCalled();
    expect(button.attributes('aria-expanded')).toBe('true');
    const menu = document.querySelector('[data-testid="activation-dot-menu"]');
    expect(menu?.getAttribute('role')).toBe('menu');
    (menu?.querySelector('[role="menuitem"]') as HTMLButtonElement).click();
    await flushPromises();
    expect(remove).toHaveBeenCalledWith(target);
    expect(document.querySelector('[data-testid="activation-dot-menu"]')).toBeNull();
  });

  it('keeps its click from the row: no select, no double-click activation', async () => {
    const row = vi.fn();
    vi.spyOn(useActivationStore(), 'toggleSession').mockResolvedValue();
    const w = mount(
      {
        components: { ActivationDot },
        setup: () => ({ row, target }),
        template: `<div @click="row" @dblclick="row" @pointerdown="row">
          <ActivationDot state="off" name="X" :target="target" /></div>`,
      },
      { attachTo: document.body },
    );
    const button = w.get('button');
    await button.trigger('pointerdown');
    await button.trigger('click');
    await button.trigger('dblclick');
    expect(row).not.toHaveBeenCalled();
  });

  it('a catalog dot is off and asks its owner to install', async () => {
    const toggle = vi.spyOn(useActivationStore(), 'toggleSession');
    const w = mount(ActivationDot, { props: { state: 'off', name: 'Roboto', catalog: true } });
    expect(w.get('button').attributes('aria-label')).toBe(
      'Roboto: not in the library. Click to install and activate it.',
    );
    await w.get('button').trigger('click');
    expect(w.emitted('activate')).toHaveLength(1);
    expect(toggle).not.toHaveBeenCalled();
  });

  it('on a Wall card it is no button (an option holds none), yet the pointer toggles it', async () => {
    const toggle = vi.spyOn(useActivationStore(), 'toggleSession').mockResolvedValue();
    const w = dot('session', { pointer: true });
    expect(w.find('button').exists()).toBe(false);
    const el = w.get('[data-testid="activation-dot"]');
    expect(el.element.tagName).toBe('SPAN');
    expect(el.attributes('aria-hidden')).toBe('true');
    expect(el.attributes('tabindex')).toBeUndefined();
    await el.trigger('click');
    expect(toggle).toHaveBeenCalledWith(target, true);
  });

  it('a decorative dot is no button', () => {
    const w = mount(ActivationDot, { props: { state: 'off', decorative: true } });
    expect(w.find('button').exists()).toBe(false);
    expect(w.get('.dot').attributes('aria-hidden')).toBe('true');
  });
});

describe('the activation store behind the dot (E50.5)', () => {
  it('toggles a family for the session with the usual toast and undo', async () => {
    const store = useActivationStore();
    const toasts = useToastStore();
    const family = (await client.listFamilies({})).items[0] as FamilySummary;
    const ids = (await client.getFamily(family.id)).faces.map((f) => f.id);
    const familyTarget: Target = { kind: 'family', family: family.id };

    await store.toggleSession(familyTarget, false);
    expect((await client.listActive()).map((a) => [a.face, a.mode])).toEqual(
      ids.map((id) => [id, 'session']),
    );
    expect(toasts.toast?.message).toMatch(/for this session\.$/);
    expect(toasts.toast?.action?.label).toBe('Undo');

    await store.toggleSession(familyTarget, true);
    expect(await client.listActive()).toEqual([]);
  });

  it('the menu removes a permanent activation', async () => {
    const store = useActivationStore();
    const family = (await client.listFamilies({})).items[0] as FamilySummary;
    const familyTarget: Target = { kind: 'family', family: family.id };
    await client.activate({ target: familyTarget, mode: 'permanent', onConflict: 'replace' });
    const listed = (await client.listFamilies({})).items.find((f) => f.id === family.id);
    expect(listed?.activeMode).toBe('permanent');

    await store.deactivateTarget(familyTarget);
    expect(await client.listActive()).toEqual([]);
    expect(useToastStore().toast?.message).toMatch(/^Deactivated/);
  });
});
