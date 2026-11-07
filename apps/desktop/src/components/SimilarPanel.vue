<script setup lang="ts">
// "Similar to <family style>" (E23.3): the faces `findSimilar` ranks as closest to
// one face, each previewed in its own font with the view's preview text, its score
// as a percent and fm-core's reasons. It replaces the font list while open
// (a mode, not a split); the source face is pinned on top and the inspector stays.
//
// Keys: Tab reaches the close button and every result; Up/Down (Home/End) move
// between results; Enter or a click selects the result in the list and the
// inspector; Escape closes and gives the focus back to where it was.
import { computed, nextTick, ref, watch, watchEffect, onMounted } from 'vue';
import type { FaceId, FaceSummary, SimilarFace } from '../api/types';
import { errorMessage, t } from '../i18n';
import { sampleText } from '../preview/sampleText';
import { specimenOf } from '../preview/tofu';
import { usePreviewStore, type PreviewSettings } from '../stores/preview';
import { useSamplesStore } from '../stores/samples';
import { scorePercent, useSimilarStore } from '../stores/similar';
import FontPreviewRow from './FontPreviewRow.vue';
import UiIconButton from './ui/UiIconButton.vue';

const props = defineProps<{ preview: PreviewSettings }>();
const emit = defineEmits<{
  /** A result was picked: show it in the inspector. */
  select: [face: FaceId];
}>();

const similar = useSimilarStore();
const previews = usePreviewStore();
const samples = useSamplesStore();
const root = ref<HTMLElement | null>(null);
const list = ref<HTMLElement | null>(null);
/** The picked result's name when the list cannot show it. */
const hidden = ref<string | null>(null);
let returnFocus: HTMLElement | null = null;

const nameOf = (face: FaceSummary) => `${face.familyName} ${face.style}`;
const targetName = computed(() => (similar.target ? nameOf(similar.target) : ''));
// The title around the face's name, so the name is isolated in a <bdi> (a Hebrew or
// Arabic family name keeps its own direction) wherever the language puts it.
const MARK = '\u0000';
const titleParts = computed(() => {
  const [before = '', after = ''] = t('similar.title', { name: MARK }).split(MARK);
  return { before, after };
});

function scoreText(r: SimilarFace): string {
  const percent = scorePercent(r.score);
  return percent === null ? t('similar.noScore') : t('similar.score', { percent });
}

// A reason every result shares says nothing about which one is closer: show only what differs.
const sharedReasons = computed(() => {
  const all = similar.results;
  if (all.length < 2) return new Set<string>();
  const [first, ...rest] = all.map((r) => r.reasons);
  return new Set((first ?? []).filter((x) => rest.every((o) => o.includes(x))));
});

const source = computed(() => {
  const face = similar.target;
  if (!face) return null;
  const sample = sampleText(props.preview, face, previews.pangramText);
  const got = samples.get(face.id, sample);
  return {
    face,
    name: nameOf(face),
    ...specimenOf(sample, got),
  };
});

// One row per family: its closest face (highest score), then "+N styles" for the rest.
const grouped = computed(() => {
  const byFamily = new Map<string, { best: SimilarFace; extra: number }>();
  for (const r of similar.results) {
    const g = byFamily.get(r.face.familyName);
    if (!g) byFamily.set(r.face.familyName, { best: r, extra: 0 });
    else {
      g.extra += 1;
      if ((r.score ?? -1) > (g.best.score ?? -1)) g.best = r;
    }
  }
  return [...byFamily.values()];
});

const items = computed(() =>
  grouped.value.map(({ best: r, extra }, i) => {
    const sample = sampleText(props.preview, r.face, previews.pangramText);
    const got = samples.get(r.face.id, sample);
    return {
      r,
      id: `fm-similar-${i}`,
      name: nameOf(r.face),
      score: scoreText(r),
      more: extra ? t('similar.moreStyles', { count: extra }) : '',
      percent: scorePercent(r.score) ?? 0,
      why: r.reasons.filter((x) => !sharedReasons.value.has(x)).join(', '),
      ...specimenOf(sample, got),
    };
  }),
);

watchEffect(() => {
  if (similar.target)
    samples.want(
      similar.target.id,
      sampleText(props.preview, similar.target, previews.pangramText),
    );
  for (const r of similar.results)
    samples.want(r.face.id, sampleText(props.preview, r.face, previews.pangramText));
});

const buttons = () => [...(list.value?.querySelectorAll<HTMLElement>('.result') ?? [])];

// Opening takes the focus into the panel; the first result gets it once it arrives.
watch(
  () => similar.target,
  async (target) => {
    if (!target) return;
    hidden.value = null;
    const el = document.activeElement;
    if (!root.value?.contains(el)) returnFocus = el instanceof HTMLElement ? el : null;
    await nextTick();
    root.value?.focus();
  },
);
watch(
  () => similar.results,
  async (results) => {
    if (!results.length) return;
    await nextTick();
    if (document.activeElement === root.value) buttons()[0]?.focus();
  },
);

async function select(face: FaceSummary): Promise<void> {
  emit('select', face.id);
  hidden.value = (await similar.pick(face)) ? null : nameOf(face);
}

