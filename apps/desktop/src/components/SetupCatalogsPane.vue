<script setup lang="ts">
// The right pane of the setup's catalogs step (board 13, E55.5): the folder tree an install
// lands in — the download folder, one folder per catalog that is on (its name from fm-api,
// `folderName`), and the family's folder inside it. With no catalog on it says that nothing
// is downloaded. The facts come from the setup store; this file only lays them out.
import { t } from '../i18n';
import { useSetupStore } from '../stores/setup';
import UiIcon from './ui/UiIcon.vue';

const setup = useSetupStore();
</script>

<template>
  <aside class="catalogs-pane" data-testid="setup-catalogs-pane">
    <h2 class="caption">{{ t('setup.catalogs.paneTitle') }}</h2>

    <div v-if="!setup.catalogsOn.length || !setup.downloadFolder" class="empty">
      <p class="empty-title">{{ t('setup.catalogs.paneEmptyTitle') }}</p>
      <p class="note">{{ t('setup.catalogs.paneEmptyNote') }}</p>
    </div>

    <ul v-else class="tree" :aria-label="t('setup.catalogs.treeLabel')">
      <li>
        <span class="node root">
          <UiIcon :name="setup.placeFolder?.cloud ? 'cloud' : 'folder'" />
          <bdi class="path">{{ setup.downloadFolder }}</bdi>
        </span>
        <ul>
          <li v-for="c in setup.catalogsOn" :key="c.id">
            <span class="node">
              <UiIcon name="folder" />
              <bdi>{{ c.folderName ?? c.name }}</bdi>
            </span>
            <ul>
              <li>
                <span class="node family">
                  <UiIcon name="folder" />
                  {{ t('setup.catalogs.familyPlaceholder') }}
                </span>
              </li>
            </ul>
          </li>
        </ul>
      </li>
    </ul>
    <p v-if="setup.catalogsOn.length && setup.downloadFolder" class="note footer">
      {{ t('setup.catalogs.paneFooter') }}
    </p>
  </aside>
</template>

<style scoped>
.catalogs-pane {
  display: flex;
  flex-direction: column;
  padding: 40px 64px 32px;
  overflow-y: auto;
  background-color: var(--fm-surface);
}
.caption {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.empty {
  margin: auto 0;
  max-width: 360px;
}
.empty-title {
  margin: 0 0 var(--fm-space-2);
  font-size: var(--fm-text-lg);
  font-weight: 500;
}
.note {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  line-height: 1.55;
}
.tree,
.tree ul {
  margin: 0;
  padding: 0;
  list-style: none;
}
.tree {
  margin-top: var(--fm-space-8);
}
.tree ul {
  margin-inline-start: var(--fm-space-3);
  padding-inline-start: var(--fm-space-4);
  border-inline-start: 1px solid var(--fm-border);
}
.node {
  display: flex;
  align-items: center;
  gap: var(--fm-space-2);
  min-width: 0;
  padding: var(--fm-space-1) 0;
}
.path {
  overflow: hidden;
  font-family: var(--fm-font-mono);
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.family {
  color: var(--fm-muted);
}
.footer {
  margin-top: var(--fm-space-6);
}
</style>
