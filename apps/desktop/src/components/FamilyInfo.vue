<script setup lang="ts">
// The Info section of the family page (E42.8): four groups in a two-column grid. Design (who made
// it, when, its class, axes, named styles, what the font says about itself), Licence (what the
// licence lets the user do, from fm-core's LicenseInfo.permissions, and the user's own record),
// Files (format, where it is, when it came in, health) and Metrics (the face's vertical metrics
// with an "Hxg" specimen drawn on them). Names, hash, vendor and tables are in the rail's
// "Technical details" (FamilyTechnical).
import { computed, ref } from 'vue';
import type { Axis, FaceDetail, FamilySummary } from '../api/types';
import { formatNumber, t } from '../i18n';
import { dateText } from '../list/columns';
import { SPECIMEN_TEXT, metricsSpecimen, xHeightRatio } from '../preview/metricsSpecimen';
import CopyValue from './CopyValue.vue';
import HealthBadge from './HealthBadge.vue';
import InspectorLicense from './InspectorLicense.vue';

const props = defineProps<{
  /** The detail of the hero's face, once it has loaded. */
  detail: FaceDetail | null;
  /** The family's list row. */
  summary: FamilySummary | null;
  /** The earliest year the copyright of a face states. */
  year?: number | null;
  /** The family's designer, when the face does not state one. */
  designer?: string | null;
  /** How the hero draws the face: family, variations, features. */
  faceStyle?: Record<string, string | undefined>;
}>();

const DASH = '—';
const d = computed(() => props.detail);
const joined = (parts: (string | null | undefined)[]) =>
  parts.filter((p): p is string => !!p).join(' · ');

// Design
const designer = computed(() => d.value?.designer ?? props.designer ?? null);
const foundry = computed(() => {
  const name = props.summary?.foundry ?? d.value?.foundry ?? null;
  return joined([name, props.year != null ? String(props.year) : null]);
});
const classification = computed(() => {
  const c = props.summary?.classification;
  return c ? t(`chips.classes.${c}`) : null;
});
/** "Weight 100–900 · Optical size 9–144". */
const axisRange = (a: Axis) =>
  t('family.info.axisRange', {
    name: a.name ?? a.tag,
    min: formatNumber(a.min ?? 0),
    max: formatNumber(a.max ?? 0),
  });
const axes = computed(() => joined(d.value?.axes.map(axisRange) ?? []));
const namedStyles = computed(() => {
  const count = d.value?.instances.length ?? 0;
  return count ? formatNumber(count) : null;
});

// Licence
const license = computed(() => d.value?.license ?? null);
const licenseName = computed(() => {
  const open = license.value?.openLicense;
  return open ? t(`info.openLicenses.${open}`) : (license.value?.description ?? null);
});
const permissions = computed(() => license.value?.permissions ?? null);
/** The user's declaration in a line: type, seats, purchase date. */
const record = computed(() => {
  const l = license.value;
  if (!l) return null;
  return (
    joined([
      l.declared ? t(`license.types.${l.declared}`) : null,
      l.seats != null ? t('family.info.seats', { count: l.seats }) : null,
      l.purchaseDate ? t('family.info.purchased', { date: dateText(l.purchaseDate) }) : null,
    ]) || null
  );
});
// The declaration form (folded under its own heading) opens from the record's link.
const form = ref<InstanceType<typeof InspectorLicense> | null>(null);

// Files
/** "1.1 MB", "46 kB": decimal units, as the file managers of all three systems show them. */
function size(bytes: number): string {
  const mb = bytes >= 1_000_000;
  return formatNumber(mb ? bytes / 1_000_000 : bytes / 1000, {
    style: 'unit',
    unit: mb ? 'megabyte' : 'kilobyte',
    maximumFractionDigits: 1,
  });
}
const format = computed(() => {
  const face = d.value?.face;
  if (!face) return null;
  return joined([
    t(`info.formats.${face.format}`),
    face.isVariable ? t('family.info.variable') : null,
    size(d.value!.fileSize),
  ]);
});
const added = computed(() => (props.summary ? dateText(props.summary.added) : null));

