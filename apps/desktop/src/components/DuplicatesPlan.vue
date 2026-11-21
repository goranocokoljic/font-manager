<script setup lang="ts">
// The Plan of the Duplicates module (E56.11 collects it, E56.15 shows and applies it). On
// top, what the draft holds: the identical copies, each decision and each folder pair, every
// one of them can be taken back. Below, before anything moves, the plan as the API makes
// it from the draft (`planCleanupDraft`): the files that go to the trash, the files that are
// hidden, the files that stay grouped by why, the sets answered without moving a file, the
// metadata conflicts with both values and a choice, and the space freed. Apply names the
// number of files and this system's trash. While the plan is applied the progress comes
// from the `cleanupProgress` events, with a Cancel that stops between two files; at the end
// one line says what was done, with Undo.
import { computed, nextTick, ref, watch } from 'vue';
import type { CleanupFile, MetadataConflict, MetadataValue } from '../api/types';
import { errorMessage, formatNumber, t, type MessageKey } from '../i18n';
import { conflictKey, useCleanupStore } from '../stores/cleanup';
import { useDuplicatesStore } from '../stores/duplicates';
import UiButton from './ui/UiButton.vue';
import UiChevron from './ui/UiChevron.vue';

const props = defineProps<{
  trash: string;
  /** "1.4 GB": the module's size text. */
  sizeText: (bytes: number) => string;
}>();
const emit = defineEmits<{ history: [] }>();

const store = useDuplicatesStore();
const cleanup = useCleanupStore();

const o = computed(() => store.overview);
const decisionLabel = (d: (typeof store.draft.decisions & object)[number]) =>
  d.decision.kind === 'copies'
    ? t('duplicatesModule.plan.kind.copies', {
        count: d.decision.copies.filter((c) => c.choice !== 'keep').length,
      })
    : t(`duplicatesModule.plan.kind.${d.decision.kind}`);
const hasDraft = computed(
  () => !!store.draft.decisions?.length || !!store.draft.identical || !!store.draft.pairs?.length,
);

// The plan follows the draft: every change to it is planned again.
watch(
  () => store.draft,
  () => {
    if (!cleanup.run) void cleanup.planDraft();
  },
  { immediate: true, deep: true },
);

const plan = computed(() => cleanup.plan);
const files = (n: number) => t('duplicatesModule.history.files', { count: n });
const name = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** Which groups of the plan are open, by key. */
const open = ref<Record<string, boolean>>({});
const toggle = (key: string) => (open.value = { ...open.value, [key]: !open.value[key] });

const valueText = (v: MetadataValue): string => {
  if (v.kind === 'rating') return t('duplicatesModule.plan.stars', { count: v.stars });
  if (v.kind === 'notes') return v.text;
  const seats = v.seats != null ? t('duplicatesModule.plan.seats', { count: v.seats }) : null;
  return [v.license ?? t('duplicatesModule.plan.noLicense'), seats].filter(Boolean).join(' · ');
};
const fieldText = (c: MetadataConflict) => t(`duplicatesModule.plan.field.${c.field}`);
const faceFile = (face: number) =>
  name(plan.value?.files.find((f) => f.faces.includes(face))?.path ?? '');

const applyLabel = computed(() => {
  const trashed = cleanup.toTrash.length;
  const hidden = cleanup.toHide.length;
  if (trashed && hidden)
    return t('duplicatesModule.plan.applyBoth', {
      trashed: files(trashed),
      hidden: files(hidden),
      trash: props.trash,
    });
  if (trashed) return t('duplicatesModule.plan.applyTrash', { count: trashed, trash: props.trash });
  if (hidden) return t('duplicatesModule.plan.applyHide', { hidden: files(hidden) });
  return t('duplicatesModule.plan.applyAnswers', { count: plan.value?.answers?.length ?? 0 });
});
const canApply = computed(
  () => !!plan.value && (cleanup.leaving > 0 || !!plan.value.answers?.length) && !cleanup.run,
);

const progress = ref<HTMLElement | null>(null);
async function apply(): Promise<void> {
  await cleanup.apply();
  await nextTick();
  (progress.value ?? document.getElementById('dup-result'))?.focus();
}
// The end of the apply: the result line takes the focus from the progress.
watch(
  () => cleanup.result,
  async (r) => {
    if (!r) return;
    await nextTick();
    document.getElementById('dup-result')?.focus();
  },
);

const resultLine = computed(() => {
  const r = cleanup.result;
  if (!r) return '';
  const parts = [t('duplicatesModule.plan.moved', { count: r.trashed, trash: props.trash })];
  if (r.hidden) parts.push(t('duplicatesModule.plan.hiddenCount', { count: r.hidden }));
  const skipped = r.files.filter((f) => f.status === 'skipped' || f.status === 'failed').length;
  if (skipped) parts.push(t('duplicatesModule.plan.skipped', { count: skipped }));
  return parts.join(' · ');
});
const canUndo = computed(() => cleanup.result?.files.some((f) => f.status === 'trashed') ?? false);

