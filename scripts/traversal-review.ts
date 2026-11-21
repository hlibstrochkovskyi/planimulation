// Capture both seasons and projections for four terrain origins.
// Usage: node --import tsx scripts/traversal-review.ts [OUTPUT_DIR] [SEED]
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';
import { BIOMES } from '../src/core/climate-normals';
import { NEW_WORLD_RECIPE } from '../src/core/recipe';
import { traceTravelRoute } from '../src/core/traversal';
import type { TravelSeason } from '../src/core/traversal';
import { NativeSession, decodeTraversalCosts, decodeTraversalField, decodeVegetation, decodeWorld } from '../src/native/client';

const [output = 'artifacts/traversal-review', seed = 'first-light'] = process.argv.slice(2);
const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const session = new NativeSession(executable);
const recipe = { ...NEW_WORLD_RECIPE, seed };
const samples = await (async () => {
  try {
    const world = decodeWorld(await session.request({ command: 'generate', recipe }));
    const n = world.stats.regionCount, { centers, neighborOffsets, neighbors } = world.surface;
    const vegetation = decodeVegetation(await session.request({ command: 'initializeVegetation' }), n, 1);
    const costs = decodeTraversalCosts(await session.request({ command: 'traversalCosts' }), n, 1);
    const field = async (origin: number, season: TravelSeason) =>
      decodeTraversalField(await session.request({ command: 'travelTime', season, sources: [origin] }), n, 1);
    const coordinates = (id: number) => ({
      id, x: Math.atan2(centers[id * 3 + 2], centers[id * 3]) / Math.PI,
      y: Math.asin(centers[id * 3 + 1]) / Math.PI,
    });
    const seen = new Uint8Array(n);
    let land: number[] = [];
    for (let start = 0; start < n; start++) {
      if (seen[start] || world.water.bodyIds[start]) continue;
      const component = [start];
      seen[start] = 1;
      for (const u of component) {
        for (let k = neighborOffsets[u]; k < neighborOffsets[u + 1]; k++) {
          const v = neighbors[k];
          if (!seen[v] && !world.water.bodyIds[v]) { seen[v] = 1; component.push(v); }
        }
      }
      if (component.length > land.length) land = component;
    }
    const results: Array<{
      kind: string; origin: ReturnType<typeof coordinates>; target: ReturnType<typeof coordinates>;
      label: string; relief: number; season: Record<TravelSeason, { days: number; hops: number }>;
    }> = [];
    for (const kind of ['grassland', 'forest', 'mountains', 'coast'] as const) {
      const candidates = land.filter(id => {
        const { x, y } = coordinates(id);
        if (Math.abs(x) > .83 || Math.abs(y) > .37 || results.some(({ origin }) => {
          const a = origin.id * 3, b = id * 3;
          return centers[a] * centers[b] + centers[a + 1] * centers[b + 1] + centers[a + 2] * centers[b + 2] > .988;
        })) return false;
        const label = BIOMES[vegetation.label[id]].name;
        if (kind === 'grassland') return /grassland/i.test(label) && costs.localReliefMeters[id] < 100 && vegetation.woody[id] < .15;
        if (kind === 'forest') return /forest/i.test(label) && vegetation.woody[id] > .65;
        if (kind === 'mountains') return costs.localReliefMeters[id] >= 600;
        return neighbors.subarray(neighborOffsets[id], neighborOffsets[id + 1])
          .some(v => world.water.bodyIds[v] === world.water.mainOceanId);
      });
      const score = (id: number): number => {
        const { x, y } = coordinates(id);
        return 2 * Math.abs(x - .14) + 3 * Math.abs(y - .17)
          - (kind === 'forest' ? 2 * (1 - costs.snowFactorWinter[id]) : 0)
          - (kind === 'mountains' ? costs.localReliefMeters[id] / 600 : 0);
      };
      candidates.sort((a, b) => score(a) - score(b) || a - b);
      const origin = candidates[0];
      if (origin === undefined) throw new Error(`No suitable ${kind} origin for ${seed}.`);
      const summer = await field(origin, 'summer'), winter = await field(origin, 'winter');
      const start = coordinates(origin);
      const targets = land.filter(id => {
        const { x, y } = coordinates(id);
        return summer.days[id] > 40 && summer.days[id] < 230 && Math.abs(x) < .9
          && Math.abs(x - start.x) < .25 && Math.abs(y - start.y) < .22
          && summer.distanceMeters[id] > 1.8e6 && summer.distanceMeters[id] < 4.4e6;
      }).sort((a, b) => Math.abs(summer.days[a] - 105) - Math.abs(summer.days[b] - 105) || a - b);
      const target = targets[0];
      if (target === undefined) throw new Error(`No nearby target for ${kind} origin ${origin}.`);
      results.push({
        kind, origin: start, target: coordinates(target),
        label: BIOMES[vegetation.label[origin]].name, relief: costs.localReliefMeters[origin],
        season: {
          summer: { days: summer.days[target], hops: traceTravelRoute(summer, target)!.length - 1 },
          winter: { days: winter.days[target], hops: traceTravelRoute(winter, target)!.length - 1 },
        },
      });
    }
    return results;
  } finally {
    session.close();
  }
})();

