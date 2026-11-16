// Capture flat-map and globe screenshots of selected worlds for visual review.
// Usage: node --import tsx scripts/render-review.ts OUTPUT_DIR [family] [subdivision] [seed] [zoomX zoomY as canvas fractions]
// REVIEW_LAYER=biome (any data-layer name) selects a layer before capturing.
import { mkdir, mkdtemp } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';

const [output = 'artifacts/render-review', family = 'terrain-prep-1', subdivision = '5', seed = 'first-light', zoomX = '.3', zoomY = '.45'] = process.argv.slice(2);
await mkdir(output, { recursive: true });
const temporary = await mkdtemp(path.join(os.tmpdir(), 'planimulation-review-'));
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const app = await electron.launch({ args: ['.', `--user-data-dir=${temporary}`], env, timeout: 30_000 });
try {
  const page = await app.firstWindow();
  page.setDefaultTimeout(120_000);
  page.on('pageerror', error => console.error('page error:', error.message));
  page.on('console', message => { if (message.type() === 'error' || message.type() === 'warning') console.error(message.type(), message.text()); });
  await page.setViewportSize({ width: 1600, height: 1000 });
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 60_000 });
  await page.locator('[data-dialog="world-dialog"]').click();
  await page.locator('#generator-family').selectOption(family);
  await page.locator('#subdivision').selectOption(subdivision);
  await page.locator('#seed').fill(seed);
  await page.locator('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 300_000 });
  const layer = process.env.REVIEW_LAYER;
  if (layer) await page.locator(`[data-layer="${layer}"]`).click();
  await page.waitForTimeout(1500);
  const name = `${family}-s${subdivision}-${seed}${layer ? `-${layer}` : ''}`;
  await page.locator('#map').screenshot({ path: path.join(output, `${name}-flat.png`) });
  await page.locator('[data-view="globe"]').click();
  await page.waitForTimeout(1500);
  await page.locator('#map').screenshot({ path: path.join(output, `${name}-globe.png`) });
  await page.locator('#display-trigger').click();
  await page.locator('#exaggeration').selectOption('0');
  await page.keyboard.press('Escape');
  await page.waitForTimeout(800);
  await page.locator('#map').screenshot({ path: path.join(output, `${name}-globe-flat-relief.png`) });
  await page.locator('#display-trigger').click();
  await page.locator('#exaggeration').selectOption('10');
  await page.keyboard.press('Escape');
  await page.locator('[data-view="flat"]').click();
  await page.waitForTimeout(500);
  const box = (await page.locator('#map').boundingBox())!;
  for (let i = 0; i < 6; i++) { await page.mouse.move(box.x + box.width * Number(zoomX), box.y + box.height * Number(zoomY)); await page.mouse.wheel(0, -300); await page.waitForTimeout(100); }
  await page.waitForTimeout(1200);
  await page.locator('#map').screenshot({ path: path.join(output, `${name}-flat-zoom.png`) });
  console.log(`wrote ${name}`, await page.locator('#map').getAttribute('data-drainage-line-segments'));
} finally { await app.close(); }
