// The interface size (E58.7, ADR 0029). In the app the shell zooms the webview to the
// factor fm-api gives (`InterfaceSizeChoice.factor`); the page only needs the factor
// on the root, for the surfaces that take the zoom back (`.fm-true-size`, theme.css).
// Browser mode (the mock) has no shell: a CSS zoom on the root stands in for the
// webview's, so the Playwright specs see the same sizes.
import { isTauri } from '@tauri-apps/api/core';
import type { FullSettings } from './client';

/** The variable `.fm-true-size` divides by. */
export const INTERFACE_FACTOR_VAR = '--fm-interface-factor';

/** The factor of the size set, from the choices fm-api sends with the settings; 1 when unknown. */
export function interfaceFactor(settings: FullSettings | null | undefined): number {
  const size = settings?.interfaceSize;
  return settings?.interfaceSizes?.find((c) => c.size === size)?.factor ?? 1;
}

/** Applies a factor to this page, at once. */
export function applyInterfaceSize(
  factor: number,
  root: HTMLElement = document.documentElement,
  shellZooms: boolean = isTauri(),
): void {
  root.style.setProperty(INTERFACE_FACTOR_VAR, String(factor));
  if (!shellZooms) root.style.zoom = factor === 1 ? '' : String(factor);
}
