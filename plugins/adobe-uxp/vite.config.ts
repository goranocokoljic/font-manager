import { readFileSync } from 'node:fs';
import { defineConfig } from 'vitest/config';

// UXP loads `index.html` and a classic script; its own modules (`uxp`,
// `indesign`, `photoshop`, `os`, `fs`) come from the host at run time.
export default defineConfig({
  build: {
    target: 'es2020',
    lib: { entry: 'src/main.ts', formats: ['cjs'], fileName: () => 'index.js' },
    rollupOptions: { external: ['uxp', 'indesign', 'photoshop', 'os', 'fs'] },
  },
  plugins: [
    {
      name: 'uxp-files',
      generateBundle() {
        // The `$schema` key is for the test and editors, not for the host.
        const manifest = JSON.parse(readFileSync('manifest.json', 'utf-8'));
        delete manifest.$schema;
        const source = `${JSON.stringify(manifest, null, 2)}\n`;
        this.emitFile({ type: 'asset', fileName: 'manifest.json', source });
        const html = readFileSync('index.html', 'utf-8');
        this.emitFile({ type: 'asset', fileName: 'index.html', source: html });
      },
    },
  ],
  test: { include: ['test/**/*.test.ts'] },
});
