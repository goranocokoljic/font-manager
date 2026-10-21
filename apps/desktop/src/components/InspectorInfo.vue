<script setup lang="ts">
// The Info tab: every field fm-api reports for the selected face, its health
// and the activation button. Names and paths copy on click. Tags, favorite,
// rating, the notes of the face and of its family, and the family's declared
// license are edited here; the API stores them. A color face's palette picker
// (E16.3) comes first.
import { computed, nextTick, ref, watch } from 'vue';
import type { Axis, Conflict, OutcomeStatus } from '../api/types';
import { errorMessage, formatNumber, t } from '../i18n';
import { useSelectionStore } from '../stores/selection';
import { sameTagName, useTagsStore } from '../stores/tags';
import CopyValue from './CopyValue.vue';
import HealthBadge from './HealthBadge.vue';
import InspectorLicense from './InspectorLicense.vue';
import InspectorPalettes from './InspectorPalettes.vue';

const selection = useSelectionStore();
const tags = useTagsStore();
const d = computed(() => selection.detail);

// Tag names to complete from: the library's tags the face does not have yet.
const suggestions = computed(() => {
  const mine = d.value?.tags ?? [];
  return tags.tags.filter((tag) => !mine.some((name) => sameTagName(name, tag.name)));
});
const newTag = ref('');
async function addTag(): Promise<void> {
  if (!newTag.value.trim()) return;
  if (await selection.addTag(newTag.value)) newTag.value = '';
}
// The notes being typed; each saves when its field loses the focus, and follows
// the stored value when another face is shown or the save comes back.
const faceNotes = ref('');
const familyNotes = ref('');
watch(
  () => [d.value?.face.id, d.value?.notes] as const,
  () => (faceNotes.value = d.value?.notes ?? ''),
  { immediate: true },
);
watch(
  () => [d.value?.face.id, d.value?.familyNotes] as const,
  () => (familyNotes.value = d.value?.familyNotes ?? ''),
  { immediate: true },
);
function saveFaceNotes(): void {
  if (d.value && faceNotes.value !== (d.value.notes ?? ''))
    void selection.setNotes(faceNotes.value);
}
function saveFamilyNotes(): void {
  if (d.value && familyNotes.value !== (d.value.familyNotes ?? ''))
    void selection.setFamilyNotes(familyNotes.value);
}
const STARS = [1, 2, 3, 4, 5] as const;
const starLabel = (n: number) => t('info.user.stars', { count: n });
/** Choosing the rating the face has clears it. */
function rate(n: number): void {
  void selection.setRating(d.value?.rating === n ? 0 : n);
}

// Format, embedding, open-license and slant names come from the catalog
// (`info.formats.*`, `info.embeddings.*`, `info.openLicenses.*`, `info.slants.*`).
const show = (v: string | number | null | undefined) =>
  v === null || v === undefined || v === '' ? '—' : typeof v === 'number' ? formatNumber(v) : v;
const list = (items: string[]) => (items.length ? items.join(', ') : '—');
/** "Weight: 100–900 (default 400)". */
const axisText = (a: Axis) =>
  t('info.style.axis', {
    name: a.name,
    min: a.min ?? '',
    max: a.max ?? '',
    default: a.default ?? '',
  });
const degrees = (v: number | null | undefined) => (v === null || v === undefined ? '—' : `${v}°`);

const canActivate = computed(() => d.value?.face.health.kind === 'ok');
const activationState = computed(() => {
  const active = d.value?.face.active;
  if (active === 'permanent') return t('info.state.permanent');
  return active ? t('info.state.session') : t('info.state.inactive');
});

function message(o: OutcomeStatus | null): string | null {
  if (!o) return null;
  switch (o.status) {
    case 'done':
    case 'unchanged':
      return null;
    case 'conflict':
      return o.with.kind === 'system'
        ? t('info.outcome.system', { path: o.with.path })
        : t('info.outcome.active', { face: String(o.with.face) });
    case 'skipped':
      return o.reason;
    case 'failed':
      return errorMessage(o.error);
  }
}
const outcomeMessage = computed(() => message(selection.outcome));

// The question takes the focus, so the keyboard answers it at once.
const keepButton = ref<HTMLButtonElement | null>(null);
watch(
  () => selection.pending,
  async (p) => {
    if (!p) return;
    await nextTick();
    keepButton.value?.focus();
  },
);

function clashText(c: Conflict): string {
  return c.with.kind === 'system'
    ? t('info.clash.system', { name: c.postscriptName, path: c.with.path })
    : t('info.clash.active', { name: c.postscriptName, face: String(c.with.face) });
}
</script>

