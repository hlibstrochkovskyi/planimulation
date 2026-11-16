// Capture vegetation-1 layers and the region history for visual review.
// Usage: node --import tsx scripts/vegetation-review.ts OUTPUT_DIR [seed] [years]
import { mkdir, mkdtemp } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';

const [output = 'artifacts/vegetation-review', seed = 'first-light', years = '100'] = process.argv.slice(2);
await mkdir(output, { recursive: true });
const temporary = await mkdtemp(path.join(os.tmpdir(), 'planimulation-review-'));
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const app = await electron.launch({ args: ['.', `--user-data-dir=${temporary}`], env, timeout: 30_000 });
try {
  const page = await app.firstWindow();
  page.setDefaultTimeout(120_000);
  page.on('pageerror', error => console.error('page error:', error.message));
  await page.setViewportSize({ width: 1600, height: 1000 });
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 60_000 });
  await page.locator('[data-dialog="world-dialog"]').click();
  await page.locator('#seed').fill(seed);
  await page.locator('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 300_000 });
  await expect(page.locator('[data-layer="vegetationState"]')).toBeEnabled({ timeout: 60_000 });
  await page.locator('[data-layer="vegetationState"]').click();
  await page.waitForTimeout(1000);
  await page.locator('#map').screenshot({ path: path.join(output, `${seed}-year0-state.png`) });
  await page.locator('#tab-vegetation').click();
  for (let year = 0; year < Number(years); year += 100) {
    await page.locator('#vegetation-100').click();
    await expect(page.locator('#vegetation-time')).toHaveText(`Year ${year + 100}`, { timeout: 120_000 });
  }
  for (const layer of ['vegetationState', 'woodyCover', 'precipitationAnomaly']) {
    await page.locator(`[data-layer="${layer}"]`).click();
    await page.waitForTimeout(1000);
    await page.locator('#map').screenshot({ path: path.join(output, `${seed}-year${years}-${layer}.png`) });
  }
  await page.locator('[data-layer="woodyCover"]').click();
  // Find a land region whose history shows a drought mark, then capture the whole window.
  const box = (await page.locator('#map').boundingBox())!;
  if (await page.locator('#toggle-inspector').getAttribute('aria-expanded') === 'false') await page.locator('#toggle-inspector').click();
  await page.locator('#tab-factors').click();
  for (let attempt = 0; attempt < 40; attempt++) {
    const x = box.x + box.width * (0.2 + 0.6 * ((attempt * 0.618) % 1)), y = box.y + box.height * (0.25 + 0.5 * ((attempt * 0.382) % 1));
    await page.mouse.click(x, y);
    await page.waitForTimeout(400);
    if (/Woody cover fell/.test(await page.locator('#vegetation-note').textContent() ?? '')) break;
  }
  await page.screenshot({ path: path.join(output, `${seed}-year${years}-window.png`) });
  console.log(await page.locator('#vegetation-note').textContent());
  console.log(await page.locator('#vegetation-summary').textContent());
} finally {
  await app.close();
}
