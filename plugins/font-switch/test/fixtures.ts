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

/**
 * A payload whose family and style are those of Inter-Regular, which the
 * hosts list first: only its PostScript name picks Inter-Display.
 */
export const INTER_DISPLAY: FontSwitchPayload = {
  postscriptName: 'Inter-Display',
  family: 'Inter',
  style: 'Regular',
  fileUrl: 'file:///Users/ana/Fonts/Inter-Display.otf',
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
  ['Inter-Display', 'Inter', 'Regular'],
];

export const SELECT_TEXT_FIRST = { applied: false, message: 'Select some text first' };
