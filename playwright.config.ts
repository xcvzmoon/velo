import { defineConfig, devices } from '@playwright/test';
import process from 'node:process';
import * as v from 'valibot';

const environment = v.parse(
  v.object({
    PLAYWRIGHT_BASE_URL: v.optional(
      v.pipe(
        v.string('UI_TEST_INVALID_BASE_URL: Expected a URL.'),
        v.url('UI_TEST_INVALID_BASE_URL: Expected an absolute URL.'),
        v.regex(/^https?:\/\//, 'UI_TEST_INVALID_BASE_URL: Use an HTTP or HTTPS URL.'),
      ),
    ),
  }),
  { PLAYWRIGHT_BASE_URL: process.env.PLAYWRIGHT_BASE_URL },
);
const baseURL = environment.PLAYWRIGHT_BASE_URL ?? 'http://127.0.0.1:4173';

export default defineConfig({
  testDir: './tests/browser',
  testMatch: '**/*.pw.ts',
  fullyParallel: true,
  retries: 0,
  expect: { timeout: 10000 },
  use: {
    baseURL,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'webkit',
      use: { ...devices['Desktop Safari'] },
    },
  ],
  webServer: environment.PLAYWRIGHT_BASE_URL
    ? undefined
    : {
        command:
          'pnpm nuxt:generate && pnpm exec sirv .output/public --single --host 127.0.0.1 --port 4173',
        url: 'http://127.0.0.1:4173',
        reuseExistingServer: !process.env.CI,
        timeout: 120000,
      },
});
