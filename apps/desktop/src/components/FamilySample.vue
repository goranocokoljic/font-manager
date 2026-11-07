<script setup lang="ts">
// One line of text in a face at the given coordinates, for the family page's Styles rows and
// rail. Never a fallback font: nothing shows until the face has loaded.
import { toRef } from 'vue';
import type { FaceId } from '../api/types';
import { useHeldFace } from '../preview/useHeldFace';

const props = defineProps<{
  face: FaceId;
  text: string;
  /** The `font-variation-settings` value. */
  variation: string;
  /** The `font-feature-settings` value: the features switched on the page (E42.6). */
  features?: string;
}>();

const family = useHeldFace(toRef(props, 'face'));
</script>

<template>
  <span
    class="family-sample"
    :class="{ loading: !family }"
    :style="{
      fontFamily: family ? `'${family}'` : undefined,
      fontVariationSettings: variation,
      fontFeatureSettings: features,
    }"
    >{{ text }}</span
  >
</template>

<style scoped>
.family-sample {
  display: block;
  overflow: hidden;
  min-width: 0;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.loading {
  visibility: hidden;
}
</style>
