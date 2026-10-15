import { API_VERSION, type ApiVersion } from './types';

/** The API contract version as `major.minor.patch`. */
export function formatApiVersion(v: ApiVersion = API_VERSION): string {
  return `${v.major}.${v.minor}.${v.patch}`;
}
