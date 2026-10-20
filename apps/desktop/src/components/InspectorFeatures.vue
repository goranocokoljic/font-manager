<script setup lang="ts">
// The Features tab: every GSUB and GPOS feature of the selected face, by its
// friendly name and tag (fm-core names them). Each feature a user can switch
// has a switch that starts at the browser's default; switching it changes the
// face's preview rows through font-feature-settings (the features store).
// Features the shaper applies by itself are listed without a switch.
import { computed } from 'vue';
import type { FeatureInfo } from '../api/types';
import { useFeaturesStore } from '../stores/features';
import { useSelectionStore } from '../stores/selection';

const selection = useSelectionStore();
const features = useFeaturesStore();
const d = computed(() => selection.detail);
const faceId = computed(() => d.value?.face.id ?? null);
const switchable = computed(() => d.value?.features.filter((f) => !f.automatic) ?? []);
const automatic = computed(() => d.value?.features.filter((f) => f.automatic) ?? []);

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
    <p v-if="!d" class="placeholder">Select a face to see its features.</p>
    <p v-else-if="!d.features.length" class="placeholder">This face has no OpenType features.</p>
    <template v-else>
      <div class="head">
        <p class="hint">Switches change this face's previews.</p>
        <button
          type="button"
          class="reset"
          :disabled="faceId === null || !features.isChanged(faceId)"
          @click="reset"
        >
          Reset
        </button>
      </div>
      <ul v-if="switchable.length" class="plain list" aria-label="Features">
        <li v-for="f in switchable" :key="f.tag" class="row">
          <span class="name">
            {{ f.name ?? f.tag }}
            <span v-if="f.name" class="tag">{{ f.tag }}</span>
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
        <h3 id="fm-features-automatic">Applied automatically</h3>
        <ul class="plain list" aria-label="Applied automatically">
          <li v-for="f in automatic" :key="f.tag" class="row" :data-tag="f.tag">
            <span class="name">
              {{ f.name ?? f.tag }}
              <span v-if="f.name" class="tag">{{ f.tag }}</span>
            </span>
            <span class="auto">applied automatically</span>
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
.placeholder,
.hint {
  margin: 0;
  color: var(--fm-muted);
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding-bottom: 8px;
}
.reset {
  padding: 2px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
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
  font-size: 11.5px;
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
  min-height: 28px;
}
.name {
  min-width: 0;
  overflow-wrap: anywhere;
}
.tag {
  margin-left: 4px;
  color: var(--fm-muted);
  font-family: ui-monospace, monospace;
}
.auto {
  flex: none;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.switch {
  position: relative;
  flex: none;
  width: 32px;
  height: 18px;
  padding: 0;
  border: 1px solid var(--fm-control-border);
  border-radius: 9px;
  background: transparent;
}
.knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--fm-control-border);
  transition: left 0.1s;
}
.switch[aria-checked='true'] {
  border-color: var(--fm-accent);
  background: var(--fm-accent);
}
.switch[aria-checked='true'] .knob {
  left: 16px;
  background: var(--fm-bg);
}
</style>
