import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests',
  use: { baseURL: 'http://127.0.0.1:6006', trace: 'retain-on-failure', reducedMotion: 'reduce' },
  webServer: {
    command: 'npm run storybook -- --ci',
    url: 'http://127.0.0.1:6006',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
