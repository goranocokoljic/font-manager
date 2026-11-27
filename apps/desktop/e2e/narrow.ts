import type { Page } from '@playwright/test';

// E58.6: the small windows and the six views that must stay usable in them; shared by
// narrow.spec.ts (layout) and a11y.spec.ts (the WCAG scan).

/** The smallest window (1000 × 700) at 125 and at 150 per cent. */
export const SMALL_VIEWPORTS = [
  { width: 800, height: 560 },
  { width: 667, height: 467 },
] as const;

/** The six views of the story, each with what shows it is ready. */
export const NARROW_VIEWS = [
  {
    name: 'library',
    route: '/',
    ready: (p: Page) => p.getByRole('tree', { name: 'Font list' }),
    toolbar: true,
  },
  {
    name: 'family page',
    route: '/family/1',
    ready: (p: Page) => p.getByTestId('family-panel'),
    toolbar: true,
  },
  {
    name: 'Studio',
    route: '/studio?face=1',
    ready: (p: Page) => p.getByTestId('studio-toolbar'),
    toolbar: true,
  },
  {
    name: 'Settings',
    route: '/settings',
    ready: (p: Page) => p.getByRole('navigation', { name: 'Settings' }),
    toolbar: false,
  },
  {
    name: 'Duplicates',
    route: '/duplicates',
    ready: (p: Page) => p.getByRole('navigation', { name: 'Duplicates' }),
    toolbar: false,
  },
  {
    name: 'Problems',
    route: '/problems',
    ready: (p: Page) => p.getByRole('navigation', { name: 'Problems' }),
    toolbar: false,
  },
] as const;
