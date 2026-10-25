<script setup lang="ts">
// Settings → AI assistants (E27.1): the opt-in switch for the MCP server on the
// local API endpoint, and copyable config snippets that fm-core writes with the
// real address and token. The snippets carry the token, so they stay hidden
// until the user asks to see them; Copy works either way. Below them, one switch
// per tool that changes something (E27.6): fm-core lists the tools and their
// grants, all off by default.
import { computed, onMounted, ref } from 'vue';
import type { McpClient, McpGrant } from '../api/types';
import { t, type MessageKey } from '../i18n';
import { useSettingsStore } from '../stores/settings';
import CopyValue from './CopyValue.vue';
import { useCopy } from './useCopy';

const store = useSettingsStore();
const shown = ref(false);
const busy = ref(false);
const { status: copyStatus, copy } = useCopy();

onMounted(() => {
  void store.loadMcp();
});

const status = computed(() => store.mcp);

const titles: Record<McpClient, 'settings.mcp.claudeCode' | 'settings.mcp.http'> = {
  claudeCode: 'settings.mcp.claudeCode',
  http: 'settings.mcp.http',
};
const copyLabels: Record<McpClient, 'settings.mcp.copyClaudeCode' | 'settings.mcp.copyHttp'> = {
  claudeCode: 'settings.mcp.copyClaudeCode',
  http: 'settings.mcp.copyHttp',
};

/** The switch label of each tool fm-core lists; a tool this UI does not know shows its name. */
const toolLabels: Record<string, MessageKey> = {
  activate_fonts: 'settings.mcp.tools.activate_fonts',
  deactivate_fonts: 'settings.mcp.tools.deactivate_fonts',
  tag_fonts: 'settings.mcp.tools.tag_fonts',
  export_web: 'settings.mcp.tools.export_web',
};
function toolLabel(tool: string): string {
  const key = toolLabels[tool];
  return key ? t(key) : tool;
}

const grantBusy = ref(false);

async function toggleGrant(grant: McpGrant, event: globalThis.Event): Promise<void> {
  const input = event.target as HTMLInputElement;
  grantBusy.value = true;
  const saved = await store.setMcpGrant(grant.tool, input.checked);
  grantBusy.value = false;
  if (!saved) input.checked = grant.granted;
}

async function toggle(event: globalThis.Event): Promise<void> {
  const input = event.target as HTMLInputElement;
  busy.value = true;
  const saved = await store.setMcpEnabled(input.checked);
  busy.value = false;
  // A refused change leaves the switch where the API has it.
  if (!saved) input.checked = status.value?.enabled ?? false;
}
</script>

<template>
  <section class="mcp" aria-labelledby="mcp-title">
    <h2 id="mcp-title" class="heading">{{ t('settings.sections.mcp') }}</h2>
    <p class="note">{{ t('settings.mcp.note') }}</p>
    <template v-if="status">
      <label class="choice">
        <input
          type="checkbox"
          role="switch"
          name="mcp-enabled"
          aria-describedby="mcp-enabled-hint"
          :checked="status.enabled"
          :aria-checked="status.enabled"
          :disabled="busy"
          @change="toggle"
        />
        {{ t('settings.mcp.enable') }}
      </label>
      <p id="mcp-enabled-hint" class="note hint">{{ t('settings.mcp.enableHint') }}</p>

      <template v-if="status.enabled">
        <div v-if="status.url" class="field">
          <span>{{ t('settings.mcp.address') }}</span>
          <span>
            <CopyValue :value="status.url" :label="t('settings.mcp.addressLabel')" />
          </span>
          <p class="note hint">{{ t('settings.mcp.addressHint') }}</p>
        </div>
        <p v-else class="note">{{ t('settings.mcp.notRunning') }}</p>

        <div v-if="status.snippets?.length" class="snippets">
          <div class="row">
            <button
              type="button"
              class="button"
              aria-controls="mcp-snippets"
              :aria-pressed="shown"
              @click="shown = !shown"
            >
              {{ shown ? t('settings.mcp.hideSnippets') : t('settings.mcp.showSnippets') }}
            </button>
          </div>
          <ul id="mcp-snippets" class="list">
            <li v-for="snippet in status.snippets" :key="snippet.client" class="field">
              <span :id="`mcp-snippet-${snippet.client}`">{{ t(titles[snippet.client]) }}</span>
              <div class="row">
                <button
                  type="button"
                  class="button"
                  :aria-label="t(copyLabels[snippet.client])"
                  @click="copy(snippet.text)"
                >
                  {{ t('settings.mcp.copy') }}
                </button>
              </div>
              <pre
                v-if="shown"
                class="snippet"
                :aria-labelledby="`mcp-snippet-${snippet.client}`"
              ><code>{{ snippet.text }}</code></pre>
            </li>
          </ul>
          <p class="note hint">{{ t('settings.mcp.tokenHint') }}</p>
        </div>

        <fieldset v-if="status.grants?.length" class="grants" aria-describedby="mcp-grants-note">
          <legend class="legend">{{ t('settings.mcp.grantsTitle') }}</legend>
          <p id="mcp-grants-note" class="note hint">{{ t('settings.mcp.grantsNote') }}</p>
          <div v-for="grant in status.grants" :key="grant.tool" class="grant">
            <label class="choice">
              <input
                type="checkbox"
                role="switch"
                :name="`mcp-grant-${grant.tool}`"
                :checked="grant.granted"
                :aria-checked="grant.granted"
                :aria-describedby="grant.confirm ? `mcp-grant-${grant.tool}-hint` : undefined"
                :disabled="grantBusy"
                @change="toggleGrant(grant, $event)"
              />
              {{ toolLabel(grant.tool) }}
            </label>
            <p v-if="grant.confirm" :id="`mcp-grant-${grant.tool}-hint`" class="note grant-hint">
              {{ t('settings.mcp.asksEachTime') }}
            </p>
          </div>
        </fieldset>
      </template>
      <p role="status" class="note status">
        <template v-if="copyStatus === 'copied'">{{ t('common.copied') }}</template>
        <template v-else-if="copyStatus === 'failed'">{{ t('copy.failed') }}</template>
      </p>
    </template>
  </section>
</template>

<style scoped>
.heading {
  margin: 0 0 12px;
  font-size: 15px;
}
.note {
  margin: 0 0 12px;
  color: var(--fm-muted);
}
.hint {
  margin: 4px 0 12px;
}
.status {
  margin-top: 8px;
}
.status:empty {
  display: none;
}
.grants {
  margin: 0 0 16px;
  padding: 0;
  border: 0;
}
.legend {
  margin: 0 0 4px;
  padding: 0;
  font-weight: 600;
}
.grant {
  margin: 0 0 8px;
}
.grant-hint {
  margin-block: 2px 0;
  margin-inline: 24px 0;
}
.choice {
  display: flex;
  align-items: center;
  gap: 6px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0 0 12px;
}
.field > span:first-child {
  font-weight: 600;
}
.list {
  margin: 12px 0 0;
  padding: 0;
  list-style: none;
}
.row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}
.snippet {
  margin: 0;
  padding: 8px;
  border: 1px solid var(--fm-border);
  border-radius: 6px;
  font-family: ui-monospace, monospace;
  font-size: 12px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.button {
  padding: 4px 10px;
  border: 1px solid var(--fm-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.button:disabled {
  opacity: 0.5;
}
</style>
