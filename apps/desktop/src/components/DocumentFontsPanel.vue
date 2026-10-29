<script setup lang="ts">
// "Document fonts" (E32.2): every font a design document names, each Active, In library
// or Missing as `checkDocumentFonts` answers, and "Activate N fonts" for the in-library
// ones. Docked below the font list like the similar fonts, not a modal: the list and the
// inspector stay usable. Opened from the command palette ("Check a document's fonts…").
//
// Keys: Tab reaches the close button, every row and the Activate button; Up/Down
// (Home/End) move between rows; Enter or a click on a library font shows it in the
// inspector; Escape closes and gives the focus back to where it was.
import { computed, nextTick, ref, watch } from 'vue';
import type { DocumentFontCheck, FaceId } from '../api/types';
import { t } from '../i18n';
import { useActivationStore } from '../stores/activation';
import { useActions } from '../stores/commands';
import { useDocumentFontsStore } from '../stores/documentFonts';

const emit = defineEmits<{
  /** A library font was picked: show its face in the inspector. */
  select: [face: FaceId];
}>();

const docs = useDocumentFontsStore();
const activation = useActivationStore();
const root = ref<HTMLElement | null>(null);
const list = ref<HTMLElement | null>(null);
let returnFocus: HTMLElement | null = null;

useActions([
  {
    id: 'document.checkFonts',
    get title() {
      return t('documentFonts.action');
    },
    get group() {
      return t('palette.groups.activation');
    },
    run: () => docs.choose(),
  },
]);

/** The document's own name for a font: its PostScript name, else family and style. */
function nameOf(f: DocumentFontCheck): string {
  const { postscriptName, family, style } = f.font;
  return postscriptName ?? [family, style].filter(Boolean).join(' ');
}

const fileName = computed(() => docs.path?.split(/[\\/]/).pop() ?? '');
const rows = computed(() =>
  (docs.report?.fonts ?? []).map((f, i) => ({
    key: `${i}-${nameOf(f)}`,
    font: f,
    name: nameOf(f),
    status: t(`documentFonts.status.${f.status}`),
  })),
);
const toActivate = computed(() => docs.report?.toActivate.length ?? 0);
const anyMissing = computed(() => rows.value.some((r) => r.font.status === 'missing'));

const buttons = () => [...(list.value?.querySelectorAll<HTMLElement>('.row') ?? [])];

// A new document takes the focus into the panel, on its first row; a re-check after an
// activation keeps the focus where it is.
watch(
  () => docs.path,
  async (path) => {
    if (!path) return;
    const el = document.activeElement;
    if (!root.value?.contains(el)) returnFocus = el instanceof HTMLElement ? el : null;
    await nextTick();
    (buttons()[0] ?? root.value)?.focus();
  },
);

// The Activate button goes once every library font is active: the focus moves to the
// rows instead of falling back to the page.
watch(toActivate, async (n) => {
  if (n || !root.value) return;
  await nextTick();
  const el = document.activeElement;
  if (!el || el === document.body) (buttons()[0] ?? root.value)?.focus();
});

function pick(f: DocumentFontCheck): void {
  if (f.face != null) emit('select', f.face);
}

function close(): void {
  void docs.close();
  const target = returnFocus;
  returnFocus = null;
  void nextTick(() => {
    if (target?.isConnected) target.focus();
  });
}

function onKeyDown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    close();
  } else if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) {
    const all = buttons();
    const at = all.indexOf(document.activeElement as HTMLElement);
    if (!all.length || at < 0) return;
    const next =
      e.key === 'Home'
        ? 0
        : e.key === 'End'
          ? all.length - 1
          : e.key === 'ArrowDown'
            ? Math.min(all.length - 1, at + 1)
            : Math.max(0, at - 1);
    all[next]?.focus();
  } else {
    return;
  }
  e.preventDefault();
  e.stopPropagation();
}
</script>

<template>
  <section
    v-if="docs.open && docs.report"
    ref="root"
    class="docs"
    role="region"
    aria-labelledby="fm-docfonts-title"
    tabindex="-1"
    data-testid="document-fonts"
    @keydown="onKeyDown"
  >
    <header class="head">
      <h2 id="fm-docfonts-title" class="title">{{ t('documentFonts.title') }}</h2>
      <p class="file">
        <bdi>{{ t('documentFonts.document', { name: fileName }) }}</bdi>
      </p>
      <p class="hint">{{ t('documentFonts.hint') }}</p>
      <button type="button" class="close" :aria-label="t('documentFonts.close')" @click="close">
        ×
      </button>
    </header>
    <p v-if="!rows.length" class="message" role="status">{{ t('documentFonts.empty') }}</p>
    <ul v-else ref="list" class="list" :aria-label="t('documentFonts.fonts')">
      <li v-for="row in rows" :key="row.key">
        <button
          type="button"
          class="row"
          :data-status="row.font.status"
          :aria-label="t('documentFonts.rowLabel', { name: row.name, status: row.status })"
          :aria-disabled="row.font.face == null ? 'true' : undefined"
          @click="pick(row.font)"
        >
          <bdi class="name">{{ row.name }}</bdi>
          <span class="status" :class="row.font.status" data-testid="document-font-status">{{
            row.status
          }}</span>
        </button>
      </li>
    </ul>
    <footer class="foot">
      <p v-if="anyMissing" class="note">{{ t('documentFonts.missingNote') }}</p>
      <p v-if="rows.length && !toActivate" class="note" role="status">
        {{ t('documentFonts.allActive') }}
      </p>
      <button
        v-if="toActivate"
        type="button"
        class="activate"
        :disabled="activation.busy"
        @click="docs.activate()"
      >
        {{ t('documentFonts.activate', { count: toActivate }) }}
      </button>
    </footer>
  </section>
</template>

<style scoped>
.docs {
  display: flex;
  flex: none;
  flex-direction: column;
  max-height: 45%;
  border-top: 1px solid var(--fm-border);
  background: var(--fm-bg);
}
.docs:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 8px 16px 4px;
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.file {
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.hint {
  flex: 1;
  margin: 0;
  color: var(--fm-muted);
  font-size: 11.5px;
}
.close {
  padding: 0 8px;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.message,
.note {
  margin: 0;
  padding: 4px 16px;
  color: var(--fm-muted);
}
.list {
  margin: 0;
  padding: 0;
  overflow-y: auto;
  list-style: none;
}
.row {
  display: flex;
  align-items: baseline;
  gap: 12px;
  width: 100%;
  padding: 6px 16px;
  border: 0;
  border-bottom: 1px solid var(--fm-border);
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: start;
}
.row:focus-visible,
.close:focus-visible,
.activate:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: -2px;
}
.name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.status {
  font-size: 11.5px;
  font-weight: 600;
}
.status.active {
  color: var(--fm-accent);
}
.status.inLibrary {
  color: var(--fm-fg);
}
.status.missing {
  color: var(--fm-danger);
}
.foot {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding: 6px 0 10px;
}
.activate {
  margin-inline: 16px;
  padding: 2px 12px;
  border: 1px solid var(--fm-accent);
  border-radius: 6px;
  background: transparent;
  color: var(--fm-accent);
  font: inherit;
  font-weight: 600;
}
</style>
