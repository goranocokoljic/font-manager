// The filters chosen now, for the two places that show them: the chips beside the
// Filters button and the count line above the wall.
import { computed } from 'vue';
import type { SourceId } from '../api/types';
import { t } from '../i18n';
import { useSearchStore } from '../stores/search';
import { useSourcesStore } from '../stores/sources';
import { useTagsStore } from '../stores/tags';
import { chipGroups, chosenLanguages, languageName } from './chips';
import { traitChips, traitLineLabel } from './traits';

export function useActiveFilters() {
  const search = useSearchStore();
  const sources = useSourcesStore();
  const tags = useTagsStore();

  /** A source chip shows the folder's name. */
  function sourceName(id: SourceId): string {
    const path = sources.sources.find((s) => s.id === id)?.path;
    return path?.split(/[\\/]/).filter(Boolean).pop() ?? t('chips.source', { id: String(id) });
  }

  const groups = computed(() => chipGroups(search.filters, search.counts, sourceName, tags.nameOf));
  const languages = computed(() => chosenLanguages(search.filters));
  const chips = computed(() => groups.value.flatMap((g) => g.chips.filter((c) => c.selected)));
  /** The glyph traits chosen on the Letters preset's letters or typed in the box (E49.6). */
  const traits = computed(() => traitChips(search.traits, search.traitCatalog));
  /** The count line's switch hides the families missing glyphs of your text (E43.3). */
  const rendersText = computed(() => search.hiding);
  /** Every chosen filter's name, in the order the chips show them. */
  const labels = computed(() => [
    ...chips.value.map((c) => c.label),
    ...languages.value.map((code) => languageName(search.languages, code)),
    ...traits.value.map(traitLineLabel),
    ...(rendersText.value ? [t('chips.rendersText')] : []),
  ]);

  return { groups, languages, traits, rendersText, chips, labels };
}