// Metrics
const metrics = computed(() => d.value?.metrics ?? null);
const specimen = computed(() => (metrics.value ? metricsSpecimen(metrics.value) : null));
const SPECIMEN_WIDTH = 240;
const unit = (v: number | null | undefined) => (v == null ? DASH : formatNumber(v));
const xHeight = computed(() => {
  const m = metrics.value;
  if (!m || m.xHeight == null) return DASH;
  const ratio = xHeightRatio(m);
  return ratio === null
    ? formatNumber(m.xHeight)
    : t('family.info.xHeightRatio', {
        value: formatNumber(m.xHeight),
        ratio: formatNumber(ratio, { minimumFractionDigits: 2, maximumFractionDigits: 2 }),
      });
});
</script>

<template>
  <p v-if="!d" class="quiet">{{ t('info.loading') }}</p>
  <div v-else class="family-info" data-testid="family-info">
    <section class="group" aria-labelledby="fm-family-info-design">
      <h3 id="fm-family-info-design">{{ t('family.info.design') }}</h3>
      <dl>
        <dt>{{ t('family.info.designer') }}</dt>
        <dd data-testid="info-designer">{{ designer ?? DASH }}</dd>
        <dt>{{ t('family.info.foundry') }}</dt>
        <dd data-testid="info-foundry">{{ foundry || DASH }}</dd>
        <dt>{{ t('family.info.classification') }}</dt>
        <dd data-testid="info-classification">{{ classification ?? DASH }}</dd>
        <dt>{{ t('family.info.axes') }}</dt>
        <dd data-testid="info-axes">{{ axes || DASH }}</dd>
        <dt>{{ t('family.info.namedStyles') }}</dt>
        <dd data-testid="info-named-styles" class="num">{{ namedStyles ?? DASH }}</dd>
        <dt>{{ t('family.info.description') }}</dt>
        <dd data-testid="info-description" dir="auto">{{ d.description ?? DASH }}</dd>
      </dl>
    </section>

    <section class="group" aria-labelledby="fm-family-info-licence">
      <h3 id="fm-family-info-licence">{{ t('family.info.licence') }}</h3>
      <dl>
        <dt>{{ t('family.info.licenceName') }}</dt>
        <dd data-testid="info-licence">{{ licenseName ?? DASH }}</dd>
        <template v-if="permissions">
          <dt>{{ t('family.info.youMay') }}</dt>
          <dd data-testid="info-may">{{ joined(permissions.may) || DASH }}</dd>
          <dt>{{ t('family.info.youMayNot') }}</dt>
          <dd data-testid="info-may-not">{{ joined(permissions.mayNot) || DASH }}</dd>
        </template>
        <dt>{{ t('family.info.record') }}</dt>
        <dd data-testid="info-record">
          <span v-if="record" class="record">{{ record }}</span>
          <button
            type="button"
            class="link"
            data-testid="info-declare"
            aria-controls="fm-license-fields"
            @click="form?.openForm()"
          >
            {{ record ? t('family.info.editRecord') : t('family.info.addRecord') }}
          </button>
        </dd>
      </dl>
      <InspectorLicense ref="form" />
    </section>

    <section class="group" aria-labelledby="fm-family-info-files">
      <h3 id="fm-family-info-files">{{ t('family.info.files') }}</h3>
      <dl>
        <dt>{{ t('family.info.format') }}</dt>
        <dd data-testid="info-format">{{ format ?? DASH }}</dd>
        <dt>{{ t('family.info.location') }}</dt>
        <dd class="path">
          <CopyValue :label="t('info.copyLabels.path')" :value="d.path" />
        </dd>
        <dt>{{ t('family.info.added') }}</dt>
        <dd data-testid="info-added">{{ added || DASH }}</dd>
        <dt>{{ t('family.info.health') }}</dt>
        <dd><HealthBadge :health="d.face.health" /></dd>
      </dl>
    </section>

    <section class="group" aria-labelledby="fm-family-info-metrics">
      <h3 id="fm-family-info-metrics">{{ t('family.info.metrics') }}</h3>
      <div class="metrics">
        <svg
          v-if="specimen"
          class="specimen"
          data-testid="metrics-specimen"
          :width="SPECIMEN_WIDTH"
          :height="specimen.height"
          :viewBox="`0 0 ${SPECIMEN_WIDTH} ${specimen.height}`"
          aria-hidden="true"
        >
          <line
            v-if="specimen.capLine !== null"
            class="guide dashed"
            data-line="cap"
            x1="0"
            :x2="SPECIMEN_WIDTH"
            :y1="specimen.capLine"
            :y2="specimen.capLine"
          />
          <line
            v-if="specimen.xLine !== null"
            class="guide dashed"
            data-line="x"
            x1="0"
            :x2="SPECIMEN_WIDTH"
            :y1="specimen.xLine"
            :y2="specimen.xLine"
          />
          <line
            class="guide baseline"
            data-line="baseline"
            x1="0"
            :x2="SPECIMEN_WIDTH"
            :y1="specimen.baseline"
            :y2="specimen.baseline"
          />
          <!-- Never a fallback font: the letters wait for the face, the guides do not. -->
          <text
            v-if="faceStyle"
            class="hxg"
            x="0"
            :y="specimen.baseline"
            :font-size="specimen.size"
            :style="faceStyle"
          >
            {{ SPECIMEN_TEXT }}
          </text>
        </svg>
        <dl class="values">
          <dt>{{ t('family.info.unitsPerEm') }}</dt>
          <dd data-testid="info-upm">{{ unit(metrics?.unitsPerEm) }}</dd>
          <dt>{{ t('family.info.capHeight') }}</dt>
          <dd data-testid="info-cap">{{ unit(metrics?.capHeight) }}</dd>
          <dt>{{ t('family.info.xHeight') }}</dt>
          <dd data-testid="info-x">{{ xHeight }}</dd>
          <dt>{{ t('family.info.ascender') }}</dt>
          <dd data-testid="info-ascender">{{ unit(metrics?.ascender) }}</dd>
          <dt>{{ t('family.info.descender') }}</dt>
          <dd data-testid="info-descender">{{ unit(metrics?.descender) }}</dd>
        </dl>
      </div>
    </section>
  </div>