<template>
  <div class="info">
    <p v-if="selection.error" role="alert" class="error">{{ errorMessage(selection.error) }}</p>
    <p v-else-if="!d" class="placeholder">
      {{ selection.loading ? t('info.loading') : t('info.nothingSelected') }}
    </p>
    <template v-else>
      <header class="head">
        <h2 class="name">{{ d.face.familyName }} {{ d.face.style }}</h2>
        <HealthBadge :health="d.face.health" />
        <div class="activation">
          <button
            type="button"
            class="activate"
            :aria-pressed="d.face.active !== null"
            :disabled="!canActivate || selection.busy"
            :title="d.face.health.kind === 'ok' ? undefined : d.face.health.reason"
            @click="selection.toggleActivation()"
          >
            {{ d.face.active ? t('info.deactivate') : t('info.activate') }}
          </button>
          <span class="state" data-testid="activation-state">{{ activationState }}</span>
        </div>
        <p v-if="outcomeMessage" role="alert" class="outcome">{{ outcomeMessage }}</p>
        <div
          v-if="selection.pending"
          role="alertdialog"
          aria-labelledby="fm-clash-title"
          aria-describedby="fm-clash-text"
          class="clash"
        >
          <p id="fm-clash-title" class="clash-title">{{ t('info.clash.title') }}</p>
          <p id="fm-clash-text">
            <template v-for="c in selection.pending.conflicts" :key="clashText(c)">
              {{ clashText(c) }}
            </template>
          </p>
          <div class="clash-actions">
            <button
              ref="keepButton"
              type="button"
              class="choice"
              @click="selection.resolveConflict('keep')"
            >
              {{ t('info.clash.keep') }}
            </button>
            <button type="button" class="choice" @click="selection.resolveConflict('replace')">
              {{ t('info.clash.replace') }}
            </button>
            <button type="button" class="choice" @click="selection.resolveConflict('cancel')">
              {{ t('common.cancel') }}
            </button>
          </div>
        </div>
      </header>

      <InspectorPalettes />

      <section aria-labelledby="fm-info-names">
        <h3 id="fm-info-names">{{ t('info.names.title') }}</h3>
        <dl>
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
            <template v-else>—</template>
          </dd>
          <dt>{{ t('info.names.legacyStyle') }}</dt>
          <dd>
            <CopyValue
              v-if="d.legacyStyle"
              :label="t('info.copyLabels.legacyStyle')"
              :value="d.legacyStyle"
            />
            <template v-else>—</template>
          </dd>
          <dt>{{ t('info.names.postscript') }}</dt>
          <dd>
            <CopyValue :label="t('info.copyLabels.postscript')" :value="d.face.postscriptName" />
          </dd>
          <dt>{{ t('info.names.version') }}</dt>
          <dd>{{ show(d.version) }}</dd>
          <dt>{{ t('info.names.foundry') }}</dt>
          <dd>{{ show(d.foundry) }}</dd>
          <dt>{{ t('info.names.vendorId') }}</dt>
          <dd>{{ show(d.vendorId) }}</dd>
          <dt>{{ t('info.names.designer') }}</dt>
          <dd>{{ show(d.designer) }}</dd>
          <dt>{{ t('info.names.copyright') }}</dt>
          <dd>{{ show(d.copyright) }}</dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-file">
        <h3 id="fm-info-file">{{ t('info.file.title') }}</h3>
        <dl>
          <dt>{{ t('info.file.path') }}</dt>
          <dd><CopyValue :label="t('info.copyLabels.path')" :value="d.path" /></dd>
          <dt>{{ t('info.file.format') }}</dt>
          <dd>{{ t(`info.formats.${d.face.format}`) }}</dd>
          <dt>{{ t('info.file.faceInFile') }}</dt>
          <dd>{{ show(d.collectionIndex) }}</dd>
          <dt>{{ t('info.file.size') }}</dt>
          <dd>{{ t('info.file.bytes', { size: show(d.fileSize) }) }}</dd>
          <dt>{{ t('info.file.hash') }}</dt>
          <dd class="mono">{{ d.hash }}</dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-style">
        <h3 id="fm-info-style">{{ t('info.style.title') }}</h3>
        <dl>
          <dt>{{ t('info.style.weight') }}</dt>
          <dd>{{ show(d.face.weight) }}</dd>
          <dt>{{ t('info.style.width') }}</dt>
          <dd>{{ show(d.face.width) }}</dd>
          <dt>{{ t('info.style.slant') }}</dt>
          <dd class="cap">{{ t(`info.slants.${d.face.slant}`) }}</dd>
          <dt>{{ t('info.style.italicAngle') }}</dt>
          <dd>{{ degrees(d.italicAngle) }}</dd>
          <dt>{{ t('info.style.variable') }}</dt>
          <dd>{{ d.face.isVariable ? t('info.style.yes') : t('info.style.no') }}</dd>
          <dt>{{ t('info.style.axes') }}</dt>
          <dd>
            <ul v-if="d.axes.length" class="plain">
              <li v-for="a in d.axes" :key="a.tag">
                <span class="mono">{{ a.tag }}</span>
                {{ axisText(a) }}
              </li>
            </ul>
            <template v-else>—</template>
          </dd>
          <dt>{{ t('info.style.namedInstances') }}</dt>
          <dd>{{ list(d.instances.map((i) => i.name)) }}</dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-metrics">
        <h3 id="fm-info-metrics">{{ t('info.metrics.title') }}</h3>
        <dl>
          <dt>{{ t('info.metrics.unitsPerEm') }}</dt>
          <dd>{{ show(d.metrics.unitsPerEm) }}</dd>
          <dt>{{ t('info.metrics.ascender') }}</dt>
          <dd>{{ show(d.metrics.ascender) }}</dd>
          <dt>{{ t('info.metrics.descender') }}</dt>
          <dd>{{ show(d.metrics.descender) }}</dd>
          <dt>{{ t('info.metrics.xHeight') }}</dt>
          <dd>{{ show(d.metrics.xHeight) }}</dd>
          <dt>{{ t('info.metrics.capHeight') }}</dt>
          <dd>{{ show(d.metrics.capHeight) }}</dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-layout">
        <h3 id="fm-info-layout">{{ t('info.layout.title') }}</h3>
        <dl>
          <dt>{{ t('info.layout.codePoints') }}</dt>
          <dd>{{ show(d.codePointCount) }}</dd>
          <dt>{{ t('info.layout.scripts') }}</dt>
          <dd class="mono">{{ list(d.scripts) }}</dd>
          <dt>{{ t('info.layout.features') }}</dt>
          <dd class="mono">
            {{ list(d.features.map((f) => (f.name ? `${f.tag} (${f.name})` : f.tag))) }}
          </dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-license">
        <h3 id="fm-info-license">{{ t('info.license.title') }}</h3>
        <dl>
          <dt>{{ t('info.license.license') }}</dt>
          <dd>{{ show(d.license.description) }}</dd>
          <dt>{{ t('info.license.url') }}</dt>
          <dd>
            <CopyValue
              v-if="d.license.url"
              :label="t('info.copyLabels.licenseUrl')"
              :value="d.license.url"
            />
            <template v-else>—</template>
          </dd>
          <dt>{{ t('info.license.recognized') }}</dt>
          <dd>
            {{ d.license.openLicense ? t(`info.openLicenses.${d.license.openLicense}`) : '—' }}
          </dd>
          <dt>{{ t('info.license.embedding') }}</dt>
          <dd>{{ t(`info.embeddings.${d.license.embedding}`) }}</dd>
          <dt>{{ t('info.license.webUse') }}</dt>
          <dd>{{ show(d.license.webUse?.reason) }}</dd>
        </dl>
        <InspectorLicense />
      </section>

      <section aria-labelledby="fm-info-user">
        <h3 id="fm-info-user">{{ t('info.user.title') }}</h3>
        <p v-if="selection.editError" role="alert" class="error">
          {{ errorMessage(selection.editError) }}
        </p>
        <dl>
          <dt id="fm-info-tags">{{ t('info.user.tags') }}</dt>
          <dd>
            <ul class="tags" aria-labelledby="fm-info-tags">
              <li v-for="name in d.tags" :key="name" class="tag">
                {{ name }}
                <button
                  type="button"
                  class="untag"
                  :aria-label="t('info.user.removeTag', { tag: name })"
                  @click="selection.removeTag(name)"
                >
                  ×
                </button>
              </li>
            </ul>
            <form class="add-tag" @submit.prevent="addTag">
              <input
                v-model="newTag"
                class="tag-input"
                list="fm-tag-names"
                :aria-label="t('info.user.addTag')"
                :placeholder="t('info.user.addTagPlaceholder')"
                autocomplete="off"
              />
              <datalist id="fm-tag-names">
                <option v-for="tag in suggestions" :key="tag.id" :value="tag.name" />
              </datalist>
            </form>
          </dd>
          <dt>{{ t('info.user.favorite') }}</dt>
          <dd>
            <button
              type="button"
              class="favorite"
              :aria-pressed="d.favorite"
              @click="selection.setFavorite(!d.favorite)"
            >
              <span aria-hidden="true">{{ d.favorite ? '♥' : '♡' }}</span>
              {{ t('info.user.favorite') }}
            </button>
          </dd>
          <dt id="fm-info-rating">{{ t('info.user.rating') }}</dt>
          <dd>
            <div class="stars" role="group" aria-labelledby="fm-info-rating">
              <button
                v-for="n in STARS"
                :key="n"
                type="button"
                class="star"
                :class="{ on: (d.rating ?? 0) >= n }"
                :aria-label="starLabel(n)"
                :aria-pressed="d.rating === n"
                :title="d.rating === n ? t('info.user.clearRating') : starLabel(n)"
                @click="rate(n)"
              >
                {{ (d.rating ?? 0) >= n ? '★' : '☆' }}
              </button>
              <span class="state" data-testid="rating">{{
                d.rating ? t('info.user.ratingOf', { rating: d.rating }) : t('info.user.notRated')
              }}</span>
            </div>
          </dd>
          <dt>
            <label for="fm-info-notes">{{ t('info.user.notes') }}</label>
          </dt>
          <dd>
            <textarea
              id="fm-info-notes"
              v-model="faceNotes"
              class="notes"
              rows="3"
              :placeholder="t('info.user.notesPlaceholder')"
              @blur="saveFaceNotes"
            ></textarea>
          </dd>
          <dt>
            <label for="fm-info-family-notes">{{ t('info.user.familyNotes') }}</label>
          </dt>
          <dd>
            <textarea
              id="fm-info-family-notes"
              v-model="familyNotes"
              class="notes"
              rows="3"
              :placeholder="t('info.user.familyNotesPlaceholder', { family: d.face.familyName })"
              @blur="saveFamilyNotes"
            ></textarea>
          </dd>
        </dl>
      </section>
    </template>
  </div>
