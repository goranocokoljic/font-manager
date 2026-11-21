<script setup lang="ts">
// One copy of a duplicate set in the Duplicates module (E56.11, board 14 "Sets"): a
// specimen in the copy's own face (a glyph it lacks is tofu, never a fallback), then
// its version, format, year, glyph count, size and path, the user's languages each
// with a check or "missing", and what the user put on it. The recommended copy has the
// accent ring and a "Keep" badge with the API's reason; a copy that may leave has the
// three outcomes (the trash of this system, Hide, Keep); a copy that cannot leave says
// why in their place. Everything shown comes from the API; the card only lays it out.
import { computed } from 'vue';
import type { CleanupFile, CopyChoice, DuplicateSetCopy, LanguagePreset } from '../api/types';
import { t } from '../i18n';
import { fileSizeText } from '../list/columns';
import { useHeldFace } from '../preview/useHeldFace';
import { useSpecimen } from '../preview/useSpecimen';
import SpecimenText from './SpecimenText';
import UiBadge from './ui/UiBadge.vue';
import UiSegmentedControl from './ui/UiSegmentedControl.vue';

const props = defineProps<{
  copy: DuplicateSetCopy;
  /** Its place in the set, 1-based: the key that keeps it. */
  index: number;
  /** What the plan says about it. */
  file: CleanupFile | undefined;
  choice: CopyChoice;
  recommended: boolean;
  /** The API's reason for the recommended copy. */
  reason: string | null;
  fixed: boolean;
  /** The user's languages, in their order. */
  languages: LanguagePreset[];
  /** The sample in the user's languages. */
  sample: string;
  /** The name of this system's trash. */
  trash: string;
}>();
const emit = defineEmits<{ choose: [choice: CopyChoice] }>();

const face = computed(() => props.copy.face);
const family = useHeldFace(face);
const specimen = useSpecimen(
  () => props.copy.face,
  () => props.sample,
);

const name = computed(() => props.copy.path.split(/[\\/]/).pop() ?? props.copy.path);
const facts = computed(() => props.copy.facts);
const year = computed(() => facts.value?.modified?.slice(0, 4) ?? null);
const details = computed(() =>
  [
    props.copy.format ? t(`info.formats.${props.copy.format}`) : null,
    year.value,
    facts.value?.glyphCount != null
      ? t('duplicatesModule.set.glyphs', { count: facts.value.glyphCount })
      : null,
    fileSizeText(props.copy.size),
  ].filter((x): x is string => !!x),
);
const covered = computed(() => new Set(facts.value?.languagesFull ?? []));
const userMeta = computed(() => {
  const f = facts.value;
  if (!f) return [];
  return [
    f.favorite ? t('duplicatesModule.set.favorite') : null,
    f.tags.length ? t('duplicatesModule.set.tags', { count: f.tags.length }) : null,
    f.collections.length
      ? t('duplicatesModule.set.collections', { count: f.collections.length })
      : null,
  ].filter((x): x is string => !!x);
});

const options = computed(() => [
  { value: 'trash' as const, label: props.trash },
  { value: 'hide' as const, label: t('duplicatesModule.set.hide') },
  { value: 'keep' as const, label: t('duplicatesModule.set.keep') },
]);
/** The line under the facts: the API's reason for the keeper, else the plan's sentence. */
const line = computed(() =>
  props.recommended && props.reason ? props.reason : (props.file?.sentence ?? ''),
);
</script>

<template>
  <article
    class="copy"
    :class="{ recommended }"
    :aria-label="name"
    :data-choice="choice"
    :data-recommended="recommended ? 'true' : undefined"
    data-testid="duplicate-copy"
  >
    <div class="head">
      <p
        class="specimen"
        :style="family ? { fontFamily: family } : undefined"
        :data-loaded="family ? 'true' : 'false'"
        dir="auto"
      >
        <SpecimenText :text="specimen.text" :missing="specimen.tofu" />
      </p>
      <UiBadge v-if="recommended" tone="accent" :title="t('duplicatesModule.set.recommendedTitle')">
        {{ t('duplicatesModule.set.recommended') }}
      </UiBadge>
      <UiBadge v-else-if="fixed">{{ t('duplicatesModule.set.stays') }}</UiBadge>
      <kbd class="key" aria-hidden="true">{{ index }}</kbd>
    </div>
    <p class="facts">
      <strong class="version">{{ copy.version ?? t('duplicatesModule.set.noVersion') }}</strong>
      <span class="muted">{{ details.join(' · ') }}</span>
      <span class="path" :title="copy.path"
        ><bdi>{{ copy.path }}</bdi></span
      >
    </p>
    <div class="langs">
      <ul v-if="languages.length" class="languages">
        <li
          v-for="l in languages"
          :key="l.code"
          :class="covered.has(l.code) ? 'has' : 'lacks'"
          :data-language="l.code"
        >
          <template v-if="covered.has(l.code)">
            <span aria-hidden="true">✓</span> {{ l.name }}
          </template>
          <template v-else>
            <span aria-hidden="true">×</span>
            {{ t('duplicatesModule.set.missing', { name: l.name }) }}
          </template>
        </li>
      </ul>
      <p v-if="userMeta.length" class="meta">{{ userMeta.join(' · ') }}</p>
    </div>
    <div class="foot">
      <p class="line">{{ line }}</p>
      <UiSegmentedControl
        v-if="!fixed"
        class="outcome"
        :model-value="choice"
        :options="options"
        :label="t('duplicatesModule.set.outcome', { name })"
        data-testid="copy-outcome"
        @update:model-value="emit('choose', $event)"
      />
    </div>
  </article>
</template>

<style scoped>
.copy {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  padding: var(--fm-space-4) var(--fm-space-5);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
  transition: border-color var(--fm-motion-fast) var(--fm-ease);
}
.copy.recommended {
  border: 2px solid var(--fm-accent);
  padding: calc(var(--fm-space-4) - 1px) calc(var(--fm-space-5) - 1px);
}
.head {
  display: flex;
  align-items: flex-start;
  gap: var(--fm-space-3);
}
.specimen {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: 32px;
  line-height: 1.25;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.specimen[data-loaded='false'] {
  color: var(--fm-muted);
}
.key {
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-xs);
  color: var(--fm-muted);
}
.facts {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1) var(--fm-space-4);
  align-items: baseline;
  margin: 0;
}
.muted,
.meta,
.line {
  color: var(--fm-muted);
}
.path {
  min-width: 0;
  overflow: hidden;
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.langs {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: var(--fm-space-2);
}
.languages {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1) var(--fm-space-4);
  margin: 0;
  padding: 0;
  list-style: none;
}
.languages .lacks {
  color: var(--fm-muted);
}
.meta {
  margin: 0;
}
.foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-3);
  padding-top: var(--fm-space-2);
  border-top: 1px solid var(--fm-border);
}
.line {
  margin: 0;
}
</style>
