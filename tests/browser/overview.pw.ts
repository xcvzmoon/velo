import type { Page } from '@playwright/test';
import type { HistoryEntry } from '../../app/types/contracts';
import { AxeBuilder } from '@axe-core/playwright';
import { expect, test } from '@playwright/test';
import { runFixture } from './fixtures/run';

const historyEntry: Omit<HistoryEntry, 'configuration'> & { configuration: null } = {
  id: 'history-1',
  method: 'GET',
  created_at: '1794139200',
  interrupted: false,
  incomplete: false,
  stride: 1,
  metadata: {
    git_sha: '',
    release: '',
    tags: [],
    notes: '',
  },
  definition_identity: null,
  configuration: null,
  snapshot: runFixture,
  points: [],
};

async function openOverview(
  page: Page,
  fixtureMode: 'empty' | 'stored' | 'live' | 'unavailable',
): Promise<void> {
  await page.addInitScript(
    ({ entry, snapshot, mode }) => {
      if (mode === 'live') {
        snapshot.state = 'running';
      }
      if (mode === 'stored') {
        entry.interrupted = true;
        entry.incomplete = true;
        entry.snapshot.state = 'running';
      }
      Object.defineProperty(window, '__TAURI_INTERNALS__', {
        value: {
          transformCallback: () => 1,
          unregisterCallback: () => undefined,
          invoke: (command: string) => {
            if (command === 'current_run') {
              return Promise.resolve(mode === 'live' ? structuredClone(snapshot) : null);
            }
            if (command === 'stop_run') {
              snapshot.state = 'cancelled';
              entry.snapshot = structuredClone(snapshot);
              return Promise.resolve(structuredClone(snapshot));
            }
            if (command === 'list_history') {
              return mode === 'unavailable'
                ? Promise.reject(new Error('private-native-detail'))
                : Promise.resolve(mode === 'empty' ? [] : [structuredClone(entry)]);
            }
            if (command === 'history_status') {
              return Promise.resolve(mode === 'stored' ? 'HISTORY_BACKPRESSURE' : null);
            }
            if (command === 'plugin:event|listen') {
              return Promise.resolve(1);
            }
            if (command === 'plugin:event|unlisten') {
              return Promise.resolve(null);
            }
            return Promise.reject(new Error('IPC_TEST_UNSUPPORTED'));
          },
        },
      });
    },
    {
      entry: historyEntry,
      snapshot: runFixture,
      mode: fixtureMode,
    },
  );
  await page.goto('/');
  await expect(
    page.getByRole('heading', {
      name: 'Overview',
      exact: true,
    }),
  ).toBeVisible();
}

test('empty dashboard shows unavailable metrics and working next actions', async ({ page }) => {
  await openOverview(page, 'empty');
  await expect(
    page.getByRole('region', { name: 'Run metrics' }).getByText('—', { exact: true }),
  ).toHaveCount(4);
  await expect(page.getByRole('heading', { name: 'Your first run starts here' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'No stored runs yet' })).toBeVisible();
  await expect(
    page.getByRole('link', {
      name: 'Open API workspace',
      exact: true,
    }),
  ).toHaveAttribute('href', '/api');
  await page
    .getByRole('button', {
      name: 'Refresh',
      exact: true,
    })
    .click();
  await expect(
    page.getByRole('button', {
      name: 'Refresh',
      exact: true,
    }),
  ).toBeEnabled();
});

test('history populates metrics without presenting interrupted runs as active', async ({
  page,
}) => {
  await openOverview(page, 'stored');
  await expect(page.getByText('9,007,199,254,740,993', { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Latest stored run' })).toBeVisible();
  await expect(page.getByText('interrupted', { exact: true })).toHaveCount(2);
  await expect(page.getByText('Sample gaps', { exact: true })).toBeVisible();
  await expect(
    page.getByRole('button', {
      name: 'Stop run',
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByText('The application closed before this run finished.', { exact: false }),
  ).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Timeline available in Results' })).toBeVisible();
});

test('live dashboard retains the explicit stop action and partial-run feedback', async ({
  page,
}) => {
  await openOverview(page, 'live');
  await expect(page.getByRole('heading', { name: 'Current run' })).toBeVisible();
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
    page.getByText('This run has an incomplete measurement window.', { exact: false }),
  ).toBeVisible();
});

test('history failures remain explicit and do not expose native error details', async ({
  page,
}) => {
  await openOverview(page, 'unavailable');
  await expect(page.getByText('History unavailable', { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Recent runs unavailable' })).toBeVisible();
  await expect(page.getByText('No stored runs yet', { exact: true })).toHaveCount(0);
  await expect(page.getByText('private-native-detail')).toHaveCount(0);
});

for (const theme of ['light', 'dark']) {
  test(`${theme} dashboard is accessible at the minimum desktop size`, async ({
    page,
  }, testInfo) => {
    await page.setViewportSize({
      width: 640,
      height: 480,
    });
    await page.addInitScript((preference) => {
      localStorage.setItem('nuxt-color-mode', preference);
    }, theme);
    await openOverview(page, 'stored');
    const dashboard = page.getByRole('region', {
      name: 'Overview dashboard',
      exact: true,
    });
    await expect(dashboard.getByText('Sample gaps')).toBeVisible();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth),
    ).toBe(false);
    const results = await new AxeBuilder({ page })
      .include('[aria-label="Overview dashboard"]')
      .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
      .analyze();
    expect(results.violations).toEqual([]);
    await page.setViewportSize({
      width: 1440,
      height: 1000,
    });
    await page.screenshot({ path: testInfo.outputPath(`overview-${theme}.png`) });
  });
}
