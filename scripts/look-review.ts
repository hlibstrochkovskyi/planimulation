// Capture before/after visual review screenshots and basic frame/memory metrics.
// Usage: node --import tsx scripts/look-review.ts [OUTPUT_DIR] [seed] [family] [subdivision]
import { strict as assert } from 'node:assert';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';
import { NEW_WORLD_RECIPE, parseRecipe } from '../src/core/recipe';
import type { Recipe } from '../src/core/recipe';
import { decodeClimateNormals, decodeRivers, decodeWorld, NativeSession } from '../src/native/client';

interface TargetPoint {
  name: string;
  regionId: number;
  x: number; // [-1, 1] flat map world X
  y: number; // [-0.5, 0.5] flat map world Y
  latDeg: number;
  lonDeg: number;
}

const FAMILY_ALIASES: Record<string, Recipe['modelVersion']> = {
  v4: 'multiscale-geography-4',
  v5: 'multiscale-geography-5',
};

const [
  output = 'artifacts/look-review',
  seed = NEW_WORLD_RECIPE.seed,
  rawFamily = NEW_WORLD_RECIPE.modelVersion,
  rawSubdivision = String(NEW_WORLD_RECIPE.subdivision),
] = process.argv.slice(2);
const family = (FAMILY_ALIASES[rawFamily] ?? rawFamily) as Recipe['modelVersion'];
const subdivision = Number(rawSubdivision);
const targetRecipe = parseRecipe({ ...NEW_WORLD_RECIPE, seed, modelVersion: family, subdivision });
const needsGenerate = targetRecipe.seed !== NEW_WORLD_RECIPE.seed
  || targetRecipe.modelVersion !== NEW_WORLD_RECIPE.modelVersion
  || targetRecipe.subdivision !== NEW_WORLD_RECIPE.subdivision;
const outputAbs = path.resolve(output);
const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');

function selectTargets(): Promise<TargetPoint[]> {
  return (async () => {
    const session = new NativeSession(executable);
    try {
      const world = decodeWorld(await session.request({ command: 'generate', recipe: targetRecipe }));
      const climate = decodeClimateNormals(await session.request({ command: 'climateNormals' }), world.stats.regionCount, 1);
      const rivers = decodeRivers(await session.request({ command: 'rivers' }), world.stats.regionCount, 1);
      const { surface, terrain, water } = world;
      const n = world.stats.regionCount;
      const posOf = (id: number): Omit<TargetPoint, 'name'> => {
        const cx = surface.centers[id * 3], cy = surface.centers[id * 3 + 1], cz = surface.centers[id * 3 + 2];
        const lon = Math.atan2(cz, cx);
        const lat = Math.asin(Math.max(-1, Math.min(1, cy)));
        return { regionId: id, x: lon / Math.PI, y: lat / Math.PI, latDeg: lat * 180 / Math.PI, lonDeg: lon * 180 / Math.PI };
      };
      const isCentral = (id: number): boolean => {
        const p = posOf(id);
        return Math.abs(p.x) < 0.85 && Math.abs(p.latDeg) < 62;
      };
      const countWetNeighbors = (id: number): number => {
        let count = 0;
        for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
          if (water.bodyIds[surface.neighbors[k]] !== 0) count++;
        }
        return count;
      };
      const hasCoastalNeighbor = (id: number): boolean => countWetNeighbors(id) > 0;

      // 1. Major river mouth & coastline
      let bestMouth = -1, bestMouthFlow = -1;
      for (let id = 0; id < n; id++) {
        if (water.bodyIds[id] !== 0 || !isCentral(id) || !hasCoastalNeighbor(id)) continue;
        const flow = rivers.meanDischarge[id];
        if (flow > bestMouthFlow) { bestMouthFlow = flow; bestMouth = id; }
      }

      // 2. High mountain belt near coast / relief contrast (distinct from point 1)
      const p1 = posOf(bestMouth >= 0 ? bestMouth : 0);
      let bestMountain = -1, bestMountainScore = -1;
      for (let id = 0; id < n; id++) {
        if (water.bodyIds[id] !== 0 || !isCentral(id)) continue;
        const p = posOf(id);
        if (Math.hypot(p.x - p1.x, p.y - p1.y) < 0.25) continue;
        let maxRelief = 0;
        for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
          maxRelief = Math.max(maxRelief, Math.abs(terrain.elevation[id] - terrain.elevation[surface.neighbors[k]]));
        }
        const elev = terrain.elevation[id] - water.levelMeters;
        const score = elev * 0.6 + maxRelief * 1.4 + (hasCoastalNeighbor(id) ? 600 : 0);
        if (score > bestMountainScore) { bestMountainScore = score; bestMountain = id; }
      }

      // 3. Biome transition / inland river valley (distinct from points 1 & 2)
      const p2 = posOf(bestMountain >= 0 ? bestMountain : 1);
      let bestTransition = -1, bestTransitionScore = -1;
      for (let id = 0; id < n; id++) {
        if (water.bodyIds[id] !== 0 || !isCentral(id)) continue;
        const p = posOf(id);
        if (Math.hypot(p.x - p1.x, p.y - p1.y) < 0.25 || Math.hypot(p.x - p2.x, p.y - p2.y) < 0.25) continue;
        const biomes = new Set<number>([climate.biome[id]]);
        let wetNeighbors = 0;
        for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
          const nb = surface.neighbors[k];
          if (water.bodyIds[nb] !== 0) wetNeighbors++;
          else biomes.add(climate.biome[nb]);
        }
        const flowBonus = Math.log1p(rivers.meanDischarge[id]) * 0.35;
        const score = biomes.size * 2.5 + flowBonus + (wetNeighbors > 0 ? 1.5 : 0);
        if (score > bestTransitionScore) { bestTransitionScore = score; bestTransition = id; }
      }

      // 4. Small island or narrow coastal peninsula (high wet-neighbor ratio with relief)
      let bestIsland = -1, bestIslandScore = -1;
      for (let id = 0; id < n; id++) {
        if (water.bodyIds[id] !== 0 || !isCentral(id)) continue;
        const wetN = countWetNeighbors(id);
        const deg = surface.neighborOffsets[id + 1] - surface.neighborOffsets[id];
        if (wetN < deg - 2) continue;
        const elev = terrain.elevation[id] - water.levelMeters;
        // Prefer small islands (1-4 cells) or crisp peninsulas with visible topography.
        const score = wetN * 400 + Math.min(1800, elev) + (wetN === deg ? 250 : 0);
        if (score > bestIslandScore) { bestIslandScore = score; bestIsland = id; }
      }

      return [
        { name: 'mouth', ...posOf(bestMouth >= 0 ? bestMouth : 0) },
        { name: 'mountains', ...posOf(bestMountain >= 0 ? bestMountain : 1) },
        { name: 'transition', ...posOf(bestTransition >= 0 ? bestTransition : 2) },
        { name: 'island', ...posOf(bestIsland >= 0 ? bestIsland : (bestMouth >= 0 ? bestMouth : 0)) },
      ];
    } finally {
      session.close();
    }
  })();
}

