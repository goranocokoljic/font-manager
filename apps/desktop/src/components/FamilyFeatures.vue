<script setup lang="ts">
// The Features section of the family page (E42.6): one row per feature a user can switch, with
// its friendly name, the tag in mono under it, the sample without the feature (muted) and with
// it, and a switch. The switches live in the features store, so the hero, the Styles rows and the
// Text section follow them. Rare tags (cvXX, locl, the shaper's own) sit behind "N more".
import { computed, ref, useId } from 'vue';
import type { FaceDetail, FeatureInfo } from '../api/types';
import { t } from '../i18n';
import { featureRows, featureSample } from '../preview/featureRows';
import { useHeldFace } from '../preview/useHeldFace';
import { useFeaturesStore } from '../stores/features';
import UiButton from './ui/UiButton.vue';
import UiChevron from './ui/UiChevron.vue';
import UiSwitch from './ui/UiSwitch.vue';
import { useCopy } from './useCopy';

const props = defineProps<{
  /** The detail of the hero's face, once it has loaded. */
  detail: FaceDetail | null;
  /** The preview text, as the hero shows it: the sample of a stylistic set. */
  text: string;
}>();

const features = useFeaturesStore();
const uid = useId();
const faceId = computed(() => props.detail?.face.id ?? null);
const family = useHeldFace(faceId);
const rows = computed(() => featureRows(props.detail?.features ?? []));
const moreOpen = ref(false);

/** The features switched away from their default: what the copied line writes. */
const switchedCount = computed(() =>
  faceId.value === null ? 0 : features.switches(faceId.value).length,
);
const { status, copy } = useCopy();
function copyCss(): void {
  if (faceId.value !== null) void copy(features.css(faceId.value));
}

const sample = (f: FeatureInfo) => featureSample(f, props.text);
/** The same text twice: with the feature forced off, then forced on. Never a fallback font. */
const sampleStyle = (f: FeatureInfo, value: 0 | 1) => ({
  fontFamily: family.value ? `'${family.value}'` : undefined,
  fontFeatureSettings: `"${f.tag}" ${value}`,
});

/** "Small capitals (smcp)", or the tag alone when fm-core has no name for it. */
const label = (f: FeatureInfo) => (f.name ? `${f.name} (${f.tag})` : f.tag);
const on = (f: FeatureInfo) => faceId.value !== null && features.isOn(faceId.value, f);
function toggle(f: FeatureInfo): void {
  if (faceId.value !== null) features.set(faceId.value, f, !on(f));
}
function reset(): void {
  if (faceId.value !== null) features.reset(faceId.value);
}
</script>

<template>
  <div class="family-features" data-testid="family-features">
    <p v-if="!detail" class="quiet">{{ t('features.noFace') }}</p>
    <p v-else-if="!detail.features.length" class="quiet">{{ t('features.none') }}</p>
    <template v-else>
      <div class="head">
        <p class="quiet hint">{{ t('features.pageHint') }}</p>
        <span class="status" role="status">
          <template v-if="status === 'copied'">{{ t('common.copied') }}</template>
          <template v-else-if="status === 'failed'">{{ t('copy.failed') }}</template>
        </span>
        <UiButton data-testid="features-copy" :disabled="!switchedCount" @click="copyCss">
          {{ t('features.copySwitched', { count: switchedCount }) }}
        </UiButton>
        <UiButton :disabled="faceId === null || !features.isChanged(faceId)" @click="reset">
          {{ t('common.reset') }}
        </UiButton>
      </div>
      <ul v-if="rows.common.length" class="plain list" :aria-label="t('features.list')">
        <li v-for="f in rows.common" :key="f.tag" class="row" :data-tag="f.tag">
          <span class="name">
            {{ f.name ?? f.tag }}
            <span v-if="f.name" class="tag">{{ f.tag }}</span>
          </span>
          <span class="sample off" :class="{ loading: !family }" :style="sampleStyle(f, 0)">{{
            sample(f)
          }}</span>
          <span class="sample on" :class="{ loading: !family }" :style="sampleStyle(f, 1)">{{
            sample(f)
          }}</span>
          <UiSwitch :checked="on(f)" :aria-label="label(f)" :data-tag="f.tag" @click="toggle(f)" />
        </li>
      </ul>
      <template v-if="rows.rare.length">
        <button
          type="button"
          class="more fm-directional"
          :aria-controls="`${uid}-more`"
          :aria-expanded="moreOpen"
          @click="moreOpen = !moreOpen"
        >
          <UiChevron :open="moreOpen" />
          {{ t('features.more', { count: rows.rare.length }) }}
        </button>
        <ul v-if="moreOpen" :id="`${uid}-more`" class="plain list" :aria-label="t('features.rare')">
          <li v-for="f in rows.rare" :key="f.tag" class="row" :data-tag="f.tag">
            <span class="name">
              {{ f.name ?? f.tag }}
              <span v-if="f.name" class="tag">{{ f.tag }}</span>
            </span>
            <template v-if="!f.automatic">
              <span class="sample off" :class="{ loading: !family }" :style="sampleStyle(f, 0)">{{
                sample(f)
              }}</span>
              <span class="sample on" :class="{ loading: !family }" :style="sampleStyle(f, 1)">{{
                sample(f)
              }}</span>
              <UiSwitch
                :checked="on(f)"
                :aria-label="label(f)"
                :data-tag="f.tag"
                @click="toggle(f)"
              />
            </template>
            <span v-else class="auto quiet">{{ t('features.automatic') }}</span>
          </li>
        </ul>
      </template>
    </template>
  </div>
</template>

<style scoped>
.quiet {
  margin: 0;
  color: var(--fm-muted);
}
.head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--fm-space-2);
  margin-block-end: var(--fm-space-3);
}
.hint {
  flex: 1;
  font-size: var(--fm-text-sm);
}
.status {
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.plain {
  margin: 0;
  padding: 0;
  list-style: none;
}
/* Name, off, on, switch: the two samples share the width side by side. */
.row {
  display: grid;
  grid-template-columns: minmax(140px, 1fr) minmax(0, 2fr) minmax(0, 2fr) auto;
  align-items: center;
  gap: var(--fm-space-3);
  min-height: 48px;
  padding-block: var(--fm-space-2);
  border-block-end: 1px solid var(--fm-border);
}
.row:last-child {
  border-block-end: 0;
}
.name {
  display: flex;
  flex-direction: column;
  min-width: 0;
  overflow-wrap: anywhere;
}
.tag {
  color: var(--fm-muted);
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-xs);
}
.sample {
  overflow: hidden;
  min-width: 0;
  font-size: 24px;
  line-height: 1.3;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.sample.off {
  color: var(--fm-muted);
}
.sample.loading {
  visibility: hidden;
}
.auto {
  grid-column: 2 / -1;
  font-size: var(--fm-text-sm);
}
.more {
  display: flex;
  align-items: center;
  gap: var(--fm-space-1);
  width: 100%;
  margin-block-start: var(--fm-space-3);
  padding: var(--fm-space-2) 0;
  border: 0;
  border-block-start: 1px solid var(--fm-border);
  background: none;
  color: var(--fm-muted);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
</style>
