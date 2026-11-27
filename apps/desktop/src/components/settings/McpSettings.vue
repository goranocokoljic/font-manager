<script setup lang="ts">
// Settings › AI assistants (E27.1; board 15, E57.6): the opt-in switch for the MCP server on
// the local API endpoint as a setting row; with it on, the server's address and "Connect an
// assistant", one block: a segmented control with the clients fm-api lists, the chosen
// client's snippet (written by fm-core with the real address and token) in the mono face,
// Copy, and the line that the snippet holds the token, with "How to connect" (the user docs
// page on AI assistants, E27.8). The snippet shows dots where the token is until Show: the
// dots plainly replace the token the store already holds (Settings › Local API's endpoint);
// Copy copies the real snippet either way. The Claude Desktop snippet (E27.7) starts the
// bundled stdio shim and holds no token. Below, one switch row per tool that changes
// something (E27.6): fm-core lists the tools and their grants, all off by default.
import { computed, onMounted, ref } from 'vue';
import { useClient } from '../../api/client';
import type { McpClient, McpGrant } from '../../api/types';
import { t, type MessageKey } from '../../i18n';
import { useSettingsStore } from '../../stores/settings';
import CopyValue from '../CopyValue.vue';
import UiButton from '../ui/UiButton.vue';
import UiSegmentedControl, { type SegmentOption } from '../ui/UiSegmentedControl.vue';
import UiSettingGroup from '../ui/UiSettingGroup.vue';
import UiSettingRow from '../ui/UiSettingRow.vue';
import UiSwitch from '../ui/UiSwitch.vue';
import { useCopy } from '../useCopy';
import SettingsSection from './SettingsSection.vue';

const store = useSettingsStore();
const shown = ref(false);
const busy = ref(false);
const { status: copyStatus, copy } = useCopy();

onMounted(() => {
  void store.loadMcp();
  // The token the snippets hold, so the hidden snippet can show dots in its place.
  void store.loadEndpoint();
});

const status = computed(() => store.mcp);

const titles: Record<McpClient, MessageKey> = {
  claudeCode: 'settings.mcp.clients.claudeCode',
  http: 'settings.mcp.clients.http',
  claudeDesktop: 'settings.mcp.clients.claudeDesktop',
};
const copyLabels: Record<McpClient, MessageKey> = {
  claudeCode: 'settings.mcp.copyClaudeCode',
  http: 'settings.mcp.copyHttp',
  claudeDesktop: 'settings.mcp.copyClaudeDesktop',
};

const snippets = computed(() => status.value?.snippets ?? []);
/** The client chosen in the segmented control; the first one fm-api lists until the user picks. */
const chosen = ref<McpClient | null>(null);
const snippet = computed(
  () => snippets.value.find((s) => s.client === chosen.value) ?? snippets.value[0] ?? null,
);
const clientOptions = computed<SegmentOption<McpClient>[]>(() =>
  snippets.value.map((s) => ({ value: s.client, label: t(titles[s.client]) })),
);
const choose = (client: McpClient) => (chosen.value = client);

/** The token as the store holds it; empty while the endpoint is not read yet. */
const token = computed(() => store.endpoint?.token ?? '');
/** Whether the chosen snippet holds the token (Claude Desktop's does not). */
const holdsToken = computed(() => !!token.value && !!snippet.value?.text.includes(token.value));
/**
 * The snippet as drawn: the real text once shown or when it holds no token; otherwise the
 * token replaced by as many dots. While the token is not known yet, nothing is drawn.
 */
const drawn = computed(() => {
  const s = snippet.value;
  if (!s) return null;
  if (shown.value) return s.text;
  if (!token.value) return null;
  return s.text.replaceAll(token.value, '•'.repeat(token.value.length));
});

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

async function toggleGrant(grant: McpGrant): Promise<void> {
  grantBusy.value = true;
  await store.setMcpGrant(grant.tool, !grant.granted);
  grantBusy.value = false;
}

/** A refused change leaves the switch where the API has it: the switch draws the store. */
async function toggle(): Promise<void> {
  busy.value = true;
  await store.setMcpEnabled(!(status.value?.enabled ?? false));
  busy.value = false;
}

const openHelp = () => useClient().openHelp('mcp');
</script>

