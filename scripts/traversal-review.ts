// Capture traversal-1 layers, summer/winter contrast, isochrones, least-time route
// polyline on flat map and globe, and the inspector breakdown for visual review.
// Usage: node --import tsx scripts/traversal-review.ts [OUTPUT_DIR] [seed]
import { mkdir, mkdtemp } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';

const [output = 'artifacts/traversal-review', seed = 'first-light'] = process.argv.slice(2);
await mkdir(output, { recursive: true });
const temporary = await mkdtemp(path.join(os.tmpdir(), 'planimulation-traversal-review-'));
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const app = await electron.launch({ args: ['.', `--user-data-dir=${temporary}`], env, timeout: 30_000 });
try {
  const page = await app.firstWindow();
  page.setDefaultTimeout(120_000);
  page.on('pageerror', error => console.error('page error:', error.message));
  await page.setViewportSize({ width: 1600, height: 1000 });
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 90_000 });
  if (seed !== 'first-light') {
    await page.locator('[data-dialog="world-dialog"]').click();
    await page.locator('#seed').fill(seed);
    await page.locator('#generate').click();
    await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 300_000 });
  }
  await expect(page.locator('[data-layer="travelCost"]')).toBeEnabled({ timeout: 60_000 });

  // 1. Travel cost — Summer and Winter on flat map.
  await page.locator('[data-layer="travelCost"]').click();
  await page.waitForTimeout(800);
  await page.locator('#map').screenshot({ path: path.join(output, `${seed}-cost-summer-flat.png`) });
  await page.locator('#traversal-winter').click();
  await page.waitForTimeout(800);
  await page.locator('#map').screenshot({ path: path.join(output, `${seed}-cost-winter-flat.png`) });
  await page.locator('#traversal-summer').click();
  await page.waitForTimeout(600);

  // 2. Pick a passable land origin, compute Travel time, then pick a distant reachable target to draw a route.
  const box = (await page.locator('#map').boundingBox())!;
  let originClicked = false;
  for (let attempt = 0; attempt < 50; attempt++) {
    const x = box.x + box.width * (0.18 + 0.64 * ((attempt * 0.618) % 1));
    const y = box.y + box.height * (0.25 + 0.50 * ((attempt * 0.382) % 1));
    await page.mouse.click(x, y);
    await page.waitForTimeout(250);
    if (await page.locator('#travel-from-here').isEnabled()) {
      await page.locator('#travel-from-here').click();
      await expect(page.locator('#map')).toHaveAttribute('data-travel-origin', /^\d+$/, { timeout: 30_000 });
      originClicked = true;
      break;
    }
  }
  if (!originClicked) throw new Error('Could not find a passable land origin on the map.');

  // Pick a target on the same landmass with a multi-hop route (>= 6 edges).
  for (let attempt = 0; attempt < 60; attempt++) {
    const x = box.x + box.width * (0.15 + 0.70 * (((attempt + 11) * 0.7549) % 1));
    const y = box.y + box.height * (0.20 + 0.60 * (((attempt + 7) * 0.5698) % 1));
    await page.mouse.click(x, y);
    await page.waitForTimeout(250);
    const hops = Number(await page.locator('#map').getAttribute('data-travel-route-hops') ?? '0');
    if (hops >= 6) break;
  }
  await page.waitForTimeout(800);
  await page.locator('#map').screenshot({ path: path.join(output, `${seed}-time-summer-route-flat.png`) });

  // Switch to Factors tab and capture full window with inspector breakdown.
  await page.locator('#tab-factors').click();
  await page.waitForTimeout(500);
  await page.screenshot({ path: path.join(output, `${seed}-window-factors.png`) });

  // Switch to Winter and capture winter isochrones + shifted route.
  await page.locator('#traversal-winter').click();
  await page.waitForTimeout(1000);
  await page.locator('#map').screenshot({ path: path.join(output, `${seed}-time-winter-route-flat.png`) });

  // Switch to Globe projection, rotate to face the origin & route, and capture both seasons.
  await page.locator('[data-view="globe"]').click();
  await page.waitForTimeout(800);
  const gbox = (await page.locator('#map').boundingBox())!;
  const gx = gbox.x + gbox.width * 0.5, gy = gbox.y + gbox.height * 0.5;
  await page.mouse.move(gx, gy);
  await page.mouse.down();
  await page.mouse.move(gx + gbox.width * 0.28, gy + gbox.height * 0.24, { steps: 16 });
  await page.mouse.up();
  await page.waitForTimeout(1000);
  await page.locator('#map').screenshot({ path: path.join(output, `${seed}-time-winter-route-globe.png`) });
  await page.locator('#traversal-summer').click();
  await page.waitForTimeout(1000);
  await page.locator('#map').screenshot({ path: path.join(output, `${seed}-time-summer-route-globe.png`) });

  console.log('Origin:', await page.locator('#map').getAttribute('data-travel-origin'));
  console.log('Route hops:', await page.locator('#map').getAttribute('data-travel-route-hops'));
  console.log('Traversal note:', await page.locator('#traversal-note').textContent());
  console.log('Traversal details:', await page.locator('#traversal-details').textContent());
} finally {
  await app.close();
}
