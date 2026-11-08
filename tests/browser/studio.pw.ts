import { AxeBuilder } from '@axe-core/playwright';
import { expect, test } from '@playwright/test';
import { Buffer } from 'node:buffer';

test('authoring preserves Formisch edits across sections and navigation', async ({ page }) => {
  await page.goto('/tests');
  const target = page.getByLabel('Target URL', { exact: true });
  await target.fill('https://example.com/performance');
  await page
    .getByRole('tab', {
      name: 'Data',
      exact: true,
    })
    .click();
  await expect(page.getByRole('heading', { name: 'Test data' })).toBeVisible();
  await page
    .getByRole('tab', {
      name: 'Request',
      exact: true,
    })
    .click();
  await expect(target).toHaveValue('https://example.com/performance');

  await page.locator('#workspace-navigation').getByRole('link', { name: 'Overview' }).click();
  await page.locator('#workspace-navigation').getByRole('link', { name: 'Test studio' }).click();
  await expect(target).toHaveValue('https://example.com/performance');

  await page
    .getByRole('combobox', {
      name: 'Method',
      exact: true,
    })
    .click();
  await page
    .getByRole('option', {
      name: 'POST',
      exact: true,
    })
    .click();
  await expect(
    page.getByRole('combobox', {
      name: 'Method',
      exact: true,
    }),
  ).toContainText('POST');

  await target.fill('invalid');
  await page
    .getByRole('button', {
      name: 'Review run',
      exact: true,
    })
    .click();
  await expect(target).toHaveAttribute('aria-invalid', 'true');
  await expect(page.getByRole('button', { name: 'I am authorized. Start run' })).toHaveCount(0);
  await target.fill('https://example.com/performance');
  await page
    .getByRole('button', {
      name: 'JSON and environments',
      exact: true,
    })
    .click();
  const editor = page.getByLabel('Version 1 test definition', { exact: true });
  await expect(editor).toBeVisible();
  expect(await editor.evaluate((element) => getComputedStyle(element).fontFamily)).toContain(
    'monospace',
  );
});

test('keyboard sections, theme controls, and unavailable native service remain usable', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/settings');
  await expect(page.locator('h1'), JSON.stringify(errors)).toBeVisible();
  await expect(
    page.getByRole('heading', {
      name: 'Native connection',
      exact: true,
    }),
  ).toBeVisible();
  const workspace = page.getByRole('tab', {
    name: 'Workspace',
    exact: true,
  });
  await workspace.focus();
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('tab', { name: 'AI providers' })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await expect(page.getByRole('heading', { name: 'Native AI assistance' })).toBeVisible();
  await workspace.click();
  await page
    .getByRole('button', {
      name: 'Check connection',
      exact: true,
    })
    .click();
  await expect(page.getByRole('status').filter({ hasText: 'IPC_UNAVAILABLE' })).toBeVisible();
});

test('destructive dialogs focus Cancel and restore the trigger on Escape', async ({ page }) => {
  await page.goto('/api');
  await page.getByLabel('New collection name', { exact: true }).fill('Review fixture');
  await page
    .getByRole('button', {
      name: 'Create',
      exact: true,
    })
    .click();
  const trigger = page.getByRole('button', {
    name: 'Delete collection',
    exact: true,
  });
  await trigger.click();
  const dialog = page.getByRole('dialog', { name: 'Delete collection?' });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole('button', {
      name: 'Cancel',
      exact: true,
    }),
  ).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();
  await expect(
    page.getByRole('button', {
      name: 'Review fixture',
      exact: false,
    }),
  ).toBeVisible();
});

for (const width of [640, 800, 1440]) {
  for (const theme of ['light', 'dark']) {
    for (const route of ['/', '/tests', '/api', '/results', '/settings']) {
      test(`${theme} ${route} remains accessible at ${width}px`, async ({ page }, testInfo) => {
        await page.setViewportSize({
          width,
          height: width === 640 ? 480 : 800,
        });
        await page.addInitScript((preference) => {
          localStorage.setItem('nuxt-color-mode', preference);
        }, theme);
        await page.goto(route);
        await expect(page.locator('h1')).toBeVisible();
        await expect(page.locator('html')).toHaveClass(new RegExp(theme));
        const overflow = await page.evaluate(
          () => document.documentElement.scrollWidth > window.innerWidth,
        );
        expect(overflow).toBe(false);
        const accessibility = await new AxeBuilder({ page })
          .include('#__nuxt')
          .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
          .analyze();
        expect(accessibility.violations).toEqual([]);
        if (width === 1440 && (route === '/' || route === '/settings')) {
          await page.screenshot({ path: testInfo.outputPath('workspace.png') });
        }
      });
    }
  }
}

test('CSV imports prepare real records and allow selecting the same filename again', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/tests');
  await page
    .getByRole('tab', {
      name: 'Data',
      exact: true,
    })
    .click();
  await page
    .getByRole('combobox', {
      name: 'Source',
      exact: true,
    })
    .click();
  await page
    .getByRole('option', {
      name: 'csv',
      exact: true,
    })
    .click();
  await page
    .getByRole('button', {
      name: 'Apply data recipe',
      exact: true,
    })
    .click();
  const file = page.getByLabel('CSV or JSON file (up to 8 MiB)', { exact: true });
  await file.setInputFiles({
    name: 'records.csv',
    mimeType: 'text/csv',
    buffer: Buffer.from('id\nalpha\n'),
  });
  await page
    .getByRole('button', {
      name: 'Prepare and preview records',
      exact: true,
    })
    .click();
  const preview = page.getByRole('region', { name: 'Prepared data preview' });
  await expect(preview.locator('pre')).toContainText('alpha');
  await file.setInputFiles({
    name: 'records.csv',
    mimeType: 'text/csv',
    buffer: Buffer.from('id\nbeta\n'),
  });
  await page
    .getByRole('button', {
      name: 'Prepare and preview records',
      exact: true,
    })
    .click();
  await expect(preview.locator('pre')).toContainText('beta');
  await expect(preview.locator('pre')).not.toContainText('alpha');
  expect(errors).toEqual([]);
});
