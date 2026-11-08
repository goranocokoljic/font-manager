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
//
// Under the cards, "Try for <role>" (E45.3) lists the three faces of the library that pair best
// in the selected block's role (the first role while none is selected) with the locked roles,
// each set in itself with fm-core's reasons. A click sets that role alone; Ctrl/Cmd-Z undoes it.
// Nothing in the library to suggest: a button opens the online catalogs (`browse`).
import { computed, nextTick, onBeforeUnmount, ref, useId, watch } from 'vue';
import type {
  BlockRole,
  FaceId,
  FaceSummary,
  Layout,
  LayoutRole,
  PairingSuggestion,
} from '../api/types';
import { errorMessage, t } from '../i18n';
import { faceFamily, useFaceLoader } from '../preview/faceLoader';
import { partnerOf, useStudioStore } from '../stores/studio';
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

const emit = defineEmits<{
  /** "Browse catalogs": the library has nothing to suggest. */
  browse: [];
}>();

const studio = useStudioStore();
const loader = useFaceLoader();
const headingId = `fm-roles-${useId()}`;
const tryId = `fm-try-${useId()}`;

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

/** The role "Try for" suggests faces for: the selected block's, else the first. */
const tryRole = computed<LayoutRole | null>(
  () => roles.value.find((r) => r.role === props.selected) ?? roles.value[0] ?? null,
);
/** The list of `tryRole`, once the store holds it. */
const tryList = computed(() => {
  const list = studio.triesOf(props.scope);
  const role = tryRole.value;
  return list && role && list.key === studio.roleKey(props.scope, role.role) ? list : null;
});
const trySuggestions = computed<PairingSuggestion[]>(() => tryList.value?.suggestions ?? []);

// Asked again when the role, its face or the face it must go with changes.
watch(
  () => {
    const role = tryRole.value;
    if (!role) return null;
    const partner = partnerOf(
      roles.value,
      role.role,
      faceOf,
      (r) => studio.isLocked(props.scope, r),
      props.face,
    );
    return `${props.scope}@${role.role}:${faceOf(role)?.id ?? ''}:${partner?.id ?? ''}`;
  },
  (asked) => {
    const role = tryRole.value;
    if (asked && role) void studio.loadTries(props.scope, props.layout, role, props.face);
  },
  { immediate: true },
);

const trySection = ref<HTMLElement | null>(null);
const tryHeading = ref<HTMLElement | null>(null);
/** What the last Try pick did, for screen readers (a polite status). */
const announced = ref('');
/**
 * The place of the candidate a pick was made from, while the list asks again. A pick leaves the
 * picked family out of the next answer, so its button goes; the focus then moves to the candidate
 * now at that place (else the last one, the Browse button or the heading), never to the page body.
 */
let refocusAt: number | null = null;

/** A Try candidate was clicked: it sets its role alone and says so. */
function apply(s: PairingSuggestion, index: number): void {
  const role = tryRole.value;
  if (!role) return;
  const active = document.activeElement;
  refocusAt = active && trySection.value?.contains(active) ? index : null;
  studio.applySuggestion(props.scope, role, s);
  announced.value = t('studio.tryApplied', {
    role: roleName(role.role),
    family: s.face.familyName,
  });
}

// The list answered after a pick: the focus goes back into it.
watch(
  () => (tryList.value && !tryList.value.loading ? tryList.value : null),
  (list) => {
    if (!list || refocusAt === null) return;
    const at = refocusAt;
    refocusAt = null;
    void nextTick(() => {
      const section = trySection.value;
      const buttons = [
        ...(section?.querySelectorAll<HTMLElement>('[data-testid="studio-try-candidate"]') ?? []),
      ];
      const target =
        buttons[Math.min(at, buttons.length - 1)] ??
        section?.querySelector<HTMLElement>('[data-testid="studio-try-browse"]') ??
        tryHeading.value;
      target?.focus();
    });
  },
);

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
    ids.push(...trySuggestions.value.map((s) => s.face.id));
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
    <section
      v-if="tryRole"
      ref="trySection"
      class="try"
      :aria-labelledby="tryId"
      data-testid="studio-try"
    >
      <h3 :id="tryId" ref="tryHeading" class="title" tabindex="-1">
        {{ t('studio.tryFor', { role: roleName(tryRole.role) }) }}
      </h3>
      <p v-if="tryList?.error" class="error" role="alert">
        {{ errorMessage(tryList.error) }}
      </p>
      <div
        v-else-if="tryList && !tryList.loading && !trySuggestions.length"
        class="empty"
        data-testid="studio-try-empty"
      >
        <p class="note">{{ t('studio.tryNone', { role: roleName(tryRole.role) }) }}</p>
        <button
          type="button"
          class="browse"
          data-testid="studio-try-browse"
          @click="emit('browse')"
        >
          {{ t('family.browseCatalogs') }}
        </button>
      </div>
      <ul v-else class="candidates" :aria-busy="tryList?.loading ? 'true' : undefined">
        <li v-for="(s, i) in trySuggestions" :key="s.face.id">
          <button
            type="button"
            class="candidate"
            data-testid="studio-try-candidate"
            :aria-label="
              t('studio.tryApply', {
                family: s.face.familyName,
                role: roleName(tryRole.role),
                reasons: s.reasons.join(', '),
              })
            "
            @click="apply(s, i)"
          >
            <span class="family" :style="{ fontFamily: familyCss(s.face.id) }">
              {{ s.face.familyName }}
            </span>
            <span class="reasons" data-testid="studio-try-reasons">{{
              s.reasons.join(' · ')
            }}</span>
          </button>
        </li>
      </ul>
      <p class="sr-only" role="status" data-testid="studio-try-status">{{ announced }}</p>
    </section>
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
.try {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-2);
  padding-block-start: var(--fm-space-2);
}
.candidates {
  display: flex;
  flex-direction: column;
  gap: var(--fm-space-1);
  margin: 0;
  padding: 0;
  list-style: none;
}
.candidate {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  padding: var(--fm-space-2) var(--fm-space-3);
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-md);
  background-color: var(--fm-bg);
  color: var(--fm-fg);
  text-align: start;
  cursor: pointer;
  transition: background-color var(--fm-motion-fast) var(--fm-ease);
}
.candidate:hover {
  background-color: var(--fm-surface-2);
}
.candidate:focus-visible,
.browse:focus-visible {
  outline: 2px solid var(--fm-accent);
  outline-offset: 1px;
}
.family {
  overflow: hidden;
  font-size: var(--fm-text-lg);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.reasons {
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
}
.empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--fm-space-2);
}
.empty .note {
  text-align: start;
}
/* As the family page's "Browse catalogs" (E42.7). */
.browse {
  padding: var(--fm-space-1) var(--fm-space-3);
  border: 1px solid var(--fm-control-border);
  border-radius: var(--fm-radius-sm);
  background: none;
  color: var(--fm-fg);
  font: inherit;
  font-size: var(--fm-text-sm);
  cursor: pointer;
}
.detail {
  margin: 0;
  color: var(--fm-muted);
  font-size: var(--fm-text-xs);
  font-variant-numeric: tabular-nums;
}
</style>
