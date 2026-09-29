import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

// The Rust/Postgres integration suite exercises real admin credentials, isolation,
// revocation, outbox delivery, and audit persistence. These browser fixtures cover
// console interaction and accessibility without provisioning an installation admin.
test('admin console manages users and renders history accessibly', async ({ page }) => {
  let authenticated = false;
  const admin = {
    id: 'a48f17ea-14d7-4f44-a247-bf5202c3222f',
    name: 'Test Operator',
    email: 'operator@example.test',
    status: 'active',
    deleted_at: null,
  };
  const user = {
    id: '1421d30e-20fa-46a3-bd16-14a2e40c3637',
    name: 'Test User',
    email: 'user@example.test',
    status: 'active',
    deleted_at: null,
    email_verified_at: null,
  };
  await page.route('**/api/admin/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith('/login')) {
      authenticated = true;
      await route.fulfill({ json: admin });
      return;
    }
    if (path.endsWith('/logout')) {
      authenticated = false;
      await route.fulfill({ json: { message: 'Signed out.' } });
      return;
    }
    if (!authenticated) {
      await route.fulfill({
        status: 401,
        json: { code: 'unauthorized', message: 'Sign in to continue.' },
      });
      return;
    }
    if (path.endsWith('/session')) await route.fulfill({ json: admin });
    else if (path.endsWith('/history'))
      await route.fulfill({
        json: [
          {
            id: '696879c8-8515-4bac-b24b-bc0159e6b341',
            operation: 'UPDATE',
            changes: { email: { from: 'old@example.test', to: user.email } },
            actor_id: admin.id,
            actor_kind: 'admin',
            changed_at: '2026-09-28T12:00:00Z',
          },
        ],
      });
    else if (path.endsWith('/reset-password'))
      await route.fulfill({
        json: { message: 'Sessions revoked and a password reset email queued.' },
      });
    else if (path.endsWith('/users')) await route.fulfill({ json: [user] });
    else if (path.endsWith('/accounts')) await route.fulfill({ json: [admin] });
    else {
      const body = route.request().postDataJSON() as {
        email: string;
        name: string;
        status: string;
      };
      user.email = body.email;
      user.name = body.name;
      user.status = body.status;
      await route.fulfill({ json: { message: 'User updated.' } });
    }
  });
  await page.goto('/admin');
  await expect(page).toHaveURL(/\/admin\/login$/);
  await page.getByLabel('Admin email').fill(admin.email);
  await page.getByLabel('Admin password').fill('test operator password');
  await page.getByLabel('Admin password').press('Enter');
  await expect(page.getByRole('heading', { name: 'Account operations' })).toBeVisible();
  await page.getByRole('button', { name: `Manage ${user.email}` }).click();
  await expect(page.getByRole('heading', { name: 'Change history' })).toBeVisible();
  await page.getByLabel('Email', { exact: true }).fill('corrected@example.test');
  await page.getByRole('button', { name: 'Save user changes' }).click();
  await expect(page.getByRole('button', { name: 'Manage corrected@example.test' })).toBeVisible();
  await page.getByRole('button', { name: 'Send password reset' }).click();
  await expect(
    page.getByRole('status').filter({ hasText: 'password reset email queued' }),
  ).toBeVisible();
  for (const theme of ['light', 'dark', 'system']) {
    await page.getByRole('button', { name: `${theme} theme`, exact: true }).click();
    const results = await new AxeBuilder({ page })
      .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
      .analyze();
    expect(results.violations).toEqual([]);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth),
    ).toBe(true);
  }
  await page.screenshot({
    path: `test-results/admin-${test.info().project.name}.png`,
    fullPage: true,
  });
  await page.getByRole('button', { name: 'Administrators', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Add administrator' })).toBeVisible();
  await page.getByRole('button', { name: `Manage ${admin.email}` }).click();
  await expect(page.getByRole('heading', { name: 'Edit administrator' })).toBeVisible();
  await page.getByRole('button', { name: 'Sign out', exact: true }).click();
  await expect(page).toHaveURL(/\/admin\/login$/);
});
