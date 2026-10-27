import { describe, expect, it } from 'vitest';
import manifestFile from '../manifest.json';
import schemaFile from '../manifest.schema.json';
import { validate } from './json-schema';

const schema = schemaFile as Record<string, unknown>;
const manifest = manifestFile as Record<string, unknown>;

function changed(edit: (m: Record<string, unknown>) => void): Record<string, unknown> {
  const copy = structuredClone(manifest);
  edit(copy);
  return copy;
}

describe('manifest.json', () => {
  it('fits manifest.schema.json', () => {
    expect(validate(schema, manifest)).toEqual([]);
  });

  it('is manifest v5 for InDesign and Photoshop with minimum versions', () => {
    expect(manifest.manifestVersion).toBe(5);
    const hosts = manifest.host as { app: string; minVersion: string }[];
    expect(hosts.map((h) => h.app).sort()).toEqual(['ID', 'PS']);
    for (const h of hosts) expect(h.minVersion).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it('lets the plugin talk only to 127.0.0.1', () => {
    const permissions = manifest.requiredPermissions as { network: { domains: string[] } };
    expect(permissions.network.domains).toEqual(['http://127.0.0.1']);
  });
});

describe('manifest.schema.json', () => {
  it.each<[string, (m: Record<string, unknown>) => void]>([
    ['an older manifest version', (m) => (m.manifestVersion = 4)],
    ['a missing host', (m) => (m.host = [{ app: 'ID', minVersion: '18.5.0' }])],
    [
      'a host twice',
      (m) =>
        (m.host = [
          { app: 'ID', minVersion: '18.5.0' },
          { app: 'ID', minVersion: '19.0.0' },
        ]),
    ],
    [
      'a host without a minimum version',
      (m) => (m.host = [{ app: 'ID' }, { app: 'PS', minVersion: '24.1.0' }]),
    ],
    [
      'another network domain',
      (m) =>
        (m.requiredPermissions = {
          network: { domains: ['http://127.0.0.1', 'https://example.com'] },
        }),
    ],
    [
      'localhost instead of 127.0.0.1',
      (m) => (m.requiredPermissions = { network: { domains: ['http://localhost'] } }),
    ],
    ['no network permission', (m) => (m.requiredPermissions = {})],
    [
      'another permission',
      (m) =>
        (m.requiredPermissions = {
          network: { domains: ['http://127.0.0.1'] },
          clipboard: 'readAndWrite',
        }),
    ],
  ])('refuses %s', (_, edit) => {
    expect(validate(schema, changed(edit))).not.toEqual([]);
  });
});