const stayLabel = (reason: string) => t(`duplicatesModule.plan.stays.${reason}` as MessageKey);
const sentence = (f: CleanupFile) => f.sentence;
</script>

<template>
  <div class="page" data-testid="duplicates-plan">
    <h2 class="heading">{{ t('duplicatesModule.plan.heading') }}</h2>
    <p class="lead">{{ t('duplicatesModule.plan.note') }}</p>
    <p class="plan-identical">
      <template v-if="store.draft.identical && o">
        {{ t('duplicatesModule.plan.identicalIn', { count: o.identical.files }) }}
      </template>
      <template v-else>{{ t('duplicatesModule.plan.identicalOut') }}</template>
      <UiButton
        v-if="o && o.identical.sets > 0"
        class="outlined"
        :disabled="!!cleanup.run"
        @click="store.includeIdentical(!store.draft.identical)"
      >
        {{
          store.draft.identical
            ? t('duplicatesModule.plan.removeIdentical')
            : t('duplicatesModule.plan.addIdentical')
        }}
      </UiButton>
    </p>
    <h3 class="section-label">{{ t('duplicatesModule.plan.decisions') }}</h3>
    <ul v-if="store.draft.decisions?.length" class="rows decisions">
      <li v-for="d in store.draft.decisions" :key="d.set" class="decision" :data-set="d.set">
        <span>
          <span>{{ d.postscriptName ?? d.set }}</span>
          <span class="muted small">
            {{ t(`duplicatesModule.queue.level.${d.level}`) }} · {{ decisionLabel(d) }}
          </span>
        </span>
        <UiButton
          :aria-label="t('duplicatesModule.plan.takeBackName', { name: d.postscriptName ?? d.set })"
          :disabled="!!cleanup.run"
          @click="store.takeBack(d.set)"
        >
          {{ t('duplicatesModule.plan.takeBack') }}
        </UiButton>
      </li>
    </ul>
    <p v-else class="muted">{{ t('duplicatesModule.plan.empty') }}</p>
    <template v-if="store.draft.pairs?.length">
      <h3 class="section-label">{{ t('duplicatesModule.plan.pairs') }}</h3>
      <ul class="rows decisions" data-testid="plan-pairs">
        <li v-for="p in store.draft.pairs" :key="`${p.keep}|${p.remove}`" class="decision">
          <span class="pair-line">
            {{
              t('duplicatesModule.plan.pairLine', {
                keep: p.keep,
                remove: p.remove,
                count: p.identical,
              })
            }}
          </span>
          <UiButton
            :aria-label="t('duplicatesModule.plan.takeBackName', { name: p.remove })"
            :disabled="!!cleanup.run"
            @click="store.takeBackPair(p.keep, p.remove)"
          >
            {{ t('duplicatesModule.plan.takeBack') }}
          </UiButton>
        </li>
      </ul>
    </template>
    <UiButton
      v-if="hasDraft"
      class="outlined"
      :disabled="!!cleanup.run"
      @click="store.clearDraft()"
    >
      {{ t('duplicatesModule.plan.clear') }}
    </UiButton>

    <p v-if="cleanup.error" role="alert" class="error">{{ errorMessage(cleanup.error) }}</p>

    <!-- The end of the last apply: what was done, and Undo. -->
    <section
      v-if="cleanup.result"
      id="dup-result"
      class="result"
      tabindex="-1"
      data-testid="plan-result"
      :aria-label="t('duplicatesModule.plan.resultLabel')"
    >
      <p class="result-line">
        <strong>{{
          cleanup.result.cancelled
            ? t('duplicatesModule.plan.cancelledTitle')
            : t('duplicatesModule.plan.doneTitle')
        }}</strong>
        {{ resultLine }}
      </p>
      <p v-if="cleanup.result.cancelled" class="muted small">
        {{ t('duplicatesModule.plan.cancelledNote') }}
      </p>
      <div class="result-actions">
        <UiButton
          v-if="canUndo"
          class="outlined"
          :aria-label="t('duplicatesModule.plan.undoLabel', { name: cleanup.result.name })"
          @click="cleanup.undo(cleanup.result.id)"
        >
          {{ t('duplicatesModule.plan.undo') }}
        </UiButton>
        <a href="#" class="link small" @click.prevent="emit('history')">
          {{ t('duplicatesModule.plan.seeHistory') }}
        </a>
      </div>
    </section>

    <!-- The apply in progress, from the events. -->
    <section
      v-if="cleanup.run"
      ref="progress"
      class="progress"
      tabindex="-1"
      data-testid="plan-progress"
      :aria-label="t('duplicatesModule.plan.progressLabel')"
    >
      <progress
        :value="cleanup.run.done"
        :max="cleanup.run.total || 1"
        :aria-label="t('duplicatesModule.plan.progressLabel')"
      />
      <span role="status">{{
        cleanup.cancelling
          ? t('duplicatesModule.plan.cancelling')
          : t('duplicatesModule.plan.progress', {
              done: formatNumber(cleanup.run.done),
              total: formatNumber(cleanup.run.total),
            })
      }}</span>
      <UiButton class="outlined" :disabled="cleanup.cancelling" @click="cleanup.cancel()">
        {{ t('duplicatesModule.plan.cancel') }}
      </UiButton>
    </section>

    <!-- What the plan does, before anything moves. -->
    <section
      v-else-if="plan && (plan.files.length || plan.answers?.length)"
      class="review"
      aria-labelledby="dup-review"
      data-testid="plan-review"
    >
      <h3 id="dup-review" class="review-heading">{{ t('duplicatesModule.plan.review') }}</h3>
      <ul class="rows groups">
        <li v-if="cleanup.toTrash.length" data-group="trash">
          <button
            type="button"
            class="group"
            :aria-expanded="!!open.trash"
            @click="toggle('trash')"
          >
            <span>{{
              t('duplicatesModule.plan.trashGroup', {
                count: cleanup.toTrash.length,
                trash: props.trash,
              })
            }}</span>
            <span class="muted small">{{
              t('duplicatesModule.plan.frees', { size: props.sizeText(plan.frees) })
            }}</span>
            <UiChevron :open="!!open.trash" />
          </button>
          <ul v-if="open.trash" class="files">
            <li v-for="f in cleanup.toTrash" :key="f.path">
              <code class="path"
                ><bdi>{{ f.path }}</bdi></code
              >
              <span class="muted small">{{ sentence(f) }}</span>
            </li>
          </ul>
        </li>
        <li v-if="cleanup.toHide.length" data-group="hide">
          <button type="button" class="group" :aria-expanded="!!open.hide" @click="toggle('hide')">
            <span>{{
              t('duplicatesModule.plan.hideGroup', { count: cleanup.toHide.length })
            }}</span>
            <span class="muted small">{{ t('duplicatesModule.plan.hideNote') }}</span>
            <UiChevron :open="!!open.hide" />
          </button>
          <ul v-if="open.hide" class="files">
            <li v-for="f in cleanup.toHide" :key="f.path">
              <code class="path"
                ><bdi>{{ f.path }}</bdi></code
              >
              <span class="muted small">{{ sentence(f) }}</span>
            </li>
          </ul>
        </li>
        <li v-for="g in cleanup.stays" :key="g.reason" :data-group="`stays-${g.reason}`">
          <button
            type="button"
            class="group"
            :aria-expanded="!!open[g.reason]"
            @click="toggle(g.reason)"
          >
            <span>{{ t('duplicatesModule.plan.stayGroup', { count: g.files.length }) }}</span>
            <span class="muted small">{{ stayLabel(g.reason) }}</span>
            <UiChevron :open="!!open[g.reason]" />
          </button>
          <ul v-if="open[g.reason]" class="files">
            <li v-for="f in g.files" :key="f.path">
              <code class="path"
                ><bdi>{{ f.path }}</bdi></code
              >
              <span class="muted small">{{ sentence(f) }}</span>
            </li>
          </ul>
        </li>
        <li v-if="plan.answers?.length" data-group="answers">
          <button
            type="button"
            class="group"
            :aria-expanded="!!open.answers"
            @click="toggle('answers')"
          >
            <span>{{ t('duplicatesModule.plan.answers', { count: plan.answers.length }) }}</span>
            <span class="muted small">{{ t('duplicatesModule.plan.answersNote') }}</span>
            <UiChevron :open="!!open.answers" />
          </button>
          <ul v-if="open.answers" class="files">
            <li v-for="a in plan.answers" :key="a.set">
              <span>{{ a.postscriptName ?? a.set }}</span>
              <span class="muted small">{{
                t(`duplicatesModule.decisions.answer.${a.answer}`)
              }}</span>
            </li>
          </ul>
        </li>
      </ul>

      <template v-if="plan.conflicts.length">
        <h3 class="section-label">{{ t('duplicatesModule.plan.conflicts') }}</h3>
        <p class="muted small">{{ t('duplicatesModule.plan.conflictsNote') }}</p>
        <ul class="rows conflicts" data-testid="plan-conflicts">
          <li v-for="c in plan.conflicts" :key="conflictKey(c)" class="conflict">
            <fieldset>
              <legend>
                {{ fieldText(c) }} ·
                <bdi>{{ faceFile(c.keptFace) }}</bdi>
              </legend>
              <label>
                <input
                  type="radio"
                  :name="`dup-conflict-${conflictKey(c)}`"
                  :checked="!cleanup.taken[conflictKey(c)]"
                  @change="cleanup.take(c, false)"
                />
                {{ t('duplicatesModule.plan.keepValue', { value: valueText(c.default) }) }}
              </label>
              <label>
                <input
                  type="radio"
                  :name="`dup-conflict-${conflictKey(c)}`"
                  :checked="!!cleanup.taken[conflictKey(c)]"
                  @change="cleanup.take(c, true)"
                />
                {{
                  t('duplicatesModule.plan.takeValue', {
                    value: valueText(c.other),
                    file: faceFile(c.otherFace),
                  })
                }}
              </label>
            </fieldset>
          </li>
        </ul>
      </template>

      <div class="apply-row">
        <button
          type="button"
          class="primary"
          data-testid="plan-apply"
          :disabled="!canApply"
          @click="apply"
        >
          {{ applyLabel }}
        </button>
        <span v-if="plan.frees" class="muted small">{{
          t('duplicatesModule.plan.frees', { size: props.sizeText(plan.frees) })
        }}</span>
      </div>
    </section>
    <p v-else-if="cleanup.planning" class="muted">{{ t('duplicatesModule.plan.planning') }}</p>
  </div>
