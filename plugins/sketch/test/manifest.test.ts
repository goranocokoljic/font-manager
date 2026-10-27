import { describe, expect, it } from 'vitest';
import manifest from '../manifest.json';
import main from '../src/main.ts?raw';
import type { SketchHandlers } from '../src/sketch';

describe('manifest.json', () => {
  const command = manifest.commands[0];

  it('runs plugin.js on OpenDocument and CloseDocument', () => {
    expect(manifest.commands).toHaveLength(1);
    expect(command.script).toBe('plugin.js');
    expect(command.handlers.actions).toEqual({
      OpenDocument: 'onOpenDocument',
      CloseDocument: 'onCloseDocument',
    });
  });

  it('names the handlers main.ts makes global', () => {
    const names: (keyof SketchHandlers)[] = ['onOpenDocument', 'onCloseDocument'];
    expect(Object.values(command.handlers.actions).sort()).toEqual([...names].sort());
    expect(main).toContain('Object.assign(globalThis, handlers)');
  });

  it('has an identifier and a version', () => {
    expect(manifest.identifier).toMatch(/^[a-z]+(\.[a-z]+)+$/);
    expect(manifest.version).toMatch(/^\d+\.\d+\.\d+$/);
  });
});
