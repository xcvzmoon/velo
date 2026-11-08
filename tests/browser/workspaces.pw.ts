import { AxeBuilder } from '@axe-core/playwright';
import { expect, test } from '@playwright/test';

for (const theme of ['light', 'dark']) {
  test(`API request drafts survive workspace sections in ${theme} mode`, async ({
    page,
  }, testInfo) => {
    await page.addInitScript((preference) => {
      localStorage.setItem('nuxt-color-mode', preference);
    }, theme);
    await page.goto('/api');
    await page.getByLabel('New collection name', { exact: true }).fill('Orders');
    await page
      .getByRole('button', {
        name: 'Create',
        exact: true,
      })
      .click();
    await page
      .getByRole('button', {
        name: 'New request',
        exact: true,
      })
      .click();
    await page.getByLabel('New request name', { exact: true }).fill('List orders');
    await page
      .getByRole('button', {
        name: 'Create',
        exact: true,
      })
      .click();
    const target = page.getByLabel('Target URL', { exact: true });
    await target.fill('https://example.com/orders');
    await page
      .getByRole('tab', {
        name: 'Run collection',
        exact: true,
      })
      .click();
    await expect(
      page.getByRole('heading', { name: 'Send or run ordered functional checks' }),
    ).toBeVisible();
    await page
      .getByRole('tab', {
        name: 'Secrets',
        exact: true,
      })
      .click();
    await expect(page.getByRole('heading', { name: 'OS secrets' })).toBeVisible();
    await page
      .getByRole('tab', {
        name: 'Requests',
        exact: true,
      })
      .click();
    await expect(target).toHaveValue('https://example.com/orders');
    await page.setViewportSize({
      width: 640,
      height: 480,
    });
    expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(
      false,
    );
    const accessibility = await new AxeBuilder({ page })
      .include('#__nuxt')
      .withTags(['wcag2a', 'wcag2aa', 'wcag21aa'])
      .analyze();
    expect(accessibility.violations).toEqual([]);
    await page.setViewportSize({
      width: 1440,
      height: 1100,
    });
    await page.screenshot({ path: testInfo.outputPath(`api-${theme}.png`) });
  });
}

test('settings retain provider drafts across sections without submitting keys', async ({
  page,
}) => {
  await page.goto('/settings');
  await expect(
    page.getByRole('button', {
      name: 'Color theme',
      exact: true,
    }),
  ).toBeVisible();
  await page
    .getByRole('tab', {
      name: 'AI providers',
      exact: true,
    })
    .click();
  const key = page.getByLabel('Provider API key', { exact: true });
  await key.fill('local-draft-key');
  await page
    .getByRole('tab', {
      name: 'Workspace',
      exact: true,
    })
    .click();
  await expect(
    page.getByRole('heading', {
      name: 'Native connection',
      exact: true,
    }),
  ).toBeVisible();
  await page
    .getByRole('tab', {
      name: 'AI providers',
      exact: true,
    })
    .click();
  await expect(key).toHaveValue('local-draft-key');
});

test('API edit confirmations preserve drafts on cancellation and discard only unapplied edits', async ({
  page,
}) => {
  await page.goto('/api');
  await page.getByLabel('New collection name', { exact: true }).fill('Confirmation review');
  await page
    .getByRole('button', {
      name: 'Create',
      exact: true,
    })
    .click();
  await page
    .getByRole('button', {
      name: 'New request',
      exact: true,
    })
    .click();
  await page.getByLabel('New request name', { exact: true }).fill('Draft request');
  await page
    .getByRole('button', {
      name: 'Create',
      exact: true,
    })
    .click();
  const target = page.getByLabel('Target URL', { exact: true });
  await target.fill('https://example.com/preserved');
  await page
    .getByRole('button', {
      name: 'Close Draft request',
      exact: true,
    })
    .click();
  const close = page.getByRole('dialog', {
    name: 'Close request tab?',
    exact: true,
  });
  await expect(close).toBeVisible();
  await expect(
    close.getByRole('button', {
      name: 'Cancel',
      exact: true,
    }),
  ).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(close).toBeHidden();
  await expect(target).toHaveValue('https://example.com/preserved');
  await page
    .getByRole('button', {
      name: 'Close Draft request',
      exact: true,
    })
    .click();
  await close
    .getByRole('button', {
      name: 'Confirm',
      exact: true,
    })
    .click();
  await expect(
    page.getByRole('heading', {
      name: 'Select a request',
      exact: true,
    }),
  ).toBeVisible();
  await page
    .getByRole('button', {
      name: 'Request · Draft request',
      exact: true,
    })
    .click();
  await expect(target).toHaveValue('https://example.com/preserved');

  await page
    .getByRole('button', {
      name: 'Edit collection JSON',
      exact: true,
    })
    .click();
  const document = page.getByLabel('Version 1 API collection', { exact: true });
  await document.fill('{');
  await page
    .getByRole('button', {
      name: 'Back to requests',
      exact: true,
    })
    .click();
  const discard = page.getByRole('dialog', {
    name: 'Discard collection edits?',
    exact: true,
  });
  await expect(discard).toBeVisible();
  await discard
    .getByRole('button', {
      name: 'Cancel',
      exact: true,
    })
    .click();
  await expect(document).toHaveValue('{');
  await page
    .getByRole('button', {
      name: 'Back to requests',
      exact: true,
    })
    .click();
  await discard
    .getByRole('button', {
      name: 'Confirm',
      exact: true,
    })
    .click();
  await expect(document).toBeHidden();
  await expect(target).toHaveValue('https://example.com/preserved');

  const navigation = page.locator('#workspace-navigation');
  await navigation
    .getByRole('link', {
      name: 'Overview',
      exact: true,
    })
    .click();
  await navigation
    .getByRole('link', {
      name: 'API workspace',
      exact: true,
    })
    .click();
  await page
    .getByRole('button', {
      name: 'Close Draft request',
      exact: true,
    })
    .click();
  await expect(close).toBeVisible();
  await page.goBack();
  await expect(
    page.getByRole('heading', {
      name: 'Overview',
      exact: true,
    }),
  ).toBeVisible();
  await expect(close).toBeHidden();
  await page
    .getByRole('button', {
      name: 'Go forward',
      exact: true,
    })
    .click();
  await expect(target).toHaveValue('https://example.com/preserved');
});
