<script setup lang="ts">
// The color palette picker (E16.3): the CPAL palettes fm-api reports for the selected
// COLR face, each with a strip of its colors and its name ("Palette N" when the font
// names none). A native radio group: Tab reaches the chosen palette, the arrow keys
// move between palettes and choose. The choice is the family's in this view; every row
// of the family then draws in it (the palettes store).
import { computed } from 'vue';
import type { Palette } from '../api/types';
import { t } from '../i18n';
import { usePalettesStore } from '../stores/palettes';
import { ALL_FONTS_VIEW } from '../stores/preview';
import { useSelectionStore } from '../stores/selection';

const props = withDefaults(defineProps<{ view?: string }>(), { view: ALL_FONTS_VIEW });

const selection = useSelectionStore();
const palettes = usePalettesStore();

const family = computed(() => selection.detail?.face.familyName ?? null);
const list = computed<Palette[]>(() => selection.detail?.palettes ?? []);
const chosen = computed(() => {
  const own = family.value ? palettes.chosenFor(props.view, family.value) : 0;
  // A choice the face does not have (another face of the family) shows the default.
  return list.value.some((p) => p.index === own) ? own : 0;
});

/** The palette's own name, else "Palette N" counting from 1. */
const nameOf = (p: Palette) => p.name ?? t('palettes.numbered', { number: p.index + 1 });

function choose(p: Palette): void {
  if (family.value) palettes.choose(props.view, family.value, p.index);
}
</script>

<template>
  <section v-if="list.length" aria-labelledby="fm-info-palettes" data-testid="palette-picker">
    <h3 id="fm-info-palettes">{{ t('palettes.title') }}</h3>
    <div class="palettes" role="radiogroup" aria-labelledby="fm-info-palettes">
      <label
        v-for="p in list"
        :key="p.index"
        class="palette"
        :class="{ chosen: chosen === p.index }"
        :data-index="p.index"
      >
        <input
          type="radio"
          name="fm-palette"
          :value="p.index"
          :checked="chosen === p.index"
          @change="choose(p)"
        />
        <span class="swatches" aria-hidden="true">
          <span
            v-for="(c, k) in p.colors"
            :key="k"
            class="swatch"
            data-testid="palette-swatch"
            :style="{ backgroundColor: c }"
          />
        </span>
        <span class="label">{{ nameOf(p) }}</span>
      </label>
    </div>
  </section>
</template>

<style scoped>
h3 {
  margin: 16px 0 8px;
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
  text-transform: uppercase;
}
.palettes {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.palette {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border: 1px solid transparent;
  border-radius: 6px;
  cursor: pointer;
}
.palette.chosen {
  border-color: var(--fm-accent);
}
.palette:focus-within {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.swatches {
  display: flex;
  flex: none;
  overflow: hidden;
  border: 1px solid var(--fm-control-border);
  border-radius: 3px;
}
.swatch {
  width: 14px;
  height: 14px;
}
.label {
  min-width: 0;
  overflow-wrap: anywhere;
}
</style>
