// Capture wild-food-1 layers, an inspected region and a foraging experiment for visual review.
// Usage: node --import tsx scripts/wild-food-review.ts OUTPUT_DIR [seed] [years]
import { mkdir, mkdtemp } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';

const [output = 'artifacts/wild-food-review', seed = 'first-light', years = '200'] = process.argv.slice(2);
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
  await expect(page.locator('[data-layer="foragerCapacity"]')).toBeEnabled({ timeout: 60_000 });
  await page.locator('#tab-vegetation').click();
  for (let year = 0; year < Number(years); year += 100) {
    await page.locator('#vegetation-100').click();
    await expect(page.locator('#vegetation-time')).toHaveText(`Year ${year + 100}`, { timeout: 120_000 });
  }
  await expect(page.locator('#vegetation-summary')).toContainText('foragers year-round', { timeout: 60_000 });
  for (const layer of ['foragerCapacity', 'leanSeason', 'gameStock']) {
    await page.locator(`[data-layer="${layer}"]`).click();
    await page.waitForTimeout(1000);
    await page.locator('#map').screenshot({ path: path.join(output, `${seed}-year${years}-${layer}.png`) });
  }
  await page.locator('[data-layer="foragerCapacity"]').click();
  const box = (await page.locator('#map').boundingBox())!;
  if (await page.locator('#toggle-inspector').getAttribute('aria-expanded') === 'false') await page.locator('#toggle-inspector').click();
  await page.locator('#tab-factors').click();
  // Find a land region with a sizable capacity, then forage there with three times the sustainable group.
  for (let attempt = 0; attempt < 40; attempt++) {
    const x = box.x + box.width * (0.2 + 0.6 * ((attempt * 0.618) % 1)), y = box.y + box.height * (0.25 + 0.5 * ((attempt * 0.382) % 1));
    await page.mouse.click(x, y);
    await page.waitForTimeout(400);
    const note = await page.locator('#food-note').textContent() ?? '';
    const density = Number(/^([\d.]+) foragers/.exec(note)?.[1] ?? 0);
    if (density < 5) continue;
    await expect(page.locator('#food-details')).not.toContainText('waiting', { timeout: 30_000 });
    break;
  }
  await page.locator('#forage-3').click();
  await expect(page.locator('#forage-note')).toContainText('foraged here', { timeout: 30_000 });
  await page.locator('#food-note').scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, `${seed}-year${years}-window.png`) });
  console.log(await page.locator('#food-note').textContent());
  console.log(await page.locator('#food-details').textContent());
  console.log(await page.locator('#forage-note').textContent());
  console.log(await page.locator('#vegetation-summary').textContent());
} finally {
  await app.close();
}
