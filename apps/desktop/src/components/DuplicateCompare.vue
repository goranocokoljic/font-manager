<script setup lang="ts">
// Two copies of a duplicate set side by side (E56.13, board 14 "Compare"). The user
// picks both copies of the set; each half shows a specimen in the copy's own face
// (registered with the webview through the asset protocol, a glyph it lacks is tofu),
// or, for a copy that could not be read, the API's message. Languages come first:
// what each copy covers of the user's languages and where it has `locl` forms, then
// what the second copy gains or loses; a language chip sets the specimens to that
// language's sample, with `lang` set so `locl` forms show. Then the glyphs added and
// removed (drawn in the copy that has them), the features and axes, and the metrics
// with the line that text may reflow. Everything shown is the API's answer
// (`compareCopies`); this component only lays it out. Keep this one, Hide, Keep both
// and Not duplicates call the same store actions as the queue.
import { computed, nextTick, onMounted, ref } from 'vue';
import type {
  CopyChoice,
  CopyComparison,
  DuplicateSet,
  GlyphChange,
  LanguagePreset,
  SetDecision,
} from '../api/types';
import { t } from '../i18n';
import { sampleLanguages as languagesToSample } from '../search/languages';
import { useHeldFace } from '../preview/useHeldFace';
import { useSpecimen } from '../preview/useSpecimen';
import SpecimenText from './SpecimenText';
import UiBadge from './ui/UiBadge.vue';
import UiButton from './ui/UiButton.vue';
import UiIconButton from './ui/UiIconButton.vue';
import UiSelect from './ui/UiSelect.vue';

const props = defineProps<{
  set: DuplicateSet;
  comparison: CopyComparison;
  /** The user's languages, in their order. */
  languages: LanguagePreset[];
  /** Every language preset, for the names and samples of the languages that differ. */
  presets: LanguagePreset[];
  /** The sample in the user's languages. */
  sample: string;
  /** What the draft says about each copy. */
  choices: Record<string, CopyChoice>;
  /** The copy the API recommends keeping. */
  keeper: string | null;
  /** Whether a copy cannot leave (system, active, …). */
  fixed: (path: string) => boolean;
  /** The name of this system's trash. */
  trash: string;
}>();
const emit = defineEmits<{
  pick: [first: string, second: string];
  keepOnly: [path: string];
  choose: [path: string, choice: CopyChoice];
  decide: [kind: SetDecision['kind']];
  close: [];
}>();

const heading = ref<HTMLElement | null>(null);
onMounted(async () => {
  await nextTick();
  heading.value?.focus();
});

// The file with its folder: copies of one font often share the file name.
const fileName = (path: string) =>
  path
    .split(/[\\/]/)
    .filter((part) => part !== '')
    .slice(-2)
    .join('/') || path;
const copyOf = (path: string) => props.set.copies.find((c) => c.path === path);
const indexOf = (path: string) => props.set.copies.findIndex((c) => c.path === path);
const title = computed(() =>
  t('duplicatesModule.compare.title', {
    a: fileName(props.comparison.a.path),
    b: fileName(props.comparison.b.path),
  }),
);
const copyOptions = computed(() =>
  props.set.copies.map((c, i) => ({
    value: c.path,
    label: t('duplicatesModule.compare.copyOption', {
      n: i + 1,
      name: fileName(c.path),
      version: c.version ?? t('duplicatesModule.set.noVersion'),
    }),
  })),
);
function pickFirst(path: string | null): void {
  if (!path) return;
  const second =
    path === props.comparison.b.path ? props.comparison.a.path : props.comparison.b.path;
  emit('pick', path, second);
}
function pickSecond(path: string | null): void {
  if (!path) return;
  const first =
    path === props.comparison.a.path ? props.comparison.b.path : props.comparison.a.path;
  emit('pick', first, path);
}

const d = computed(() => props.comparison.differences);
const presetOf = (code: string) => props.presets.find((p) => p.code === code);
const languageName = (code: string) => presetOf(code)?.name ?? code;
const names = (codes: readonly string[] | undefined) => (codes ?? []).map(languageName);

// The languages to switch the sample to: the user's and those that differ, in the API's
// order (E58.3), so the user's lead.
const sampleLanguages = computed(() => {
  const x = d.value;
  const codes = [
    ...props.languages.map((l) => l.code),
    ...(x?.languagesLost ?? []),
    ...(x?.languagesGained ?? []),
    ...(x?.loclLost ?? []),
    ...(x?.loclGained ?? []),
  ];
  return languagesToSample(props.presets, codes);
});
const sampleLanguage = ref<string | null>(null);
const sampleText = computed(() => {
  const p = sampleLanguage.value ? presetOf(sampleLanguage.value) : undefined;
  return (p && (p.pangram ?? p.sampleGlyphs)) || props.sample;
});
const sampleLang = computed(() => sampleLanguage.value ?? undefined);

