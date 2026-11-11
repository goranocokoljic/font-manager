<script setup lang="ts">
// The trait tags of the Characters section (E49.10): the face's detected letter shapes as quiet
// tags under the glyph grid ('R leg · straight', 'O · round'), each with the value's drawn
// example. fm-core picks them (sure detections of the traits the catalog offers, in catalog
// order); the catalog gives the words. Each tag has 'find fonts with this', shown on hover or
// focus: the page goes back to the wall with that letter in the Letters preset and the value
// as a chip. A face with no detected traits shows nothing, not even the heading.
import { computed, watch } from 'vue';
import type { DetectedTrait } from '../api/types';
import { t } from '../i18n';
import { traitLineLabel, traitTags, type TraitTag } from '../search/traits';
import { useSearchStore } from '../stores/search';

const props = defineProps<{
  /** The face's detected traits, from its detail. */
  traits: readonly DetectedTrait[];
}>();
const emit = defineEmits<{
  /** 'find fonts with this' on a tag. */
  find: [tag: TraitTag];
}>();

const search = useSearchStore();
watch(
  () => props.traits.length,
  (n) => {
    if (n) void search.loadTraitCatalog();
  },
  { immediate: true },
);

const tags = computed(() => traitTags(props.traits, search.traitCatalog));
</script>

<template>
  <section v-if="tags.length" class="traits" aria-labelledby="fm-family-traits">
    <h3 id="fm-family-traits" class="heading">{{ t('glyphs.traits') }}</h3>
    <ul class="tags">
      <li v-for="tag in tags" :key="tag.traitId" class="tag" data-testid="trait-tag">
        <svg v-if="tag.example" class="example" viewBox="0 0 56 56" aria-hidden="true">
          <path :d="tag.example" />
        </svg>
        <span class="words">
          <span class="glyph">{{ tag.glyph }}</span>
          <template v-if="tag.aspect">
            <span class="dot" aria-hidden="true"> · </span>
            {{ tag.aspect }}
          </template>
          <span class="dot" aria-hidden="true"> · </span>
          {{ tag.valueLabel }}
        </span>
        <button
          type="button"
          class="find"
          :aria-label="t('glyphs.findWithTraitLabel', { trait: traitLineLabel(tag) })"
          @click="emit('find', tag)"
        >
          {{ t('glyphs.findWithTrait') }}
        </button>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.traits {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
}
.heading {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 500;
}
.tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--fm-space-1) var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
/* Quiet: muted words on the surface, no border; the action shows on hover or focus. */
.tag {
  display: inline-flex;
  align-items: center;
  gap: var(--fm-space-1);
  padding: 2px var(--fm-space-2);
  border-radius: 999px;
  background-color: var(--fm-surface);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.example {
  width: 18px;
  height: 18px;
  fill: none;
  stroke: currentColor;
  stroke-width: 4;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.glyph {
  color: var(--fm-fg);
  font-weight: 600;
}
.find {
  max-inline-size: 0;
  overflow: hidden;
  padding: 0;
  border: 0;
  border-radius: var(--fm-radius-sm);
  background: none;
  /* The accent is never a text color (contrast.test.ts): the link is the text color, underlined. */
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-xs);
  text-decoration: underline;
  white-space: nowrap;
  cursor: pointer;
  /* Out of sight and out of the way, not out of reach: Tab still finds it, and focus shows it. */
  opacity: 0;
}
.tag:hover .find,
.find:focus {
  max-inline-size: 24ch;
  padding: 0 var(--fm-space-1);
  opacity: 1;
}
.find:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
</style>
