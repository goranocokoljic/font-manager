import { describe, expect, it } from 'vitest';
import manifestFile from '../manifest.json';
import schemaFile from '../manifest.schema.json';
import { HOST_FOLDERS, type SourceManifest, hostManifest, hostManifests } from '../manifests';
import { validate } from '@fm/plugin-shared/json-schema';

const schema = schemaFile as Record<string, unknown>;
const manifest = manifestFile as Record<string, unknown>;
const source = manifestFile as unknown as SourceManifest;

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

  it('lets the plugin talk to localhost only, on any port', () => {
    // UXP drops a list entry whose host has no top-level domain (127.0.0.1)
    // and takes `localhost` as one; an entry without a port matches any port.
    const permissions = manifest.requiredPermissions as { network: { domains: string[] } };
    expect(permissions.network.domains).toEqual(['http://localhost']);
  });
});

describe('hostManifests', () => {
  it('gives one manifest per host with host as an object and its own id', () => {
    const all = hostManifests(source);
    expect([...all.keys()].sort()).toEqual(['indesign', 'photoshop']);
    const ps = all.get('photoshop')!;
    expect(ps.host).toEqual({ app: 'PS', minVersion: '24.1.0' });
    expect(ps.id).toBe('org.fontmanager.photoshop');
    expect(ps).not.toHaveProperty('$schema');
    const id = all.get('indesign')!;
    expect(id.host).toEqual({ app: 'ID', minVersion: '18.5.0' });
    expect(id.id).toBe('org.fontmanager.indesign');
  });

  it('keeps everything else of the source manifest', () => {
    const ps = hostManifest(source, { app: 'PS', minVersion: '24.1.0' });
    for (const [key, value] of Object.entries(source)) {
      if (key === '$schema' || key === 'host' || key === 'id') continue;
      expect(ps[key]).toEqual(value);
    }
  });

  it('refuses a host without a dist folder', () => {
    expect(Object.keys(HOST_FOLDERS).sort()).toEqual(['ID', 'PS']);
    expect(() => hostManifest(source, { app: 'ILST', minVersion: '29.0.0' })).toThrow(/ILST/);
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
          network: { domains: ['http://localhost', 'https://example.com'] },
        }),
    ],
    [
      'localhost with a port',
      (m) => (m.requiredPermissions = { network: { domains: ['http://localhost:27501'] } }),
    ],
    [
      '127.0.0.1, which UXP drops from the list',
      (m) => (m.requiredPermissions = { network: { domains: ['http://127.0.0.1'] } }),
    ],
    ['all domains', (m) => (m.requiredPermissions = { network: { domains: 'all' } })],
    ['no network permission', (m) => (m.requiredPermissions = {})],
    [
      'another permission',
      (m) =>
        (m.requiredPermissions = {
          network: { domains: ['http://localhost'] },
          clipboard: 'readAndWrite',
        }),
    ],
  ])('refuses %s', (_, edit) => {
    expect(validate(schema, changed(edit))).not.toEqual([]);
  });
});
