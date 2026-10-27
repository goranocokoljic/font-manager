// The few Node members the IPC tests use. The workspace has no `@types/node`
// (the spec's library table does not list it), so they are declared here.

declare module 'node:net' {
  interface Socket {
    setEncoding(encoding: 'utf8'): this;
    on(event: 'connect' | 'close' | 'end', listener: () => void): this;
    on(event: 'data', listener: (chunk: string) => void): this;
    on(event: 'error', listener: (error: Error) => void): this;
    write(data: string): boolean;
    end(): this;
    destroy(): this;
  }
  interface Server {
    listen(path: string, listening: () => void): this;
    close(closed?: () => void): this;
  }
  export function createServer(connection: (socket: Socket) => void): Server;
  export function createConnection(path: string): Socket;
}

declare module 'node:os' {
  export function tmpdir(): string;
}

declare const process: { platform: string; pid: number };
