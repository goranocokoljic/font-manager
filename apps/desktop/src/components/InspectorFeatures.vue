<script setup lang="ts">
// The Features tab: every GSUB and GPOS feature of the selected face, by its
// friendly name and tag (fm-core names them). Each feature a user can switch
// has a switch that starts at the browser's default; switching it changes the
// face's preview rows through font-feature-settings (the features store).
// Features the shaper applies by itself are listed without a switch. "Copy
// CSS" copies font-feature-settings for the switched features (E10.5).
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { FeatureInfo } from '../api/types';
import { t } from '../i18n';
import { useFeaturesStore } from '../stores/features';
import { useSelectionStore } from '../stores/selection';
import { useFaceLoader } from '../preview/faceLoader';
import InspectorTabHead from './InspectorTabHead.vue';
import CopyCss from './CopyCss.vue';

const selection = useSelectionStore();
const features = useFeaturesStore();
const d = computed(() => selection.detail);
const faceId = computed(() => d.value?.face.id ?? null);
const switchable = computed(() => d.value?.features.filter((f) => !f.automatic) ?? []);
const automatic = computed(() => d.value?.features.filter((f) => f.automatic) ?? []);

// The face's font, registered with the webview while the tab shows it, so each
// row can show its feature at work.
const loader = useFaceLoader();
const family = ref<string | null>(null);
let held: number | null = null;
function hold(id: number | null): void {
  if (held !== null) loader.release(held);
  held = id;
  family.value = null;
  if (id === null) return;
  loader.acquire(id).then(
    (name) => {
      if (held === id) family.value = name;
    },
    () => undefined,
  );
}
watch(faceId, hold, { immediate: true });
onBeforeUnmount(() => hold(null));

// Presentation only: a short text that shows what a registered feature does.
const SAMPLES: Record<string, string> = {
  liga: 'fi fl ffi ffl',
  clig: 'fi fl ffi ffl',
  dlig: 'st ct sp',
  kern: 'AVATAR Toy',
  smcp: 'Small Capitals',
  c2sc: 'SMALL CAPITALS',
  onum: '0123456789',
  lnum: '0123456789',
  tnum: '1111 0000',
  pnum: '1111 0000',
  zero: '0 O 100',
  frac: '1/2 3/4 7/8',
  case: '(H-O) [a]',
  sups: 'x2 n1',
  subs: 'H2O CO2',
  ordn: '1a 2o Nº',
  swsh: 'Hamburg',
  calt: 'a->b ==',
};
const sample = (f: FeatureInfo) => SAMPLES[f.tag] ?? 'Rag Agy 0123';
const sampleStyle = (f: FeatureInfo) => ({
  fontFamily: family.value ? `"${family.value}", sans-serif` : undefined,
  fontFeatureSettings: `"${f.tag}" 1`,
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
  <div class="features">
    <p v-if="!d" class="placeholder">{{ t('features.noFace') }}</p>
    <p v-else-if="!d.features.length" class="placeholder">{{ t('features.none') }}</p>
    <template v-else>
      <InspectorTabHead :hint="t('features.hint')">
        <CopyCss
          v-if="faceId !== null"
          :label="t('features.cssLabel')"
          :css="() => features.css(faceId!)"
        />
        <button
          type="button"
          class="reset"
          :disabled="faceId === null || !features.isChanged(faceId)"
          @click="reset"
        >
          {{ t('common.reset') }}
        </button>
      </InspectorTabHead>
      <ul v-if="switchable.length" class="plain list" :aria-label="t('features.list')">
        <li v-for="f in switchable" :key="f.tag" class="row">
          <span class="name">
            {{ f.name ?? f.tag }}
            <span v-if="f.name" class="tag">{{ f.tag }}</span>
            <span v-if="family" class="sample" :style="sampleStyle(f)" aria-hidden="true">{{
              sample(f)
            }}</span>
          </span>
          <button
            type="button"
            role="switch"
            class="switch"
            :aria-checked="on(f)"
            :aria-label="label(f)"
            :data-tag="f.tag"
            @click="toggle(f)"
          >
            <span class="knob" aria-hidden="true" />
          </button>
        </li>
      </ul>
      <section v-if="automatic.length" aria-labelledby="fm-features-automatic">
        <h3 id="fm-features-automatic">{{ t('features.automatic') }}</h3>
        <ul class="plain list" :aria-label="t('features.automatic')">
          <li v-for="f in automatic" :key="f.tag" class="row" :data-tag="f.tag">
            <span class="name">
              {{ f.name ?? f.tag }}
              <span v-if="f.name" class="tag">{{ f.tag }}</span>
            </span>
          </li>
        </ul>
      </section>
    </template>
  </div>
</template>

<style scoped>
.features {
  padding: 16px;
}
.placeholder {
  margin: 0;
  color: var(--fm-muted);
}
.reset {
  padding: 2px 10px;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: transparent;
  color: inherit;
  font: inherit;
}
.reset:disabled {
  border-color: var(--fm-border);
  color: var(--fm-muted);
}
h3 {
  margin: 16px 0 8px;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  text-transform: uppercase;
}
.plain {
  margin: 0;
  padding: 0;
  list-style: none;
}
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-height: 32px;
  padding-block: var(--fm-space-1);
  border-block-end: 1px solid var(--fm-border);
}
.row:last-child {
  border-block-end: 0;
}
.name {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  min-width: 0;
  overflow-wrap: anywhere;
}
.sample {
  flex-basis: 100%;
  font-size: var(--fm-text-lg);
  line-height: 1.5;
}
.tag {
  margin-inline-start: 4px;
  color: var(--fm-muted);
  font-family: var(--fm-font-mono);
}
.switch {
  position: relative;
  box-sizing: border-box;
  flex: none;
  width: 32px;
  height: 18px;
  padding: 0;
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface-2);
  transition:
    background-color var(--fm-motion-fast) var(--fm-ease),
    border-color var(--fm-motion-fast) var(--fm-ease);
}
.knob {
  position: absolute;
  top: 50%;
  inset-inline-start: 3px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--fm-control-border);
  transform: translateY(-50%);
  transition: inset-inline-start var(--fm-motion-fast) var(--fm-ease);
}
.switch[aria-checked='true'] {
  border-color: var(--fm-accent);
  background: var(--fm-accent);
}
.switch[aria-checked='true'] .knob {
  inset-inline-start: 17px;
  background-color: var(--fm-bg);
}
</style>
