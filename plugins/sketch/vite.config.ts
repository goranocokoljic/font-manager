import { readFileSync } from 'node:fs';
import { defineConfig } from 'vitest/config';
import { manifestVersion } from '../../scripts/release/dev-build.mjs';
import { BUNDLE, SCRIPT, sketchBundle } from './build/bundle.ts';

const read = (path: string) => readFileSync(new URL(path, import.meta.url), 'utf-8');

// Sketch runs `Contents/Sketch/plugin.js` as a classic script and calls the
// action handlers the manifest names as globals; its own modules (`sketch/*`,
// `os`, `fs`) come from the host at run time through `require`.
export default defineConfig({
  build: {
    target: 'es2020',
    outDir: `dist/${BUNDLE}`,
    lib: { entry: 'src/main.ts', formats: ['cjs'], fileName: () => SCRIPT },
    rollupOptions: {
      external: ['sketch/dom', 'sketch/ui', 'sketch/settings', 'sketch/async', 'os', 'fs'],
    },
  },
  plugins: [
    sketchBundle({
      manifest: read('manifest.json'),
      version: manifestVersion(read('../../Cargo.toml')),
      resources: {
        'LICENSE-MIT': read('../../LICENSE-MIT'),
        'LICENSE-APACHE': read('../../LICENSE-APACHE'),
      },
    }),
  ],
  test: { include: ['test/**/*.test.ts'], testTimeout: 60_000 },
});
