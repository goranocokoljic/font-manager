import { defineStore } from 'pinia';
import { shallowReactive } from 'vue';
import { useClient } from '../api/client';
import type { CatalogId, CatalogSampleText } from '../api/types';

/** Answers kept; past this many the oldest are forgotten (and asked again when needed). */
export const MAX_CATALOG_SAMPLE_ANSWERS = 2000;

const key = (catalog: CatalogId, family: string, text: string) =>
  `${catalog}\u0000${family}\u0000${text}`;

/**
 * The text each catalog family previews for a sample (E54.6): the catalog's twin of the
 * samples store. The API decides (fm-core reads the family's preview file): the sample with
 * the characters the file lacks, which the rows draw as tofu (E43.4), or text from the
 * file's own coverage. Rows ask for the families on screen only; asks of one tick go out as
 * one call per catalog and sample. A failed ask is forgotten, so a later one tries again.
 */
export const useCatalogSamplesStore = defineStore('catalogSamples', () => {
  const answers = shallowReactive(new Map<string, CatalogSampleText>());
  /** Asked and not answered yet. */
  const asked = new Set<string>();
  let queue = new Map<string, { catalog: CatalogId; text: string; families: Set<string> }>();
  let queued = false;

  /** The answer for `family` of `catalog` and the sample `text`, if it has arrived. */
  function get(catalog: CatalogId, family: string, text: string): CatalogSampleText | undefined {
    return answers.get(key(catalog, family, text));
  }

  /** Asks for the family's preview text for `text` unless it is known or asked already. */
  function want(catalog: CatalogId, family: string, text: string): void {
    if (!text) return;
    const k = key(catalog, family, text);
    if (answers.has(k) || asked.has(k)) return;
    asked.add(k);
    const batch = `${catalog}\u0000${text}`;
    let entry = queue.get(batch);
    if (!entry) queue.set(batch, (entry = { catalog, text, families: new Set() }));
    entry.families.add(family);
    if (!queued) {
      queued = true;
      queueMicrotask(flush);
    }
  }

  function flush(): void {
    queued = false;
    const batch = queue;
    queue = new Map();
    for (const { catalog, text, families } of batch.values()) {
      const names = [...families];
      void useClient()
        .catalogSampleTexts({ id: catalog, families: names, text })
        .then(
          (got) => {
            if (answers.size + got.length > MAX_CATALOG_SAMPLE_ANSWERS) answers.clear();
            for (const s of got) {
              const k = key(catalog, s.family, text);
              asked.delete(k);
              answers.set(k, s);
            }
          },
          () => {
            for (const name of names) asked.delete(key(catalog, name, text));
          },
        );
    }
  }

  return { get, want };
});
