<script setup lang="ts">
// The rail's "Technical details" (E42.8): what a developer looks up and a reader does not. First
// the Developer part (the CSS for the current axes and features, "Copy for Font Switch"), then the
// face's name IDs, its vendor ID, the file's hash and the font's full table list. Names and the
// hash copy on click.
import { computed } from 'vue';
import type { FaceDetail } from '../api/types';
import { formatNumber, t } from '../i18n';
import CopyValue from './CopyValue.vue';
import InspectorDeveloper from './InspectorDeveloper.vue';

const props = defineProps<{ detail: FaceDetail | null }>();
const d = computed(() => props.detail);
const DASH = '—';
</script>

<template>
  <div class="technical" data-testid="family-technical">
    <InspectorDeveloper />
    <dl v-if="d" class="facts">
      <dt>{{ t('info.names.family') }}</dt>
      <dd><CopyValue :label="t('info.copyLabels.family')" :value="d.face.familyName" /></dd>
      <dt>{{ t('info.names.style') }}</dt>
      <dd><CopyValue :label="t('info.copyLabels.style')" :value="d.face.style" /></dd>
      <dt>{{ t('info.names.legacyFamily') }}</dt>
      <dd>
        <CopyValue
          v-if="d.legacyFamily"
          :label="t('info.copyLabels.legacyFamily')"
          :value="d.legacyFamily"
        />
        <template v-else>{{ DASH }}</template>
      </dd>
      <dt>{{ t('info.names.legacyStyle') }}</dt>
      <dd>
        <CopyValue
          v-if="d.legacyStyle"
          :label="t('info.copyLabels.legacyStyle')"
          :value="d.legacyStyle"
        />
        <template v-else>{{ DASH }}</template>
      </dd>
      <dt>{{ t('info.names.postscript') }}</dt>
      <dd>
        <CopyValue :label="t('info.copyLabels.postscript')" :value="d.face.postscriptName" />
      </dd>
      <dt>{{ t('info.names.version') }}</dt>
      <dd>{{ d.version ?? DASH }}</dd>
      <dt>{{ t('info.names.copyright') }}</dt>
      <dd>{{ d.copyright ?? DASH }}</dd>
      <dt>{{ t('info.names.vendorId') }}</dt>
      <dd class="mono" data-testid="technical-vendor">{{ d.vendorId ?? DASH }}</dd>
      <dt>{{ t('info.file.faceInFile') }}</dt>
      <dd>{{ formatNumber(d.collectionIndex) }}</dd>
      <dt>{{ t('info.file.hash') }}</dt>
      <dd class="mono" data-testid="technical-hash">
        <CopyValue :label="t('family.technicalHash')" :value="d.hash" />
      </dd>
      <dt id="fm-family-tables">
        {{ t('family.technicalTables', { count: d.tables?.length ?? 0 }) }}
      </dt>
      <dd>
        <ul
          v-if="d.tables?.length"
          class="tables mono"
          data-testid="technical-tables"
          aria-labelledby="fm-family-tables"
        >
          <li v-for="tag in d.tables" :key="tag">{{ tag }}</li>
        </ul>
        <template v-else>{{ DASH }}</template>
      </dd>
    </dl>
  </div>
</template>

<style scoped>
.facts {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: var(--fm-space-1) var(--fm-space-3);
  margin: var(--fm-space-2) 0 0;
  font-size: var(--fm-text-sm);
}
.facts dt {
  color: var(--fm-muted);
}
.facts dd {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
}
.mono {
  font-family: var(--fm-font-mono);
}
.tables {
  display: flex;
  flex-wrap: wrap;
  gap: 0 var(--fm-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
  white-space: pre;
}
</style>
