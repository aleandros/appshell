import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
test('public shell, themes, responsive layout and accessible controls', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: /Good things/ })).toBeVisible();
  await page.getByRole('button', { name: 'dark theme', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  const dark = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
  expect(dark.violations).toEqual([]);
  await page.screenshot({
    path: `test-results/landing-dark-${test.info().project.name}.png`,
    fullPage: true,
  });
  await page.getByRole('button', { name: 'system theme', exact: true }).click();
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.getByRole('button', { name: 'light theme', exact: true }).click();
  await page.reload();
  await expect(page.getByRole('heading', { name: /Good things/ })).toBeVisible();
  await expect(page.locator('body')).toHaveCSS('color', 'rgb(37, 51, 45)');
  await page.screenshot({
    path: `test-results/landing-${test.info().project.name}.png`,
    fullPage: true,
  });
  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
    .analyze();
  expect(results.violations).toEqual([]);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
  await page.screenshot({
    path: `test-results/landing-${test.info().project.name}.png`,
    fullPage: true,
  });
  await page.goto('/app');
  await expect(page).toHaveURL(/\/login$/);
});
test('signup, private workspace and logout against the real API', async ({ page }) => {
  const address = `browser-${crypto.randomUUID()}@example.com`;
  await page.goto('/signup');
  await page.getByLabel('Your name').fill('Alex Morgan');
  await page.getByLabel('Workspace name').fill('Acme Studio');
  await page.getByLabel('Email address').fill(address);
  await page.getByLabel('Password', { exact: true }).fill('a browser test password');
  await page.getByRole('button', { name: 'Create your workspace' }).click();
  await expect(page.getByRole('heading', { name: 'Hello, Alex.' })).toBeVisible();
  await expect(page.getByText('One small step: verify your email.')).toBeVisible();
  expect(await page.evaluate(() => document.cookie.includes('appshell_session'))).toBe(false);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(
    true,
  );
  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
    .analyze();
  expect(results.violations).toEqual([]);
  await page.screenshot({
    path: `test-results/workspace-${test.info().project.name}.png`,
    fullPage: true,
  });
  await page.goto('/app/settings');
  await page.getByRole('button', { name: 'dark theme', exact: true }).last().click();
  await expect(
    page.getByRole('button', { name: 'dark theme', exact: true }).first(),
  ).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('status').filter({ hasText: 'One small step' })).toHaveCSS(
    'color',
    'rgb(166, 215, 175)',
  );
  await expect(page.getByText('One small step: verify your email.')).toHaveCSS(
    'color',
    'rgb(166, 215, 175)',
  );
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Your settings.' })).toBeVisible();
  await expect(page.locator('body')).toHaveCSS('color', 'rgb(229, 237, 230)');
  await expect(page.getByRole('heading', { name: 'Your settings.' })).toHaveCSS(
    'color',
    'rgb(229, 237, 230)',
  );
  await page.screenshot({
    path: `test-results/settings-dark-${test.info().project.name}.png`,
    fullPage: true,
  });
  const dark = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa']).analyze();
  expect(dark.violations).toEqual([]);
  if (test.info().project.name === 'mobile')
    await page.getByRole('button', { name: 'Open navigation' }).click();
  await page.getByRole('button', { name: 'Sign out', exact: true }).click();
  await expect(page).toHaveURL(/\/login$/);
  await page.goto('/app');
  await expect(page).toHaveURL(/\/login$/);
});
test('friendly 404 and invalid action links', async ({ page }) => {
  await page.goto('/not-a-page');
  await expect(page.getByRole('heading', { name: 'This space isn’t here.' })).toBeVisible();
  await page.goto('/verify-email');
  await expect(page.getByRole('alert')).toContainText('missing its token');
});
