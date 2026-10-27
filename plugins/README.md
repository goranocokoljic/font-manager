# plugins

Adobe UXP/CEP and Sketch plugins (Phase 4, epic E30+). Each plugin is a pnpm
workspace package that talks only to the running app (the local API, ADR 0015,
or the IPC endpoint, ADR 0016); no font logic lives here. UXP cannot open a
named pipe or a Unix socket, so the Adobe plugin uses the local API.

- `shared/` (`@fm/plugin-shared`): the framework-free client of the local API
  (`endpoint.json`, bearer token, ADR 0015) and the document open/close logic
  every plugin uses, on the generated `apps/desktop/src/api/types.ts`.
- `adobe-uxp/` (`@fm/adobe-uxp`): the UXP plugin for InDesign and Photoshop (E30.1).
