<script setup lang="ts">
// 'Similar to <base>' (E48.3): the second list of the Compare sheet, in place of Pairings
// (pairing is answered on the Family page, E42.7, and in the Studio, E45.3). It lists the
// faces `findSimilar` ranks closest to the base, best first, each in the comparison's own
// sample text at half the compare size, so the list continues the comparison: one text,
// one scale. No match percent: the rows sit under a running header naming the reason they
// share, said once per group.
//
// It reads as a second list, not more of the set: a full-width sunken band (`--fm-surface`)
// with white cards. A row's text is not a control; the compare icon (E48.4) on its right adds
// the face to the set (filled once added; a click again removes it). With a full set the
// icon is disabled, the reason in its title. A face already in the set is listed once, filled.
import { computed, watch } from 'vue';
import type { FaceSummary } from '../api/types';
import { errorMessage, t } from '../i18n';
import { fontNameSample } from '../preview/sampleText';
import { MAX_PINS, useCompareStore } from '../stores/compare';
import { groupByReason, useCompareSimilarStore } from '../stores/compareSimilar';
import CompareToggle from './CompareToggle.vue';

const props = defineProps<{
  /** The compare size in px; the rows show the text at half of it. */
  size: number;
  /** The comparison's sample text for a face. */
  textOf: (face: FaceSummary) => string;
  /** The face's loaded font as a CSS `font-family`, or undefined until it has loaded. */
  fontOf: (face: FaceSummary) => string | undefined;
  lineHeight: number;
  trackingEm: number;
}>();
const emit = defineEmits<{
  /** A row's icon added its face to the set (the sheet plays the row-to-face move). */
  added: [face: FaceSummary];
}>();

const compare = useCompareStore();
const similar = useCompareSimilarStore();

// A new base ('Only this', a fresh C, E48.5) asks again.
watch(
  () => compare.base?.id,
  () => void similar.load(compare.base),
  { immediate: true },
);

const baseName = computed(() => similar.base?.familyName ?? '');
// The title around the base's name, so the name sits in a <bdi> (a Hebrew or Arabic family
// name keeps its own direction) wherever the language puts it.
const MARK = '\u0000';
const titleParts = computed(() => {
  const [before = '', after = ''] = t('compare.similar.title', { name: MARK }).split(MARK);
  return { before, after };
});

const groups = computed(() => groupByReason(similar.results, similar.base));
const rowSize = computed(() => Math.round(props.size * 0.5));
const full = computed(() => compare.pins.length >= MAX_PINS);

/** Why the icon is disabled: the set is full and the face is not in it. */
function disabledReason(face: FaceSummary): string | undefined {
  return full.value && !compare.isPinned(face.id)
    ? t('compare.similar.full', { max: MAX_PINS, name: fontNameSample(face) })
    : undefined;
}

function onToggle(face: FaceSummary): void {
  if (compare.toggle(face) === 'pinned') emit('added', face);
}
</script>

<template>
  <section
    v-if="similar.base"
    class="band"
    role="region"
    aria-labelledby="fm-compare-similar-title"
    data-testid="compare-similar"
  >
    <header class="head">
      <h3 id="fm-compare-similar-title" class="title">
        {{ titleParts.before }}<bdi>{{ baseName }}</bdi
        >{{ titleParts.after }}
      </h3>
      <p class="hint">{{ t('compare.similar.hint') }}</p>
    </header>
    <p v-if="similar.loading" class="message" role="status">
      {{ t('compare.similar.loading') }}
    </p>
    <p v-else-if="similar.error" class="message error" role="alert">
      {{ t('compare.similar.error', { message: errorMessage(similar.error) }) }}
    </p>
    <p v-else-if="!groups.length" class="message" role="status">
      {{ t('compare.similar.none', { name: fontNameSample(similar.base) }) }}
    </p>
    <div v-else class="groups" role="list" :aria-label="t('compare.similar.list')">
      <div
        v-for="(g, gi) in groups"
        :key="g.key"
        class="group"
        role="listitem"
        data-testid="similar-group"
      >
        <h4 :id="`fm-compare-similar-${gi}`" class="reason" data-testid="similar-reason">
          {{ g.key || t('compare.similar.otherReason') }}
        </h4>
        <ul class="rows" :aria-labelledby="`fm-compare-similar-${gi}`">
          <li
            v-for="r in g.rows"
            :key="r.face.id"
            class="row"
            :data-face="r.face.id"
            data-testid="similar-row"
          >
            <div class="text">
              <p
                class="row-sample"
                dir="auto"
                :style="{
                  fontFamily: fontOf(r.face),
                  fontSize: `${rowSize}px`,
                  lineHeight,
                  letterSpacing: `${trackingEm}em`,
                }"
              >
                {{ textOf(r.face) }}
              </p>
              <p class="meta">
                <bdi>{{ r.face.familyName }}</bdi> {{ r.face.style }}
              </p>
            </div>
            <CompareToggle
              manual
              data-testid="similar-compare"
              :face="r.face"
              :disabled-reason="disabledReason(r.face)"
              @toggle="onToggle(r.face)"
            />
          </li>
        </ul>
      </div>
    </div>
  </section>
</template>

<style scoped>
/* The sunken paper band: full width of the sheet (the body's side padding is undone). */
.band {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
  margin-inline: calc(-1 * var(--fm-space-4));
  margin-block-end: calc(-1 * var(--fm-space-4));
  padding: var(--fm-space-5) var(--fm-space-4) var(--fm-space-6);
  border-block-start: 1px solid var(--fm-border);
  background-color: var(--fm-surface);
}
.head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--fm-space-1) var(--fm-space-3);
}
.title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}
.hint {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.message {
  margin: 0;
  color: var(--fm-muted);
}
.error {
  color: var(--fm-danger);
}
.groups {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-4);
}
.group {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
}
/* The running header: small, muted, said once for the rows under it. */
.reason {
  margin: 0;
  color: var(--fm-muted);
  font-size: 12px;
  font-weight: 500;
}
.reason::first-letter {
  text-transform: uppercase;
}
.rows {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
/* A white card with a hairline ring. */
.row {
  display: flex;
  align-items: center;
  gap: var(--fm-space-3);
  padding-block: var(--fm-space-3);
  padding-inline: var(--fm-space-4) var(--fm-space-3);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
  box-shadow: 0 0 0 1px var(--fm-border);
}
.text {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--fm-space-1);
  min-width: 0;
}
.row-sample {
  margin: 0;
  overflow: hidden;
  color: var(--fm-fg);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.meta {
  margin: 0;
  overflow: hidden;
  color: var(--fm-muted);
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