</template>

<style scoped>
.quiet {
  margin: 0;
  color: var(--fm-muted);
}
.family-info {
  display: grid;
  /* Each track is at least half the width, so never more than two fit; under 620 px, one. */
  grid-template-columns: repeat(
    auto-fit,
    minmax(max(300px, calc((100% - var(--fm-space-5)) / 2)), 1fr)
  );
  gap: var(--fm-space-5) var(--fm-space-5);
  align-items: start;
}
.group {
  min-width: 0;
}
h3 {
  margin: 0 0 var(--fm-space-2);
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
dl {
  display: grid;
  grid-template-columns: minmax(96px, auto) minmax(0, 1fr);
  gap: var(--fm-space-1) var(--fm-space-3);
  margin: 0;
}
dt {
  color: var(--fm-muted);
}
dd {
  min-width: 0;
  margin: 0;
  overflow-wrap: anywhere;
}
.num,
.values dd {
  font-variant-numeric: tabular-nums;
}
.record {
  margin-inline-end: var(--fm-space-2);
}
.link {
  padding: 0;
  border: 0;
  background: none;
  color: inherit;
  font: inherit;
  text-decoration: underline;
  text-underline-offset: 2px;
  cursor: pointer;
}
.path {
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
}
.metrics {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  gap: var(--fm-space-4);
}
.specimen {
  flex: none;
  overflow: visible;
}
.hxg {
  fill: var(--fm-fg);
}
.guide {
  stroke-width: 1;
}
.dashed {
  stroke: var(--fm-accent);
  stroke-dasharray: 4 3;
}
.baseline {
  stroke: var(--fm-fg);
}
.values {
  grid-template-columns: auto auto;
}
</style>
