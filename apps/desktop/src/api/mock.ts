// Sample answers for browser mode (Playwright) and unit tests, shaped like
// `fm_api::mock::MockApi`. No logic: fixed data only.
import type { FmClient } from './client';
import { API_VERSION, type AppInfo } from './types';

export const mockAppInfo: AppInfo = {
  name: 'Font Manager (mock)',
  version: '0.1.0',
  apiVersion: API_VERSION,
  platform: 'linux',
};

export const mockClient: FmClient = {
  getAppInfo: async () => mockAppInfo,
};
