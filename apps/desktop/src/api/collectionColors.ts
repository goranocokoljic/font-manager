// The keys of a collection's colour, as fm-api's `COLLECTION_COLORS` lists them (a test in
// crates/fm-api/tests/mock.rs keeps the two lists and the theme's tokens in step). A key is
// never a colour value: `--fm-collection-<key>` in styles/theme.css draws it, per theme.
export const COLLECTION_COLORS = [
  'coral',
  'amber',
  'green',
  'teal',
  'blue',
  'violet',
  'orchid',
  'slate',
] as const;

/** The CSS colour of a collection's key; nothing for a collection without a colour. */
export function collectionColorVar(key: string | null | undefined): string | undefined {
  return key ? `var(--fm-collection-${key})` : undefined;
}
