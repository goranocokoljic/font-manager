# plugins

Adobe UXP/CEP and Sketch plugins (Phase 4, epic E30+). Each plugin is a pnpm
workspace package that talks only to the running app (the local API, ADR 0015,
or the IPC endpoint, ADR 0016); no font logic lives here. UXP cannot open a
named pipe or a Unix socket, so the UXP plugin uses the local API; the CEP
extension has Node and uses the pipe or socket first.

- `shared/` (`@fm/plugin-shared`): the framework-free client of the app (the
  local API with `endpoint.json` and the bearer token, ADR 0015, and the pipe
  or socket over Node's `net`, ADR 0016) and the document open/close logic
  every plugin uses, on the generated `apps/desktop/src/api/types.ts`.
- `adobe-uxp/` (`@fm/adobe-uxp`): the UXP plugin for InDesign and Photoshop (E30.1).
- `adobe-cep-illustrator/` (`@fm/adobe-cep-illustrator`): the CEP extension for
  Illustrator (E30.2); its README says why CEP and when to move to UXP.
- `sketch/` (`@fm/sketch`): the Sketch plugin (E31.1), over the local API.