// One half per side.
const sideA = computed(() => props.comparison.a);
const sideB = computed(() => props.comparison.b);
const faceA = computed(() => (sideA.value.health.kind === 'ok' ? sideA.value.face : null));
const faceB = computed(() => (sideB.value.health.kind === 'ok' ? sideB.value.face : null));
const familyA = useHeldFace(faceA);
const familyB = useHeldFace(faceB);
const specimenA = useSpecimen(
  () => faceA.value,
  () => sampleText.value,
);
const specimenB = useSpecimen(
  () => faceB.value,
  () => sampleText.value,
);
const halves = computed(() =>
  [
    { key: 'a', side: sideA.value, family: familyA.value, specimen: specimenA.value },
    { key: 'b', side: sideB.value, family: familyB.value, specimen: specimenB.value },
  ].map((h) => {
    const copy = copyOf(h.side.path);
    const format = h.side.format ?? copy?.format ?? null;
    return {
      ...h,
      name: fileName(h.side.path),
      index: indexOf(h.side.path),
      details: [
        copy?.version ?? t('duplicatesModule.set.noVersion'),
        format ? t(`info.formats.${format}`) : null,
      ]
        .filter((x): x is string => !!x)
        .join(' · '),
      broken: h.side.health.kind === 'ok' ? null : h.side.health.reason,
      covers: new Set(h.side.languages ?? []),
      locl: new Set(h.side.locl ?? []),
      choice: props.choices[h.side.path] ?? 'keep',
      recommended: h.side.path === props.keeper,
      fixed: props.fixed(h.side.path),
    };
  }),
);
const choiceName = (c: CopyChoice) =>
  c === 'trash' ? props.trash : t(`duplicatesModule.compare.choice.${c}`);

// The glyphs of a change, each drawn in the face of the copy that has it.
function glyphs(change: GlyphChange) {
  return change.first.map((label, i) => {
    const cp = change.codePoints?.[i] ?? null;
    return { label, char: cp == null ? null : String.fromCodePoint(cp) };
  });
}
const added = computed(() => (d.value ? glyphs(d.value.glyphsAdded) : []));
const removed = computed(() => (d.value ? glyphs(d.value.glyphsRemoved) : []));
const more = (change: GlyphChange) => change.count - change.first.length;

const COVERS = 'duplicatesModule.compare.onlyCovers' as const;
const LOCL = 'duplicatesModule.compare.onlyLocl' as const;
const languageLines = computed(() => {
  const x = d.value;
  if (!x) return [];
  const a = fileName(sideA.value.path);
  const b = fileName(sideB.value.path);
  return [
    { key: 'lost', codes: x.languagesLost, text: COVERS, name: a },
    { key: 'gained', codes: x.languagesGained, text: COVERS, name: b },
    { key: 'loclLost', codes: x.loclLost ?? [], text: LOCL, name: a },
    { key: 'loclGained', codes: x.loclGained ?? [], text: LOCL, name: b },
  ]
    .filter((l) => l.codes.length > 0)
    .map((l) => ({
      key: l.key,
      codes: l.codes,
      text: t(l.text, {
        name: l.name,
        languages: names(l.codes).join(', '),
      }),
    }));
});

const FEATURES = 'duplicatesModule.compare.featuresOnly' as const;
const AXES = 'duplicatesModule.compare.axesOnly' as const;
const lists = computed(() => {
  const x = d.value;
  if (!x) return [];
  return [
    { key: 'featuresAdded', label: FEATURES, side: sideB.value, items: x.featuresAdded },
    { key: 'featuresRemoved', label: FEATURES, side: sideA.value, items: x.featuresRemoved },
    { key: 'axesAdded', label: AXES, side: sideB.value, items: x.axesAdded },
    { key: 'axesRemoved', label: AXES, side: sideA.value, items: x.axesRemoved },
  ]
    .filter((l) => l.items.length > 0)
    .map((l) => ({
      key: l.key,
      label: t(l.label, { name: fileName(l.side.path) }),
      items: l.items,
    }));
});

