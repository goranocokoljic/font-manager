<script setup lang="ts">
// The Pairing Studio's roles rail (E45.2, board 5): one card per role of the layout, in the
// order the core lists them (each context defines its own). A card has the role's name, a lock
// toggle, a font picker and a line with the face's style and the size / leading of the role's
// first block.
//
// The picker is a `UiSelect` that searches the library (fm-core's search, through the studio
// store); every family in it is set in itself (its best face, loaded through the face loader
// while it is listed). Choosing one sets every block of the role in that family's best face.
//
// The card of the selected block's role has the accent ring; selecting a block on the page
// scrolls its card into view (`reveal`). The focus stays on the block, so its keys (Enter edits
// the text, the arrows resize) keep working.
//
// Locks live in the studio store for the session; Shuffle (E45.3) leaves locked roles alone.
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue';
import type { BlockRole, FaceId, FaceSummary, Layout, LayoutRole } from '../api/types';
import { errorMessage, t } from '../i18n';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import { useStudioStore } from '../stores/studio';
import { sizeLabel } from '../textLayout/gesture';
import UiIconButton from './ui/UiIconButton.vue';
import UiSelect from './ui/UiSelect.vue';
import type { SelectOption } from './ui/types';

const props = defineProps<{
  /** The layout's block-font scope (`blockScope`). */
  scope: string;
  /** The layout the roles belong to (the draft, so sizes follow every edit). */
  layout: Layout;
  /** The panel's face: a role without a face of its own is set in it. */
  face: FaceSummary | null;
  /** The role of the selected block; its card has the ring. */
  selected: BlockRole | null;
}>();

const studio = useStudioStore();
const loader = useFaceLoader();
const headingId = `fm-roles-${useId()}`;

const roles = computed(() => props.layout.roles ?? []);

/** The face role `r` is set in: its own, else the panel's. */
const faceOf = (r: LayoutRole) => studio.roleFace(props.scope, r) ?? props.face;

const familyCss = (face: FaceId) => `"${faceFamily(face)}", var(--fm-font-ui)`;

/** The picker's options for role `r`: the last search's families, the role's own first. */
function optionsOf(r: LayoutRole): SelectOption<number>[] {
  const own = faceOf(r);
  const hits = studio.searchFor === studio.roleKey(props.scope, r.role) ? studio.results : [];
  const options = hits.flatMap((h) =>
    h.faces[0] === undefined
      ? []
      : [{ value: h.family.id, label: h.family.name, fontFamily: familyCss(h.faces[0]) }],
  );
  if (own && !options.some((o) => o.value === own.family)) {
    options.unshift({ value: own.family, label: own.familyName, fontFamily: familyCss(own.id) });
  }
  return options;
}

/** A family chosen in role `r`'s picker: its best face from the last search sets the role. */
function choose(r: LayoutRole, family: number): void {
  const hit = studio.results.find((h) => h.family.id === family);
  if (hit) void studio.pickRoleFamily(props.scope, r, hit);
}

/** "SemiBold · 40 pt · 1.05": the face's style and the first block's size and leading. */
function detailOf(r: LayoutRole): string {
  const block = props.layout.blocks[r.blocks[0] ?? -1];
  const parts = [faceOf(r)?.style ?? ''];
  if (block) {
    parts.push(
      sizeLabel({ sizePt: block.sizePt ?? Number.NaN, lineHeight: block.lineHeight ?? Number.NaN }),
    );
  }
  return parts.filter(Boolean).join(' · ');
}

// Every face a picker shows stays loaded while it is listed, so each family is drawn in itself.
const held = new Set<FaceId>();
function hold(ids: FaceId[]): void {
  const wanted = new Set(ids);
  for (const id of held) {
    if (!wanted.has(id)) {
      held.delete(id);
      loader.release(id);
    }
  }
  for (const id of wanted) {
    if (held.has(id)) continue;
    held.add(id);
    loader.acquire(id).catch(() => undefined);
  }
}
watch(
  () => {
    const ids = roles.value.flatMap((r) => {
      const own = faceOf(r);
      return own ? [own.id] : [];
    });
    const listed = roles.value.some(
      (r) => studio.searchFor === studio.roleKey(props.scope, r.role),
    );
    if (listed)
      ids.push(...studio.results.flatMap((h) => (h.faces[0] === undefined ? [] : [h.faces[0]])));
    return ids;
  },
  (ids) => hold(ids),
  { immediate: true },
);
onBeforeUnmount(() => hold([]));

