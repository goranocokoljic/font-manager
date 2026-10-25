<script setup lang="ts">
// The coverage list of the Glyphs tab (E11.3): the Unicode blocks the face maps
// anything of, with covered / assigned code points, and the language presets it
// covers, complete ones first, as fm-core lists them. A block is a button that jumps
// the glyph map to it. A language is a disclosure button: a partial one shows the
// characters it lacks (fm-core's `check_coverage`), a complete one says so. Every row
// is a button, so Tab reaches each one, and each has a label that reads out its
// numbers and its `locl` support.
import { computed, ref } from 'vue';
import type { BlockCoverage, FaceLanguageCoverage } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useCoverageStore } from '../stores/coverage';

const emit = defineEmits<{
  /** The user chose a block: jump the glyph map to it. */
  block: [name: string];
}>();

const store = useCoverageStore();
const open = ref(new Set<string>());

const blocks = computed(() => store.coverage?.blocks ?? []);
const languages = computed(() => store.coverage?.languages ?? []);

// The visible counts read as words ("95 of 128"), so each row's label starts with
// what the row shows (WCAG 2.5.3, label in name).
const ofCount = (covered: number, count: number) => t('coverage.ofCount', { covered, count });
const isComplete = (l: FaceLanguageCoverage) => l.covered === l.total;

function blockLabel(b: BlockCoverage): string {
  return t('coverage.blockLabel', { name: b.name, covered: b.covered, count: b.assigned });
}

function languageLabel(l: FaceLanguageCoverage): string {
  const amount = isComplete(l)
    ? t('coverage.complete')
    : t('coverage.ofCharacters', { covered: l.covered, count: l.total });
  return t(l.hasLocl ? 'coverage.languageLabelLocl' : 'coverage.languageLabel', {
    name: l.name,
    amount,
  });
}

const panelId = (l: FaceLanguageCoverage) => `fm-coverage-${l.code}`;

function toggle(l: FaceLanguageCoverage): void {
  const next = new Set(open.value);
  if (next.has(l.code)) next.delete(l.code);
  else {
    next.add(l.code);
    if (!isComplete(l)) void store.loadMissing(l.code);
  }
  open.value = next;
}

/** The characters `l` lacks, once fm-core answers. */
const missingOf = (l: FaceLanguageCoverage) => store.missing.get(l.code);
</script>

<template>
  <section class="coverage" aria-labelledby="fm-coverage-title">
    <h3 id="fm-coverage-title">{{ t('coverage.title') }}</h3>
    <p v-if="store.error" class="muted" role="alert">{{ errorMessage(store.error) }}</p>
    <p v-else-if="store.loading" class="muted" role="status">{{ t('coverage.counting') }}</p>
    <template v-else-if="store.coverage">
      <h4 id="fm-coverage-blocks">{{ t('coverage.blocks') }}</h4>
      <p v-if="!blocks.length" class="muted">{{ t('coverage.noBlocks') }}</p>
      <ul v-else class="rows" aria-labelledby="fm-coverage-blocks">
        <li v-for="b in blocks" :key="b.name">
          <button
            type="button"
            class="row"
            :aria-label="blockLabel(b)"
            :title="t('coverage.showInMap', { name: b.name })"
            @click="emit('block', b.name)"
          >
            <span class="name">{{ b.name }}</span
            >{{ ' ' }}
            <span class="count">{{ ofCount(b.covered, b.assigned) }}</span>
          </button>
        </li>
      </ul>
      <h4 id="fm-coverage-languages">{{ t('coverage.languages') }}</h4>
      <p v-if="!languages.length" class="muted">{{ t('coverage.noLanguages') }}</p>
      <ul v-else class="rows" aria-labelledby="fm-coverage-languages">
        <li v-for="l in languages" :key="l.code">
          <button
            type="button"
            class="row"
            :aria-label="languageLabel(l)"
            :aria-expanded="open.has(l.code)"
            :aria-controls="panelId(l)"
            @click="toggle(l)"
          >
            <span class="name">{{ l.name }}</span
            >{{ ' ' }}
            <span class="count">{{
              isComplete(l) ? t('coverage.complete') : ofCount(l.covered, l.total)
            }}</span>
            <template v-if="l.hasLocl"
              >{{ ' '
              }}<span class="badge" :title="t('coverage.hasLocl')">{{
                t('coverage.locl')
              }}</span></template
            >
          </button>
          <div v-if="open.has(l.code)" :id="panelId(l)" class="panel">
            <p v-if="isComplete(l)" class="muted">
              {{ t('coverage.allCovered', { count: l.total }) }}
            </p>
            <p v-else-if="store.missingErrors.get(l.code)" class="muted" role="alert">
              {{ errorMessage(store.missingErrors.get(l.code)) }}
            </p>
            <p v-else-if="!missingOf(l)" class="muted" role="status">
              {{ t('coverage.listing') }}
            </p>
            <p v-else class="missing" :aria-label="t('coverage.missingFrom', { name: l.name })">
              <span v-for="c in missingOf(l)" :key="c" class="char">{{ c }}</span>
            </p>
          </div>
        </li>
      </ul>
    </template>
  </section>
</template>

<style scoped>
.coverage {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
h3 {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
h4 {
  margin: 6px 0 0;
  font-size: 12px;
  font-weight: 600;
}
.muted {
  margin: 0;
  color: var(--fm-muted);
}
.rows {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}
.row {
  display: flex;
  gap: 8px;
  align-items: baseline;
  width: 100%;
  padding: 2px 6px;
  border: none;
  border-radius: 4px;
  background: none;
  color: var(--fm-fg);
  font: inherit;
  text-align: start;
}
.row:hover {
  background: var(--fm-surface);
}
.row:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.name {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}
.count {
  color: var(--fm-muted);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.badge {
  padding: 0 4px;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  font-family: ui-monospace, monospace;
  font-size: 11px;
}
.panel {
  padding-block: 2px 6px;
  padding-inline: 18px 6px;
}
.missing {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 8px;
  margin: 0;
}
.char {
  font-size: 16px;
}
</style>