</template>

<style scoped>
.page {
  max-width: 960px;
  padding: var(--fm-space-6) var(--fm-space-8);
}
.heading {
  margin: 0 0 var(--fm-space-2);
  font-size: var(--fm-text-xl);
}
.lead {
  max-width: 720px;
  margin: 0 0 var(--fm-space-5);
  color: var(--fm-muted);
}
.muted {
  color: var(--fm-muted);
}
.small {
  font-size: var(--fm-text-sm);
}
.section-label {
  margin: var(--fm-space-5) 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.plan-identical {
  display: flex;
  align-items: center;
  gap: var(--fm-space-3);
  margin: 0 0 var(--fm-space-5);
}
.rows {
  margin: 0 0 var(--fm-space-5);
  padding: 0;
  list-style: none;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
}
.rows > li + li {
  border-top: 1px solid var(--fm-border);
}
.decision,
.conflict {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-4);
  padding: var(--fm-space-3) var(--fm-space-4);
}
.decision > span:first-child {
  display: grid;
  gap: 2px;
  min-width: 0;
}
.pair-line {
  overflow-wrap: anywhere;
}
.outlined {
  height: 32px;
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  color: var(--fm-fg);
}
.review {
  margin-top: var(--fm-space-6);
}
.review-heading {
  margin: 0 0 var(--fm-space-3);
  font-size: var(--fm-text-lg);
}
.group {
  display: grid;
  grid-template-columns: 1fr auto auto;
  align-items: center;
  gap: var(--fm-space-4);
  width: 100%;
  padding: var(--fm-space-3) var(--fm-space-4);
  border: 0;
  background-color: transparent;
  color: var(--fm-fg);
  font: inherit;
  text-align: start;
}
.group:hover {
  background-color: var(--fm-hover);
}
.files {
  margin: 0;
  padding: 0 var(--fm-space-4) var(--fm-space-3);
  list-style: none;
}
.files > li {
  display: grid;
  gap: 2px;
  padding: var(--fm-space-2) 0;
}
.path {
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
  overflow-wrap: anywhere;
}
.conflict fieldset {
  display: grid;
  gap: var(--fm-space-2);
  margin: 0;
  padding: 0;
  border: 0;
  min-width: 0;
}
.conflict legend {
  margin-bottom: var(--fm-space-1);
  padding: 0;
  font-weight: 600;
}
.conflict label {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  overflow-wrap: anywhere;
}
.apply-row,
.progress,
.result-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-3);
}
.progress,
.result {
  margin-top: var(--fm-space-6);
  padding: var(--fm-space-4) var(--fm-space-5);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-surface);
}
.progress progress {
  flex: 1;
  min-width: 160px;
}
.result-line {
  margin: 0 0 var(--fm-space-2);
}
.primary {
  display: inline-flex;
  align-items: center;
  height: 36px;
  padding: 0 var(--fm-space-4);
  border: 1px solid var(--fm-accent);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
  font: inherit;
  font-weight: 600;
}
.primary:disabled {
  opacity: 0.6;
}
.link {
  color: var(--fm-fg);
  text-decoration: underline;
  text-underline-offset: 2px;
}
.error {
  color: var(--fm-danger);
}
</style>
