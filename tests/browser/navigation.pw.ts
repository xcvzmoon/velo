import { expect, test } from '@playwright/test';

test('titlebar history follows route changes, keeps drafts, and survives reload', async ({
  page,
}) => {
  await page.goto('/');
  const back = page.getByRole('button', {
    name: 'Go back',
    exact: true,
  });
  const forward = page.getByRole('button', {
    name: 'Go forward',
    exact: true,
  });
  const workspace = page.getByRole('status', {
    name: 'Active workspace',
    exact: true,
  });
  const navigation = page.locator('#workspace-navigation');
  await expect(workspace).toHaveText('Overview');
  await expect(back).toBeDisabled();
  await expect(forward).toBeDisabled();

  await navigation
    .getByRole('link', {
      name: 'Test studio',
      exact: true,
    })
    .click();
  await expect(workspace).toHaveText('Test studio');
  const target = page.getByLabel('Target URL', { exact: true });
  await target.fill('https://example.com/history-draft');
  await navigation
    .getByRole('link', {
      name: 'Results',
      exact: true,
    })
    .click();
  await expect(workspace).toHaveText('Results');
  await expect(back).toBeEnabled();
  await expect(forward).toBeDisabled();

  await back.click();
  await expect(workspace).toHaveText('Test studio');
  await expect(target).toHaveValue('https://example.com/history-draft');
  await back.click();
  await expect(workspace).toHaveText('Overview');
  await expect(back).toBeDisabled();
  await expect(forward).toBeEnabled();
  await forward.click();
  await expect(workspace).toHaveText('Test studio');
  await forward.click();
  await expect(workspace).toHaveText('Results');
  await expect(forward).toBeDisabled();

  await back.click();
  await expect(workspace).toHaveText('Test studio');
  await navigation
    .getByRole('link', {
      name: 'Settings',
      exact: true,
    })
    .click();
  await expect(workspace).toHaveText('Settings');
  await expect(forward).toBeDisabled();
  await back.click();
  await expect(workspace).toHaveText('Test studio');
  await page.reload();
  await expect(workspace).toHaveText('Test studio');
  await expect(back).toBeEnabled();
  await expect(forward).toBeEnabled();
  await forward.click();
  await expect(workspace).toHaveText('Settings');
  await expect(forward).toBeDisabled();
});

for (const [name, path] of [
  ['Overview', '/'],
  ['API workspace', '/api'],
  ['Test studio', '/tests'],
  ['Results', '/results'],
  ['Settings', '/settings'],
] as const) {
  test(`workspace badge identifies ${name} with query and hash details`, async ({ page }) => {
    await page.goto(`${path}?view=details#workspace-navigation`);
    const workspace = page.getByRole('status', {
      name: 'Active workspace',
      exact: true,
    });
    await expect(workspace).toHaveText(name);
    const destination = name === 'Overview' ? 'Settings' : 'Overview';
    await page
      .locator('#workspace-navigation')
      .getByRole('link', {
        name: destination,
        exact: true,
      })
      .click();
    await expect(workspace).toHaveText(destination);
    await page
      .getByRole('button', {
        name: 'Go back',
        exact: true,
      })
      .click();
    await expect(workspace).toHaveText(name);
  });
}
