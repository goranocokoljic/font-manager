import { describe, expect, it } from 'vitest';
import { uxpUrl } from '../src/endpoint-url';

describe('uxpUrl', () => {
  it('calls localhost where endpoint.json says 127.0.0.1', () => {
    expect(uxpUrl('http://127.0.0.1:27501/v1/version')).toBe('http://localhost:27501/v1/version');
    expect(uxpUrl('http://127.0.0.1:60664')).toBe('http://localhost:60664');
  });

  it('leaves any other address alone', () => {
    expect(uxpUrl('http://localhost:27501/v1/version')).toBe('http://localhost:27501/v1/version');
    expect(uxpUrl('https://127.0.0.1:1/x')).toBe('https://127.0.0.1:1/x');
  });
});
