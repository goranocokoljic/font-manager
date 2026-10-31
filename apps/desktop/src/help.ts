// The pages of the user docs the palette's Help actions open: the same pages, in
// the same order, as the native Help menu (`HELP_PAGES` in src-tauri/src/help.rs).
// scripts/docs/site.test.mjs checks both lists against docs/user.

export interface HelpPage {
  slug: string;
  title: string;
}

export const HELP_PAGES: readonly HelpPage[] = [
  { slug: 'getting-started', title: 'Getting started' },
  { slug: 'sources', title: 'Sources' },
  { slug: 'activation', title: 'Activation' },
  { slug: 'keyboard-shortcuts', title: 'Keyboard shortcuts' },
  { slug: 'shared-libraries', title: 'Shared libraries' },
  { slug: 'mcp', title: 'AI assistants' },
];