const metricLines = computed(() => {
  const x = d.value;
  if (!x) return [];
  return [
    x.verticalMetricsChanged ? t('duplicatesModule.compare.verticalMetrics') : null,
    x.unitsPerEmChanged ? t('duplicatesModule.compare.unitsPerEm') : null,
    x.advanceWidthsChanged > 0
      ? t('duplicatesModule.compare.advanceWidths', { count: x.advanceWidthsChanged })
      : null,
  ].filter((l): l is string => !!l);
});

const nothing = computed(() => {
  const x = d.value;
  return (
    !!x &&
    !x.containerOnly &&
    languageLines.value.length === 0 &&
    x.glyphsAdded.count + x.glyphsRemoved.count === 0 &&
    lists.value.length === 0 &&
    metricLines.value.length === 0
  );
});

const formatName = (path: string, side: CopyComparison['a']) => {
  const f = side.format ?? copyOf(path)?.format;
  return f ? t(`info.formats.${f}`) : fileName(path);
};

function onKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' && !e.defaultPrevented) {
    e.preventDefault();
    e.stopPropagation();
    emit('close');
  }
}
</script>

<template>
  <section class="comparison" :aria-label="title" data-testid="copy-comparison" @keydown="onKey">
    <header class="head">
      <h3 ref="heading" tabindex="-1">{{ title }}</h3>
      <UiIconButton
        icon="close"
        :label="t('duplicatesModule.compare.close')"
        shortcut="Esc"
        aria-keyshortcuts="Escape"
        data-testid="compare-close"
        @click="emit('close')"
      />
      <div class="pick">
        <UiSelect
          :model-value="comparison.a.path"
          :options="copyOptions"
          :aria-label="t('duplicatesModule.compare.first')"
          data-testid="compare-first"
          @update:model-value="pickFirst"
        />
        <UiSelect
          :model-value="comparison.b.path"
          :options="copyOptions"
          :aria-label="t('duplicatesModule.compare.second')"
          data-testid="compare-second"
          @update:model-value="pickSecond"
        />
      </div>
    </header>

    <div
      v-if="sampleLanguages.length"
      class="samples"
      role="group"
      :aria-label="t('duplicatesModule.compare.sampleLabel')"
      data-testid="compare-samples"
    >
      <button
        type="button"
        class="chip"
        :aria-pressed="sampleLanguage === null"
        @click="sampleLanguage = null"
      >
        {{ t('duplicatesModule.compare.yourSample') }}
      </button>
      <button
        v-for="l in sampleLanguages"
        :key="l.code"
        type="button"
        class="chip"
        :aria-pressed="sampleLanguage === l.code"
        :data-language="l.code"
        @click="sampleLanguage = l.code"
      >
        {{ l.name }}
      </button>
    </div>

    <div class="halves">
      <article
        v-for="h in halves"
        :key="h.key"
        class="half"
        :class="{ recommended: h.recommended }"
        :aria-label="h.name"
        :data-copy-index="h.index >= 0 ? h.index : undefined"
        :data-side="h.key"
        :data-choice="h.choice"
        data-testid="compare-half"
      >
        <div class="half-head">
          <p class="name">
            <strong
              ><bdi>{{ h.name }}</bdi></strong
            >
            <span class="muted">{{ h.details }}</span>
          </p>
          <UiBadge v-if="h.recommended" tone="accent">
            {{ t('duplicatesModule.set.recommended') }}
          </UiBadge>
        </div>
        <p v-if="h.broken" class="broken" data-testid="compare-broken">
          <strong>{{ t('duplicatesModule.compare.unreadable') }}</strong>
          {{ h.broken }}
        </p>
        <p
          v-else
          class="specimen"
          :style="h.family ? { fontFamily: h.family } : undefined"
          :data-loaded="h.family ? 'true' : 'false'"
          :lang="sampleLang"
          dir="auto"
          data-testid="compare-specimen"
        >
          <SpecimenText :text="h.specimen.text" :missing="h.specimen.tofu" />
        </p>
        <ul v-if="languages.length && !h.broken" class="my-languages">
          <li
            v-for="l in languages"
            :key="l.code"
            :class="h.covers.has(l.code) ? 'has' : 'lacks'"
            :data-language="l.code"
          >
            <template v-if="h.covers.has(l.code)">
              <span aria-hidden="true">✓</span> {{ l.name }}
            </template>
            <template v-else>
              <span aria-hidden="true">×</span>
              {{ t('duplicatesModule.set.missing', { name: l.name }) }}
            </template>
            <UiBadge v-if="h.locl.has(l.code)" :title="t('duplicatesModule.compare.loclTitle')">
              {{ t('duplicatesModule.compare.locl') }}
            </UiBadge>
          </li>
        </ul>
        <p class="path" :title="h.side.path">
          <bdi>{{ h.side.path }}</bdi>
        </p>
        <div class="half-foot">
          <span class="choice" data-testid="compare-choice">
            {{ t('duplicatesModule.compare.inDraft', { choice: choiceName(h.choice) }) }}
          </span>
          <template v-if="!h.fixed">
            <UiButton
              :aria-label="t('duplicatesModule.compare.keepThisName', { name: h.name })"
              @click="emit('keepOnly', h.side.path)"
            >
              {{ t('duplicatesModule.compare.keepThis') }}
            </UiButton>
            <UiButton
              :aria-label="t('duplicatesModule.compare.hideName', { name: h.name })"
              @click="emit('choose', h.side.path, 'hide')"
            >
              {{ t('duplicatesModule.set.hide') }}
            </UiButton>
          </template>
          <span v-else class="muted">{{ t('duplicatesModule.set.stays') }}</span>
        </div>
      </article>
    </div>

    <div v-if="d" class="differences" data-testid="compare-differences">
      <p v-if="d.containerOnly" class="container-only" data-testid="compare-container-only">
        {{
          t('duplicatesModule.compare.containerOnly', {
            a: formatName(sideA.path, sideA),
            b: formatName(sideB.path, sideB),
          })
        }}
      </p>
      <section v-if="languageLines.length" class="group" data-testid="compare-languages">
        <h4>{{ t('duplicatesModule.compare.languages') }}</h4>
        <ul>
          <li v-for="l in languageLines" :key="l.key" :data-kind="l.key">{{ l.text }}</li>
        </ul>
      </section>
      <section
        v-if="d.glyphsAdded.count || d.glyphsRemoved.count"
        class="group"
        data-testid="compare-glyphs"
      >
        <h4>{{ t('duplicatesModule.compare.glyphs') }}</h4>
        <div v-if="d.glyphsAdded.count" class="glyph-row" data-kind="added">
          <p>
            {{
              t('duplicatesModule.compare.glyphsOnly', {
                count: d.glyphsAdded.count,
                name: fileName(sideB.path),
              })
            }}
          </p>
          <ul class="glyphs" :style="familyB ? { fontFamily: familyB } : undefined">
            <li v-for="g in added" :key="g.label" :title="g.label">
              <span v-if="g.char" class="glyph" role="img" :aria-label="g.label">{{ g.char }}</span>
              <span v-else class="glyph-name">{{ g.label }}</span>
            </li>
            <li v-if="more(d.glyphsAdded) > 0" class="more">
              {{ t('duplicatesModule.compare.more', { count: more(d.glyphsAdded) }) }}
            </li>
          </ul>
        </div>
        <div v-if="d.glyphsRemoved.count" class="glyph-row" data-kind="removed">
          <p>
            {{
              t('duplicatesModule.compare.glyphsOnly', {
                count: d.glyphsRemoved.count,
                name: fileName(sideA.path),
              })
            }}
          </p>
          <ul class="glyphs" :style="familyA ? { fontFamily: familyA } : undefined">
            <li v-for="g in removed" :key="g.label" :title="g.label">
              <span v-if="g.char" class="glyph" role="img" :aria-label="g.label">{{ g.char }}</span>
              <span v-else class="glyph-name">{{ g.label }}</span>
            </li>
            <li v-if="more(d.glyphsRemoved) > 0" class="more">
              {{ t('duplicatesModule.compare.more', { count: more(d.glyphsRemoved) }) }}
            </li>
          </ul>
        </div>
      </section>
      <section v-if="lists.length" class="group" data-testid="compare-lists">
        <h4>{{ t('duplicatesModule.compare.featuresAxes') }}</h4>
        <dl>
          <template v-for="l in lists" :key="l.key">
            <dt>{{ l.label }}</dt>
            <dd>
              <ul class="tags">
                <li v-for="tag in l.items" :key="tag">
                  <code>{{ tag }}</code>
                </li>
              </ul>
            </dd>
          </template>
        </dl>
      </section>
      <section v-if="metricLines.length" class="group" data-testid="compare-metrics">
        <h4>{{ t('duplicatesModule.compare.metrics') }}</h4>
        <ul>
          <li v-for="m in metricLines" :key="m">{{ m }}</li>
        </ul>
        <p v-if="d.mayReflow" class="reflow" data-testid="compare-reflow">
          {{ t('duplicatesModule.compare.mayReflow') }}
        </p>
      </section>
      <p v-if="nothing" data-testid="compare-same">
        {{
          d.tablesChanged.length
            ? t('duplicatesModule.compare.otherData', { tables: d.tablesChanged.join(', ') })
            : t('duplicatesModule.compare.same')
        }}
      </p>
    </div>
    <p v-else class="not-compared" data-testid="compare-not-compared">
      {{ t('duplicatesModule.compare.notCompared') }}
    </p>

    <footer class="set-actions">
      <UiButton aria-keyshortcuts="B" @click="emit('decide', 'keepAll')">
        {{ t('duplicatesModule.compare.keepBoth') }}
        <kbd>{{ t('duplicatesModule.set.keepAllKey') }}</kbd>
      </UiButton>
      <UiButton aria-keyshortcuts="N" @click="emit('decide', 'notDuplicates')">
        {{ t('duplicatesModule.set.notDuplicates') }}
        <kbd>{{ t('duplicatesModule.set.notDuplicatesKey') }}</kbd>
      </UiButton>
    </footer>
  </section>
