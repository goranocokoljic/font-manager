import { expect, type Locator, type Page } from '@playwright/test';

// The MockApi's faces and the families they belong to.
const FAMILY_OF: Record<number, number> = { 1: 1, 6: 3, 7: 4, 8: 8, 9: 9 };

export type Section = 'Styles' | 'Text' | 'Characters' | 'Features' | 'Pairings' | 'Info';

/** The hash of the family page that shows `face`. */
export const faceHash = (face: number) => `#/family/${FAMILY_OF[face]}?face=${face}`;

/** One section's panel. By its section: while the switch crossfades, the old panel is there too. */
export const sectionPanel = (page: Page, section: Section): Locator =>
  page.locator(`[data-testid="family-panel"][data-section="${section.toLowerCase()}"]`);

export const axesRow = (page: Page) => page.getByTestId('family-axes');
export const hero = (page: Page) => page.getByTestId('family-specimen');

export async function showSection(page: Page, section: Section): Promise<Locator> {
  await page
    .getByRole('group', { name: 'Section' })
    .getByRole('button', { name: section, exact: true })
    .click();
  const panel = sectionPanel(page, section);
  await expect(panel).toBeVisible();
  return panel;
}

/** Loads the family page of `face` on `section`; `search` is the mock's options (`?mockCjk=1`). */
export async function openSection(
  page: Page,
  face: number,
  section: Section,
  search = '',
): Promise<Locator> {
  await page.goto(`/${search}${faceHash(face)}`);
  return showSection(page, section);
}

/** Opens the Developer expander at the end of the rail. */
export async function openDeveloper(page: Page): Promise<Locator> {
  await page.getByRole('button', { name: 'Technical details', exact: true }).click();
  const part = page.locator('#fm-family-developer');
  await expect(part.getByTestId('css-snippet')).toBeVisible();
  return part;
}
