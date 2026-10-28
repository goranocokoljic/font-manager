import { validate } from '@fm/plugin-shared/json-schema';
import { describe, expect, it } from 'vitest';
import { bundleManifest } from '../build/bundle.ts';
import source from '../manifest.json?raw';
import schemaFile from '../manifest.schema.json';
import main from '../src/main.ts?raw';
import type { PluginHandlers } from '../src/plugin';

const schema = schemaFile as Record<string, unknown>;
const manifest = bundleManifest(source, '1.2.3');
interface Command {
  script: string;
  identifier: string;
  handlers?: { actions: object };
  handler?: string;
}
const [command, fontSwitch] = manifest.commands as Command[];

describe('the bundled manifest', () => {
  it('fits manifest.schema.json', () => {
    expect(validate(schema, manifest)).toEqual([]);
  });

  it('runs plugin.js on OpenDocument and CloseDocument', () => {
    expect(manifest.commands).toHaveLength(2);
    expect(command.script).toBe('plugin.js');
    expect(command.handlers?.actions).toEqual({
      OpenDocument: 'onOpenDocument',
      CloseDocument: 'onCloseDocument',
    });
  });

  it('has the Font Switch command "Apply Copied Font" in the plugin menu', () => {
    expect(fontSwitch).toEqual({
      name: 'Apply Copied Font',
      identifier: 'org.fontmanager.sketch.fontswitch',
      script: 'plugin.js',
      handler: 'onApplyCopiedFont',
    });
    expect(manifest.menu).toEqual({ title: 'Font Manager', items: [fontSwitch.identifier] });
  });

  it('names the handlers main.ts makes global', () => {
    const names: (keyof PluginHandlers)[] = [
      'onOpenDocument',
      'onCloseDocument',
      'onApplyCopiedFont',
    ];
    const named = [...Object.values(command.handlers?.actions ?? {}), fontSwitch.handler];
    expect(named.sort()).toEqual([...names].sort());
    expect(main).toContain('Object.assign(globalThis, handlers)');
  });

  it('takes the version it is given and drops $schema', () => {
    expect(manifest.identifier).toBe('org.fontmanager.sketch');
    expect(manifest.version).toBe('1.2.3');
    expect(manifest).not.toHaveProperty('$schema');
    expect(JSON.parse(source)).not.toHaveProperty('version');
  });

  it('has no update feed, so Sketch never calls the network for it', () => {
    expect(manifest).not.toHaveProperty('appcast');
    expect(source).not.toMatch(/appcast|https?:/i);
  });
});

describe('manifest.schema.json', () => {
  const changed = (edit: (m: Record<string, unknown>) => void) => {
    const m = structuredClone(manifest);
    edit(m);
    return m;
  };

  it.each<[string, (m: Record<string, unknown>) => void]>([
    ['an appcast (an update feed)', (m) => (m.appcast = 'https://example.com/appcast.xml')],
    ['another identifier', (m) => (m.identifier = 'com.example.sketch')],
    ['no version', (m) => delete m.version],
    ['a version that is not x.y.z', (m) => (m.version = 'v1.2')],
    ['no compatibleVersion', (m) => delete m.compatibleVersion],
    [
      'no CloseDocument handler',
      (m) =>
        (m.commands = [
          { ...command, handlers: { actions: { OpenDocument: 'onOpenDocument' } } },
          fontSwitch,
        ]),
    ],
    ['another script', (m) => (m.commands = [{ ...command, script: 'other.js' }, fontSwitch])],
    ['a third command', (m) => (m.commands = [command, fontSwitch, command])],
    ['no Font Switch command', (m) => (m.commands = [command, command])],
    [
      'another Font Switch handler',
      (m) => (m.commands = [command, { ...fontSwitch, handler: 'x' }]),
    ],
    ['no menu', (m) => delete m.menu],
  ])('refuses %s', (_, edit) => {
    expect(validate(schema, changed(edit))).not.toEqual([]);
  });
});
