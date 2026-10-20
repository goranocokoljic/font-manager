import { createPinia, setActivePinia } from 'pinia';
import { beforeEach, describe, expect, it } from 'vitest';
import type { FeatureInfo } from '../api/types';
import { useFeaturesStore } from './features';

const kern: FeatureInfo = { tag: 'kern', name: 'Kerning', automatic: false, defaultOn: true };
const smcp: FeatureInfo = {
  tag: 'smcp',
  name: 'Small capitals',
  automatic: false,
  defaultOn: false,
};
const ccmp: FeatureInfo = {
  tag: 'ccmp',
  name: 'Glyph composition and decomposition',
  automatic: true,
  defaultOn: false,
};
// Old payloads may lack the flags: treated as off and switchable.
const bare: FeatureInfo = { tag: 'xyzw', name: null };

beforeEach(() => setActivePinia(createPinia()));

describe('features store', () => {
  it('starts at the defaults with no settings', () => {
    const s = useFeaturesStore();
    expect(s.isOn(1, kern)).toBe(true);
    expect(s.isOn(1, smcp)).toBe(false);
    expect(s.isOn(1, bare)).toBe(false);
    expect(s.settings(1)).toBeUndefined();
    expect(s.isChanged(1)).toBe(false);
  });

  it('keeps switches per face and writes them as font-feature-settings', () => {
    const s = useFeaturesStore();
    s.set(1, smcp, true);
    s.set(1, kern, false);
    expect(s.isOn(1, smcp)).toBe(true);
    expect(s.isOn(1, kern)).toBe(false);
    expect(s.settings(1)).toBe('"kern" 0, "smcp" 1');
    // Another face is untouched.
    expect(s.isOn(2, smcp)).toBe(false);
    expect(s.settings(2)).toBeUndefined();
  });

  it('forgets a switch put back to its default', () => {
    const s = useFeaturesStore();
    s.set(1, smcp, true);
    s.set(1, smcp, false);
    expect(s.settings(1)).toBeUndefined();
    expect(s.isChanged(1)).toBe(false);
  });

  it('reset restores the defaults of one face only', () => {
    const s = useFeaturesStore();
    s.set(1, smcp, true);
    s.set(2, smcp, true);
    s.reset(1);
    expect(s.isOn(1, smcp)).toBe(false);
    expect(s.settings(1)).toBeUndefined();
    expect(s.settings(2)).toBe('"smcp" 1');
  });

  it('does not switch automatic features', () => {
    const s = useFeaturesStore();
    s.set(1, ccmp, false);
    expect(s.isOn(1, ccmp)).toBe(true);
    expect(s.settings(1)).toBeUndefined();
  });
});
