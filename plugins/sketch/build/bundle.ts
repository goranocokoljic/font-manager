// The `.sketchplugin` bundle and its zip, as a Vite plugin: Vite writes the
// bundle folder, this adds the manifest and the resources and zips the result.
//
//   font-manager.sketchplugin/Contents/Sketch/manifest.json
//   font-manager.sketchplugin/Contents/Sketch/plugin.js
//   font-manager.sketchplugin/Contents/Resources/…
//   font-manager.sketchplugin.zip (next to the folder)

import { writeFileSync } from 'node:fs';
import type { Plugin } from 'vite';
import { zip } from './zip.ts';

/** The bundle folder; Sketch installs a `.sketchplugin` on double-click. */
export const BUNDLE = 'font-manager.sketchplugin';
export const SCRIPT = 'Contents/Sketch/plugin.js';
export const MANIFEST = 'Contents/Sketch/manifest.json';
export const RESOURCES = 'Contents/Resources';

/**
 * The manifest Sketch reads: the source `manifest.json` with the workspace
 * version, without the `$schema` key (that one is for the test and editors).
 */
export function bundleManifest(source: string, version: string): Record<string, unknown> {
  const manifest = JSON.parse(source) as Record<string, unknown>;
  delete manifest.$schema;
  return { ...manifest, version };
}

export interface BundleOptions {
  /** The source `manifest.json` text. */
  manifest: string;
  /** The workspace version (`[workspace.package]` of the root Cargo.toml). */
  version: string;
  /** Files for `Contents/Resources/`, by name. */
  resources: Record<string, string>;
}

/** Adds the manifest and the resources to the build and writes `<outDir>.zip`. */
export function sketchBundle(options: BundleOptions): Plugin {
  return {
    name: 'sketch-bundle',
    generateBundle() {
      const manifest = bundleManifest(options.manifest, options.version);
      const source = `${JSON.stringify(manifest, null, 2)}\n`;
      this.emitFile({ type: 'asset', fileName: MANIFEST, source });
      for (const [name, text] of Object.entries(options.resources)) {
        this.emitFile({ type: 'asset', fileName: `${RESOURCES}/${name}`, source: text });
      }
    },
    writeBundle(output, bundle) {
      if (!output.dir) throw new Error('the Sketch bundle needs build.outDir');
      const encoder = new TextEncoder();
      const files: Record<string, Uint8Array> = {};
      for (const [fileName, item] of Object.entries(bundle)) {
        const content = item.type === 'chunk' ? item.code : item.source;
        files[`${BUNDLE}/${fileName}`] =
          typeof content === 'string' ? encoder.encode(content) : content;
      }
      writeFileSync(`${output.dir}.zip`, zip(files));
    },
  };
}