</template>

<style scoped>
.comparison {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
  min-height: 0;
  overflow-y: auto;
}
.head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2) var(--fm-space-3);
}
.head h3 {
  flex: 1 1 0;
  min-width: 0;
  margin: 0;
  font-size: var(--fm-text-md);
}
.pick {
  display: flex;
  flex: 1 1 100%;
  flex-wrap: wrap;
  gap: var(--fm-space-2);
  min-width: 0;
}
.samples {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1);
}
.chip {
  padding-block: var(--fm-space-1);
  padding-inline: var(--fm-space-3);
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
.chip[aria-pressed='true'] {
  border-color: var(--fm-accent);
  background-color: var(--fm-accent);
  color: var(--fm-on-accent);
}
.halves {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--fm-space-3);
}
.half {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  min-width: 0;
  padding-block: var(--fm-space-3);
  padding-inline: var(--fm-space-4);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
}
.half.recommended {
  border: 2px solid var(--fm-accent);
  padding-block: calc(var(--fm-space-3) - 1px);
  padding-inline: calc(var(--fm-space-4) - 1px);
}
.half-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--fm-space-2);
}
.name {
  display: flex;
  flex-direction: column;
  min-width: 0;
  margin: 0;
}
.name strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.muted,
.path,
.choice {
  color: var(--fm-muted);
}
/* Three lines at most, so the differences stay in view under both halves. */
.specimen {
  display: -webkit-box;
  margin: 0;
  overflow: hidden;
  font-size: 24px;
  line-height: 1.3;
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  line-clamp: 3;
}
.specimen[data-loaded='false'] {
  color: var(--fm-muted);
}
.broken {
  margin: 0;
  padding-block: var(--fm-space-2);
  padding-inline: var(--fm-space-3);
  border-inline-start: 3px solid var(--fm-danger);
  background-color: var(--fm-surface);
}
.broken strong {
  display: block;
}
.my-languages {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1) var(--fm-space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}
.my-languages li {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
}
.my-languages .lacks {
  color: var(--fm-muted);
}
.path {
  margin: 0;
  overflow: hidden;
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.half-foot {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2);
  margin-top: auto;
  padding-top: var(--fm-space-2);
  border-top: 1px solid var(--fm-border);
}
.choice {
  flex: 1;
}
.differences {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
}
.group h4 {
  margin-block: 0 var(--fm-space-1);
  font-size: var(--fm-text-sm);
}
.group ul,
.group dl {
  margin: 0;
  padding-inline-start: var(--fm-space-4);
}
.group dd {
  margin: 0 0 var(--fm-space-1);
}
.glyph-row p {
  margin: 0 0 var(--fm-space-1);
}
.glyphs,
.tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1);
  padding: 0 !important;
  list-style: none;
}
.glyphs li {
  display: grid;
  place-items: center;
  min-width: 44px;
  min-height: 44px;
  padding-inline: var(--fm-space-1);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-md);
}
.glyph {
  font-size: 24px;
}
.glyph-name,
.glyphs .more {
  font-family: var(--fm-font-ui);
  font-size: var(--fm-text-xs);
  color: var(--fm-muted);
}
.reflow {
  margin: var(--fm-space-1) 0 0;
  font-weight: 600;
}
.not-compared {
  margin: 0;
  color: var(--fm-muted);
}
.set-actions {
  display: flex;
  gap: var(--fm-space-2);
}
</style>
