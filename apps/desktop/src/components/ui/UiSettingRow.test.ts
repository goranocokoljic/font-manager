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
});