<template>
  <SettingsSection id="mcp" :heading="t('settings.sections.mcp')" :lead="t('settings.mcp.note')">
    <template v-if="status">
      <UiSettingGroup>
        <UiSettingRow
          v-slot="{ control }"
          :title="t('settings.mcp.enable')"
          :line="t('settings.mcp.enableHint')"
        >
          <UiSwitch
            v-bind="control"
            name="mcp-enabled"
            :checked="status.enabled"
            :disabled="busy"
            @click="toggle"
          />
        </UiSettingRow>
        <UiSettingRow
          v-if="status.enabled"
          :title="t('settings.mcp.address')"
          :empty="status.url ? undefined : t('settings.mcp.notRunning')"
          data-testid="mcp-address"
        >
          <template v-if="status.url" #value>
            <span class="dot" aria-hidden="true" />{{ status.url }}
          </template>
          <CopyValue
            v-if="status.url"
            :value="status.url"
            :label="t('settings.mcp.addressLabel')"
            :text="t('settings.mcp.copy')"
          />
        </UiSettingRow>
      </UiSettingGroup>

      <template v-if="status.enabled">
        <UiSettingGroup v-if="snippet" :heading="t('settings.mcp.connectTitle')">
          <div class="connect" data-testid="mcp-connect">
            <div class="bar">
              <UiSegmentedControl
                :model-value="snippet.client"
                :options="clientOptions"
                :label="t('settings.mcp.clientsLabel')"
                wrap
                @update:model-value="choose"
              />
              <span class="actions">
                <UiButton
                  v-if="holdsToken"
                  aria-controls="mcp-snippet"
                  :aria-pressed="shown"
                  @click="shown = !shown"
                >
                  {{ shown ? t('settings.mcp.hide') : t('settings.mcp.show') }}
                </UiButton>
                <UiButton :aria-label="t(copyLabels[snippet.client])" @click="copy(snippet.text)">
                  {{ t('settings.mcp.copy') }}
                </UiButton>
              </span>
            </div>
            <pre
              id="mcp-snippet"
              class="snippet"
              :aria-label="t(titles[snippet.client])"
              data-testid="mcp-snippet"
            ><code v-if="drawn !== null">{{ drawn }}</code></pre>
            <p class="foot">
              <span id="mcp-snippet-hint">{{ t('settings.mcp.tokenHint') }}</span>
              <button type="button" class="link" @click="openHelp">
                {{ t('settings.mcp.help') }}
              </button>
            </p>
          </div>
        </UiSettingGroup>

        <UiSettingGroup
          v-if="status.grants?.length"
          :heading="t('settings.mcp.grantsTitle')"
          :note="t('settings.mcp.grantsNote')"
          data-testid="mcp-grants"
        >
          <UiSettingRow
            v-for="grant in status.grants"
            :key="grant.tool"
            v-slot="{ control }"
            :title="toolLabel(grant.tool)"
            :line="grant.confirm ? t('settings.mcp.asksEachTime') : undefined"
          >
            <UiSwitch
              v-bind="control"
              :name="`mcp-grant-${grant.tool}`"
              :checked="grant.granted"
              :disabled="grantBusy"
              @click="toggleGrant(grant)"
            />
          </UiSettingRow>
        </UiSettingGroup>
      </template>
      <p role="status" class="status">
        <template v-if="copyStatus === 'copied'">{{ t('common.copied') }}</template>
        <template v-else-if="copyStatus === 'failed'">{{ t('copy.failed') }}</template>
      </p>
    </template>
  </SettingsSection>
</template>

<style scoped>
.dot {
  display: inline-block;
  width: 6px;
  height: 6px;
  margin-inline-end: var(--fm-space-2);
  border-radius: 50%;
  background-color: var(--fm-fg);
  vertical-align: middle;
}
.bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--fm-space-3);
  padding: var(--fm-space-3) var(--fm-space-4);
}
.actions {
  display: flex;
  flex: none;
  gap: var(--fm-space-2);
}
/* The snippet on the surface colour, edge to edge inside the card (board 15). */
.snippet {
  min-height: 2lh;
  margin: 0;
  padding: var(--fm-space-4);
  background-color: var(--fm-surface);
  font-family: var(--fm-font-mono);
  font-size: var(--fm-text-sm);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.foot {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--fm-space-4);
  margin: 0;
  padding: var(--fm-space-3) var(--fm-space-4);
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.link {
  flex: none;
  padding: 0;
  border: 0;
  background: none;
  color: var(--fm-fg);
  font: inherit;
  text-decoration: underline;
  cursor: pointer;
}
.link:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 2px;
}
.status {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-sm);
}
.status:empty {
  display: none;
}
</style>
