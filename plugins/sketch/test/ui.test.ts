import { describe, expect, it } from 'vitest';
import { sketchUi } from '../src/ui';
import { mockAlerts, mockUi } from './mock-sketch';

describe('sketchUi', () => {
  it('shows a message at the bottom of the window', () => {
    const ui = mockUi();
    sketchUi(ui, mockAlerts(0).newAlert).notify('1 font missing: Lora-Bold');
    expect(ui.message).toHaveBeenCalledWith('1 font missing: Lora-Bold');
  });

  it('asks with Deactivate first: yes for Deactivate, no for Keep', async () => {
    const yes = mockAlerts(0);
    expect(await sketchUi(mockUi(), yes.newAlert).confirm('Deactivate?')).toBe(true);
    expect(yes.shown[0]).toEqual({
      message: 'Font Manager',
      informative: 'Deactivate?',
      buttons: ['Deactivate', 'Keep'],
    });
    expect(await sketchUi(mockUi(), mockAlerts(1).newAlert).confirm('Deactivate?')).toBe(false);
  });
});
