<script setup lang="ts">
// The Info tab: every field fm-api reports for the selected face, its health
// and the activation button. Names and paths copy on click. Tags, favorite
// and rating are edited here; the API stores them.
import { computed, ref } from 'vue';
import type { Embedding, FontFormat, OpenLicense, OutcomeStatus } from '../api/types';
import { useSelectionStore } from '../stores/selection';
import { sameTagName, useTagsStore } from '../stores/tags';
import CopyValue from './CopyValue.vue';
import HealthBadge from './HealthBadge.vue';

const selection = useSelectionStore();
const tags = useTagsStore();
const d = computed(() => selection.detail);

// Tag names to complete from: the library's tags the face does not have yet.
const suggestions = computed(() => {
  const mine = d.value?.tags ?? [];
  return tags.tags.filter((t) => !mine.some((name) => sameTagName(name, t.name)));
});
const newTag = ref('');
async function addTag(): Promise<void> {
  if (!newTag.value.trim()) return;
  if (await selection.addTag(newTag.value)) newTag.value = '';
}
const STARS = [1, 2, 3, 4, 5] as const;
const starLabel = (n: number) => `${n} ${n === 1 ? 'star' : 'stars'}`;
/** Choosing the rating the face has clears it. */
function rate(n: number): void {
  void selection.setRating(d.value?.rating === n ? 0 : n);
}

const formats: Record<FontFormat, string> = {
  ttf: 'TrueType (TTF)',
  otf: 'OpenType CFF (OTF)',
  ttc: 'TrueType collection (TTC)',
  otc: 'OpenType collection (OTC)',
  woff: 'WOFF',
  woff2: 'WOFF2',
  type1: 'PostScript Type 1',
  dfont: 'Mac dfont',
  bitmap: 'Bitmap',
};
const embeddings: Record<Embedding, string> = {
  installable: 'Installable',
  editable: 'Editable',
  previewAndPrint: 'Preview and print',
  restricted: 'Restricted',
};
const openLicenses: Record<OpenLicense, string> = {
  ofl: 'SIL Open Font License',
  apache: 'Apache License 2.0',
  ufl: 'Ubuntu Font Licence',
  mit: 'MIT',
};

const number = new Intl.NumberFormat();
const show = (v: string | number | null | undefined) =>
  v === null || v === undefined || v === '' ? '—' : typeof v === 'number' ? number.format(v) : v;
const list = (items: string[]) => (items.length ? items.join(', ') : '—');
const degrees = (v: number | null | undefined) => (v === null || v === undefined ? '—' : `${v}°`);

const canActivate = computed(() => d.value?.face.health.kind === 'ok');
const activationState = computed(() => {
  const active = d.value?.face.active;
  if (active === 'permanent') return 'Active (permanent)';
  return active ? 'Active for this session' : 'Not active';
});

function message(o: OutcomeStatus | null): string | null {
  if (!o) return null;
  switch (o.status) {
    case 'done':
    case 'unchanged':
      return null;
    case 'conflict':
      return o.with.kind === 'system'
        ? `Not activated: a system font has the same PostScript name (${o.with.path}).`
        : `Not activated: another active font has the same PostScript name (face ${o.with.face}).`;
    case 'skipped':
      return o.reason;
    case 'failed':
      return o.error.message;
  }
}
const outcomeMessage = computed(() => message(selection.outcome));
</script>

