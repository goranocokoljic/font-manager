// UXP host collections (`@fm/plugin-shared`'s, which `@fm/font-switch`
// shares) and the names the hosts give.

export { elements, type HostCollection } from '@fm/plugin-shared';

/** A name the host gave, or nothing for an empty one. */
export function text(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() !== '' ? value : undefined;
}
