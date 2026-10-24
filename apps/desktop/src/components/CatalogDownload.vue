<script setup lang="ts">
// The "Download..." panel of the catalog browser (E22.3): asks which watched folder to
// download a family into (and static or variable files when the family has both), then
// shows the download, the rescan that brings its faces into the library, and the result.
// Not a modal: it sits above the list, and the list stays usable behind it. The core does
// the work (`download_family`); this only collects the choice and shows the state.
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import type { CatalogFileKind, SourceId } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useCatalogsStore } from '../stores/catalogs';
import { useSourcesStore } from '../stores/sources';

const emit = defineEmits<{ close: [] }>();

const store = useCatalogsStore();
const sourcesStore = useSourcesStore();

const state = computed(() => store.download);
const family = computed(() => state.value?.family ?? null);
/** Only folders the user added: never the system fonts or the library folder. */
const targets = computed(() => sourcesStore.sources.filter((s) => s.kind === 'watched'));
const source = ref<SourceId | null>(null);
const kind = ref<CatalogFileKind>('static');
const hasVariable = computed(() => (family.value?.axes?.length ?? 0) > 0);

watch(
  targets,
  (list) => {
    if (!list.some((s) => s.id === source.value)) source.value = list[0]?.id ?? null;
  },
  { immediate: true },
);
// A new family starts with its default kind: static, or variable when it has axes.
watch(
  family,
  (f) => {
    kind.value = (f?.axes?.length ?? 0) > 0 ? 'variable' : 'static';
  },
  { immediate: true },
);

const select = ref<HTMLSelectElement | null>(null);
const closeButton = ref<HTMLButtonElement | null>(null);
onMounted(async () => {
  if (!sourcesStore.sources.length) await sourcesStore.load();
  await nextTick();
  select.value?.focus();
});
// The result takes the focus to its Close button, so the keyboard user hears it.
watch(
  () => state.value?.phase,
  async (phase) => {
    if (phase === 'done') {
      await nextTick();
      closeButton.value?.focus();
    }
  },
);

const busy = computed(() => store.downloading);
const choosing = computed(
  () => state.value?.phase === 'choosing' || state.value?.phase === 'failed',
);

function submit(): void {
  if (source.value === null || busy.value) return;
  void store.startDownload(source.value, hasVariable.value ? kind.value : 'static');
}

function close(): void {
  if (busy.value) return;
  store.closeDownload();
  emit('close');
}

const status = computed(() => {
  const s = state.value;
  if (!s) return '';
  const name = s.family.name;
  if (s.phase === 'downloading') return t('catalogs.download.downloading', { name });
  if (s.phase === 'scanning')
    return s.scan && s.scan.total > 0
      ? t('catalogs.download.scanProgress', { name, done: s.scan.done, total: s.scan.total })
      : t('catalogs.download.scanning', { name });
  if (s.phase === 'done' && s.result) {
    const folder = s.result.download.folder;
    const count = s.result.download.files.length;
    const lines = [
      count > 0
        ? t('catalogs.download.done', { name, count, folder })
        : t('catalogs.download.nothingNew', { name, folder }),
    ];
    const skipped = s.result.skipped ?? [];
    if (count > 0 && skipped.length)
      lines.push(t('catalogs.download.skipped', { count: skipped.length }));
    return lines.join(' ');
  }
  return '';
});
const failure = computed(() => {
  const s = state.value;
  if (!s) return null;
  if (s.phase === 'failed' && s.error)
    return t('catalogs.download.failed', { message: errorMessage(s.error) });
  // Stopped after some files: those are written, kept and rescanned; the rest are not.
  const stopped = s.result?.error;
  const lines = stopped ? [t('catalogs.download.stopped', { message: errorMessage(stopped) })] : [];
  if (s.error) lines.push(t('catalogs.download.scanFailed', { message: errorMessage(s.error) }));
  return lines.length ? lines.join(' ') : null;
});
const folderName = computed(() => family.value?.name ?? '');
</script>

<template>
  <section
    v-if="state && family"
    class="download"
    aria-labelledby="catalog-download-title"
    data-testid="catalog-download"
    @keydown.esc.prevent="close"
  >
    <h2 id="catalog-download-title" class="title">
      {{ t('catalogs.download.title', { name: family.name }) }}
    </h2>
    <form v-if="choosing" class="form" @submit.prevent="submit">
      <template v-if="targets.length">
        <label class="field">
          <span>{{ t('catalogs.download.into') }}</span>
          <select ref="select" v-model="source" data-testid="catalog-download-source">
            <option v-for="s in targets" :key="s.id" :value="s.id">{{ s.path }}</option>
          </select>
        </label>
        <fieldset v-if="hasVariable" class="kinds">
          <legend>{{ t('catalogs.download.files') }}</legend>
          <label>
            <input v-model="kind" type="radio" name="catalog-download-kind" value="variable" />
            {{ t('catalogs.download.variable') }}
          </label>
          <label>
            <input v-model="kind" type="radio" name="catalog-download-kind" value="static" />
            {{ t('catalogs.download.static') }}
          </label>
        </fieldset>
        <p class="hint">
          {{ t('catalogs.download.where', { folder: folderName }) }}
        </p>
      </template>
      <p v-else class="hint" data-testid="catalog-download-no-folder">
        {{ t('catalogs.download.noFolder') }}
      </p>
      <div class="buttons">
        <button
          v-if="targets.length"
          type="submit"
          class="button primary"
          data-testid="catalog-download-start"
        >
          {{
            state.phase === 'failed' ? t('catalogs.download.retry') : t('catalogs.download.start')
          }}
        </button>
        <button type="button" class="button" @click="close">
          {{ t('catalogs.download.cancel') }}
        </button>
      </div>
    </form>
    <p class="status" role="status" data-testid="catalog-download-status">{{ status }}</p>
    <progress
      v-if="busy"
      class="bar"
      :aria-label="status"
      :max="state.scan?.total || undefined"
      :value="state.phase === 'scanning' && state.scan?.total ? state.scan.done : undefined"
    />
    <p v-if="failure" class="error" role="alert" data-testid="catalog-download-error">
      {{ failure }}
    </p>
    <div v-if="state.phase === 'done'" class="buttons">
      <button ref="closeButton" type="button" class="button" @click="close">
        {{ t('catalogs.download.close') }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.download {
  display: grid;
  gap: 8px;
  padding: 10px 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}
.form {
  display: grid;
  gap: 8px;
}
.field {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}
.field select {
  min-width: 0;
  max-width: 100%;
  padding: 4px 8px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.kinds {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin: 0;
  padding: 0;
  border: 0;
}
.kinds legend {
  float: inline-start;
  margin-inline-end: 8px;
  padding: 0;
}
.hint,
.status,
.error {
  margin: 0;
}
.hint {
  color: var(--fm-muted);
  font-size: 11.5px;
}
.status:empty {
  display: none;
}
.error {
  color: var(--fm-danger, var(--fm-warning));
}
.bar {
  width: 100%;
  max-width: 360px;
}
.buttons {
  display: flex;
  gap: 8px;
}
.button {
  padding: 4px 12px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button.primary {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
</style>
