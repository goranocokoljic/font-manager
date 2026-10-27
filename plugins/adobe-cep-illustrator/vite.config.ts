import { readFileSync } from 'node:fs';
import { defineConfig } from 'vitest/config';

// CEP loads `index.html` with a classic script and `jsx/host.jsx` into the
// host's ExtendScript engine; Node's modules come from `cep_node` at run time.
export default defineConfig({
  build: {
    target: 'es2020',
    lib: {
      entry: 'src/main.ts',
      formats: ['iife'],
      name: 'FontManager',
      fileName: () => 'index.js',
    },
  },
  plugins: [
    {
      name: 'cep-files',
      generateBundle() {
        for (const fileName of ['CSXS/manifest.xml', 'index.html', 'jsx/host.jsx']) {
          this.emitFile({ type: 'asset', fileName, source: readFileSync(fileName, 'utf-8') });
        }
      },
    },
  ],
  test: { include: ['test/**/*.test.ts'] },
});
