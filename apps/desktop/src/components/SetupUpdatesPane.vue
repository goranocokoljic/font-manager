<script setup lang="ts">
// The right pane of the setup's updates step (board 13, E55.6): what a check does. The
// release server is named as the API reports it (`UpdateSupport.releaseHost`), never written
// into a string; nothing about the user or the fonts is sent; nothing installs without asking.
import { computed } from 'vue';
import { t } from '../i18n';
import { useSetupStore } from '../stores/setup';
import UiIcon from './ui/UiIcon.vue';

const setup = useSetupStore();

/** "Asks {host} for the newest version", split around the host so it is set as code. */
const HOST = '\u0001host\u0001';
const asks = computed(() => {
  const [before = '', after = ''] = t('setup.updates.asks', { host: HOST }).split(HOST);
  return { before, after };
});
</script>

<template>
  <aside class="updates-pane" data-testid="setup-updates-pane">
    <h2 class="caption">{{ t('setup.updates.paneTitle') }}</h2>
    <ul class="facts">
      <li data-fact="request">
        <UiIcon name="globe" class="icon" />
        <div>
          <p class="title">
            <template v-if="setup.releaseHost">
              {{ asks.before
              }}<code class="host" data-testid="setup-release-host">{{ setup.releaseHost }}</code
              >{{ asks.after }}
            </template>
            <template v-else>{{ t('setup.updates.asksServer') }}</template>
          </p>
          <p class="note">{{ t('setup.updates.asksNote') }}</p>
        </div>
      </li>
      <li data-fact="private">
        <UiIcon name="lock" class="icon" />
        <div>
          <p class="title">{{ t('setup.updates.sendsNothing') }}</p>
          <p class="note">{{ t('setup.updates.sendsNothingNote') }}</p>
        </div>
      </li>
      <li data-fact="install">
        <UiIcon name="download" class="icon" />
        <div>
          <p class="title">{{ t('setup.updates.installsNothing') }}</p>
          <p class="note">{{ t('setup.updates.installsNothingNote') }}</p>
        </div>
      </li>
    </ul>
  </aside>
</template>

<style scoped>
.updates-pane {
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
.facts {
  max-width: 480px;
  margin: var(--fm-space-6) 0 0;
  padding: 0;
  border-top: 1px solid var(--fm-border);
  list-style: none;
}
.facts li {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: var(--fm-space-3);
  padding: var(--fm-space-4) 0;
  border-bottom: 1px solid var(--fm-border);
}
.icon {
  margin-top: 2px;
  color: var(--fm-muted);
}
.title {
  margin: 0 0 var(--fm-space-1);
  font-weight: 500;
}
.host {
  font-family: var(--fm-font-mono);
  font-size: 0.9em;
}
.note {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
  line-height: 1.55;
}
</style>
