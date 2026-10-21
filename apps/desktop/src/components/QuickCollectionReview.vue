<script setup lang="ts">
// The Quick Collection review (E13.3), shown in the center instead of the font
// list: its fonts with the view's preview settings, each with a Drop button, and
// "Save as collection" / "Tag all" for the fonts that are left. Long lists are
// virtualized like the font list: only the rows in the viewport exist.
//
// Keys: Tab reaches each row's Drop button; Up/Down move between them; Delete or
// Backspace drops the focused font and moves to the next one.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watchEffect } from 'vue';
import type { FaceId } from '../api/types';
import { poolSize, scrollToShow, windowRange } from '../list/virtual';
import { sampleText } from '../preview/sampleText';
import { DEFAULT_PREVIEW, previewRowHeight, type PreviewSettings } from '../stores/preview';
import { useQuickCollectionStore } from '../stores/quickCollection';
import { useSamplesStore } from '../stores/samples';
import FontPreviewRow from './FontPreviewRow.vue';

const props = withDefaults(defineProps<{ preview?: PreviewSettings; overscan?: number }>(), {
  preview: () => ({ ...DEFAULT_PREVIEW }),
  overscan: 4,
});

const store = useQuickCollectionStore();
const samples = useSamplesStore();

const FALLBACK_HEIGHT = 600;
const viewport = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(FALLBACK_HEIGHT);
let observer: ResizeObserver | undefined;

function measure(): void {
  const el = viewport.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  if (el.clientHeight > 0) height.value = el.clientHeight;
}

onMounted(() => {
  measure();
  if (typeof ResizeObserver !== 'undefined' && viewport.value) {
    observer = new ResizeObserver(measure);
    observer.observe(viewport.value);
  }
});
onBeforeUnmount(() => observer?.disconnect());

const rowPx = computed(() => previewRowHeight(props.preview));
// The Surface setting, as the font list applies it: the rows sit on the surface the
// text color was picked for, whatever the app theme.
const surfaceClass = computed(() =>
  props.preview.surface === 'auto' ? undefined : `surface-${props.preview.surface}`,
);
const count = computed(() => store.faces.length);
const range = computed(() =>
  windowRange(scrollTop.value, height.value, rowPx.value, props.overscan, count.value),
);
const pool = computed(() => poolSize(height.value, rowPx.value, props.overscan));

const visible = computed(() =>
  store.faces.slice(range.value.start, range.value.end).map((face, i) => {
    const index = range.value.start + i;
    const sample = sampleText(props.preview, face);
    const got = samples.get(face.id, sample);
    return {
      face,
      index,
      slot: index % pool.value,
      name: `${face.familyName} ${face.style}`,
      text: got?.text ?? sample,
      fallback: got?.fallback ?? null,
      missing: got?.missing ?? [],
    };
  }),
);

watchEffect(() => {
  for (const item of visible.value)
    samples.want(item.face.id, sampleText(props.preview, item.face));
});

/** Scrolls row `index` into view and focuses its Drop button. */
async function focusRow(index: number): Promise<void> {
  const el = viewport.value;
  if (!el || index < 0 || index >= count.value) return;
  const top = scrollToShow(index, rowPx.value, el.scrollTop, height.value);
  if (top !== el.scrollTop) el.scrollTop = top;
  measure();
  await nextTick();
  el.querySelector<HTMLElement>(`[data-index="${index}"] .drop`)?.focus();
}

async function drop(face: FaceId, index: number, refocus: boolean): Promise<void> {
  await store.drop(face);
  if (refocus) await focusRow(Math.min(index, count.value - 1));
}

function onRowKey(e: KeyboardEvent, face: FaceId, index: number): void {
  if (e.key === 'Delete' || e.key === 'Backspace') void drop(face, index, true);
  else if (e.key === 'ArrowDown') void focusRow(index + 1);
  else if (e.key === 'ArrowUp') void focusRow(index - 1);
  else return;
  e.preventDefault();
  e.stopPropagation();
}

const collectionName = ref('');
const tagName = ref('');
const busy = ref(false);

async function save(): Promise<void> {
  if (!collectionName.value.trim() || busy.value) return;
  busy.value = true;
  try {
    if (await store.saveAsCollection(collectionName.value)) collectionName.value = '';
  } finally {
    busy.value = false;
  }
}

