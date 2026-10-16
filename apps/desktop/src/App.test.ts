import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import App from './App.vue';
import { formatApiVersion } from './api/version';

describe('app shell', () => {
  it('shows the API version generated from fm-api', () => {
    expect(mount(App).text()).toContain(`API ${formatApiVersion()}`);
  });

  it('formats a version as major.minor.patch', () => {
    expect(formatApiVersion({ major: 1, minor: 12, patch: 3 })).toBe('1.12.3');
  });
});
