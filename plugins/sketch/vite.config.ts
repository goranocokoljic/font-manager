import { readFileSync } from 'node:fs';
import { defineConfig } from 'vitest/config';

// Sketch runs `Contents/Sketch/plugin.js` as a classic script and calls the
// action handlers the manifest names as globals; its own modules (`sketch/*`,
// `os`, `fs`) come from the host at run time through `require`.
export default defineConfig({
  build: {
    target: 'es2020',
    outDir: 'dist/FontManager.sketchplugin/Contents/Sketch',
    lib: { entry: 'src/main.ts', formats: ['cjs'], fileName: () => 'plugin.js' },
    rollupOptions: {
      external: ['sketch/dom', 'sketch/ui', 'sketch/settings', 'sketch/async', 'os', 'fs'],
    },
  },
  plugins: [
    {
      name: 'sketch-files',
      generateBundle() {
        const source = readFileSync('manifest.json', 'utf-8');
        this.emitFile({ type: 'asset', fileName: 'manifest.json', source });
      },
    },
  ],
  test: { include: ['test/**/*.test.ts'] },
});
