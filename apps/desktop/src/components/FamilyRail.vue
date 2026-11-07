<script setup lang="ts">
// The family page's rail (E42.9): the languages the face covers in full as chips, a line of the
// language's distinctive glyphs in the face, the top two pairings, the family's facts and the
// "Technical details" expander. A chip previews the whole page in that language's pangram; the
// page reads the chosen language from the familyRail store.
import { computed, ref } from 'vue';
import type {
  Collection,
  CollectionId,
  FaceDetail,
  FaceId,
  Family,
  FamilySummary,
  LanguagePreset,
  PairingSuggestion,
} from '../api/types';
import { formatNumber, t } from '../i18n';
import { fileSizeText } from '../list/columns';
import { useFamilyRailStore } from '../stores/familyRail';
import FamilySample from './FamilySample.vue';
import FamilyTechnical from './FamilyTechnical.vue';
import UiChevron from './ui/UiChevron.vue';

const props = defineProps<{
  /** The face the page shows. */
  face: FaceId | null;
  /** Its detail, once it has loaded. */
  detail: FaceDetail | null;
  /** The family, once it has loaded. */
  family: Family | null;
  /** The family's list row. */
  summary: FamilySummary | null;
  /** Every language preset, for the pangrams and glyph lines. */
  presets: readonly LanguagePreset[];
  /** Every collection, for the names of those the family is in. */
  collections: readonly Collection[];
  /** The page's specimen text: the glyph line's stand-in when no language has glyphs. */
  text: string;
  /** The `font-variation-settings` and `font-feature-settings` the hero uses. */
  variation: string;
  features?: string;
}>();

const emit = defineEmits<{
  /** "Open in Pairing Studio", with the top pairing. */
  studio: [suggestion: PairingSuggestion | null];
  /** A collection under "Used in". */
  collection: [id: CollectionId];
}>();

const rail = useFamilyRailStore();

const presetOf = (code: string | null) => props.presets.find((p) => p.code === code) ?? null;
const chosen = computed(() => presetOf(rail.language));
const chosenName = computed(
  () => rail.complete.find((l) => l.code === rail.language)?.name ?? chosen.value?.name ?? '',
);
/** The glyph line: the chosen language's, else that of the first complete one that has one. */
const glyphs = computed(() => {
  if (rail.language !== null) return chosen.value?.sampleGlyphs ?? null;
  for (const l of rail.complete) {
    const line = presetOf(l.code)?.sampleGlyphs;
    if (line) return line;
  }
  return null;
});

const DASH = '—';
const joined = (parts: (string | null | undefined)[]) =>
  parts.filter((p): p is string => !!p).join(' · ');

const designer = computed(() => props.detail?.designer ?? props.family?.designer ?? null);
const foundry = computed(() => props.summary?.foundry ?? props.detail?.foundry ?? null);
const license = computed(() => props.detail?.license ?? null);
const licenseName = computed(() => {
  const l = license.value;
  if (!l) return null;
  if (l.openLicense) return t(`info.openLicenses.${l.openLicense}`);
  return l.description ?? (l.declared ? t(`license.types.${l.declared}`) : null);
});
/** Web use as fm-core judges it; embedding from the font's own `fsType`. */
const marks = computed(() => {
  const l = license.value;
  if (!l) return [];
  return [
    { key: 'web', ok: l.webUse?.ok ?? false, what: t('family.railFacts.web') },
    { key: 'embed', ok: l.embedding !== 'restricted', what: t('family.railFacts.embed') },
  ].map((m) => ({
    ...m,
    text: t(m.ok ? 'family.railFacts.yes' : 'family.railFacts.no', { what: m.what }),
    label: t(m.ok ? 'family.railFacts.allowed' : 'family.railFacts.notAllowed', { what: m.what }),
  }));
});
const format = computed(() => {
  const s = props.summary;
  if (!s) return null;
  const fam = props.family;
  return joined([
    s.formats.map((f) => t(`info.formats.${f}`)).join(', '),
    s.isVariable ? t('family.info.variable') : null,
    fam?.fileCount ? t('family.railFacts.files', { count: fam.fileCount }) : null,
    fam?.fileBytes ? fileSizeText(fam.fileBytes) : null,
  ]);
});
/** The collections the family is in, in the sidebar's order. */
const usedIn = computed(() => {
  const held = new Set(props.family?.collections ?? []);
  return props.collections.filter((c) => held.has(c.id));
});

