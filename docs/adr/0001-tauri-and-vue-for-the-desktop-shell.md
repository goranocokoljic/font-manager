# 0001 — Tauri 2 + Vue 3 for the desktop shell

Status: Accepted (2026-10-14)

## Context

The app ships on Windows, macOS and Linux with one feature set. It must look polished,
be accessible (WCAG 2.2 AA) and be localizable from the start. Previews must use the
operating system's own text engine so a font looks as it will in other apps. Targets:
cold start under 1 s, 60 fps scrolling on 10,000 rows, idle memory under 350 MB for a
webview UI (200 MB native).

## Decision

The desktop shell is Tauri 2 with a Vue 3 + TypeScript frontend (Composition API,
`<script setup lang="ts">`, Pinia). The font list renders previews with the webview's
native font rendering: `@font-face` points at the local file through the asset
protocol. The list is virtualized. A native Rust shell (iced or GPUI) stays an option
behind the same `fm-core` and `fm-api`.

## Consequences

- The fastest path to a polished, accessible, localized UI; Linux comes with WebKitGTK.
- Higher memory than a native shell, and the virtualized list must be tuned. If the
  webview cannot hold 60 fps on 10,000 rows, or misses the memory target, E35 revisits
  the shell.
- The frontend holds no business logic: it calls the generated `fm-api` client and
  subscribes to typed events. That rule keeps a later native shell possible.
- Tauri 2's Linux backend pulls GTK3 bindings with one unmaintained macro crate
  (RUSTSEC-2024-0370), ignored in `deny.toml` with a reason.
