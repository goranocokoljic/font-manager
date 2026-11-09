import { describe, expect, it } from 'vitest';
import type { ThemeDefinition, ThemeTokens } from '../api/types';
import { clearTokens, findTheme, resolveScheme, tokenStyle, writeTokens } from './tokens';

const tokens: ThemeTokens = {
  accent: '#4f7033',
  onAccent: '#ffffff',
  bg: '#fefefb',
  surface: '#f6f7f1',
  surface2: '#fafaf6',
  border: '#2a331a17',
};

const theme = (id: string): ThemeDefinition => ({ id, name: id, light: tokens, dark: tokens });

describe('theme tokens (E47.2)', () => {
  it('maps every token to its --fm-* property', () => {
    expect(tokenStyle(tokens)).toEqual({
      '--fm-accent': '#4f7033',
      '--fm-on-accent': '#ffffff',
      '--fm-bg': '#fefefb',
      '--fm-surface': '#f6f7f1',
      '--fm-surface-2': '#fafaf6',
      '--fm-border': '#2a331a17',
    });
  });

  it('writes them onto an element as inline custom properties', () => {
    const el = document.createElement('div');
    writeTokens(el, tokens);
    expect(el.style.getPropertyValue('--fm-surface-2')).toBe('#fafaf6');
    expect(el.style.getPropertyValue('--fm-on-accent')).toBe('#ffffff');
    el.style.setProperty('--fm-fg', '#111111');
    clearTokens(el);
    expect(el.style.getPropertyValue('--fm-surface-2')).toBe('');
    expect(el.style.getPropertyValue('--fm-fg'), 'other properties stay').toBe('#111111');
  });

  it.each([
    ['light', false, 'light'],
    ['light', true, 'light'],
    ['dark', false, 'dark'],
    ['system', false, 'light'],
    ['system', true, 'dark'],
  ] as const)('the %s setting with an OS in dark %s uses the %s set', (setting, os, want) => {
    expect(resolveScheme(setting, os)).toBe(want);
  });

  it('finds a theme by id and falls back to the first', () => {
    const list = [theme('paper'), theme('moss')];
    expect(findTheme(list, 'moss')?.id).toBe('moss');
    expect(findTheme(list, 'gone')?.id).toBe('paper');
    expect(findTheme(list, undefined)?.id).toBe('paper');
    expect(findTheme([], 'moss')).toBeUndefined();
  });
});
