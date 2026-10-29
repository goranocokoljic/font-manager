import { readFileSync } from 'node:fs';
import { defineConfig } from 'vitest/config';
import { type SourceManifest, hostManifests } from './manifests';

// UXP loads `index.html` and a classic script; its own modules (`uxp`,
// `indesign`, `photoshop`, `os`, `fs`) come from the host at run time.
// The output is one folder per host (`dist/photoshop/`, `dist/indesign/`):
// Photoshop takes a third-party manifest with one `host` object only.
export default defineConfig({
  build: {
    target: 'es2020',
    lib: { entry: 'src/main.ts', formats: ['cjs'], fileName: () => 'index.js' },
    rollupOptions: { external: ['uxp', 'indesign', 'photoshop', 'os', 'fs'] },
  },
  plugins: [
    {
      name: 'uxp-host-folders',
      generateBundle(_, bundle) {
        const chunk = bundle['index.js'];
        if (!chunk || chunk.type !== 'chunk') throw new Error('index.js was not built');
        const script = chunk.code;
        delete bundle['index.js'];
        const source = JSON.parse(readFileSync('manifest.json', 'utf-8')) as SourceManifest;
        const html = readFileSync('index.html', 'utf-8');
        for (const [folder, manifest] of hostManifests(source)) {
          const json = `${JSON.stringify(manifest, null, 2)}\n`;
          this.emitFile({ type: 'asset', fileName: `${folder}/manifest.json`, source: json });
          this.emitFile({ type: 'asset', fileName: `${folder}/index.html`, source: html });
          this.emitFile({ type: 'asset', fileName: `${folder}/index.js`, source: script });
        }
      },
    },
  ],
  test: { include: ['test/**/*.test.ts'] },
});
