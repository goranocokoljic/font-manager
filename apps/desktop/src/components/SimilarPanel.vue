<script setup lang="ts">
// "Similar to <family style>" (E23.3, E54.10): the faces `findSimilar` ranks as closest to
// one face, each previewed in its own font with the view's preview text, its score as a
// percent and fm-core's reasons. It replaces the font list while open (a mode, not a
// split); the inspector stays. The header says what the panel is (the Similar icon, the
// title at the view titles' size and weight, how many fonts it found) and how to leave it
// ("Back to library", Esc). The reference face sits apart, in its own labelled block, above
// the results. The rows, their percents and their reasons come from the store
// (`useSimilarStore().rows`); this component only lays them out.
//
// Keys: Tab reaches the back and close buttons and every result; Up/Down (Home/End) move
// between results; Enter or a click selects the result in the list and the inspector;
// Escape closes and gives the focus back to where it was.
import { computed, nextTick, ref, watch, watchEffect, onMounted } from 'vue';
import type { FaceId, FaceSummary } from '../api/types';
import { errorMessage, t } from '../i18n';
import { sampleText } from '../preview/sampleText';
import { fallbackTitle, specimenOf } from '../preview/tofu';
import { usePreviewStore, type PreviewSettings } from '../stores/preview';
import { useSamplesStore } from '../stores/samples';
import { useSimilarStore } from '../stores/similar';
import FontPreviewRow from './FontPreviewRow.vue';
import UiButton from './ui/UiButton.vue';
import UiEmptyState from './ui/UiEmptyState.vue';
import UiIcon from './ui/UiIcon.vue';
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
/** How many rows the store has, once the answer is in. */
const count = computed(() =>
  similar.loading || similar.error ? '' : t('similar.count', { count: similar.rows.length }),
);
const emptyTitle = computed(() =>
  similar.empty === 'notMeasured' ? t('similar.notMeasuredTitle') : t('similar.noneTitle'),
);
// fm-core's own words for an empty answer (E54.11); the old sentence when it gave none.
const emptyBody = computed(
  () => similar.emptyMessage ?? t('similar.none', { name: targetName.value }),
);

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

const items = computed(() =>
  similar.rows.map((row, i) => {
    const r = row.result;
    const sample = sampleText(props.preview, r.face, previews.pangramText);
    const specimen = specimenOf(sample, samples.get(r.face.id, sample));
    const score =
      row.percent === null ? t('similar.noScore') : t('similar.score', { percent: row.percent });
    return {
      r,
      id: `fm-similar-${i}`,
      name: nameOf(r.face),
      score,
      more: row.moreStyles ? t('similar.moreStyles', { count: row.moreStyles }) : '',
      percent: row.percent,
      bar: row.percent === null ? score : t('similar.bar', { percent: row.percent }),
      why: row.reasons.length ? t('similar.reasons', { reasons: row.reasons.join(', ') }) : '',
      // "Fallback" in words (E54.10): what the row shows instead of the sample, and why.
      fallbackWords: specimen.fallback ? fallbackTitle(specimen.fallback, specimen.missing) : '',
      ...specimen,
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
      <UiIcon name="similar" class="icon" data-testid="similar-icon" />
      <h2 id="fm-similar-title" class="title">
        {{ titleParts.before }}<bdi>{{ targetName }}</bdi
        >{{ titleParts.after }}
      </h2>
      <span v-if="count" class="count" data-testid="similar-count">{{ count }}</span>
      <span class="spacer" />
      <UiButton
        class="back"
        data-testid="similar-back"
        aria-keyshortcuts="Escape"
        :title="t('similar.backTitle')"
        @click="close"
      >
        {{ t('similar.back') }} <kbd>{{ t('similar.backKey') }}</kbd>
      </UiButton>
      <UiIconButton
        icon="close"
        :label="t('similar.close')"
        :shortcut="t('similar.backKey')"
        @click="close"
      />
    </header>
    <section
      v-if="source"
      class="reference"
      aria-labelledby="fm-similar-reference"
      data-testid="similar-source"
    >
      <h3 id="fm-similar-reference" class="reference-label">{{ t('similar.reference') }}</h3>
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
    </section>
    <p v-if="similar.loading" class="message" role="status">{{ t('similar.loading') }}</p>
    <p v-else-if="similar.error" class="message error" role="alert">
      {{ t('similar.error', { message: errorMessage(similar.error) }) }}
    </p>
    <div v-else-if="!items.length" class="empty" data-testid="similar-empty">
      <div role="status">
        <UiEmptyState :title="emptyTitle" :body="emptyBody" />
      </div>
      <UiButton
        class="back"
        data-testid="similar-empty-back"
        aria-keyshortcuts="Escape"
        @click="close"
      >
        {{ t('similar.back') }} <kbd>{{ t('similar.backKey') }}</kbd>
      </UiButton>
    </div>
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
          :aria-describedby="`${item.id}-why ${item.id}-fallback`"
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
            :fallback-badge="false"
            :missing="item.missing"
          >
            <template #detail>
              <span class="score" data-testid="similar-score">{{ item.score }}</span>
              <span
                class="bar"
                role="meter"
                data-testid="similar-bar"
                :aria-label="item.bar"
                aria-valuemin="0"
                aria-valuemax="100"
                :aria-valuenow="item.percent ?? undefined"
                ><span class="fill" :style="{ inlineSize: `${item.percent ?? 0}%` }"
              /></span>
              <span v-if="item.more" class="more">{{ item.more }}</span>
            </template>
          </FontPreviewRow>
          <span :id="`${item.id}-why`" class="why">{{ item.why }}</span>
          <span :id="`${item.id}-fallback`" class="why" data-testid="similar-fallback">{{
            item.fallbackWords
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
/* The view titles' row (AppToolbar): a 13 px semibold title, a muted count, the actions at the end. */
.head {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--fm-space-2);
  min-height: 40px;
  padding-inline: var(--fm-space-4) var(--fm-space-2);
  border-bottom: 1px solid var(--fm-border);
}
.icon {
  flex: none;
  color: var(--fm-muted);
}
.title {
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: var(--fm-text-md);
  font-weight: 600;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.count {
  flex: none;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.spacer {
  flex: 1;
}
.back kbd {
  margin-inline-start: var(--fm-space-1);
}
/* The reference: its own block on the surface, under a small label, apart from the results. */
.reference {
  flex: none;
  padding-block-start: var(--fm-space-2);
  border-bottom: 1px solid var(--fm-border-strong);
  background-color: var(--fm-surface);
}
.reference-label {
  margin: 0;
  padding: 0 var(--fm-space-4);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.empty {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--fm-space-3);
  padding: var(--fm-space-4);
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