const developerOpen = ref(false);
</script>

<template>
  <section
    v-if="rail.complete.length"
    class="languages"
    data-testid="rail-languages"
    aria-labelledby="fm-rail-languages"
  >
    <h3 id="fm-rail-languages" class="label">
      {{ t('family.railLanguages', { count: rail.complete.length }) }}
    </h3>
    <p class="hint">{{ t('family.railLanguagesHint') }}</p>
    <div class="chips">
      <button
        v-for="l in rail.shown"
        :key="l.code"
        type="button"
        class="chip"
        data-testid="rail-language"
        :data-code="l.code"
        :aria-pressed="rail.language === l.code"
        @click="rail.choose(l.code)"
      >
        {{ l.name }}
      </button>
      <button
        v-if="rail.more"
        type="button"
        class="chip more"
        data-testid="rail-languages-more"
        @click="rail.expanded = true"
      >
        {{ t('family.railMore', { count: rail.more }) }}
      </button>
    </div>
  </section>
  <FamilySample
    v-if="face !== null"
    class="glyphs"
    data-testid="family-rail-sample"
    :face="face"
    :text="glyphs ?? text"
    :variation="variation"
    :features="features"
  />
  <div v-if="rail.language !== null" class="chosen" data-testid="rail-language-chosen">
    <p class="hint">{{ t('family.railPreviewing', { language: chosenName }) }}</p>
    <button type="button" class="link" data-testid="rail-my-text" @click="rail.choose(null)">
      {{ t('family.railMyText') }}
    </button>
    <button
      type="button"
      class="offer"
      data-testid="rail-filter-wall"
      :aria-pressed="rail.filterWall"
      @click="rail.filterWall = !rail.filterWall"
    >
      <span class="box" aria-hidden="true" />
      {{ t('family.railFilterWall', { language: chosenName }) }}
    </button>
  </div>

  <section
    v-if="rail.pairings.length"
    class="pairs"
    data-testid="rail-pairings"
    aria-labelledby="fm-rail-pairs"
  >
    <h3 id="fm-rail-pairs" class="label">{{ t('family.railPairs') }}</h3>
    <ul>
      <li v-for="p in rail.pairings" :key="p.face.id" class="pair" data-testid="rail-pairing">
        <span class="pair-name">{{ p.face.familyName }}</span>
        <span v-if="p.reasons[0]" class="pair-reason">{{ p.reasons[0] }}</span>
      </li>
    </ul>
    <button
      type="button"
      class="link"
      data-testid="rail-open-studio"
      @click="emit('studio', rail.pairings[0] ?? null)"
    >
      {{ t('family.railOpenStudio') }}
    </button>
  </section>

  <dl class="facts" data-testid="rail-facts">
    <dt>{{ t('family.designer') }}</dt>
    <dd data-testid="rail-designer">{{ designer ?? DASH }}</dd>
    <dt>{{ t('family.foundry') }}</dt>
    <dd data-testid="rail-foundry">{{ foundry ?? DASH }}</dd>
    <dt>{{ t('family.railFacts.licence') }}</dt>
    <dd data-testid="rail-licence">
      {{ licenseName ?? DASH }}
      <span v-for="m in marks" :key="m.key" class="mark" :data-mark="m.key" :data-ok="m.ok"
        ><span aria-hidden="true">{{ m.text }}</span
        ><span class="sr-only">{{ m.label }}</span></span
      >
    </dd>
    <dt>{{ t('family.railFacts.format') }}</dt>
    <dd data-testid="rail-format">{{ format || DASH }}</dd>
    <dt>{{ t('family.railFacts.usedIn') }}</dt>
    <dd data-testid="rail-used-in">
      <template v-if="usedIn.length">
        <template v-for="(c, i) in usedIn" :key="c.id">
          <button type="button" class="link" @click="emit('collection', c.id)">
            {{ c.name }}</button
          ><span v-if="i < usedIn.length - 1">{{ t('family.railFacts.separator') }}</span>
        </template>
      </template>
      <template v-else>{{ DASH }}</template>
    </dd>
    <template v-if="summary">
      <dt>{{ t('family.stylesFact') }}</dt>
      <dd class="count">{{ formatNumber(summary.styleCount) }}</dd>
    </template>
  </dl>
  <button
    type="button"
    class="developer fm-directional"
    aria-controls="fm-family-developer"
    :aria-expanded="developerOpen"
    @click="developerOpen = !developerOpen"
  >
    <UiChevron :open="developerOpen" />
    {{ t('family.technical') }}
  </button>
  <!-- Mounted only while open: it asks fm-core for the CSS as the state changes. -->
  <div id="fm-family-developer">
    <FamilyTechnical v-if="developerOpen" :detail="detail" />
  </div>