const targets = await selectTargets();
await mkdir(outputAbs, { recursive: true });
const temporary = await mkdtemp(path.join(os.tmpdir(), 'planimulation-look-review-'));
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;

const app = await electron.launch({ args: ['.', `--user-data-dir=${temporary}`], env, timeout: 45_000 });
const screenshots: string[] = [];
const errors: string[] = [];

try {
  const page = await app.firstWindow();
  page.on('pageerror', error => errors.push(`pageerror: ${error.message}`));
  page.on('console', message => {
    if (message.type() === 'error') errors.push(`console.error: ${message.text()}`);
  });
  page.setDefaultTimeout(60_000);
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.emulateMedia({ reducedMotion: 'reduce' });

  const tStart = Date.now();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 180_000 });
  let worldReadyMs = Date.now() - tStart;
  if (needsGenerate) {
    await page.locator('[data-dialog="world-dialog"]').click();
    await page.locator('#generator-family').selectOption(targetRecipe.modelVersion);
    await page.locator('#subdivision').selectOption(String(targetRecipe.subdivision));
    await page.locator('#seed').fill(targetRecipe.seed);
    const tGen = Date.now();
    await page.locator('#generate').click();
    await expect(page.locator('body')).toHaveAttribute('data-state', 'generating', { timeout: 30_000 }).catch(() => {});
    await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 600_000 });
    worldReadyMs = Date.now() - tGen;
  }

  const canvas = page.locator('#map');
  await expect(canvas).toHaveAttribute('data-river-line-mode', 'discharge', { timeout: 180_000 });
  await expect(page.locator('[data-layer="vegetationState"]')).toBeEnabled({ timeout: 180_000 });
  const fingerprint = (await page.locator('#fingerprint').textContent()) ?? '';
  const generationTimeText = (await page.locator('#generation-time').textContent()) ?? '';

  // Hide inspector and layers pane for maximum viewport detail during planet captures.
  const toggleLayers = page.locator('#toggle-layers');
  const toggleInspector = page.locator('#toggle-inspector');
  if (await toggleInspector.getAttribute('aria-expanded') === 'true') await toggleInspector.click();
  if (await toggleLayers.getAttribute('aria-expanded') === 'true') await toggleLayers.click();

  const waitFrames = async (count = 4): Promise<void> => {
    await page.evaluate(`new Promise((resolve) => {
      let left = ${count};
      function step() { if (--left <= 0) resolve(); else requestAnimationFrame(step); }
      requestAnimationFrame(step);
    })`);
  };

  const capture = async (fileName: string, fullPage = false): Promise<void> => {
    await waitFrames(5);
    await page.waitForTimeout(150);
    const fullPath = path.join(outputAbs, fileName);
    if (fullPage) await page.screenshot({ path: fullPath });
    else await canvas.screenshot({ path: fullPath });
    screenshots.push(fullPath);
  };

  // 1. Atlas full view (surface)
  await capture('01-atlas-full.png', true);

  // 2. Atlas close-ups at 3 points (8 wheel steps each)
  for (let idx = 0; idx < targets.length; idx++) {
    const target = targets[idx];
    await page.locator('#reset-view').click();
    await waitFrames(4);
    const box = await canvas.boundingBox();
    assert.ok(box, 'Expected flat map bounding box.');
    const halfHeight = Math.max(0.6, 1.1 / (box.width / box.height));
    const relX = Math.round(box.width * (0.5 + target.x / (2 * halfHeight * box.width / box.height)));
    const relY = Math.round(box.height * (0.5 - target.y / (2 * halfHeight)));
    const px = box.x + relX;
    const py = box.y + relY;
    await page.mouse.move(px, py, { steps: 3 });
    await waitFrames(2);
    for (let step = 0; step < 8; step++) {
      await page.evaluate(`(() => {
        const el = document.querySelector('#map');
        el.dispatchEvent(new WheelEvent('wheel', { deltaY: -260, clientX: ${px}, clientY: ${py}, bubbles: true, cancelable: true }));
      })()`);
      await waitFrames(2);
    }
    await page.waitForTimeout(300);
    await capture(`02-atlas-closeup-${idx + 1}-${target.name}.png`);
  }
  await page.locator('#reset-view').click();
  await waitFrames(3);

  // Measure frame time under zoom/pan
  const measureRaf = async (): Promise<{ medianMs: number; p95Ms: number; maxMs: number; fpsMedian: number }> => {
    return await page.evaluate(`(async () => {
      const intervals = [];
      let last = 0;
      const zoomIn = document.querySelector('#zoom-in');
      const zoomOut = document.querySelector('#zoom-out');
      await new Promise((resolve) => {
        requestAnimationFrame(function frame(now) {
          if (last) intervals.push(now - last);
          last = now;
          (intervals.length % 2 ? zoomIn : zoomOut).click();
          if (intervals.length < 90) requestAnimationFrame(frame); else resolve();
        });
      });
      intervals.sort((a, b) => a - b);
      const medianMs = intervals[Math.floor(intervals.length * 0.5)] || 0;
      const p95Ms = intervals[Math.floor(intervals.length * 0.95)] || 0;
      const maxMs = intervals[intervals.length - 1] || 0;
      return { medianMs: Number(medianMs.toFixed(2)), p95Ms: Number(p95Ms.toFixed(2)), maxMs: Number(maxMs.toFixed(2)), fpsMedian: medianMs > 0 ? Number((1000 / medianMs).toFixed(1)) : 0 };
    })()`);
  };
  const flatPerf = await measureRaf();
  await page.locator('#reset-view').click();

  // 3. Biome & Vegetation layers (atlas)
  await toggleLayers.click();
  await page.locator('[data-layer="biome"]').click();
  await toggleLayers.click();
  await capture('03-atlas-biome.png');

  await toggleLayers.click();
  await page.locator('[data-layer="vegetationState"]').click();
  await toggleLayers.click();
  await capture('04-atlas-vegetation.png');

  await toggleLayers.click();
  await page.locator('[data-layer="surface"]').click();
  await toggleLayers.click();

  // 4. Globe at 4 headings (longitudes 0, 90, 180, -90)
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-view', 'globe');
  await waitFrames(6);

  const dragGlobeTo = async (targetLat: number, targetLon: number): Promise<void> => {
    const globeBox = await canvas.boundingBox();
    assert.ok(globeBox);
    const cx = globeBox.x + globeBox.width / 2;
    const cy = globeBox.y + globeBox.height / 2;
    const clampDrag = (v: number): number => Math.max(-180, Math.min(180, v));
    for (let step = 0; step < 22; step++) {
      const text = (await page.locator('#camera-position').textContent()) ?? '';
      const match = text.match(/([\d.]+)° ([NS]) · ([\d.]+)° ([EW])/);
      if (!match) break;
      const lat = Number(match[1]) * (match[2] === 'N' ? 1 : -1);
      const lon = Number(match[3]) * (match[4] === 'E' ? 1 : -1);
      const dLat = targetLat - lat;
      const dLon = ((targetLon - lon + 540) % 360) - 180;
      if (Math.abs(dLat) < 5 && Math.abs(dLon) < 5) break;
      const dx = clampDrag(-dLon * 4.2 * Math.max(0.35, Math.cos(lat * Math.PI / 180)));
      const dy = clampDrag(dLat * 4.2);
      await page.mouse.move(cx, cy);
      await page.mouse.down();
      await page.mouse.move(cx + dx, cy + dy, { steps: 4 });
      await page.mouse.up();
      await canvas.press('Space');
      await waitFrames(2);
    }
  };

  for (const [idx, lon] of [0, 90, 180, -90].entries()) {
    await dragGlobeTo(12, lon);
    await capture(`05-globe-heading-${idx + 1}-lon${lon}.png`);
  }

  // 5. Globe zoomed x3, x6 (~1,250 km), x6 + tilt, x8 + tilt on mountains/coast, and x8 + tilt on island/peninsula
  const zoomTarget = targets[1];
  const islandTarget = targets[3] ?? targets[0];
  await page.locator('#reset-view').click();
  await waitFrames(4);
  await dragGlobeTo(zoomTarget.latDeg, zoomTarget.lonDeg);
  const overviewDist = Number(await canvas.getAttribute('data-view-distance')) || 2.25;

  const zoomToClicks = async (clicks: number): Promise<number> => {
    for (let i = 0; i < clicks; i++) {
      await page.locator('#zoom-in').click();
      await waitFrames(3);
    }
    await page.waitForTimeout(700);
    await waitFrames(4);
    return Number(await canvas.getAttribute('data-view-distance'));
  };

  // Zoom ~3 clicks (~0.67 R)
  const dist3x = await zoomToClicks(3);
  await capture('06-globe-zoom-3x.png');

  // Zoom to 6 clicks (~0.20 R ≈ 1,250 km)
  const dist6x = await zoomToClicks(3);
  await capture('07-globe-zoom-6x.png');

  // Tilt at 6 clicks (~1,250 km)
  await page.locator('#tilt-view').click();
  await page.waitForTimeout(800);
  await waitFrames(4);
  await capture('08-globe-zoom-6x-tilt.png');
  const zoom6Dataset = await canvas.evaluate((el) => ({
    lodTriangles: el.dataset.lodTriangles ?? null,
    lodLevel: el.dataset.lodLevel ?? null,
    exaggeration: el.dataset.exaggeration ?? null,
    viewDistance: el.dataset.viewDistance ?? null,
    tilt: el.dataset.tilt ?? null,
  }));

  // Zoom to 8 clicks (~0.09 R ≈ 560 km) + tilt
  const dist8x = await zoomToClicks(2);
  await capture('09-globe-zoom-8x-tilt.png');
  const zoom8Dataset = await canvas.evaluate((el) => ({
    lodTriangles: el.dataset.lodTriangles ?? null,
    lodLevel: el.dataset.lodLevel ?? null,
    exaggeration: el.dataset.exaggeration ?? null,
    viewDistance: el.dataset.viewDistance ?? null,
    tilt: el.dataset.tilt ?? null,
  }));

  // Measure globe RAF at zoomed-in tilted view (heaviest LOD case)
  const globeZoomPerf = await measureRaf();

  // Island / narrow peninsula at 8 clicks + tilt
  await page.locator('#reset-view').click();
  await waitFrames(4);
  await dragGlobeTo(islandTarget.latDeg, islandTarget.lonDeg);
  const distIsland = await zoomToClicks(8);
  await page.locator('#tilt-view').click();
  await page.waitForTimeout(800);
  await waitFrames(4);
  await capture('10-globe-island-8x-tilt.png');

  await page.locator('#reset-view').click();
  await waitFrames(4);
  const globeOverviewPerf = await measureRaf();

  const dataset = await canvas.evaluate((el) => ({ ...el.dataset }));
  const metrics = await app.evaluate(({ app }) => app.getAppMetrics().map(m => ({ type: m.type, workingSetMB: Number((m.memory.workingSetSize / 1024).toFixed(1)) })));

  const summary = {
    seed: targetRecipe.seed,
    family: targetRecipe.modelVersion,
    subdivision: targetRecipe.subdivision,
    fingerprint,
    worldReadyMs,
    generationTimeText,
    targets,
    distances: { overviewDist, dist3x, dist6x, dist8x, distIsland },
    zoomDataset: zoom6Dataset,
    zoom8Dataset,
    dataset,
    performance: { flat: flatPerf, globeOverview: globeOverviewPerf, globeZoom: globeZoomPerf },
    memory: metrics,
    screenshots,
    errors,
  };
  await writeFile(path.join(outputAbs, 'review-summary.json'), `${JSON.stringify(summary, null, 2)}\n`);
  console.log(JSON.stringify(summary, null, 2));
  assert.deepEqual(errors, [], `Renderer errors: ${errors.join('; ')}`);
} finally {
  await app.close();
  await rm(temporary, { recursive: true, force: true });
}