async function tag(): Promise<void> {
  if (!tagName.value.trim() || busy.value) return;
  busy.value = true;
  try {
    if (await store.tagAll(tagName.value)) tagName.value = '';
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <section class="review" aria-labelledby="fm-quick-heading">
    <header class="toolbar">
      <h1 id="fm-quick-heading" class="title">Quick Collection</h1>
      <p class="count" role="status">
        {{ count === 1 ? '1 font' : `${count} fonts` }}
      </p>
      <button type="button" class="toggle" :disabled="!count" @click="store.clear">Clear</button>
      <button type="button" class="toggle" @click="store.close">Back to fonts</button>
    </header>
    <div class="actions">
      <form class="action" @submit.prevent="save">
        <label for="fm-quick-collection-name">Collection name</label>
        <input id="fm-quick-collection-name" v-model="collectionName" type="text" />
        <button type="submit" class="toggle" :disabled="!count || !collectionName.trim() || busy">
          Save as collection
        </button>
      </form>
      <form class="action" @submit.prevent="tag">
        <label for="fm-quick-tag">Tag</label>
        <input id="fm-quick-tag" v-model="tagName" type="text" />
        <button type="submit" class="toggle" :disabled="!count || !tagName.trim() || busy">
          Tag all
        </button>
      </form>
    </div>
    <div
      v-show="count"
      ref="viewport"
      class="viewport"
      :class="surfaceClass"
      :style="{ backgroundColor: preview.backgroundColor ?? undefined }"
      @scroll="measure"
    >
      <ul
        class="spacer"
        aria-label="Fonts in the Quick Collection"
        :style="{ height: `${count * rowPx}px` }"
      >
        <li
          v-for="item in visible"
          :key="item.slot"
          class="slot"
          :data-index="item.index"
          :data-face="item.face.id"
          :aria-setsize="count"
          :aria-posinset="item.index + 1"
          :style="{ height: `${rowPx}px`, transform: `translateY(${item.index * rowPx}px)` }"
          @keydown="onRowKey($event, item.face.id, item.index)"
        >
          <FontPreviewRow
            class="preview"
            :face="item.face"
            :text="item.text"
            :size-px="preview.sizePx"
            :line-height="preview.lineHeight"
            :tracking-em="preview.trackingEm"
            :align="preview.align"
            :color="preview.textColor"
            :fallback="item.fallback"
            :missing="item.missing"
          />
          <button
            type="button"
            class="drop toggle"
            :aria-label="`Drop ${item.name} from the Quick Collection`"
            aria-keyshortcuts="Delete Backspace"
            title="Drop from the Quick Collection (Delete)"
            @click="drop(item.face.id, item.index, false)"
          >
            Drop
          </button>
        </li>
      </ul>
    </div>
    <p v-if="!count" class="empty">
      The Quick Collection is empty. Select fonts and press B to add them.
    </p>
  </section>
</template>

<style scoped>
.review {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 48px;
  padding: 0 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.count {
  flex: 1;
  margin: 0;
  color: var(--fm-muted);
}
.toggle {
  padding: 4px 8px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.toggle:disabled {
  color: var(--fm-muted);
}
.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 24px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--fm-border);
}
.action {
  display: flex;
  align-items: center;
  gap: 6px;
}
.action input {
  width: 160px;
  padding: 2px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 4px;
  background: var(--fm-bg);
  color: inherit;
  font: inherit;
}
.viewport {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  contain: strict;
  background: var(--fm-bg);
  color: var(--fm-fg);
}
.spacer {
  position: relative;
  margin: 0;
  padding: 0;
  list-style: none;
}
.slot {
  position: absolute;
  top: 0;
  right: 0;
  left: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  box-sizing: border-box;
  padding-right: 16px;
  border-bottom: 1px solid var(--fm-border);
  contain: layout paint;
}
.slot:focus-within {
  box-shadow: inset 0 0 0 2px var(--fm-accent);
}
.preview {
  flex: 1;
  min-width: 0;
  border-bottom: 0;
}
.empty {
  display: grid;
  flex: 1;
  place-items: center;
  margin: 0;
  color: var(--fm-muted);
}
</style>
