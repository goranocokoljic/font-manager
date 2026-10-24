// The path a catalog family's preview has on the app's font scheme (E22.2):
// `catalog-<catalog id>-<the family name's UTF-8 as hex>`. The shell
// (`catalog_font_response` in src-tauri) reads it back and asks the core,
// which fetches the file into the cache folder while the catalog is on.
import type { CatalogId } from './types';

/** The font-scheme path of `family`'s preview in catalog `id`. */
export function catalogFontPath(id: CatalogId, family: string): string {
  const hex = [...new TextEncoder().encode(family)]
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
  return `catalog-${id}-${hex}`;
}
