// Capture the global atlas, globe and an inland terminal balance on the desktop.
// Usage: node --import tsx scripts/rivers-review.ts [OUTPUT_DIR] [seed]
import { strict as assert } from 'node:assert';
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';
import { NEW_WORLD_RECIPE } from '../src/core/recipe';
import { decodeRivers, decodeWorld, NativeSession } from '../src/native/client';

const [output = 'artifacts/rivers-2-review', seed = NEW_WORLD_RECIPE.seed] = process.argv.slice(2);
const executable = path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core');
const session = new NativeSession(executable);
let terminal: { id: number; x: number; y: number };
let focus: { x: number; y: number };
try {
  const world = decodeWorld(await session.request({ command: 'generate', recipe: { ...NEW_WORLD_RECIPE, seed } }));
  const rivers = decodeRivers(await session.request({ command: 'rivers' }), world.stats.regionCount, 1);
  const centers = world.surface.centers;
  const position = (id: number) => ({
    x: Math.atan2(centers[id * 3 + 2], centers[id * 3]) / Math.PI,
    y: Math.asin(centers[id * 3 + 1]) / Math.PI,
  });
  const candidates = [...rivers.lakes.values()].map(({ region: id, spill }) => ({ id, spill, ...position(id) }))
    .filter(({ x, y }) => Math.abs(x) < .95 && Math.abs(y) < .45);
  candidates.sort((a, b) => b.spill - a.spill);
  assert.ok(candidates.length, 'Expected a visible inland terminal.');
  terminal = candidates[0];
  // Face the hemisphere with the largest cluster of major ocean mouths.
  const mouths = Array.from(rivers.meanDischarge, (flow, id) => ({ id, flow }))
    .filter(({ id, flow }) => world.water.mainOceanId > 0 && world.water.bodyIds[id] === world.water.mainOceanId && flow > 0)
    .sort((a, b) => b.flow - a.flow).slice(0, 20);
  assert.ok(mouths.length > 0, 'Expected an ocean mouth.');
  const score = (id: number): number => mouths.reduce((sum, mouth) => {
    let cosine = 0;
    for (let axis = 0; axis < 3; axis++) cosine += centers[id * 3 + axis] * centers[mouth.id * 3 + axis];
    return sum + Math.max(0, cosine - .5);
  }, 0);
  const front = mouths.reduce((best, mouth) => score(mouth.id) > score(best.id) ? mouth : best);
  focus = position(front.id);
} finally { session.close(); }

await mkdir(output, { recursive: true });
const temporary = await mkdtemp(path.join(os.tmpdir(), 'planimulation-rivers-review-'));
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const app = await electron.launch({ args: ['.', `--user-data-dir=${temporary}`], env, timeout: 30_000 });
try {
  const page = await app.firstWindow();
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error' && /THREE|WebGL|shader/i.test(message.text())) errors.push(message.text()); });
  page.setDefaultTimeout(45_000);
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 90_000 });
  if (seed !== NEW_WORLD_RECIPE.seed) {
    await page.locator('[data-dialog="world-dialog"]').click();
    await page.locator('#seed').fill(seed);
    await page.locator('#generate').click();
    await expect(page.locator('#world-name')).toHaveText(seed, { timeout: 120_000 });
    await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 120_000 });
  }
  if (await page.locator('#toggle-layers').getAttribute('aria-expanded') === 'false') await page.locator('#toggle-layers').click();
  const inspector = page.locator('#toggle-inspector');
  if (await inspector.getAttribute('aria-expanded') === 'true') await inspector.click();
  const canvas = page.locator('#map');
  const fingerprint = await page.locator('#fingerprint').textContent() ?? '';
  await expect(canvas).toHaveAttribute('data-river-line-mode', 'discharge');
  await expect(canvas).toHaveAttribute('data-drainage-line-segments', /^[1-9]\d*$/);
  const capture = async (name: string): Promise<void> => {
    await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    await page.screenshot({ path: path.join(output, `${seed}-${name}.png`) });
  };
  await capture('surface-atlas');
  const runoff = page.locator('[data-layer="annualRunoff"]');
  await expect(runoff).toBeEnabled();
  await runoff.click();
  await expect(page.locator('#legend-high')).toHaveText('≥2,000 mm/year');
  await capture('runoff-atlas');

  await page.locator('[data-layer="riverDischarge"]').click();
  await expect(page.locator('#legend-high')).toHaveText('≥30,000 m³/s');
  await capture('discharge-atlas');
  await inspector.click();
  await expect(inspector).toHaveAttribute('aria-expanded', 'true');
  const box = await canvas.boundingBox();
  assert.ok(box);
  const halfHeight = Math.max(.6, 1.1 / (box.width / box.height));
  await canvas.click({ position: {
    x: box.width * (.5 + terminal.x / (2 * halfHeight * box.width / box.height)),
    y: box.height * (.5 - terminal.y / (2 * halfHeight)),
  } });
  await expect(page.locator('#selection-title')).toHaveText(`Region ${terminal.id.toLocaleString('en')}`);
  await expect(page.locator('#primary-region-details')).toContainText('Terminal lake evaporation');
  await expect(page.locator('#primary-region-details')).toContainText('Terminal lake spill');
  await capture('terminal-balance');

  const details = await page.locator('#primary-region-details').textContent() ?? '';
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(page.locator('#primary-region-details')).toHaveText(details);
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await inspector.click();
  const globeBox = await canvas.boundingBox();
  assert.ok(globeBox);
  const center = { x: globeBox.x + globeBox.width / 2, y: globeBox.y + globeBox.height / 2 };
  const clampDrag = (value: number): number => Math.max(-170, Math.min(170, value));
  for (let step = 0; step < 16; step++) {
    const position = (await page.locator('#camera-position').textContent())?.match(/([\d.]+)° ([NS]) · ([\d.]+)° ([EW])/);
    assert.ok(position, 'Expected globe camera coordinates.');
    const latitude = Number(position[1]) * (position[2] === 'N' ? 1 : -1);
    const longitude = Number(position[3]) * (position[4] === 'E' ? 1 : -1);
    const deltaLat = focus.y * 180 - latitude;
    const deltaLon = (focus.x * 180 - longitude + 540) % 360 - 180;
    if (Math.abs(deltaLat) < 8 && Math.abs(deltaLon) < 8) break;
    const dx = clampDrag(-deltaLon * 5 * Math.cos(latitude * Math.PI / 180)), dy = clampDrag(deltaLat * 5);
    await page.mouse.move(center.x, center.y);
    await page.mouse.down();
    await page.mouse.move(center.x + dx, center.y + dy, { steps: 4 });
    await page.mouse.up();
    await canvas.press('Space');
  }
  const discharge = page.locator('[data-layer="riverDischarge"]');
  await discharge.focus();
  await capture('discharge-globe');
  const withRivers = await canvas.screenshot();
  const lines = page.locator('#drainage-lines');
  await page.locator('#display-trigger').click();
  await lines.uncheck();
  await page.keyboard.press('Escape');
  await discharge.focus();
  assert.notDeepEqual(await canvas.screenshot(), withRivers, 'River lines must change the rendered globe.');
  await page.locator('#display-trigger').click();
  await lines.check();
  await page.keyboard.press('Escape');
  await runoff.click();
  await capture('runoff-globe');
  await page.locator('[data-layer="surface"]').click();
  await capture('surface-globe');
  assert.deepEqual(errors, []);
} finally {
  await app.close();
  await rm(temporary, { recursive: true, force: true });
}
