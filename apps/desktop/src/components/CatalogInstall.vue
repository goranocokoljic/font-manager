<script setup lang="ts">
// The progress line of an Install in the catalog browser (E22.7). The first Install, before a
// download folder is set, proposes `<Documents>/Font Manager Fonts` with "Use this folder"
// and "Choose…" (the native folder picker). Then it says downloading, scanning, done
// ("Installed Alegreya: 6 styles active") or why it failed. The core does the work; this
// line only shows the store's state.
import { computed } from 'vue';
import { errorMessage, t } from '../i18n';
import { installedActive, useCatalogsStore } from '../stores/catalogs';

const store = useCatalogsStore();
const install = computed(() => store.install);
const name = computed(() => install.value?.family.name ?? '');

const message = computed(() => {
  const i = install.value;
  if (!i) return '';
  switch (i.phase) {
    case 'proposing':
      return i.proposed ? t('catalogs.install.propose', { folder: i.proposed }) : '';
    case 'downloading':
      return t('catalogs.install.downloading', { name: name.value });
    case 'scanning':
      return t('catalogs.install.scanning', { name: name.value });
    case 'done': {
      const r = i.result;
      if (!r) return '';
      const count = installedActive(r);
      return r.alreadyInstalled
        ? t('catalogs.install.already', { name: name.value, count })
        : t('catalogs.install.done', { name: name.value, count });
    }
    case 'failed':
      return t('catalogs.install.failed', {
        name: name.value,
        message: i.error ? errorMessage(i.error) : '',
      });
  }
  return '';
});
const stopped = computed(() => {
  const e = install.value?.result?.error;
  return e ? t('catalogs.install.stopped', { message: errorMessage(e) }) : '';
});
const running = computed(
  () => install.value?.phase === 'downloading' || install.value?.phase === 'scanning',
);
</script>

<template>
  <section
    v-if="install"
    class="install"
    :class="install.phase"
    :aria-label="t('catalogs.install.actionLabel', { name })"
    data-testid="catalog-install"
    :data-phase="install.phase"
  >
    <h2 v-if="install.phase === 'proposing'" class="title">
      {{ t('catalogs.install.proposeTitle') }}
    </h2>
    <p
      class="line"
      :role="install.phase === 'failed' ? 'alert' : 'status'"
      data-testid="catalog-install-status"
    >
      {{ message }}
    </p>
    <p v-if="stopped" class="line error" role="alert">{{ stopped }}</p>
    <div class="actions">
      <template v-if="install.phase === 'proposing'">
        <button
          type="button"
          class="button primary"
          data-testid="catalog-install-use"
          :disabled="!install.proposed"
          @click="store.useProposedFolder()"
        >
          {{ t('catalogs.install.useFolder') }}
        </button>
        <button
          type="button"
          class="button"
          data-testid="catalog-install-choose"
          @click="store.chooseInstallFolder()"
        >
          {{ t('catalogs.install.choose') }}
        </button>
      </template>
      <button
        v-if="!running"
        type="button"
        class="button"
        data-testid="catalog-install-close"
        @click="store.closeInstall()"
      >
        {{ t('catalogs.install.close') }}
      </button>
    </div>
  </section>
</template>

<style scoped>
.install {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 12px;
  margin: 0;
  padding: 6px 16px;
  border-bottom: 1px solid var(--fm-border);
}
.title {
  flex-basis: 100%;
  margin: 0;
  font-size: 13px;
}
.line {
  flex: 1 1 auto;
  margin: 0;
  min-width: 0;
}
.failed .line,
.error {
  color: var(--fm-danger, var(--fm-warning));
}
.actions {
  display: flex;
  gap: 8px;
}
.button {
  padding: 2px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: var(--fm-fg);
  font: inherit;
}
.button.primary {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
}
.button:disabled {
  color: var(--fm-muted);
}
</style>
