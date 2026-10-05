import { defineConfig, devices } from '@playwright/test';
const remoteUrl = process.env.E2E_BASE_URL;
export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  retries: 0,
  use: {
    baseURL: remoteUrl ?? 'http://localhost:5173',
    trace: 'retain-on-failure',
    reducedMotion: 'reduce',
  },
  ...(remoteUrl
    ? {}
    : {
        webServer: {
          command: 'npm run dev',
          url: 'http://localhost:5173',
          reuseExistingServer: !process.env.CI,
        },
      }),
  projects: [
    { name: 'desktop', use: { ...devices['Desktop Chrome'] } },
    { name: 'mobile', use: { ...devices['iPhone 13'], defaultBrowserType: 'chromium' } },
  ],
});
