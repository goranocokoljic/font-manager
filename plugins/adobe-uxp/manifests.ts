// One manifest per host from the source `manifest.json`. Photoshop 2025
// refuses a third-party manifest whose `host` is an array ("Expected the
// host attribute to be an object for the 3P Plugin", E30.0 spike), so the
// build emits `dist/<folder>/` per host, each with `host` as that one
// object and its own id, next to the same `index.html` and `index.js`.

/** The UXP host code → the dist folder and the id suffix. */
export const HOST_FOLDERS: Readonly<Record<string, string>> = { PS: 'photoshop', ID: 'indesign' };

/** The plugin ids the folders carry: `org.fontmanager.<folder>`. */
export const ID_PREFIX = 'org.fontmanager.';

export interface HostEntry {
  app: string;
  minVersion: string;
}

export interface SourceManifest {
  $schema?: string;
  id: string;
  host: HostEntry[];
  [key: string]: unknown;
}

export type HostManifest = Omit<SourceManifest, '$schema' | 'host'> & { host: HostEntry };

/** The manifest for one host: `host` as an object, the id for that host, no `$schema`. */
export function hostManifest(source: SourceManifest, host: HostEntry): HostManifest {
  const folder = HOST_FOLDERS[host.app];
  if (!folder) throw new Error(`no dist folder for the UXP host ${host.app}`);
  const rest: Record<string, unknown> = { ...source };
  delete rest.$schema;
  return { ...rest, id: ID_PREFIX + folder, host } as HostManifest;
}

/** Every host's manifest, by its dist folder. */
export function hostManifests(source: SourceManifest): Map<string, HostManifest> {
  return new Map(source.host.map((h) => [HOST_FOLDERS[h.app] ?? h.app, hostManifest(source, h)]));
}
