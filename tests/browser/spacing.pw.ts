import { expect, test } from '@playwright/test';

for (const route of ['/tests', '/settings', '/api']) {
  test(`${route} separates field labels from their controls`, async ({ page }) => {
    await page.goto(route);
    await expect(page.locator('h1')).toBeVisible();
    await expect(page.locator('label[for]').first()).toBeVisible();
    const fields = await page.locator('label[for]').evaluateAll((labels) => {
      const spacing: { label: string; gap: number }[] = [];
      for (const label of labels) {
        const control = document.getElementById(label.getAttribute('for') ?? '');
        if (!control || !label.getClientRects().length || !control.getClientRects().length) {
          continue;
        }
        if (control instanceof HTMLInputElement && ['checkbox', 'radio'].includes(control.type)) {
          continue;
        }
        spacing.push({
          label: label.textContent?.trim() ?? '',
          gap: control.getBoundingClientRect().top - label.getBoundingClientRect().bottom,
        });
      }
      return spacing;
    });
    expect(fields.length).toBeGreaterThan(0);
    for (const field of fields) {
      expect(field.gap, field.label).toBeCloseTo(8, 1);
    }
  });
}

test('saved configurations expand with breathing room and a single visual container', async ({
  page,
}) => {
  await page.goto('/tests');
  const summary = page.locator('summary').filter({ hasText: 'Saved request configurations' });
  await summary.click();
  const disclosure = summary.locator('..');
  const saved = disclosure.getByRole('region', {
    name: 'Saved configurations',
    exact: true,
  });
  await expect(saved).toBeVisible();
  await expect(
    saved.getByRole('button', {
      name: 'Latest configurations',
      exact: true,
    }),
  ).toBeVisible();
  const bounds = await disclosure.evaluate((element) => {
    const heading = element.querySelector('summary');
    const content = element.querySelector('section');
    if (!heading || !content) {
      throw new Error('Expected the saved configuration disclosure and content.');
    }
    return {
      gap: content.getBoundingClientRect().top - heading.getBoundingClientRect().bottom,
      shadow: getComputedStyle(content).boxShadow,
      border: getComputedStyle(content).borderTopWidth,
    };
  });
  expect(bounds.gap).toBe(16);
  expect(bounds.shadow).toBe('none');
  expect(bounds.border).toBe('0px');
  await summary.focus();
  await page.keyboard.press('Enter');
  await expect(saved).toBeHidden();
  await page.keyboard.press('Enter');
  await expect(saved).toBeVisible();
});