</template>

<style scoped>
.info {
  padding: 16px;
}
.placeholder {
  margin: 0;
  color: var(--fm-muted);
}
.error,
.outcome {
  margin: 0;
  color: var(--fm-danger);
}
.clash {
  display: grid;
  gap: 8px;
  padding: 8px 12px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
}
.clash p {
  margin: 0;
}
.clash-title {
  font-weight: 600;
}
.clash-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.choice {
  padding: 4px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.head {
  display: grid;
  gap: 8px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--fm-border);
}
.name {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  overflow-wrap: anywhere;
}
.activation {
  display: flex;
  align-items: center;
  gap: 8px;
}
.activate {
  padding: 4px 16px;
  border: 1px solid var(--fm-accent);
  border-radius: 8px;
  background: var(--fm-accent);
  color: var(--fm-bg);
  font: inherit;
  font-weight: 600;
}
.activate[aria-pressed='true'] {
  background: transparent;
  color: var(--fm-accent);
}
.activate:disabled {
  border-color: var(--fm-border);
  background: transparent;
  color: var(--fm-muted);
}
.state {
  color: var(--fm-muted);
}
h3 {
  margin: 16px 0 8px;
  color: var(--fm-muted);
  font-size: 11.5px;
  font-weight: 600;
  text-transform: uppercase;
}
dl {
  display: grid;
  grid-template-columns: 120px minmax(0, 1fr);
  gap: 4px 8px;
  margin: 0;
}
dt {
  color: var(--fm-muted);
}
dd {
  margin: 0;
  min-width: 0;
  overflow-wrap: anywhere;
}
.mono {
  font-family: ui-monospace, monospace;
}
.cap {
  text-transform: capitalize;
}
.plain {
  margin: 0;
  padding: 0;
  list-style: none;
}
.tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin: 0 0 4px;
  padding: 0;
  list-style: none;
}
.tag {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 0 0 0 8px;
  border: 1px solid var(--fm-border);
  border-radius: 999px;
}
.untag,
.favorite,
.star {
  border: 0;
  background: transparent;
  color: inherit;
  font: inherit;
  cursor: pointer;
}
.untag {
  padding: 0 6px;
  color: var(--fm-muted);
}
.tag-input {
  width: 100%;
  padding: 2px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.notes {
  display: block;
  box-sizing: border-box;
  width: 100%;
  padding: 4px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  resize: vertical;
}
.favorite {
  padding: 0;
}
.favorite[aria-pressed='true'] span {
  color: var(--fm-danger);
}
.stars {
  display: flex;
  align-items: center;
  gap: 2px;
}
.star {
  padding: 0 2px;
  color: var(--fm-muted);
  font-size: 16px;
}
.star.on {
  color: var(--fm-accent);
}
.stars .state {
  margin-left: 6px;
}
</style>
