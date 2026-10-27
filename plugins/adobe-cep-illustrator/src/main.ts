// The extension's entry in Illustrator. Wiring only: the client and the
// document logic are `@fm/plugin-shared`, the host side is `illustrator.ts`.
// The app is reached over its pipe or socket (ADR 0016) when it listens
// there, and over the local API (ADR 0015) otherwise.

import {
  DocumentWatcher,
  FmClient,
  type IpcSocket,
  type Transport,
  endpointFilePath,
  firstAvailable,
  httpTransport,
  ipcPath,
  ipcTransport,
} from '@fm/plugin-shared';
import { type CepNative, cepEvents, cepExtendScript } from './cep';
import { watchIllustrator } from './illustrator';
import { extendScriptUi } from './ui';

type NodeRequire = (id: string) => unknown;
interface CepWindow {
  __adobe_cep__: CepNative;
  /** Node's `require` with `--enable-nodejs`; also global with `--mixed-context`. */
  cep_node?: { require: NodeRequire };
  require?: NodeRequire;
}
interface NodeOs {
  platform(): string;
  homedir(): string;
}
interface NodeFs {
  promises: { readFile(path: string, encoding: 'utf8'): Promise<string> };
}
interface NodeNet {
  createConnection(path: string): IpcSocket;
}
interface NodeProcess {
  env: Record<string, string | undefined>;
}

const cep = window as unknown as CepWindow;
const nodeRequire = cep.cep_node?.require ?? cep.require;

function transports(): Transport[] {
  // Without Node (a manifest edited to turn it off) the app cannot be found:
  // the extension says "Font Manager is not running".
  if (!nodeRequire) return [];
  const os = nodeRequire('os') as NodeOs;
  const fs = nodeRequire('fs') as NodeFs;
  const net = nodeRequire('net') as NodeNet;
  const { env } = nodeRequire('process') as NodeProcess;
  const [platform, home] = [os.platform(), os.homedir()];
  const endpointFile = endpointFilePath(platform, home, env);
  return [
    ipcTransport({ path: ipcPath(platform, home, env), connect: (p) => net.createConnection(p) }),
    httpTransport({
      readEndpointFile: () => fs.promises.readFile(endpointFile, 'utf8'),
      fetch: (url, init) => fetch(url, init),
    }),
  ];
}

const script = cepExtendScript(cep.__adobe_cep__);
const client = new FmClient(firstAvailable(...transports()));
const ui = extendScriptUi(script);
watchIllustrator(script, cepEvents(cep.__adobe_cep__), new DocumentWatcher(client, ui));
