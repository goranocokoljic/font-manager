<script setup lang="ts">
// Pairings (E23.4): a panel inside the Compare sheet, not a dialog of its own. It
// lists the fonts fm-core's rules pair with the first pinned font, best first, each
// with its match as a percent and the reasons; its compare icon (E48.4) adds it to the
// set, or, filled while it is in the set, removes it.
//
// Keys: Tab reaches every compare icon (the sheet's own focus trap). The icon stays in
// place in both states, so the focus is not lost.
import { computed, watch } from 'vue';
import type { PairingSuggestion } from '../api/types';
import { errorMessage, t } from '../i18n';
import { fontNameSample } from '../preview/sampleText';
import { useCompareStore } from '../stores/compare';
import { usePairingsStore } from '../stores/pairings';
import { scorePercent } from '../stores/similar';
import CompareToggle from './CompareToggle.vue';

const compare = useCompareStore();
const pairings = usePairingsStore();

// The base is what the suggestions are for; a new base ('Only this', a fresh C) asks again.
watch(
  () => compare.base?.id,
  () => void pairings.load(compare.base),
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
  pairings.results.map((s, i) => ({
    s,
    id: `fm-pairing-${i}`,
    name: fontNameSample(s.face),
    score: scoreText(s),
  })),
);
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
        <CompareToggle
          data-testid="pairing-compare"
          :face="item.s.face"
          :aria-describedby="`${item.id}-why`"
        />
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
  font-size: var(--fm-text-md);
  font-weight: 600;
}
.hint {
  flex: 1;
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
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
  font-size: var(--fm-text-xs);
}
</style>
