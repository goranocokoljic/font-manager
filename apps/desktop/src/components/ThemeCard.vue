<script setup lang="ts">
import { computed } from 'vue';
import type { ThemeDefinition } from '../api/types';
import { tokenStyle, type Scheme } from '../themes/tokens';

// One theme of Settings › Appearance (E47.2): a live mini wall in the theme's own tokens for
// the current light or dark mode — three specimens on its surface, the middle one selected in
// its accent — and a radio, so the cards are a native radio group (arrows move, Space picks).
const props = defineProps<{
  theme: ThemeDefinition;
  scheme: Scheme;
  checked: boolean;
}>();
const emit = defineEmits<{ choose: [id: string] }>();

/** The card's own `--fm-*` tokens: everything inside it is drawn in this theme. */
const style = computed(() => tokenStyle(props.theme[props.scheme]));
/** Three specimens, the way the wall sets them: a serif, the UI sans and a mono. */
const SPECIMENS = ['serif', 'sans', 'mono'] as const;
/**
 * The glyph pair each specimen shows. A type sample, not a word: it is the same in every
 * locale and hidden from assistive tech (the wall is aria-hidden), so it is not in the catalog.
 */
const SAMPLE = 'Ag';
</script>

<template>
  <label
    class="theme-card"
    :class="{ checked }"
    :style="style"
    :data-theme-id="theme.id"
    data-testid="theme-card"
  >
    <input
      type="radio"
      class="sr-only"
      name="theme-id"
      :value="theme.id"
      :checked="checked"
      @change="emit('choose', theme.id)"
    />
    <span class="wall" aria-hidden="true" data-testid="theme-wall">
      <span
        v-for="(kind, i) in SPECIMENS"
        :key="kind"
        class="specimen"
        :class="[kind, { selected: i === 1 }]"
        data-testid="theme-specimen"
        >{{ SAMPLE }}</span
      >
    </span>
    <span class="name">{{ theme.name }}</span>
  </label>
</template>

<style scoped>
.theme-card {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  padding: var(--fm-space-2);
  border: 1px solid var(--fm-border-strong);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-surface);
  cursor: pointer;
}
/* The chosen card: a solid ring in the card's own accent (3:1 on its surfaces). */
.theme-card.checked {
  border-color: var(--fm-accent);
  box-shadow: 0 0 0 1px var(--fm-accent);
}
.theme-card:has(input:focus-visible) {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.wall {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: var(--fm-space-1);
}
.specimen {
  display: grid;
  place-items: center;
  height: 44px;
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-sm);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  font-size: var(--fm-text-xl);
  line-height: 1;
}
.specimen.serif {
  font-family: Georgia, 'Times New Roman', serif;
}
.specimen.mono {
  font-family: var(--fm-font-mono);
}
.specimen.selected {
  border: 2px solid var(--fm-accent);
}
.name {
  color: var(--fm-fg);
  font-size: var(--fm-text-sm);
  font-weight: 600;
}
</style>
