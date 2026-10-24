<script setup lang="ts">
// The license the user declared for the selected face's family (E15.2): type,
// seats, purchase date and an attached receipt. The form only collects the
// values; the API checks them, copies the receipt and stores the declaration
// for every face of the family.
import { computed, ref, watch } from 'vue';
import type { LicenseType, ReceiptChange, ReceiptPlace } from '../api/types';
import { errorMessage, t } from '../i18n';
import { useSelectionStore } from '../stores/selection';
import CopyValue from './CopyValue.vue';

const selection = useSelectionStore();
const license = computed(() => selection.detail?.license);

// Labels come from the catalog (`license.types.*`), keyed by the value.
const TYPES: LicenseType[] = ['desktop', 'web', 'app', 'trial', 'personalOnly', 'unknown'];

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
    <h4 id="fm-license-declared">{{ t('license.title') }}</h4>
    <p v-if="selection.licenseError" role="alert" class="error">
      {{ errorMessage(selection.licenseError) }}
    </p>
    <dl>
      <dt>
        <label for="fm-license-type">{{ t('license.type') }}</label>
      </dt>
      <dd>
        <select id="fm-license-type" v-model="kind" class="field">
          <option value="">{{ t('license.notDeclared') }}</option>
          <option v-for="type in TYPES" :key="type" :value="type">
            {{ t(`license.types.${type}`) }}
          </option>
        </select>
      </dd>
      <dt>
        <label for="fm-license-seats">{{ t('license.seats') }}</label>
      </dt>
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
      <dt>
        <label for="fm-license-date">{{ t('license.purchaseDate') }}</label>
      </dt>
      <dd>
        <input id="fm-license-date" v-model="date" class="field" type="date" />
      </dd>
      <dt id="fm-license-receipt">{{ t('license.receipt') }}</dt>
      <dd>
        <CopyValue
          v-if="license.receipt"
          :label="t('license.receiptLabel')"
          :value="license.receipt"
        />
        <span v-else data-testid="no-receipt">{{ t('license.none') }}</span>
        <fieldset class="place">
          <legend>{{ t('license.keepCopy') }}</legend>
          <label>
            <input v-model="place" type="radio" name="fm-license-place" value="nextToFont" />
            {{ t('license.nextToFont') }}
          </label>
          <label>
            <input v-model="place" type="radio" name="fm-license-place" value="appData" />
            {{ t('license.appData') }}
          </label>
        </fieldset>
        <div class="actions">
          <button type="button" class="choice" @click="attach">
            {{ license.receipt ? t('license.replaceReceipt') : t('license.attachReceipt') }}
          </button>
          <button
            v-if="license.receipt"
            type="button"
            class="choice"
            @click="save({ kind: 'clear' })"
          >
            {{ t('license.removeReceipt') }}
          </button>
        </div>
      </dd>
    </dl>
    <div class="actions">
      <button type="submit" class="choice primary">{{ t('license.save') }}</button>
      <button type="button" class="choice" :disabled="!declared" @click="clearAll">
        {{ t('license.clear') }}
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