<template>
  <div class="info">
    <p v-if="selection.error" role="alert" class="error">{{ selection.error.message }}</p>
    <p v-else-if="!d" class="placeholder">
      {{ selection.loading ? 'Loading…' : 'Nothing selected' }}
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
            {{ d.face.active ? 'Deactivate' : 'Activate' }}
          </button>
          <span class="state" data-testid="activation-state">{{ activationState }}</span>
        </div>
        <p v-if="outcomeMessage" role="alert" class="outcome">{{ outcomeMessage }}</p>
      </header>

      <section aria-labelledby="fm-info-names">
        <h3 id="fm-info-names">Names</h3>
        <dl>
          <dt>Family</dt>
          <dd><CopyValue label="family" :value="d.face.familyName" /></dd>
          <dt>Style</dt>
          <dd><CopyValue label="style" :value="d.face.style" /></dd>
          <dt>Family (name ID 1)</dt>
          <dd>
            <CopyValue v-if="d.legacyFamily" label="family (name ID 1)" :value="d.legacyFamily" />
            <template v-else>—</template>
          </dd>
          <dt>Style (name ID 2)</dt>
          <dd>
            <CopyValue v-if="d.legacyStyle" label="style (name ID 2)" :value="d.legacyStyle" />
            <template v-else>—</template>
          </dd>
          <dt>PostScript name</dt>
          <dd><CopyValue label="PostScript name" :value="d.face.postscriptName" /></dd>
          <dt>Version</dt>
          <dd>{{ show(d.version) }}</dd>
          <dt>Foundry</dt>
          <dd>{{ show(d.foundry) }}</dd>
          <dt>Vendor ID</dt>
          <dd>{{ show(d.vendorId) }}</dd>
          <dt>Designer</dt>
          <dd>{{ show(d.designer) }}</dd>
          <dt>Copyright</dt>
          <dd>{{ show(d.copyright) }}</dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-file">
        <h3 id="fm-info-file">File</h3>
        <dl>
          <dt>Path</dt>
          <dd><CopyValue label="path" :value="d.path" /></dd>
          <dt>Format</dt>
          <dd>{{ formats[d.face.format] }}</dd>
          <dt>Face in file</dt>
          <dd>{{ show(d.collectionIndex) }}</dd>
          <dt>Size</dt>
          <dd>{{ show(d.fileSize) }} bytes</dd>
          <dt>Hash</dt>
          <dd class="mono">{{ d.hash }}</dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-style">
        <h3 id="fm-info-style">Style</h3>
        <dl>
          <dt>Weight</dt>
          <dd>{{ show(d.face.weight) }}</dd>
          <dt>Width</dt>
          <dd>{{ show(d.face.width) }}</dd>
          <dt>Slant</dt>
          <dd class="cap">{{ d.face.slant }}</dd>
          <dt>Italic angle</dt>
          <dd>{{ degrees(d.italicAngle) }}</dd>
          <dt>Variable</dt>
          <dd>{{ d.face.isVariable ? 'Yes' : 'No' }}</dd>
          <dt>Axes</dt>
          <dd>
            <ul v-if="d.axes.length" class="plain">
              <li v-for="a in d.axes" :key="a.tag">
                <span class="mono">{{ a.tag }}</span> {{ a.name }}: {{ a.min }}–{{ a.max }} (default
                {{ a.default }})
              </li>
            </ul>
            <template v-else>—</template>
          </dd>
          <dt>Named instances</dt>
          <dd>{{ list(d.instances.map((i) => i.name)) }}</dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-metrics">
        <h3 id="fm-info-metrics">Metrics</h3>
        <dl>
          <dt>Units per em</dt>
          <dd>{{ show(d.metrics.unitsPerEm) }}</dd>
          <dt>Ascender</dt>
          <dd>{{ show(d.metrics.ascender) }}</dd>
          <dt>Descender</dt>
          <dd>{{ show(d.metrics.descender) }}</dd>
          <dt>x-height</dt>
          <dd>{{ show(d.metrics.xHeight) }}</dd>
          <dt>Cap height</dt>
          <dd>{{ show(d.metrics.capHeight) }}</dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-layout">
        <h3 id="fm-info-layout">Characters and features</h3>
        <dl>
          <dt>Code points</dt>
          <dd>{{ show(d.codePointCount) }}</dd>
          <dt>Scripts</dt>
          <dd class="mono">{{ list(d.scripts) }}</dd>
          <dt>Features</dt>
          <dd class="mono">
            {{ list(d.features.map((f) => (f.name ? `${f.tag} (${f.name})` : f.tag))) }}
          </dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-license">
        <h3 id="fm-info-license">License</h3>
        <dl>
          <dt>License</dt>
          <dd>{{ show(d.license.description) }}</dd>
          <dt>License URL</dt>
          <dd>
            <CopyValue v-if="d.license.url" label="license URL" :value="d.license.url" />
            <template v-else>—</template>
          </dd>
          <dt>Recognized as</dt>
          <dd>{{ d.license.openLicense ? openLicenses[d.license.openLicense] : '—' }}</dd>
          <dt>Embedding (fsType)</dt>
          <dd>{{ embeddings[d.license.embedding] }}</dd>
        </dl>
      </section>

      <section aria-labelledby="fm-info-user">
        <h3 id="fm-info-user">Your notes</h3>
        <p v-if="selection.editError" role="alert" class="error">
          {{ selection.editError.message }}
        </p>
        <dl>
          <dt id="fm-info-tags">Tags</dt>
          <dd>
            <ul class="tags" aria-labelledby="fm-info-tags">
              <li v-for="t in d.tags" :key="t" class="tag">
                {{ t }}
                <button
                  type="button"
                  class="untag"
                  :aria-label="`Remove tag ${t}`"
                  @click="selection.removeTag(t)"
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
                aria-label="Add a tag"
                placeholder="Add a tag…"
                autocomplete="off"
              />
              <datalist id="fm-tag-names">
                <option v-for="t in suggestions" :key="t.id" :value="t.name" />
              </datalist>
            </form>
          </dd>
          <dt>Favorite</dt>
          <dd>
            <button
              type="button"
              class="favorite"
              :aria-pressed="d.favorite"
              @click="selection.setFavorite(!d.favorite)"
            >
              <span aria-hidden="true">{{ d.favorite ? '♥' : '♡' }}</span> Favorite
            </button>
          </dd>
          <dt id="fm-info-rating">Rating</dt>
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
                :title="d.rating === n ? 'Clear the rating' : starLabel(n)"
                @click="rate(n)"
              >
                {{ (d.rating ?? 0) >= n ? '★' : '☆' }}
              </button>
              <span class="state" data-testid="rating">{{
                d.rating ? `${d.rating} of 5` : 'Not rated'
              }}</span>
            </div>
          </dd>
          <dt>Notes</dt>
          <dd>{{ show(d.notes) }}</dd>
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
