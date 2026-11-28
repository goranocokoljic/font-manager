import { describe, expect, it } from 'vitest';
import type { FullSettings } from './client';
import { applyInterfaceSize, interfaceFactor } from './interfaceSize';

const CHOICES = [
  { size: 'default', factor: 1 },
  { size: 'large', factor: 1.25 },
  { size: 'extraLarge', factor: 1.5 },
] as FullSettings['interfaceSizes'];

const settings = (size: string) =>
  ({ interfaceSize: size, interfaceSizes: CHOICES }) as unknown as FullSettings;

describe('the interface size on the page (E58.7)', () => {
  it('takes the factor fm-api gives for the size set', () => {
    expect(interfaceFactor(settings('large'))).toBe(1.25);
    expect(interfaceFactor(settings('extraLarge'))).toBe(1.5);
    expect(interfaceFactor(settings('default'))).toBe(1);
    expect(interfaceFactor(null)).toBe(1);
  });

  it('in the app sets only the factor: the shell zooms the webview', () => {
    const root = document.createElement('div');
    applyInterfaceSize(1.5, root, true);
    expect(root.style.getPropertyValue('--fm-interface-factor')).toBe('1.5');
    expect(root.style.zoom).toBe('');
  });

  it('in browser mode zooms the root as the webview would, and back', () => {
    const root = document.createElement('div');
    applyInterfaceSize(1.25, root, false);
    expect(root.style.getPropertyValue('--fm-interface-factor')).toBe('1.25');
    expect(root.style.zoom).toBe('1.25');
    applyInterfaceSize(1, root, false);
    expect(root.style.getPropertyValue('--fm-interface-factor')).toBe('1');
    expect(root.style.zoom).toBe('');
  });
});
