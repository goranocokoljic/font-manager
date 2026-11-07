<script setup lang="ts">
// "6 glyphs missing · š đ č ć ž" (E43.4): the characters of the sample a face lacks,
// in the accent's hue, beside the specimen that draws them as tofu.
import { computed } from 'vue';
import { missingLine } from '../preview/tofu';

const props = defineProps<{ missing: readonly string[] }>();
const line = computed(() => missingLine(props.missing));
</script>

<template>
  <span class="fm-missing-glyphs missing-glyphs" data-testid="missing-glyphs" :title="line">{{
    line
  }}</span>
</template>

<style scoped>
.missing-glyphs {
  overflow: hidden;
  min-width: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
