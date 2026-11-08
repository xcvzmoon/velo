import type { Locator } from '@playwright/test';
import { expect, test } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import * as v from 'valibot';

const desktop = v.parse(
  v.object({ app: v.object({ security: v.object({ csp: v.string() }) }) }),
  JSON.parse(readFileSync(new URL('../../src-tauri/tauri.conf.json', import.meta.url), 'utf8')),
);

async function expectGlyph(icon: Locator): Promise<void> {
  await expect(icon).toBeVisible();
  await expect
    .poll(() =>
      icon.evaluate((element) => {
        if (element instanceof SVGSVGElement) {
          const bounds = element.getBBox();
          return bounds.width > 0 && bounds.height > 0;
        }
        const style = getComputedStyle(element);
        return style.maskImage !== 'none' || style.backgroundImage !== 'none';
      }),
    )
    .toBe(true);
}

test('workspace icons render without runtime fetching under the desktop CSP', async ({
  page,
  baseURL,
}) => {
  if (!baseURL) {
    throw new Error('UI_TEST_MISSING_BASE_URL: The icon regression requires a frontend URL.');
  }
  const origin = new URL(baseURL).origin;
  const iconRequests: string[] = [];
  page.on('request', (request) => {
    if (/iconify|_nuxt_icon/.test(request.url())) {
      iconRequests.push(request.url());
    }
  });
  await page.route('**/*', async (route) => {
    const request = route.request();
    if (new URL(request.url()).origin !== origin) {
      await route.abort();
      return;
    }
    if (request.resourceType() !== 'document') {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    const html = await response.text();
    const hashes: string[] = [];
    for (const script of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)) {
      if (script[2] && !/\bsrc\s*=/.test(script[1] ?? '')) {
        hashes.push(`'sha256-${createHash('sha256').update(script[2]).digest('base64')}'`);
      }
    }
    const policy = desktop.app.security.csp.replace(
      /script-src\s+([^;]+)/,
      (directive) => `${directive} ${hashes.join(' ')}`,
    );
    await route.fulfill({
      response,
      headers: {
        ...response.headers(),
        'content-security-policy': policy,
      },
    });
  });
  await page.goto('/');
  const navigation = page.locator('#workspace-navigation');
  await expect(navigation.getByRole('link')).toHaveCount(5);
  await Promise.all(
    ['Overview', 'API workspace', 'Test studio', 'Results', 'Settings'].map(async (name) => {
      await expectGlyph(
        navigation
          .getByRole('link', {
            name,
            exact: true,
          })
          .locator('.iconify'),
      );
    }),
  );
  await expectGlyph(
    page
      .getByRole('button', {
        name: 'Go back',
        exact: true,
      })
      .locator('.iconify'),
  );
  await expectGlyph(
    page
      .getByRole('button', {
        name: 'Go forward',
        exact: true,
      })
      .locator('.iconify'),
  );
  const metrics = page
    .getByRole('region', {
      name: 'Run metrics',
      exact: true,
    })
    .locator('.iconify');
  await expect(metrics).toHaveCount(4);
  await Promise.all((await metrics.all()).map(expectGlyph));
  await page
    .getByRole('button', {
      name: 'Collapse sidebar',
      exact: true,
    })
    .click();
  await expectGlyph(
    page
      .getByRole('button', {
        name: 'Expand sidebar',
        exact: true,
      })
      .locator('.iconify'),
  );
  await page.getByRole('button', { name: /Switch to (light|dark) mode/ }).click();
  await expectGlyph(
    page.getByRole('button', { name: /Switch to (light|dark) mode/ }).locator('.iconify'),
  );
  await navigation
    .getByRole('link', {
      name: 'Settings',
      exact: true,
    })
    .click();
  await expectGlyph(
    page
      .getByRole('button', {
        name: 'Color theme',
        exact: true,
      })
      .locator('.iconify')
      .first(),
  );
  expect(iconRequests).toEqual([]);
});
