// The few Node members the build and its tests use. The workspace has no
// `@types/node` (the spec's library table does not list it), so they are
// declared here.

declare module 'node:fs' {
  export function readFileSync(path: string | URL): Uint8Array;
  export function readFileSync(path: string | URL, encoding: 'utf-8' | 'utf8'): string;
  export function writeFileSync(path: string, data: Uint8Array | string): void;
  export function mkdtempSync(prefix: string): string;
  export function rmSync(path: string, options: { recursive: true; force: true }): void;
  export function readdirSync(path: string, options: { recursive: true }): string[];
  export function statSync(path: string): { isFile(): boolean };
}

declare module 'node:zlib' {
  export function crc32(data: Uint8Array): number;
  export function deflateRawSync(data: Uint8Array, options: { level: number }): Uint8Array;
}

declare module 'node:os' {
  export function tmpdir(): string;
}

declare module 'node:path' {
  export function join(...parts: string[]): string;
}

declare module 'node:url' {
  export function fileURLToPath(url: URL): string;
}
