import { describe, expect, it } from 'vitest';
import { HOST_FONT_NAMES, findHostFont, hostFromName, notActive } from '../src/adapter';
import { INTER, LORA_RENAMED } from './fixtures';

const fonts = [
  { postscriptName: 'Inter-Regular', family: 'Inter', style: 'Regular' },
  { postscriptName: 'Lora-Bold', family: 'Lora', style: 'Bold' },
  { postscriptName: 'Inter-Display', family: 'Inter', style: 'Regular' },
];

describe('findHostFont', () => {
  it('finds the PostScript name first', () => {
    expect(findHostFont(fonts, HOST_FONT_NAMES, INTER)).toBe(fonts[0]);
    expect(findHostFont(fonts, HOST_FONT_NAMES, 'Inter-Display')).toBe(fonts[2]);
  });

  it('then the family and style of a payload', () => {
    expect(findHostFont(fonts, HOST_FONT_NAMES, LORA_RENAMED)).toBe(fonts[1]);
  });

  it('matches nothing else: no case folding, no family for a plain name', () => {
    expect(findHostFont(fonts, HOST_FONT_NAMES, 'inter-regular')).toBeNull();
    expect(findHostFont(fonts, HOST_FONT_NAMES, 'Lora')).toBeNull();
    expect(findHostFont(fonts, HOST_FONT_NAMES, { ...LORA_RENAMED, style: '' })).toBeNull();
    expect(findHostFont(fonts, HOST_FONT_NAMES, { ...LORA_RENAMED, family: 'lora' })).toBeNull();
    expect(findHostFont([], HOST_FONT_NAMES, INTER)).toBeNull();
  });
});

describe('hostFromName', () => {
  it.each([
    ['InDesign', 'indesign'],
    ['IDSN', 'indesign'],
    ['Photoshop', 'photoshop'],
    ['ILST', 'illustrator'],
    ['Sketch', 'sketch'],
  ])('knows %s', (name, host) => {
    expect(hostFromName(name)).toBe(host);
  });

  it('knows no other app', () => {
    expect(hostFromName('XD')).toBeNull();
    expect(hostFromName(undefined)).toBeNull();
  });
});

it('names the font that is not active', () => {
  expect(notActive(INTER)).toEqual({ applied: false, message: 'Inter-Regular is not active' });
});