// Back from a result's family page: the panel is still open, and the keys are its own.
onMounted(() => {
  if (similar.open) root.value?.focus();
});

function close(): void {
  similar.close();
  const target = returnFocus;
  returnFocus = null;
  void nextTick(() => {
    if (target?.isConnected) target.focus();
  });
}

function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    close();
  } else if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) {
    const all = buttons();
    if (!all.length) return;
    const at = all.indexOf(document.activeElement as HTMLElement);
    const next =
      e.key === 'Home'
        ? 0
        : e.key === 'End'
          ? all.length - 1
          : e.key === 'ArrowDown'
            ? Math.min(all.length - 1, at + 1)
            : Math.max(0, at < 0 ? 0 : at - 1);
    all[next]?.focus();
  } else {
    return;
  }
  e.preventDefault();
  e.stopPropagation();
}
</script>

<template>
  <section
    v-if="similar.open"
    ref="root"
    class="similar"
    role="region"
    aria-labelledby="fm-similar-title"
    tabindex="-1"
    data-testid="similar-panel"
    @keydown="onKeyDown"
  >
    <header class="head">
      <h2 id="fm-similar-title" class="title">
        {{ titleParts.before }}<bdi>{{ targetName }}</bdi
        >{{ titleParts.after }}
      </h2>
      <p class="hint">{{ t('similar.hint') }}</p>
      <UiIconButton icon="close" :label="t('similar.close')" @click="close" />
    </header>
    <div v-if="source" class="source" data-testid="similar-source">
      <FontPreviewRow
        class="preview"
        :face="source.face"
        :label="source.name"
        :text="source.text"
        :size-px="Math.min(preview.sizePx, 32)"
        :line-height="preview.lineHeight"
        :tracking-em="preview.trackingEm"
        :align="preview.align"
        :fallback="source.fallback"
        :missing="source.missing"
      />
    </div>
    <p v-if="similar.loading" class="message" role="status">{{ t('similar.loading') }}</p>
    <p v-else-if="similar.error" class="message error" role="alert">
      {{ t('similar.error', { message: errorMessage(similar.error) }) }}
    </p>
    <p v-else-if="!items.length" class="message" role="status">
      {{ t('similar.none', { name: targetName }) }}
    </p>
    <ul v-else ref="list" class="list" :aria-label="t('similar.results')">
      <li v-for="item in items" :key="item.r.face.id">
        <button
          type="button"
          class="result"
          :data-face="item.r.face.id"
          :aria-label="
            t('similar.resultLabel', { name: item.name, score: item.score }) +
            (item.more ? `, ${item.more}` : '')
          "
          :aria-describedby="`${item.id}-why`"
          :aria-current="similar.picked === item.r.face.id ? 'true' : undefined"
          @click="select(item.r.face)"
        >
          <FontPreviewRow
            class="preview"
            :face="item.r.face"
            :label="item.name"
            :text="item.text"
            :size-px="Math.min(preview.sizePx, 32)"
            :line-height="preview.lineHeight"
            :tracking-em="preview.trackingEm"
            :align="preview.align"
            :fallback="item.fallback"
            :missing="item.missing"
          >
            <template #detail>
              <span class="score" data-testid="similar-score">{{ item.score }}</span>
              <span class="bar" aria-hidden="true"
                ><span class="fill" :style="{ inlineSize: `${item.percent}%` }"
              /></span>
              <span v-if="item.more" class="more">{{ item.more }}</span>
            </template>
          </FontPreviewRow>
          <span :id="`${item.id}-why`" class="why">{{
            item.why ? t('similar.reasons', { reasons: item.why }) : ''
          }}</span>
        </button>
      </li>
    </ul>
    <p v-if="hidden" class="message" role="status">
      {{ t('similar.notInList', { name: hidden }) }}
    </p>
  </section>
</template>

<style scoped>
.similar {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  background-color: var(--fm-bg);
}
.similar:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 8px 16px 4px;
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
.source {
  border-bottom: 1px solid var(--fm-border);
  background-color: var(--fm-surface);
}
.bar {
  display: inline-block;
  inline-size: 48px;
  block-size: 3px;
  margin-inline-start: var(--fm-space-2);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-hover);
  vertical-align: middle;
}
.fill {
  display: block;
  block-size: 100%;
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-accent);
}
.more {
  margin-inline-start: var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.message {
  margin: 0;
  padding: 8px 16px 12px;
  color: var(--fm-muted);
}
.error {
  color: var(--fm-danger);
}
.list {
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}
.result {
  display: flex;
  flex-direction: column;
  width: 100%;
  padding: 4px 0 6px;
  border: 0;
  border-bottom: 1px solid var(--fm-border);
  background-color: transparent;
  color: inherit;
  font: inherit;
  text-align: start;
}
.result:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.result[aria-current='true'] {
  background: color-mix(in srgb, var(--fm-accent) 16%, transparent);
}
.preview {
  height: auto;
  border-bottom: 0;
}
.score {
  color: var(--fm-fg);
  font-weight: 600;
}
.why {
  padding: 0 16px;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.why:empty {
  display: none;
}
</style>
