// UXP host collections: InDesign's are `length` plus `item(i)`, Photoshop's
// are arrays.

export type HostCollection<T> = readonly T[] | { length: number; item(index: number): T };

export function elements<T>(collection: HostCollection<T> | null | undefined): T[] {
  if (!collection) return [];
  if (Array.isArray(collection)) return [...collection];
  const c = collection as { length: number; item(index: number): T };
  return Array.from({ length: c.length }, (_, i) => c.item(i));
}

/** A name the host gave, or nothing for an empty one. */
export function text(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() !== '' ? value : undefined;
}
