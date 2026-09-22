import { strict as assert } from 'node:assert';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';
import { DEFAULT_RECIPE, parseRecipe } from '../src/core/recipe';
import { NativeController } from '../src/native/client';

const temp = await mkdtemp(path.join(os.tmpdir(), 'planimulation-desktop-'));
const recipePath = path.join(temp, 'recipe.json');
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const executablePath = process.argv[2] ? path.resolve(process.argv[2]) : undefined;
const app = await electron.launch({ executablePath, args: [...(executablePath ? [] : ['.']), `--user-data-dir=${temp}`], env, timeout: 30_000 });
try {
  const page = await app.firstWindow();
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => { if (message.type() === 'error' && /THREE|WebGL|shader/i.test(message.text())) errors.push(message.text()); });
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
  await expect(page.locator('#region-count')).toHaveText('10,242');
  const fingerprint = await page.locator('#fingerprint').innerText();
  await expect(page.locator('#legend-title')).toContainText('Continentality');
  await expect(page.locator('#crust-summary')).toContainText('38.00% target');
  const reference = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
  try { assert.equal((await reference.generate(DEFAULT_RECIPE)).world.checksum, fingerprint, 'Headless and desktop use the same native math.'); }
  finally { reference.close(); }
  const desktopData = await page.evaluate(async (recipe) => {
    const result = await window.desktop.generate(recipe);
    await window.desktop.cancelGeneration();
    const id = result.world.tectonics.boundaryCells[0], centers = result.world.surface.centers;
    return { checksum: result.world.checksum, typed: result.world.diagnosticField instanceof Float64Array,
      boundarySample: { id, x: Math.atan2(centers[id * 3 + 2], centers[id * 3]) / Math.PI,
        y: Math.asin(centers[id * 3 + 1]) / Math.PI } };
  }, { ...DEFAULT_RECIPE });
  assert.equal(desktopData.checksum, fingerprint); assert.equal(desktopData.typed, true);
  assert.equal(await page.evaluate(() => typeof (globalThis as unknown as { require?: unknown }).require), 'undefined');
  assert.deepEqual(await page.evaluate(() => Object.keys(window.desktop).sort()), ['acceptWorld', 'advance', 'cancelGeneration', 'generate', 'openRecipe', 'saveRecipe']);

  const canvas = page.locator('#map');
  const bounds = await canvas.boundingBox();
  assert.ok(bounds);
  await canvas.click({ position: { x: bounds.width / 2, y: bounds.height / 2 } });
  await expect(page.locator('#selection-title')).toContainText('Region');
  const selectedRegion = await page.locator('#selection-title').innerText();
  const selectedDetails = await page.locator('#selection-details').innerText();
  await expect(page.locator('#selection-details')).toContainText('Crust thickness');
  await expect(page.locator('#selection-details')).toContainText('kg/m³');
  const crustExplanation = await page.locator('#crust-note').innerText();
  assert.ok(crustExplanation.includes('fitted threshold'));
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-view', 'globe');
  await expect(page.locator('#selection-title')).toHaveText(selectedRegion);
  await expect(page.locator('#selection-details')).toHaveText(selectedDetails, { useInnerText: true });
  await expect(page.locator('#crust-note')).toHaveText(crustExplanation);
  await canvas.click({ position: { x: bounds.width / 2, y: bounds.height / 2 } });
  await expect(page.locator('#selection-title')).toHaveText(selectedRegion);
  await expect(page.locator('button[data-view="globe"]')).toHaveAttribute('aria-pressed', 'true');
  await mkdir('artifacts', { recursive: true });
  await page.screenshot({ path: executablePath ? 'artifacts/globe-desktop-packaged.png' : 'artifacts/globe-desktop.png' });
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-view', 'flat');
  await page.locator('[data-layer="thickness"]').click();
  await expect(page.locator('#legend-low')).toHaveText('7 km');
  await expect(page.locator('#legend-high')).toHaveText('35 km');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('[data-layer="boundaries"]').click();
  await expect(page.locator('#legend-title')).toContainText('Boundary motion');
  await expect(page.locator('#boundary-legend')).toBeVisible();
  await expect(page.locator('#selection-details')).toContainText('Tectonic plate');
  await expect(page.locator('#selection-details')).toContainText('cm/year');
  await page.getByRole('button', { name: 'Fit map' }).click();
  const boundaryBounds = await canvas.boundingBox();
  assert.ok(boundaryBounds);
  const halfHeight = Math.max(.6, 1.1 / (boundaryBounds.width / boundaryBounds.height));
  await canvas.click({ position: {
    x: boundaryBounds.width * (.5 + desktopData.boundarySample.x / (2 * halfHeight * boundaryBounds.width / boundaryBounds.height)),
    y: boundaryBounds.height * (.5 - desktopData.boundarySample.y / (2 * halfHeight)),
  } });
  await expect(page.locator('#selection-title')).toHaveText(`Region ${desktopData.boundarySample.id.toLocaleString('en')}`);
  await expect(page.locator('#boundary-details')).toContainText('opening');
  await expect(page.locator('#boundary-details')).toContainText('shear');
  await page.locator('#boundaries').check();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('#boundaries').uncheck();
  await page.screenshot({ path: executablePath ? 'artifacts/boundaries-desktop-packaged.png' : 'artifacts/boundaries-desktop.png' });
  await page.locator('[data-layer="speed"]').click();
  await expect(page.locator('#legend-high')).toHaveText('8 cm/year');
  await expect(page.locator('#boundary-legend')).toBeHidden();
  await page.getByRole('button', { name: 'Zoom in' }).click();
  await page.getByRole('button', { name: 'Fit map' }).click();
  await page.locator('[data-layer="area"]').click();
  await expect(page.locator('#legend-title')).toContainText('Region area');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('[data-layer="signal"]').click();
  await page.locator('#play').click();
  await expect(page.locator('#diagnostic-tick')).not.toHaveText('Step 0');
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await page.getByRole('button', { name: 'Pause diagnostic', exact: true }).click();
  await expect(page.locator('#status')).toContainText('relative mass error');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.getByRole('button', { name: '2D map', exact: true }).click();

  // Stub only native dialogs; still exercise the real bridge, validation, and filesystem handlers.
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, recipePath);
  await page.getByRole('button', { name: 'Save recipe' }).click();
  await expect(page.locator('#status')).toContainText('Recipe saved');
  assert.deepEqual(parseRecipe(JSON.parse(await readFile(recipePath, 'utf8'))), DEFAULT_RECIPE);

  await page.locator('#seed').fill('desktop-roundtrip');
  await page.locator('#subdivision').selectOption('3');
  await page.locator('#plate-count').fill('7');
  await page.locator('#plate-speed').fill('0');
  await page.locator('#continental-fraction').fill('0');
  await page.locator('#continental-scale').fill('2');
  await page.locator('#generate').click();
  await expect(page.locator('#world-name')).toHaveText('desktop-roundtrip');
  await expect(page.locator('#region-count')).toHaveText('642');
  await expect(page.locator('#crust-summary')).toContainText('0.00% actual / 0.00% target');
  await page.locator('[data-layer="plates"]').click();
  await expect(page.locator('#legend-title')).toContainText('7 connected plates');
  await page.locator('[data-layer="speed"]').click();
  await expect(page.locator('#legend-high')).toHaveText('0 cm/year');
  assert.notEqual(await page.locator('#fingerprint').innerText(), fingerprint);
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });
  await expect(page.locator('#plate-count')).toHaveValue('12');
  await expect(page.locator('#plate-speed')).toHaveValue('8');
  await expect(page.locator('#continental-fraction')).toHaveValue('38');
  await expect(page.locator('#continental-scale')).toHaveValue('1');

  await writeFile(recipePath, JSON.stringify({ ...DEFAULT_RECIPE, modelVersion: 'future' }));
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#status')).toContainText('Unsupported recipe version');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);

  // Queue replacement jobs synchronously; obsolete native jobs cannot publish results.
  await page.evaluate(() => {
    const seed = document.querySelector<HTMLInputElement>('#seed')!;
    const level = document.querySelector<HTMLSelectElement>('#subdivision')!;
    const form = document.querySelector<HTMLFormElement>('#recipe-form')!;
    seed.value = 'obsolete'; level.value = '6'; form.requestSubmit();
    seed.value = 'latest'; level.value = '2'; form.requestSubmit();
  });
  await expect(page.locator('#world-name')).toHaveText('latest');
  const latestHash = await page.locator('#fingerprint').innerText();
  await page.evaluate(() => {
    document.querySelector<HTMLSelectElement>('#subdivision')!.value = '6';
    document.querySelector<HTMLFormElement>('#recipe-form')!.requestSubmit();
    document.querySelector<HTMLButtonElement>('#cancel')!.click();
  });
  await expect(page.locator('#status')).toContainText('canceled');
  await expect(page.locator('#fingerprint')).toHaveText(latestHash);
  await page.locator('#play').click();
  await expect(page.locator('#diagnostic-tick')).not.toHaveText('Step 0');
  await page.locator('#play').click();
  await page.locator('#seed').fill(DEFAULT_RECIPE.seed);
  await page.locator('#subdivision').selectOption(String(DEFAULT_RECIPE.subdivision));
  await page.locator('#generate').click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });
  await canvas.click({ position: { x: bounds.width * .6, y: bounds.height * .5 } });
  await page.locator('[data-layer="crust"]').click();
  await mkdir('artifacts', { recursive: true });
  const screenshot = executablePath ? 'artifacts/surface-desktop-packaged.png' : 'artifacts/surface-desktop.png';
  await page.screenshot({ path: screenshot });
  assert.deepEqual(errors, [], 'No renderer exceptions.');
  console.log(`Desktop checks passed. Desktop fingerprint: ${fingerprint}. Screenshot: ${screenshot}`);
} finally {
  await app.close();
}
