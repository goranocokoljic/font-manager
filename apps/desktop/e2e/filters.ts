import { expect, type Locator, type Page } from '@playwright/test';

/** The filter drawer, opened if it is not: the facet groups (Slant, Format, …) live in it. */
export async function openFilters(page: Page): Promise<Locator> {
  const drawer = page.getByRole('dialog', { name: 'Filter options' });
  if (!(await drawer.isVisible())) {
    await page
      .getByRole('region', { name: 'Filters' })
      .getByRole('button', { name: /^Filters/ })
      .click();
  }
  await expect(drawer).toBeVisible();
  return drawer;
}
