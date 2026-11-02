// Helpers for unit tests that drive a `UiSelect` the way a user does: open it, read or
// click an option. The menu lives in <body> (a `UiPopover`), outside the wrapper.
import type { DOMWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';

/** The wrapper of the Select's button, from `find`, `get` or `findAll`. */
export type Trigger = Pick<DOMWrapper<Element>, 'attributes' | 'trigger'>;

const MENU = '.ui-popover [role="listbox"]';

async function openMenu(trigger: Trigger): Promise<HTMLElement> {
  if (trigger.attributes('aria-expanded') !== 'true') {
    await trigger.trigger('click');
    await nextTick();
  }
  const menu = document.body.querySelector<HTMLElement>(MENU);
  if (!menu) throw new Error('the select did not open');
  return menu;
}

/** The options of a `UiSelect` as `[value, label]`, read from its menu; the menu closes again. */
export async function selectOptions(trigger: Trigger): Promise<[string, string][]> {
  const menu = await openMenu(trigger);
  const rows = [...menu.querySelectorAll<HTMLElement>('[role="option"]')].map(
    (o): [string, string] => [o.dataset.value ?? '', o.textContent?.trim() ?? ''],
  );
  await trigger.trigger('keydown', { key: 'Escape' });
  await nextTick();
  return rows;
}

/** Chooses the option with `value` in a `UiSelect`. */
export async function pickOption(trigger: Trigger, value: string): Promise<void> {
  const menu = await openMenu(trigger);
  const quoted = value.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
  const option = menu.querySelector<HTMLElement>(`[role="option"][data-value="${quoted}"]`);
  if (!option) throw new Error(`no option ${value}`);
  option.click();
  await nextTick();
}
