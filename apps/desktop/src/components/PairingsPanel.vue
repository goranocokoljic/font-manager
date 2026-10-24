<script setup lang="ts">
// Pairings (E23.4): a panel inside the Compare sheet, not a dialog of its own. It
// lists the fonts fm-core's rules pair with the first pinned font, best first, each
// with its match as a percent and the reasons; "Add to compare" pins one.
//
// Keys: Tab reaches every "Add to compare" button (the sheet's own focus trap). A
// pinned suggestion keeps its button, marked unavailable, so the focus is not lost.
import { computed, watch } from 'vue';
import type { FaceSummary, PairingSuggestion } from '../api/types';
import { errorMessage, t } from '../i18n';
import { fontNameSample } from '../preview/sampleText';
import { useCompareStore } from '../stores/compare';
import { usePairingsStore } from '../stores/pairings';
import { scorePercent } from '../stores/similar';

const compare = useCompareStore();
const pairings = usePairingsStore();

// The first pin is what the suggestions are for; a new first pin asks again.
watch(
  () => compare.pins[0]?.id,
  () => void pairings.load(compare.pins[0] ?? null),
  { immediate: true },
);

const baseName = computed(() => (pairings.base ? fontNameSample(pairings.base) : ''));
// The title around the face's name, so the name sits in a <bdi> (a Hebrew or Arabic
// family name keeps its own direction) wherever the language puts it.
const MARK = '\u0000';
const titleParts = computed(() => {
  const [before = '', after = ''] = t('compare.pairings.title', { name: MARK }).split(MARK);
  return { before, after };
});

function scoreText(s: PairingSuggestion): string {
  const percent = scorePercent(s.score);
  return percent === null ? '' : t('compare.pairings.score', { percent });
}

const items = computed(() =>
  pairings.results.map((s, i) => {
    const name = fontNameSample(s.face);
    const pinned = compare.isPinned(s.face.id);
    return {
      s,
      id: `fm-pairing-${i}`,
      name,
      pinned,
      score: scoreText(s),
      label: pinned
        ? t('compare.pairings.pinnedLabel', { name })
        : t('compare.pairings.addLabel', { name }),
    };
  }),
);

function add(face: FaceSummary, pinned: boolean): void {
  if (!pinned) pairings.add(face);
}
</script>

<template>
  <section
    v-if="pairings.base"
    class="pairings"
    role="region"
    aria-labelledby="fm-pairings-title"
    data-testid="pairings-panel"
  >
    <header class="head">
      <h3 id="fm-pairings-title" class="title">
        {{ titleParts.before }}<bdi>{{ baseName }}</bdi
        >{{ titleParts.after }}
      </h3>
      <p class="hint">{{ t('compare.pairings.hint') }}</p>
    </header>
    <p v-if="pairings.loading" class="message" role="status">
      {{ t('compare.pairings.loading') }}
    </p>
    <p v-else-if="pairings.error" class="message error" role="alert">
      {{ t('compare.pairings.error', { message: errorMessage(pairings.error) }) }}
    </p>
    <p v-else-if="!items.length" class="message" role="status">
      {{ t('compare.pairings.none', { name: baseName }) }}
    </p>
    <ul v-else class="list" :aria-label="t('compare.pairings.list')">
      <li v-for="item in items" :key="item.s.face.id" class="item" :data-face="item.s.face.id">
        <div class="text">
          <span class="name"
            ><bdi>{{ item.name }}</bdi></span
          >
          <span v-if="item.score" class="score" data-testid="pairing-score">{{ item.score }}</span>
          <span :id="`${item.id}-why`" class="why">{{
            t('compare.pairings.reasons', { reasons: item.s.reasons.join(', ') })
          }}</span>
        </div>
        <button
          type="button"
          class="button"
          :aria-label="item.label"
          :aria-describedby="`${item.id}-why`"
          :aria-disabled="item.pinned ? 'true' : undefined"
          @click="add(item.s.face, item.pinned)"
        >
          {{ item.pinned ? t('compare.pairings.pinned') : t('compare.pairings.add') }}
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.pairings {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 12px;
  border-top: 1px solid var(--fm-border);
}
.head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 8px;
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.hint {
  flex: 1;
  margin: 0;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.message {
  margin: 0;
  color: var(--fm-muted);
}
.error {
  color: var(--fm-danger);
}
.list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}
.item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 6px 0;
  border-bottom: 1px solid var(--fm-border);
}
.text {
  display: flex;
  flex: 1;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 4px 10px;
  min-width: 0;
}
.name {
  font-weight: 600;
}
.score {
  color: var(--fm-fg);
}
.why {
  flex-basis: 100%;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.button {
  flex: none;
  padding: 2px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
}
.button:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.button[aria-disabled='true'] {
  opacity: 0.6;
}
</style>