await mkdir(output, { recursive: true });
const temporary = await mkdtemp(path.join(os.tmpdir(), 'planimulation-traversal-review-'));
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const app = await electron.launch({ args: ['.', `--user-data-dir=${temporary}`], env, timeout: 30_000 });
const errors: string[] = [];
try {
  const page = await app.firstWindow();
  page.setDefaultTimeout(30_000);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.setViewportSize({ width: 1640, height: 1000 });
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error' && /shader|webgl|three/i.test(message.text())) errors.push(message.text());
  });
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 90_000 });
  if (seed !== 'first-light') {
    await page.locator('[data-dialog="world-dialog"]').click();
    await page.locator('#seed').fill(seed);
    await page.locator('#generate').click();
    await expect(page.locator('#region-count')).toHaveText('40,962');
    await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 90_000 });
  }
  const map = page.locator('#map');
  const image = (name: string) => path.join(output, `${seed}-${name}.png`);
  const capture = async (name: string) => {
    await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    await page.locator('.workspace').screenshot({ path: image(name) });
  };
  const clickRegion = async ({ x, y, id }: { x: number; y: number; id: number }) => {
    const box = (await map.boundingBox())!;
    const pixels = box.height / (2 * Math.max(.6, 1.1 / (box.width / box.height)));
    await page.mouse.click(box.x + box.width / 2 + x * pixels, box.y + box.height / 2 - y * pixels);
    await expect(page.locator('#selection-title')).toHaveText(`Region ${id.toLocaleString('en')}`);
  };
  await page.locator('#collapse-dock').click();
  const travelCost = page.locator('[data-layer="travelCost"]');
  await expect(travelCost).toBeEnabled();
  await travelCost.click();
  if (await page.locator('#toggle-inspector').getAttribute('aria-expanded') === 'false') await page.locator('#toggle-inspector').click();

  await capture('cost-summer-flat');
  await page.locator('#traversal-winter').click();
  await capture('cost-winter-flat');
  await page.locator('#traversal-summer').click();
  for (const sample of samples) {
    await page.locator('button[data-view="flat"]').click();
    await page.locator('#reset-view').click();
    await page.locator('#traversal-summer').click();
    await clickRegion(sample.origin);
    await page.locator('#travel-from-here').click();
    await expect(map).toHaveAttribute('data-travel-origin', String(sample.origin.id));
    await clickRegion(sample.target);
    for (const season of ['summer', 'winter'] as const) {
      await page.locator(`#traversal-${season}`).click();
      await expect(page.locator('#legend-title')).toContainText(`· ${season} ·`);
      await expect(map).toHaveAttribute('data-travel-route-hops', String(sample.season[season].hops));
      await capture(`${sample.kind}-time-${season}-flat`);

      await page.locator('button[data-view="globe"]').click();
      await page.locator('#traversal-frame').click();
      await capture(`${sample.kind}-time-${season}-globe`);
      if (sample.kind === 'forest' && season === 'winter') {
        await page.locator('#tab-factors').click();
        await page.locator('#traversal-note').scrollIntoViewIfNeeded();
        await expect(page.locator('#traversal-details')).toContainText(`${sample.season.winter.days.toFixed(1)} days`);
        await page.screenshot({ path: image('window-factors') });
      }
      if (sample.kind === 'forest') {
        await travelCost.click();
        await capture(`cost-${season}-globe`);
        await page.locator('[data-layer="travelTime"]').click();
      }
      await page.locator('button[data-view="flat"]').click();
    }
    await page.locator('#traversal-clear-source').click();
    console.log(JSON.stringify(sample));
  }
  if (errors.length) throw new Error(errors.join('\n'));
} finally {
  await app.close();
  await rm(temporary, { recursive: true, force: true });
}
