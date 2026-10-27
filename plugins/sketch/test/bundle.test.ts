// The real build, twice: `vite build` with this package's config into two
// temporary folders. It proves the bundle layout, the version and the zip.

import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { validate } from '@fm/plugin-shared/json-schema';
import { build } from 'vite';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { manifestVersion } from '../../../scripts/release/dev-build.mjs';
import { extractZip } from '../../../scripts/reference-library/lib.mjs';
import { BUNDLE, MANIFEST } from '../build/bundle.ts';
import { zip } from '../build/zip.ts';
import schemaFile from '../manifest.schema.json';

const root = fileURLToPath(new URL('..', import.meta.url));
const tmp = mkdtempSync(join(tmpdir(), 'fm-sketch-'));

async function buildInto(name: string): Promise<string> {
  const outDir = join(tmp, name, BUNDLE);
  await build({
    root,
    configFile: join(root, 'vite.config.ts'),
    logLevel: 'silent',
    build: { outDir, emptyOutDir: true },
  });
  return outDir;
}

/** The files under `dir`, as sorted paths with `/`. */
function filesOf(dir: string): string[] {
  return readdirSync(dir, { recursive: true })
    .filter((p) => statSync(join(dir, p)).isFile())
    .map((p) => p.split('\\').join('/'))
    .sort();
}

let first: string;
let second: string;

beforeAll(async () => {
  first = await buildInto('a');
  second = await buildInto('b');
});
afterAll(() => rmSync(tmp, { recursive: true, force: true }));

describe('pnpm --filter @fm/sketch-plugin build', () => {
  it('writes the .sketchplugin bundle: manifest, one script, resources', () => {
    expect(filesOf(first)).toEqual([
      'Contents/Resources/LICENSE-APACHE',
      'Contents/Resources/LICENSE-MIT',
      'Contents/Sketch/manifest.json',
      'Contents/Sketch/plugin.js',
    ]);
  });

  it('bundles one classic script that loads only the host modules', () => {
    const script = readFileSync(join(first, 'Contents/Sketch/plugin.js'), 'utf-8');
    expect(script).not.toMatch(/^\s*(import|export)\s/m);
    const required = [...script.matchAll(/require\(["']([^"']+)["']\)/g)].map((m) => m[1]);
    expect(new Set(required)).toEqual(
      new Set(['os', 'fs', 'sketch/dom', 'sketch/ui', 'sketch/settings', 'sketch/async']),
    );
  });

  it('writes a manifest with the workspace version that fits the schema', () => {
    const manifest = JSON.parse(readFileSync(join(first, MANIFEST), 'utf-8'));
    const cargo = readFileSync(join(root, '..', '..', 'Cargo.toml'), 'utf-8');
    expect(manifest.version).toBe(manifestVersion(cargo));
    expect(validate(schemaFile as Record<string, unknown>, manifest)).toEqual([]);
    expect(manifest).not.toHaveProperty('appcast');
  });

  it('zips the bundle, and the zip unpacks to the same files', () => {
    const out = join(tmp, 'unzipped');
    const written = extractZip(readFileSync(`${first}.zip`), out);
    expect([...written].sort()).toEqual(filesOf(first).map((p) => `${BUNDLE}/${p}`));
    for (const p of filesOf(first)) {
      expect(readFileSync(join(out, BUNDLE, p))).toEqual(readFileSync(join(first, p)));
    }
  });

  it('gives a byte-identical zip on a second build', () => {
    expect(readFileSync(`${second}.zip`)).toEqual(readFileSync(`${first}.zip`));
  });
});

describe('CI', () => {
  it('builds the bundle on every OS of the matrix', () => {
    const ci = readFileSync(join(root, '..', '..', '.github/workflows/ci.yml'), 'utf-8');
    for (const os of ['windows-latest', 'macos-latest', 'ubuntu-latest']) {
      expect(ci).toContain(`os: ${os}`);
    }
    expect(ci).toContain('pnpm --filter @fm/sketch-plugin build');
    expect(ci).toContain('test -f plugins/sketch/dist/font-manager.sketchplugin.zip');
  });
});

describe('docs/sketch-plugin.md', () => {
  it('says how to install the plugin and what it does', () => {
    const page = readFileSync(join(root, '..', '..', 'docs/sketch-plugin.md'), 'utf-8');
    expect(page).toMatch(/^## Install the plugin$/m);
    expect(page).toMatch(/^## What it does$/m);
    expect(page).toContain('Double-click `font-manager.sketchplugin`');
    expect(page).toContain('font-manager_<version>_sketch-plugin.zip');
  });
});

describe('zip', () => {
  const text = (s: string) => new TextEncoder().encode(s);

  it('does not depend on the order of the files', () => {
    const a = zip({ 'x/b.txt': text('b'), 'x/a.txt': text('a') });
    const b = zip({ 'x/a.txt': text('a'), 'x/b.txt': text('b') });
    expect(b).toEqual(a);
  });

  it('refuses a path that leaves the archive', () => {
    expect(() => zip({ '../evil.txt': text('x') })).toThrow(/not a relative file path/);
    expect(() => zip({ '/abs.txt': text('x') })).toThrow(/not a relative file path/);
  });
});
