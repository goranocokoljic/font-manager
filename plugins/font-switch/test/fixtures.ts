// Fonts and payloads the adapter tests share.

import type { FontSwitchPayload } from '@fm/plugin-shared';

export const INTER: FontSwitchPayload = {
  postscriptName: 'Inter-Regular',
  family: 'Inter',
  style: 'Regular',
  fileUrl: 'file:///Users/ana/Fonts/Inter-Regular.otf',
};

/** A payload whose PostScript name the host does not list, but its family and style. */
export const LORA_RENAMED: FontSwitchPayload = {
  postscriptName: 'Lora-BoldV2',
  family: 'Lora',
  style: 'Bold',
  fileUrl: 'file:///Users/ana/Fonts/Lora-Bold.ttf',
};

export const UNKNOWN: FontSwitchPayload = {
  postscriptName: 'Nope-Regular',
  family: 'Nope',
  style: 'Regular',
  fileUrl: 'file:///Users/ana/Fonts/Nope-Regular.otf',
};

/** The fonts every mocked host has: PostScript name, family, style. */
export const HOST_FONTS: [string, string, string][] = [
  ['Inter-Regular', 'Inter', 'Regular'],
  ['Inter-Bold', 'Inter', 'Bold'],
  ['Lora-Bold', 'Lora', 'Bold'],
];

export const SELECT_TEXT_FIRST = { applied: false, message: 'Select some text first' };
