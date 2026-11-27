import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import { h } from 'vue';
import UiSettingGroup from './UiSettingGroup.vue';
import UiSettingRow from './UiSettingRow.vue';
import UiSwitch from './UiSwitch.vue';

describe('UiSettingRow and UiSettingGroup', () => {
  function mountGroup() {
    return mount(UiSettingGroup, {
      props: { heading: 'While the app runs', note: 'These apply at once.' },
      slots: {
        default: () => [
          h(
            UiSettingRow,
            { title: 'Start at login', line: 'Font Fu starts in the tray.' },
            {
              default: ({ control }: { control: Record<string, string | undefined> }) =>
                h(UiSwitch, { checked: false, ...control }),
            },
          ),
          h(
            UiSettingRow,
            { title: 'Logs folder', value: 'C:\\Users\\me\\AppData\\logs' },
            {
              default: ({ control }: { control: Record<string, string | undefined> }) =>
                h('button', { type: 'button', ...control }, 'Copy'),
            },
          ),
        ],
      },
      attachTo: document.body,
    });
  }

  it("makes the row's title the control's label and its line the description", () => {
    const wrapper = mountGroup();
    const sw = wrapper.get('[role="switch"]');
    expect(document.getElementById(sw.attributes('aria-labelledby')!)?.textContent).toBe(
      'Start at login',
    );
    expect(document.getElementById(sw.attributes('aria-describedby')!)?.textContent).toBe(
      'Font Fu starts in the tray.',
    );
    // A row without a line describes nothing; its value is in the mono face.
    const copy = wrapper.get('button:not([role])');
    expect(document.getElementById(copy.attributes('aria-labelledby')!)?.textContent).toBe(
      'Logs folder',
    );
    expect(copy.attributes('aria-describedby')).toBeUndefined();
    expect(wrapper.get('.value').text()).toBe('C:\\Users\\me\\AppData\\logs');
    // Two rows, two different ids.
    expect(sw.attributes('aria-labelledby')).not.toBe(copy.attributes('aria-labelledby'));
    wrapper.unmount();
  });

  it('is a card under an optional small-caps heading and note that name the group', () => {
    const wrapper = mountGroup();
    const group = wrapper.get('section');
    expect(wrapper.get(`#${group.attributes('aria-labelledby')}`).text()).toBe(
      'While the app runs',
    );
    expect(wrapper.get(`#${group.attributes('aria-describedby')}`).text()).toBe(
      'These apply at once.',
    );
    expect(wrapper.findAll('.card .ui-setting-row')).toHaveLength(2);
    wrapper.unmount();

    const bare = mount(UiSettingGroup);
    expect(bare.find('h3').exists()).toBe(false);
    expect(bare.get('section').attributes('aria-labelledby')).toBeUndefined();
    bare.unmount();
  });
  it('says a failed change on the row, as an alert that also describes the control (E57.4)', async () => {
    const wrapper = mount(UiSettingRow, {
      props: { title: 'Start at login', line: 'Font Fu starts in the tray.' },
      slots: {
        default: ({ control }: { control: Record<string, string | undefined> }) =>
          h(UiSwitch, { checked: false, ...control }),
      },
      attachTo: document.body,
    });
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    await wrapper.setProps({ error: 'Start at login could not be changed.' });
    const alert = wrapper.get('[role="alert"]');
    expect(alert.text()).toBe('Start at login could not be changed.');
    const ids = wrapper.get('[role="switch"]').attributes('aria-describedby')!.split(' ');
    expect(ids.map((id) => document.getElementById(id)?.textContent)).toEqual([
      'Font Fu starts in the tray.',
      'Start at login could not be changed.',
    ]);
    await wrapper.setProps({ error: undefined });
    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    wrapper.unmount();
  });

  it('says `empty` where the value goes while there is none; the value slot keeps the mono face (E57.5)', async () => {
    const wrapper = mount(UiSettingRow, {
      props: { title: 'Folder for installed fonts', empty: 'Not chosen yet.' },
    });
    expect(wrapper.find('.value').exists()).toBe(false);
    expect(wrapper.get('.empty').text()).toBe('Not chosen yet.');
    await wrapper.setProps({ value: '/Users/me/Type' });
    expect(wrapper.get('.value').text()).toBe('/Users/me/Type');
    expect(wrapper.find('.empty').exists()).toBe(false);
    wrapper.unmount();

    const slotted = mount(UiSettingRow, {
      props: { title: 'Folder', empty: 'Not chosen yet.' },
      slots: { value: () => h('span', { 'data-testid': 'path' }, '/Users/me/Type') },
    });
    expect(slotted.get('.value [data-testid="path"]').text()).toBe('/Users/me/Type');
    expect(slotted.find('.empty').exists()).toBe(false);
    slotted.unmount();
  });
});
