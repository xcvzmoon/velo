import { AxeBuilder } from '@axe-core/playwright';
import { expect, test } from '@playwright/test';
import { runFixture } from './fixtures/run';

test('native snapshot controls retain stopping, precision, result tables, and theme behavior', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.addInitScript((fixture) => {
    fixture.state = 'running';
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      value: {
        transformCallback: () => 1,
        unregisterCallback: () => undefined,
        invoke: (command: string) => {
          if (command === 'current_run') {
            return Promise.resolve(structuredClone(fixture));
          }
          if (command === 'stop_run') {
            fixture.state = 'cancelled';
            return Promise.resolve(structuredClone(fixture));
          }
          if (command === 'plugin:event|listen') {
            return Promise.resolve(1);
          }
          if (
            command === 'plugin:event|unlisten' ||
            command === 'history_status' ||
            command === 'pinned_baseline'
          ) {
            return Promise.resolve(null);
          }
          if (command === 'list_history') {
            return Promise.resolve([]);
          }
          return Promise.reject(new Error(`IPC_TEST_UNSUPPORTED: ${command}`));
        },
      },
    });
  }, runFixture);
  await page.goto('/results');
  const monitor = page.getByRole('region', { name: 'Current run' });
  await expect(monitor.getByText('9007199254740993', { exact: true })).toBeVisible();
  await expect(page.getByRole('progressbar', { name: 'Scheduling progress' })).toBeVisible();
  await page
    .getByRole('button', {
      name: 'Stop run',
      exact: true,
    })
    .click();
  await expect(
    page.getByRole('button', {
      name: 'Stop run',
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByRole('heading', {
      name: 'Final summary',
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByText('Partial run. Measurements cover observed work only.', { exact: false }),
  ).toBeVisible();
  const durations = page.getByRole('table', { name: /Evaluated approximate HDR/ });
  await expect(durations.getByRole('rowgroup').last().getByRole('row')).toHaveCount(3);
  await expect(
    durations.getByRole('cell', {
      name: '1.000',
      exact: true,
    }),
  ).toHaveCount(21);
  await expect(
    page.getByRole('table', { name: /Final thresholds/ }).getByText('passed', { exact: true }),
  ).toBeVisible();
  await expect(
    page
      .getByRole('table', { name: 'Per-assertion outcomes' })
      .last()
      .getByText('status-check', { exact: true }),
  ).toBeVisible();
  await expect(
    page
      .getByRole('table', { name: /Measured scenario iterations/ })
      .last()
      .getByText('journey', { exact: true }),
  ).toBeVisible();
  await page.getByRole('button', { name: /Switch to (light|dark) mode/ }).click();
  await expect(
    page.getByRole('heading', {
      name: 'Final summary',
      exact: true,
    }),
  ).toBeVisible();
  await page.setViewportSize({
    width: 640,
    height: 480,
  });
  expect(await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth)).toBe(
    false,
  );
  expect(errors).toEqual([]);
  const accessibility = await new AxeBuilder({ page })
    .include('#__nuxt')
    .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
    .analyze();
  expect(accessibility.violations).toEqual([]);
});