</template>

<style scoped>
.label {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.hint {
  margin: var(--fm-space-1) 0 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-2);
  margin-block: var(--fm-space-2);
}
.chip {
  padding: var(--fm-space-1) var(--fm-space-3);
  border: 1px solid var(--fm-border);
  border-radius: 999px;
  background-color: var(--fm-surface);
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
/* The chosen language: words on the accent take the paired token (DESIGN.md). */
.chip[aria-pressed='true'] {
  border-color: var(--fm-accent);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
}
.more {
  border-style: dashed;
  color: var(--fm-muted);
}
/* The language's distinctive glyphs, in the face only (FamilySample hides it until loaded). */
.glyphs {
  margin-block: var(--fm-space-2);
  font-size: 22px;
  line-height: 1.3;
}
.chosen {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--fm-space-1) var(--fm-space-3);
}
.chosen .hint {
  flex: 1 1 12em;
  margin: 0;
}
.link {
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  font-size: var(--fm-text-sm);
  text-decoration: underline;
  cursor: pointer;
}
.offer {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-2);
  padding: 0;
  border: 0;
  background: none;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
.box {
  display: inline-grid;
  width: 14px;
  height: 14px;
  place-items: center;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
}
.offer[aria-pressed='true'] .box {
  border-color: var(--fm-accent);
  background-color: var(--fm-accent);
  box-shadow: inset 0 0 0 2px var(--fm-on-accent);
}
.pairs {
  margin-block-start: var(--fm-space-4);
}
.pairs ul {
  display: grid;
  gap: var(--fm-space-2);
  margin: var(--fm-space-2) 0;
  padding: 0;
  list-style: none;
}
.pair {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--fm-space-3);
  padding: var(--fm-space-2) var(--fm-space-3);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
}
.pair-reason {
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  text-align: end;
}
.facts {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: var(--fm-space-1) var(--fm-space-3);
  margin: var(--fm-space-4) 0 0;
  font-size: var(--fm-text-sm);
}
.facts dt {
  color: var(--fm-muted);
}
.facts dd {
  margin: 0;
}
.mark::before {
  content: ' · ';
}
.mark[data-ok='false'] {
  color: var(--fm-muted);
}
.count {
  font-variant-numeric: tabular-nums;
}
.developer {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  width: 100%;
  margin-block-start: var(--fm-space-4);
  padding: var(--fm-space-2) 0 0;
  border: 0;
  border-block-start: 1px solid var(--fm-border);
  background: none;
  color: var(--fm-muted);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
</style>
