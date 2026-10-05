import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
const stories = [
  'primary',
  'secondary',
  'quiet',
  'pending',
  'disabled',
  'fields',
  'feedback',
  'loading-state',
  'surfaces',
  'appearance',
];
for (const theme of ['light', 'dark']) {
  test(`shared components: ${theme}, keyboard, states, and accessibility`, async ({ page }) => {
    await page.addInitScript((value) => localStorage.setItem('appshell-theme', value), theme);
    await page.setViewportSize({ width: theme === 'dark' ? 390 : 1280, height: 900 });
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    for (const story of stories) {
      // Playwright owns the Axe scan on these pages. Leave the addon in manual
      // mode here so its render-time scan cannot race with analyze() below.
      await page.goto(
        `/iframe.html?id=design-system-controls--${story}&viewMode=story&globals=theme:${theme};a11y.manual:!true`,
      );
      await expect(page.locator('#storybook-root > div')).toBeVisible();
      await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
      await expect(page.locator('.sb-errordisplay')).not.toBeVisible();
      if (story === 'pending') {
        await expect(page.getByRole('button')).toBeDisabled();
        await expect(page.getByRole('button')).toHaveAttribute('aria-busy', 'true');
      }
      if (story === 'fields') {
        await expect(page.getByLabel('Email', { exact: true })).toHaveValue('person@example.com');
        await expect(page.getByLabel('Role')).toHaveValue('admin');
        await page.getByLabel('Email', { exact: true }).focus();
        await page.keyboard.press('Tab');
        await expect(page.getByLabel('Invalid email')).toBeFocused();
      }
      const accessibility = await new AxeBuilder({ page })
        .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
        .analyze();
      expect(accessibility.violations, `${story} (${theme})`).toEqual([]);
      await expect(page.locator('body')).toHaveJSProperty(
        'scrollWidth',
        await page.locator('body').evaluate((node) => node.clientWidth),
      );
      if (story === 'surfaces')
        await page.screenshot({ path: `test-results/components-${theme}.png`, fullPage: true });
    }
    expect(errors).toEqual([]);
  });
}
test('system appearance follows OS preference', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto(
    '/iframe.html?id=design-system-controls--primary&viewMode=story&globals=theme:system',
  );
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
});

test('localized controls expose translated accessible labels', async ({ page }) => {
  await page.goto(
    '/iframe.html?id=design-system-controls--localized-controls&viewMode=story&globals=a11y.manual:!true',
  );
  await expect(page.getByRole('group', { name: 'Apariencia' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Tema oscuro' })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.getByRole('button', { name: 'Reintentar' })).toBeVisible();
  const accessibility = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
    .analyze();
  expect(accessibility.violations).toEqual([]);
});
