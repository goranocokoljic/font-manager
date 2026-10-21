<script setup lang="ts">
// The license the user declared for the selected face's family (E15.2): type,
// seats, purchase date and an attached receipt. The form only collects the
// values; the API checks them, copies the receipt and stores the declaration
// for every face of the family.
import { computed, ref, watch } from 'vue';
import type { LicenseType, ReceiptChange, ReceiptPlace } from '../api/types';
import { useSelectionStore } from '../stores/selection';
import CopyValue from './CopyValue.vue';

const selection = useSelectionStore();
const license = computed(() => selection.detail?.license);

const TYPES: { value: LicenseType; label: string }[] = [
  { value: 'desktop', label: 'Desktop' },
  { value: 'web', label: 'Web' },
  { value: 'app', label: 'App' },
  { value: 'trial', label: 'Trial' },
  { value: 'personalOnly', label: 'Personal use only' },
  { value: 'unknown', label: 'Unknown' },
];

// The values being edited; they follow the stored declaration when another
// face is shown or a save comes back.
const kind = ref<LicenseType | ''>('');
const seats = ref<number | ''>('');
const date = ref('');
const place = ref<ReceiptPlace>('nextToFont');
watch(
  () =>
    [
      selection.detail?.face.id,
      license.value?.declared,
      license.value?.seats,
      license.value?.purchaseDate,
    ] as const,
  () => {
    kind.value = license.value?.declared ?? '';
    seats.value = license.value?.seats ?? '';
    date.value = license.value?.purchaseDate ?? '';
  },
  { immediate: true },
);

const fields = () => ({
  licenseType: kind.value || null,
  seats: seats.value === '' ? null : seats.value,
  purchaseDate: date.value || null,
});
const declared = computed(
  () =>
    !!license.value &&
    (license.value.declared != null ||
      license.value.seats != null ||
      license.value.purchaseDate != null ||
      license.value.receipt != null),
);

function save(receipt: ReceiptChange = { kind: 'keep' }): void {
  void selection.declareLicense({ ...fields(), receipt });
}
function attach(): void {
  void selection.attachReceipt(fields(), place.value);
}
function clearAll(): void {
  void selection.declareLicense({
    licenseType: null,
    seats: null,
    purchaseDate: null,
    receipt: { kind: 'clear' },
  });
}
</script>

<template>
  <form
    v-if="license"
    class="declare"
    aria-labelledby="fm-license-declared"
    novalidate
    @submit.prevent="save()"
  >
    <h4 id="fm-license-declared">Your license for this family</h4>
    <p v-if="selection.licenseError" role="alert" class="error">
      {{ selection.licenseError.message }}
    </p>
    <dl>
      <dt><label for="fm-license-type">License type</label></dt>
      <dd>
        <select id="fm-license-type" v-model="kind" class="field">
          <option value="">Not declared</option>
          <option v-for="t in TYPES" :key="t.value" :value="t.value">{{ t.label }}</option>
        </select>
      </dd>
      <dt><label for="fm-license-seats">Seats</label></dt>
      <dd>
        <input
          id="fm-license-seats"
          v-model.number="seats"
          class="field"
          type="number"
          min="0"
          step="1"
          inputmode="numeric"
        />
      </dd>
      <dt><label for="fm-license-date">Purchase date</label></dt>
      <dd>
        <input id="fm-license-date" v-model="date" class="field" type="date" />
      </dd>
      <dt id="fm-license-receipt">Receipt</dt>
      <dd>
        <CopyValue v-if="license.receipt" label="receipt" :value="license.receipt" />
        <span v-else data-testid="no-receipt">None</span>
        <fieldset class="place">
          <legend>Keep the copy</legend>
          <label>
            <input v-model="place" type="radio" name="fm-license-place" value="nextToFont" />
            Next to the font (synced with the folder)
          </label>
          <label>
            <input v-model="place" type="radio" name="fm-license-place" value="appData" />
            In the app's data folder
          </label>
        </fieldset>
        <div class="actions">
          <button type="button" class="choice" @click="attach">
            {{ license.receipt ? 'Replace receipt…' : 'Attach receipt…' }}
          </button>
          <button
            v-if="license.receipt"
            type="button"
            class="choice"
            @click="save({ kind: 'clear' })"
          >
            Remove receipt
          </button>
        </div>
      </dd>
    </dl>
    <div class="actions">
      <button type="submit" class="choice primary">Save license</button>
      <button type="button" class="choice" :disabled="!declared" @click="clearAll">
        Clear license
      </button>
    </div>
  </form>
</template>

<style scoped>
.declare {
  display: grid;
  gap: 8px;
  margin-top: 12px;
}
.error {
  margin: 0;
  color: var(--fm-danger);
}
h4 {
  margin: 0;
  font-size: inherit;
  font-weight: 600;
}
dl {
  display: grid;
  grid-template-columns: 120px minmax(0, 1fr);
  gap: 4px 8px;
  margin: 0;
}
dt {
  color: var(--fm-muted);
}
dd {
  margin: 0;
  min-width: 0;
  overflow-wrap: anywhere;
}
.field {
  box-sizing: border-box;
  width: 100%;
  padding: 2px 6px;
  border: 1px solid var(--fm-control-border);
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
}
select.field option {
  background: var(--fm-bg);
}
.place {
  display: grid;
  gap: 2px;
  margin: 6px 0 0;
  padding: 0;
  border: 0;
}
.place legend {
  padding: 0;
  color: var(--fm-muted);
}
.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 6px;
}
.choice {
  padding: 4px 10px;
  border: 1px solid var(--fm-control-border);
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font: inherit;
}
.choice.primary {
  border-color: var(--fm-accent);
  color: var(--fm-accent);
  font-weight: 600;
}
.choice:disabled {
  color: var(--fm-muted);
}
</style>