const cards = ref<HTMLElement[]>([]);

/** Scrolls the card of role `role` into view. */
function reveal(role: BlockRole): void {
  void nextTick(() =>
    cards.value
      .find((c) => c.dataset.role === role)
      ?.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' }),
  );
}
defineExpose({ reveal });

const roleName = (role: BlockRole) => t(`studio.role.${role}`);
</script>

<template>
  <aside class="rail fm-no-print" :aria-labelledby="headingId" data-testid="studio-roles">
    <div class="head">
      <h2 :id="headingId" class="title">{{ t('studio.roles') }}</h2>
      <span class="note">{{ t('studio.rolesNote') }}</span>
    </div>
    <p v-if="studio.error" class="error" role="alert">{{ errorMessage(studio.error) }}</p>
    <ul class="cards">
      <li
        v-for="r in roles"
        :key="r.role"
        ref="cards"
        class="card"
        :class="{ selected: selected === r.role }"
        data-testid="studio-role"
        :data-role="r.role"
        :aria-current="selected === r.role ? 'true' : undefined"
      >
        <div class="top">
          <h3 class="name">{{ roleName(r.role) }}</h3>
          <UiIconButton
            :icon="studio.isLocked(scope, r.role) ? 'lock' : 'unlock'"
            class="lock"
            :class="{ locked: studio.isLocked(scope, r.role) }"
            data-testid="studio-role-lock"
            :label="t('studio.lockRole', { role: roleName(r.role) })"
            :title="
              studio.isLocked(scope, r.role) ? t('studio.lockedTitle') : t('studio.unlockedTitle')
            "
            :aria-pressed="studio.isLocked(scope, r.role)"
            @click="studio.toggleLock(scope, r.role)"
          />
        </div>
        <UiSelect
          class="picker"
          data-testid="studio-role-font"
          :model-value="faceOf(r)?.family ?? null"
          :options="optionsOf(r)"
          :placeholder="t('studio.noFont')"
          :aria-label="t('studio.roleFont', { role: roleName(r.role) })"
          remote
          :fit="false"
          @search="(q) => studio.searchRole(scope, r.role, q)"
          @update:model-value="(family) => choose(r, family)"
        />
        <p class="detail" data-testid="studio-role-detail">{{ detailOf(r) }}</p>
      </li>
    </ul>
  </aside>
</template>

<style scoped>
.rail {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--fm-space-3);
  box-sizing: border-box;
  width: 300px;
  min-height: 0;
  padding: var(--fm-space-4);
  overflow-y: auto;
  border-inline-end: 1px solid var(--fm-border);
  background-color: var(--fm-surface);
}
.head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--fm-space-2);
}
.title {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
.note {
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  text-align: end;
}
.error {
  margin: 0;
  color: var(--fm-danger);
  font-size: var(--fm-text-sm);
}
.cards {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}
.card {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  padding: var(--fm-space-3);
  border: 1px solid var(--fm-border);
  border-radius: var(--fm-radius-lg);
  background-color: var(--fm-bg);
  transition: box-shadow var(--fm-motion-fast) var(--fm-ease);
}
/* The selected block's role: the accent ring, as the block on the page has. */
.card.selected {
  border-color: var(--fm-accent);
  box-shadow: 0 0 0 1px var(--fm-accent);
}
.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.name {
  margin: 0;
  font-size: var(--fm-text-sm);
  font-weight: 600;
}
.lock.locked {
  color: var(--fm-fg);
}
.card .picker {
  width: 100%;
  height: 40px;
  font-size: var(--fm-text-lg);
}
.detail {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-variant-numeric: tabular-nums;
}
</style>
