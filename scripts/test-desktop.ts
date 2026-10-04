import { strict as assert } from 'node:assert';
import { mkdir, mkdtemp, open, readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';
import { CONTINUOUS_PLATES_MODEL_VERSION, DEFAULT_RECIPE, parseRecipe } from '../src/core/recipe';
import { NativeController } from '../src/native/client';
import { basinTree } from '../src/core/basins';
import { MAX_SEASONAL_CHECKPOINT_BYTES } from '../src/shared/seasonal-checkpoint';

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
  await expect(page.locator('#legend-title')).toContainText('Land and water surface');
  await expect(page.locator('#water-summary')).toContainText('71.00% target');
  await expect(page.locator('#drainage-summary')).toContainText('closed dry sinks');
  await expect(page.locator('#basin-summary')).toContainText('224 minima · 447 hierarchy branches');
  await expect(page.locator('#terrain-preparation-passes')).toHaveValue('4');
  await expect(page.locator('#height-summary')).toContainText('dry preparation 4 / 4 passes');
  await expect(page.locator('#crust-summary')).toContainText('38.00% target');
  const reference = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
  try { assert.equal((await reference.generate(DEFAULT_RECIPE)).world.checksum, fingerprint, 'Headless and desktop use the same native math.'); }
  finally { reference.close(); }
  const desktopData = await page.evaluate(async (recipe) => {
    const result = await window.desktop.generate(recipe);
    await window.desktop.cancelGeneration();
    const id = result.world.tectonics.boundaryCells[0], centers = result.world.surface.centers;
    const routed = result.world.drainage.receivers.findIndex((receiver, region) => receiver !== region && result.world.drainage.flatSteps[region] === 0);
    const basinRegion = result.world.basins.regionNodes.findIndex((node, id) => result.world.basins.parents[node] !== node
      && Math.abs(Math.atan2(centers[id * 3 + 2], centers[id * 3]) / Math.PI) < 0.8 && Math.abs(centers[id * 3 + 1]) < 0.7);
    const basinNode = result.world.basins.regionNodes[basinRegion];
    return { checksum: result.world.checksum, typed: result.world.diagnosticField instanceof Float64Array,
      basinSample: { id: basinRegion, node: basinNode, parent: result.world.basins.parents[basinNode],
        from: result.world.basins.spillFrom[basinNode], to: result.world.basins.spillTo[basinNode],
        x: Math.atan2(centers[basinRegion * 3 + 2], centers[basinRegion * 3]) / Math.PI,
        y: Math.asin(centers[basinRegion * 3 + 1]) / Math.PI },
      drainageSample: { id: routed, x: Math.atan2(centers[routed * 3 + 2], centers[routed * 3]) / Math.PI,
        y: Math.asin(centers[routed * 3 + 1]) / Math.PI },
      boundarySample: { id, x: Math.atan2(centers[id * 3 + 2], centers[id * 3]) / Math.PI,
        y: Math.asin(centers[id * 3 + 1]) / Math.PI } };
  }, { ...DEFAULT_RECIPE });
  assert.equal(desktopData.checksum, fingerprint); assert.equal(desktopData.typed, true);
  assert.equal(await page.evaluate(() => typeof (globalThis as unknown as { require?: unknown }).require), 'undefined');
  assert.deepEqual(await page.evaluate(() => Object.keys(window.desktop).sort()), ['acceptWorld', 'advance', 'cancelGeneration', 'exportView', 'generate', 'inspectWaterBudget', 'openRecipe', 'openSeasonalCheckpoint', 'openWaterCheckpoint', 'prescribeWater', 'saveRecipe', 'saveResolvedWorld', 'saveSeasonalCheckpoint', 'saveWaterCheckpoint', 'seasonalMoisture', 'seasonalTemperature', 'seasonalWind']);
  await expect(page.locator('[data-layer="temperature"]')).toBeEnabled();
  await expect(page.locator('#temperature-month')).toBeEnabled();
  await page.locator('[data-layer="temperature"]').click();
  await expect(page.locator('#legend-title')).toContainText('temperature normal');
  await expect(page.locator('#legend-low')).toContainText('°C');
  await expect(page.locator('#map')).toHaveAttribute('data-temperature-month', '3');
  await page.locator('#temperature-month').selectOption('9');
  await expect(page.locator('#map')).toHaveAttribute('data-temperature-month', '9');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('[data-layer="surface"]').click();
  await expect(page.locator('[data-layer="windSpeed"]')).toBeEnabled();
  await page.locator('[data-layer="windSpeed"]').click();
  await expect(page.locator('#legend-title')).toContainText('surface-wind speed');
  await expect(page.locator('#legend-high')).toContainText('m/s');
  await mkdir('artifacts', { recursive: true });
  await page.locator('#map').screenshot({ path: executablePath ? 'artifacts/wind-map-packaged.png' : 'artifacts/wind-map.png' });
  await page.locator('#temperature-month').selectOption('9');
  await expect(page.locator('#map')).toHaveAttribute('data-climate-month', '9');
  await page.locator('[data-layer="surface"]').click();
  await page.locator('#temperature-month').selectOption('3');
  await page.locator('#water-budget-refresh').click();
  await expect(page.locator('#water-budget-note')).toContainText('Prescribed step 0');
  await expect(page.locator('#water-budget-details')).toContainText('Exact balance residual');
  await expect(page.locator('#water-budget-details')).toContainText('0 units');
  assert.equal(await page.evaluate(async () => {
    try { await window.desktop.exportView({ x: -1, y: 0, width: 100, height: 100 }); return false; }
    catch { return true; }
  }), true, 'The desktop bridge rejects an invalid map region before opening a save dialog.');

  const resolvedPath = path.join(temp, 'resolved-initial-world.json');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, resolvedPath);
  await page.locator('#save-resolved').click();
  await expect(page.locator('#status')).toContainText('Calculated initial world data exported');
  const resolved = JSON.parse(await readFile(resolvedPath, 'utf8'));
  assert.equal(resolved.format, 'planimulation-resolved-initial-world');
  assert.equal(resolved.formatVersion, 2);
  assert.equal(resolved.initialFingerprint, fingerprint);
  assert.equal(resolved.stats.regionCount, 10_242);
  assert.equal(resolved.terrain.elevation.length, 10_242);
  assert.equal(resolved.terrain.preparation.length, 10_242);
  assert.equal(resolved.terrain.appliedPasses, 4);
  assert.ok(resolved.terrain.transportedCubicMeters > 0);
  assert.equal(resolved.water.depthMeters.length, 10_242);
  assert.equal(resolved.basins.regionNodes.length, 10_242);

  const canvas = page.locator('#map');
  const atlasExportPath = path.join(temp, 'atlas-export.png');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, atlasExportPath);
  await page.locator('#export-view').click();
  await expect(page.locator('#status')).toContainText('exported as PNG');
  const atlasPng = await readFile(atlasExportPath);
  assert.deepEqual(atlasPng.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
  assert.ok(atlasPng.readUInt32BE(16) > 200 && atlasPng.readUInt32BE(20) > 200);
  assert.ok(atlasPng.length > 10_000, 'The exported atlas must contain more than a blank background.');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  const clickAtlas = async (sample: { x: number; y: number }) => {
    const bounds = await canvas.boundingBox();
    assert.ok(bounds);
    const halfHeight = Math.max(.6, 1.1 / (bounds.width / bounds.height));
    await canvas.click({ position: {
      x: bounds.width * (.5 + sample.x / (2 * halfHeight * bounds.width / bounds.height)),
      y: bounds.height * (.5 - sample.y / (2 * halfHeight)),
    } });
  };
  // Locator clicks resolve the current center; window-manager resizing must not
  // leave later globe picks using dimensions captured at application startup.
  await canvas.click();
  await expect(page.locator('#selection-title')).toContainText('Region');
  await expect(page.locator('#selection-details')).toContainText('temperature normal');
  await expect(page.locator('#selection-details')).toContainText('wind speed normal');
  await expect(page.locator('#wind-note')).toContainText('prescribed east/north');
  await expect(page.locator('#temperature-note')).toContainText('Initial');
  const selectedRegion = await page.locator('#selection-title').innerText();
  const selectedDetails = await page.locator('#selection-details').innerText();
  await expect(page.locator('#elevation-details')).toContainText('Total elevation');
  const elevationExplanation = await page.locator('#elevation-details').innerText();
  await expect(page.locator('#selection-details')).toContainText('Crust thickness');
  await expect(page.locator('#selection-details')).toContainText('kg/m³');
  await expect(page.locator('#selection-details')).toContainText('Initial water depth');
  await expect(page.locator('#water-note')).toContainText('Regional stock');
  const waterExplanation = await page.locator('#water-note').innerText();
  const drainageExplanation = await page.locator('#drainage-note').innerText();
  await expect(page.locator('#selection-details')).toContainText('Contributing land area');
  const crustExplanation = await page.locator('#crust-note').innerText();
  assert.ok(crustExplanation.includes('fitted threshold'));
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-view', 'globe');
  await page.locator('[data-layer="temperature"]').click();
  await expect(canvas).toHaveAttribute('data-active-layer', 'temperature');
  await page.locator('[data-layer="windSpeed"]').click();
  await expect(canvas).toHaveAttribute('data-active-layer', 'windSpeed');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('[data-layer="surface"]').click();
  const globeExportPath = path.join(temp, 'globe-export.png');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, globeExportPath);
  await page.locator('#export-view').click();
  await expect(page.locator('#status')).toContainText('exported as PNG');
  const globePng = await readFile(globeExportPath);
  assert.deepEqual(globePng.subarray(0, 8), atlasPng.subarray(0, 8));
  assert.ok(globePng.length > 10_000, 'The exported globe must contain more than a blank background.');
  assert.notDeepEqual(globePng, atlasPng);
  await expect(canvas).toHaveAttribute('data-view', 'globe');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await expect(page.locator('#selection-title')).toHaveText(selectedRegion);
  await expect(page.locator('#selection-details')).toHaveText(selectedDetails, { useInnerText: true });
  await expect(page.locator('#crust-note')).toHaveText(crustExplanation);
  await expect(page.locator('#elevation-details')).toHaveText(elevationExplanation, { useInnerText: true });
  await expect(page.locator('#water-note')).toHaveText(waterExplanation);
  await expect(page.locator('#drainage-note')).toHaveText(drainageExplanation);
  await page.locator('#exaggeration').selectOption('0');
  await expect(canvas).toHaveAttribute('data-exaggeration', '0');
  await canvas.click();
  await expect(canvas).toHaveAttribute('data-picked-surface', 'water');
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.locator('#exaggeration').selectOption('25');
  await expect(canvas).toHaveAttribute('data-exaggeration', '25');
  await expect(page.locator('#selection-details')).toHaveText(selectedDetails, { useInnerText: true });
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('#exaggeration').selectOption('10');
  await canvas.click();
  await expect(page.locator('#selection-title')).toHaveText(selectedRegion);
  await expect(page.locator('button[data-view="globe"]')).toHaveAttribute('aria-pressed', 'true');
  await expect(canvas).toHaveAttribute('data-picked-surface', 'water');
  await page.locator('[data-layer="depth"]').click();
  await canvas.click();
  await expect(canvas).toHaveAttribute('data-picked-surface', 'bed');
  await expect(page.locator('#selection-title')).toHaveText(selectedRegion);
  await expect(page.locator('#selection-details')).toHaveText(selectedDetails, { useInnerText: true });
  await page.locator('[data-layer="surface"]').click();
  await page.locator('#boundaries').check();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('#boundaries').uncheck();
  await mkdir('artifacts', { recursive: true });
  await page.screenshot({ path: executablePath ? 'artifacts/globe-desktop-packaged.png' : 'artifacts/globe-desktop.png' });
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-view', 'flat');
  await page.locator('[data-layer="catchments"]').click();
  await expect(page.locator('#legend-title')).toContainText('Drainage catchments');
  await expect(page.locator('#legend-scale')).toBeHidden();
  await expect(page.locator('#boundary-legend')).toBeHidden();
  await page.locator('[data-layer="contributingArea"]').click();
  await expect(page.locator('#legend-title')).toContainText('not river discharge');
  await expect(page.locator('#legend-low')).toHaveText('0 km²');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('[data-layer="waterBodies"]').click();
  await expect(page.locator('#legend-title')).toContainText('Connected water bodies');
  await expect(page.locator('#boundary-legend')).toBeHidden();
  await expect(page.locator('#legend-scale')).toBeHidden();
  await page.locator('[data-layer="depth"]').click();
  await expect(page.locator('#legend-low')).toHaveText('0 m');
  await expect(page.locator('#legend-gradient')).toHaveClass('water-gradient');
  await page.locator('[data-layer="thickness"]').click();
  await expect(page.locator('#legend-low')).toHaveText('7 km');
  await expect(page.locator('#legend-high')).toHaveText('35 km');
  await page.locator('[data-layer="uplift"]').click();
  await expect(page.locator('#legend-title')).toContainText('Convergence uplift');
  await expect(page.locator('#legend-high')).toHaveText('12,000 m');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('[data-layer="boundaries"]').click();
  await expect(page.locator('#legend-title')).toContainText('Boundary motion');
  await expect(page.locator('#boundary-legend')).toBeVisible();
  await expect(page.locator('#selection-details')).toContainText('Tectonic plate');
  await expect(page.locator('#selection-details')).toContainText('cm/year');
  await page.getByRole('button', { name: 'Fit map' }).click();
  await clickAtlas(desktopData.boundarySample);
  await expect(page.locator('#selection-title')).toHaveText(`Region ${desktopData.boundarySample.id.toLocaleString('en')}`);
  await expect(page.locator('#boundary-details')).toContainText('opening');
  await expect(page.locator('#boundary-details')).toContainText('shear');
  await page.locator('#boundaries').check();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('#boundaries').uncheck();
  await page.screenshot({ path: executablePath ? 'artifacts/boundaries-desktop-packaged.png' : 'artifacts/boundaries-desktop.png' });
  await page.locator('[data-layer="catchments"]').click();
  await clickAtlas(desktopData.drainageSample);
  await expect(page.locator('#selection-title')).toHaveText(`Region ${desktopData.drainageSample.id.toLocaleString('en')}`);
  await expect(page.locator('#drainage-note')).toContainText('Steepest bed descent');
  await page.screenshot({ path: executablePath ? 'artifacts/catchments-desktop-packaged.png' : 'artifacts/catchments-desktop.png' });
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(page.locator('#drainage-note')).toContainText('Steepest bed descent');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await page.locator('[data-layer="speed"]').click();
  await expect(page.locator('#legend-high')).toHaveText('8 cm/year');
  await page.locator('[data-layer="basins"]').click();
  await expect(page.locator('#legend-scale')).toBeHidden();
  await expect(page.locator('#legend-title')).toContainText('exclusive ownership');
  await clickAtlas(desktopData.basinSample);
  await expect(page.locator('#selection-title')).toHaveText(`Region ${desktopData.basinSample.id.toLocaleString('en')}`);
  const basinRegionTitle = await page.locator('#selection-title').innerText();
  const basinDetails = await page.locator('#basin-details').innerText();
  await expect(page.locator('#basin-title')).toContainText(`Branch ${desktopData.basinSample.node} ·`);
  await expect(page.locator('#basin-details')).toContainText('not current water level');
  await expect(page.locator('#basin-details')).toContainText('includes children');
  await expect(canvas).toHaveAttribute('data-spill-from', String(desktopData.basinSample.from));
  await expect(canvas).toHaveAttribute('data-spill-to', String(desktopData.basinSample.to));
  await page.screenshot({ path: executablePath ? 'artifacts/basins-desktop-packaged.png' : 'artifacts/basins-desktop.png' });
  await page.locator('#basin-parent').click();
  await expect(page.locator('#basin-title')).toContainText(`Branch ${desktopData.basinSample.parent} ·`);
  await expect(page.locator('#selection-title')).toHaveText(basinRegionTitle);
  const beforeBasinPlayback = await page.locator('#diagnostic-tick').innerText();
  await page.locator('#play').click();
  await expect(page.locator('#diagnostic-tick')).not.toHaveText(beforeBasinPlayback);
  await page.locator('#play').click();
  await expect(page.locator('#basin-title')).toContainText(`Branch ${desktopData.basinSample.parent} ·`);
  await page.locator('#basin-owner').click();
  await expect(page.locator('#basin-details')).toHaveText(basinDetails, { useInnerText: true });
  await page.locator('#basin-details').scrollIntoViewIfNeeded();
  await page.screenshot({ path: executablePath ? 'artifacts/basin-inspector-packaged.png' : 'artifacts/basin-inspector.png' });
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(page.locator('#basin-details')).toHaveText(basinDetails, { useInnerText: true });
  await page.locator('[data-layer="spill"]').click();
  await expect(page.locator('#legend-title')).toContainText('not current water level');
  await expect(page.locator('#legend-scale')).toBeVisible();
  await expect(page.locator('#boundary-legend')).toBeHidden();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.screenshot({ path: executablePath ? 'artifacts/spill-globe-packaged.png' : 'artifacts/spill-globe.png' });
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await page.locator('[data-layer="speed"]').click();
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
  const advancedSettings = page.locator('details.advanced-settings');
  if (await advancedSettings.count()) await advancedSettings.locator('summary').click();
  await page.locator('#plate-count').fill('7');
  await page.locator('#plate-speed').fill('0');
  await page.locator('#continental-fraction').fill('0');
  await page.locator('#continental-scale').fill('2');
  await page.locator('#relief-scale').fill('0');
  await page.locator('#detail-amplitude').fill('0');
  await page.locator('#boundary-width').fill('500');
  await page.locator('#water-mode').selectOption('volume');
  await expect(page.locator('#water-coverage')).toBeDisabled();
  await page.locator('#water-volume').fill('0');
  await page.locator('#generate').click();
  await expect(page.locator('#world-name')).toHaveText('desktop-roundtrip');
  await expect(page.locator('#region-count')).toHaveText('642');
  await expect(page.locator('#crust-summary')).toContainText('0.00% actual / 0.00% target');
  await expect(page.locator('#water-summary')).toContainText('0.00% actual / 0 km³ requested');
  await expect(page.locator('#drainage-summary')).toContainText('1 terminal catchments · 1 closed dry sinks');
  await expect(page.locator('#basin-summary')).toContainText('1 minima · 1 hierarchy branches');
  await expect(page.locator('#basin-title')).toHaveText('Select a region');
  await expect(page.locator('#basin-parent')).toBeDisabled();
  await page.locator('[data-layer="surface"]').click();
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await canvas.click();
  await expect(canvas).toHaveAttribute('data-picked-surface', 'bed');
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await expect(page.locator('#basin-title')).toContainText('global root');
  await expect(page.locator('#basin-details')).toContainText('No finite spill capacity');
  await expect(page.locator('#basin-parent')).toBeDisabled();
  await page.locator('#water-spill').click();
  await expect(page.locator('#status')).toContainText('no spill threshold');
  await expect(page.locator('#water-step')).toContainText('Select a source region');
  await expect(canvas).toHaveAttribute('data-spill-from', '-1');
  await page.locator('[data-layer="spill"]').click();
  await expect(page.locator('#legend-low')).toHaveText('No finite thresholds');
  await page.locator('[data-layer="plates"]').click();
  await expect(page.locator('#legend-title')).toContainText('7 connected plates');
  await page.locator('[data-layer="speed"]').click();
  await expect(page.locator('#legend-high')).toHaveText('0 cm/year');
  assert.notEqual(await page.locator('#fingerprint').innerText(), fingerprint);
  // A uniform bed with positive volume is fully wet; save/import retains volume mode.
  await page.locator('#water-volume').fill('1000000');
  await page.locator('#generate').click();
  await expect(page.locator('#water-summary')).toContainText('100.00% actual / 1,000,000 km³ requested');
  await expect(page.locator('#drainage-summary')).toContainText('1 terminal catchments · 0 closed dry sinks');
  const volumeHash = await page.locator('#fingerprint').innerText();
  await page.locator('[data-layer="surface"]').click();
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  for (const factor of ['0', '1', '10']) {
    await page.locator('#exaggeration').selectOption(factor);
    await canvas.click();
    await expect(canvas).toHaveAttribute('data-picked-surface', 'water');
    await expect(page.locator('#fingerprint')).toHaveText(volumeHash);
  }
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await page.getByRole('button', { name: 'Save recipe' }).click();
  await expect(page.locator('#status')).toContainText('Recipe saved');
  const volumeRecipe = parseRecipe(JSON.parse(await readFile(recipePath, 'utf8')));
  assert.deepEqual(volumeRecipe.water, { mode: 'volume', volumeCubicMeters: 1e15 });
  await page.locator('#water-volume').fill('0');
  await page.locator('#generate').click();
  await expect(page.locator('#water-summary')).toContainText('0.00% actual / 0 km³ requested');
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#water-mode')).toHaveValue('volume');
  await expect(page.locator('#water-volume')).toHaveValue('1000000');
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
  await expect(page.locator('#fingerprint')).toHaveText(volumeHash);
  await writeFile(recipePath, JSON.stringify(DEFAULT_RECIPE));
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });
  await expect(page.locator('#plate-count')).toHaveValue('12');
  await expect(page.locator('#plate-speed')).toHaveValue('8');
  await expect(page.locator('#continental-fraction')).toHaveValue('38');
  await expect(page.locator('#continental-scale')).toHaveValue('1');
  await expect(page.locator('#relief-scale')).toHaveValue('1');
  await expect(page.locator('#detail-amplitude')).toHaveValue('300');
  await expect(page.locator('#boundary-width')).toHaveValue('300');
  await expect(page.locator('#water-mode')).toHaveValue('coverage');
  await expect(page.locator('#water-coverage')).toHaveValue('71');
  await expect(page.locator('#water-volume')).toBeDisabled();

  const continuousRecipe = parseRecipe({ ...DEFAULT_RECIPE, modelVersion: CONTINUOUS_PLATES_MODEL_VERSION });
  const continuousCore = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
  let continuousHash: string;
  try { continuousHash = (await continuousCore.generate(continuousRecipe)).world.checksum; }
  finally { continuousCore.close(); }
  await writeFile(recipePath, JSON.stringify(continuousRecipe));
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#fingerprint')).toHaveText(continuousHash, { timeout: 30_000 });
  await expect(page.locator('#model-label')).toContainText('CONTINUOUS-PLATES-1');
  await page.getByRole('button', { name: 'Save recipe' }).click();
  assert.deepEqual(parseRecipe(JSON.parse(await readFile(recipePath, 'utf8'))), continuousRecipe);
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(page.locator('#fingerprint')).toHaveText(continuousHash);
  await page.getByRole('button', { name: '2D map', exact: true }).click();

  const { terrainPreparationPasses: _legacyPasses, ...legacyBase } = DEFAULT_RECIPE;
  const legacyRecipe = parseRecipe({ ...legacyBase, modelVersion: 'basins-1' });
  const legacyCore = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
  let legacyHash: string;
  try { legacyHash = (await legacyCore.generate(legacyRecipe)).world.checksum; }
  finally { legacyCore.close(); }
  await writeFile(recipePath, JSON.stringify(legacyRecipe));
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#fingerprint')).toHaveText(legacyHash, { timeout: 30_000 });
  await expect(page.locator('#model-label')).toContainText('BASINS-1');
  await expect(page.locator('#terrain-preparation-passes')).toHaveValue('0');
  await page.getByRole('button', { name: 'Save recipe' }).click();
  assert.deepEqual(parseRecipe(JSON.parse(await readFile(recipePath, 'utf8'))), legacyRecipe);
  await writeFile(recipePath, JSON.stringify(DEFAULT_RECIPE));
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });
  await expect(page.locator('#terrain-preparation-passes')).toHaveValue('4');

  // Experimental small planets must limit display distortion, never physical heights.
  await page.locator('#radius').fill('100');
  await page.locator('#subdivision').selectOption('2');
  await page.locator('#generate').click();
  await expect(page.locator('#region-count')).toHaveText('162');
  const smallHash = await page.locator('#fingerprint').innerText();
  await page.locator('#exaggeration').selectOption('50');
  await expect(page.locator('#exaggeration-note')).toContainText('20% radius display limit');
  assert.ok(Number(await canvas.getAttribute('data-exaggeration')) < 50);
  await expect(page.locator('#fingerprint')).toHaveText(smallHash);
  await page.locator('#exaggeration').selectOption('10');
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });

  await writeFile(recipePath, JSON.stringify({ ...DEFAULT_RECIPE, modelVersion: 'future' }));
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#status')).toContainText('Unsupported recipe or random version');
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
  const dryRecipe = { ...DEFAULT_RECIPE, subdivision: 2, water: { mode: 'coverage' as const, fraction: 0 } };
  const spillReference = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
  let spillSample: { id: number; x: number; y: number };
  try {
    const generated = (await spillReference.generate(dryRecipe)).world;
    const children = basinTree(generated.basins).children;
    const id = generated.drainage.receivers.findIndex((receiver, region) => {
      if (receiver !== region) return false;
      const branch = generated.basins.regionNodes[region], parent = generated.basins.parents[branch];
      const x = Math.atan2(generated.surface.centers[region * 3 + 2], generated.surface.centers[region * 3]) / Math.PI;
      const y = Math.asin(generated.surface.centers[region * 3 + 1]) / Math.PI;
      return Math.abs(x) < .8 && Math.abs(y) < .7 && children[parent].length === 2
        && children[parent].every((child) => children[child].length === 0 && generated.basins.capacities[child] > 1e9);
    });
    assert.ok(id >= 0, 'The generated desktop fixture needs a visible two-leaf spill source.');
    spillSample = { id, x: Math.atan2(generated.surface.centers[id * 3 + 2], generated.surface.centers[id * 3]) / Math.PI,
      y: Math.asin(generated.surface.centers[id * 3 + 1]) / Math.PI };
  } finally { spillReference.close(); }
  await page.locator('#subdivision').selectOption('2');
  await page.locator('#water-mode').selectOption('coverage');
  await page.locator('#water-coverage').fill('0');
  await page.locator('#generate').click();
  await expect(page.locator('#region-count')).toHaveText('162');
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await page.getByRole('button', { name: 'Fit map' }).click();
  await clickAtlas(spillSample);
  await expect(page.locator('#selection-title')).toHaveText(`Region ${spillSample.id}`);
  await page.locator('#water-spill').click();
  await expect(page.locator('#water-step')).toContainText('Prescribed step 1');
  await expect(page.locator('#water-budget-note')).toContainText('Prescribed step 1');
  await expect(page.locator('#water-budget-details')).toContainText('Destination branch stock');
  await expect(page.locator('#water-budget-details')).toContainText('0 units');
  await expect(page.locator('#water-summary')).toContainText('manual input');
  await expect(page.locator('#selection-details')).toContainText('Displayed prescribed-water depth');
  await expect(page.locator('#moisture-start')).toBeDisabled();
  await page.locator('[data-layer="surface"]').click();
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await page.screenshot({ path: executablePath ? 'artifacts/spill-desktop-packaged.png' : 'artifacts/spill-desktop.png' });
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  const waterCheckpointPath = path.join(temp, 'water-checkpoint.json');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, waterCheckpointPath);
  await page.locator('#save-water').click();
  await expect(page.locator('#status')).toContainText('checkpoint saved');
  const savedWater = JSON.parse(await readFile(waterCheckpointPath, 'utf8'));
  assert.equal(savedWater.step, 1);
  assert.equal(savedWater.origin.originRecipe.water.fraction, 0);
  await page.locator('#water-add').click();
  await expect(page.locator('#water-step')).toContainText('Prescribed step 2');
  const continuedStep = await page.locator('#water-step').innerText();
  const continuedBudget = await page.locator('#water-budget-details').innerText();
  await page.locator('#seed').fill('unsaved-form-edit');
  await page.locator('#water-coverage').fill('17');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, waterCheckpointPath);
  await page.locator('#open-water').click();
  await expect(page.locator('#status')).toContainText('Water checkpoint restored');
  await expect(page.locator('#water-step')).toContainText('Prescribed step 1');
  await expect(page.locator('#water-budget-note')).toContainText('Prescribed step 1');
  await expect(page.locator('#region-count')).toHaveText('162');
  await expect(page.locator('#seed')).toHaveValue(DEFAULT_RECIPE.seed);
  await expect(page.locator('#water-coverage')).toHaveValue('0');
  await page.getByRole('button', { name: 'Fit map' }).click();
  await clickAtlas(spillSample);
  await page.locator('#water-add').click();
  await expect(page.locator('#water-step')).toHaveText(continuedStep);
  await expect(page.locator('#water-budget-details')).toHaveText(continuedBudget, { useInnerText: true });
  const damagedCheckpointPath = path.join(temp, 'damaged-water-checkpoint.json');
  await writeFile(damagedCheckpointPath, JSON.stringify({ ...savedWater, acceptedInputUnits: '0' }));
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, damagedCheckpointPath);
  await page.locator('#open-water').click();
  await expect(page.locator('#status')).toContainText('balance');
  await expect(page.locator('#water-step')).toHaveText(continuedStep);
  await expect(page.locator('#region-count')).toHaveText('162');
  const oversizedCheckpointPath = path.join(temp, 'oversized-water-checkpoint.json');
  await writeFile(oversizedCheckpointPath, Buffer.alloc(8 * 2 ** 20 + 1));
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, oversizedCheckpointPath);
  await page.locator('#open-water').click();
  await expect(page.locator('#status')).toContainText('exceeds the 8 MiB limit');
  await expect(page.locator('#water-step')).toHaveText(continuedStep);
  await page.locator('#water-coverage').fill('71');
  await page.locator('#subdivision').selectOption(String(DEFAULT_RECIPE.subdivision));
  await page.locator('#generate').click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });
  await canvas.click();
  // Use the real native calculation, bridge, GPU fields, and inspector. Seasonal
  // playback must not mutate the original geography or manual inventory.
  await expect(page.locator('#moisture-start')).toBeEnabled();
  await expect(page.locator('[data-layer="vaporWater"]')).toBeDisabled();
  await page.locator('#moisture-start').click();
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '0');
  await expect(page.locator('#moisture-budget-details')).toContainText('Initial mobile partition');
  await expect(page.locator('#moisture-selection-details')).toContainText('Soil water');
  await expect(page.locator('#water-add')).toBeDisabled();
  await expect(page.locator('#save-water')).toBeDisabled();
  await expect(page.locator('#water-budget-refresh')).toBeDisabled();
  await expect(page.locator('#play')).toBeDisabled();
  await page.locator('[data-layer="vaporWater"]').click();
  await expect(page.locator('#legend-title')).toContainText('fixed logarithmic scale');
  await expect(page.locator('#legend-high')).toHaveText('≥ 60 mm WE');
  const initialVaporImage = await canvas.screenshot();
  await page.locator('#moisture-interval').selectOption('3600');
  await page.locator('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '3600');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await expect(page.locator('#moisture-step-note')).toContainText('Last interval 1 h');
  await page.locator('#moisture-interval').selectOption('86400');
  await page.locator('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '90000');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  const seasonalDetails = await page.locator('#moisture-selection-details').innerText();
  const seasonalBudget = await page.locator('#moisture-budget-details').innerText();
  assert.notDeepEqual(await canvas.screenshot(), initialVaporImage, 'Vapor texture updates must change the rendered map, not just inspector values.');
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await page.locator('[data-layer="terminalWater"]').click();
  await expect(page.locator('#legend-title')).toContainText('not lake depth');
  await expect(page.locator('#moisture-selection-details')).toHaveText(seasonalDetails, { useInnerText: true });
  await expect(page.locator('#moisture-budget-details')).toHaveText(seasonalBudget, { useInnerText: true });
  await page.locator('#temperature-month').selectOption('9');
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '90000');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await canvas.screenshot({ path: executablePath ? 'artifacts/seasonal-water-globe-packaged.png' : 'artifacts/seasonal-water-globe.png' });
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await page.locator('[data-layer="precipitation"]').click();
  await expect(page.locator('#legend-high')).toHaveText('≥ 20 mm/day');
  const beforePlay = Number(await canvas.getAttribute('data-moisture-seconds'));
  await page.locator('#moisture-play').click();
  await expect(canvas).not.toHaveAttribute('data-moisture-seconds', String(beforePlay));
  await expect(page.locator('#save-seasonal')).toBeDisabled();
  await expect(page.locator('#open-seasonal')).toBeDisabled();
  await page.getByRole('button', { name: 'Pause seasonal water', exact: true }).click();
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  const pausedTime = await canvas.getAttribute('data-moisture-seconds');
  await page.waitForTimeout(150);
  await expect(canvas).toHaveAttribute('data-moisture-seconds', pausedTime!);
  // A pending old-world step is kept when replacement generation is canceled.
  await page.evaluate(() => {
    document.querySelector<HTMLButtonElement>('#moisture-play')!.click();
    document.querySelector<HTMLInputElement>('#seed')!.value = 'canceled-seasonal-replacement';
    document.querySelector<HTMLFormElement>('#recipe-form')!.requestSubmit();
    document.querySelector<HTMLButtonElement>('#cancel')!.click();
  });
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  const seasonalSeconds = Number(await canvas.getAttribute('data-moisture-seconds'));
  assert.equal(seasonalSeconds, Number(pausedTime) + 86400);
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  const seasonalReference = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
  try {
    const initial = await seasonalReference.generate(DEFAULT_RECIPE); seasonalReference.accept(initial.epoch);
    await seasonalReference.seasonalMoisture(initial.epoch, 0);
    let expected = await seasonalReference.seasonalMoisture(initial.epoch, 3600);
    while (expected.elapsedSeconds < seasonalSeconds) expected = await seasonalReference.seasonalMoisture(initial.epoch, 86400);
    const id = Number((await page.locator('#moisture-selection-title').innerText()).match(/Region (\d+)/)![1]);
    const actual = await page.locator('#moisture-selection-details').evaluate((dl) => {
      const terms = Array.from(dl.querySelectorAll('dt')), descriptions = Array.from(dl.querySelectorAll('dd'));
      return Object.fromEntries(terms.map((term, i) => [term.textContent, descriptions[i].textContent]));
    });
    const format = new Intl.NumberFormat('en', { maximumSignificantDigits: 6 });
    for (const [name, key] of [['Local liquid', 'surfaceKilograms'], ['Snow water equivalent', 'snowKilograms'],
      ['Soil water', 'soilKilograms'], ['Runoff in transit', 'pendingRunoffKilograms'],
      ['Terminal water', 'terminalWaterKilograms'], ['Atmospheric vapor', 'vaporKilograms']] as const) {
      assert.equal(actual[name], `${format.format(expected.stocks[key][id] / initial.world.surface.areasSquareMeters[id])} mm WE`);
    }
  } finally { seasonalReference.close(); }
  await page.locator('[data-layer="vaporWater"]').click();
  await canvas.screenshot({ path: executablePath ? 'artifacts/seasonal-water-map-packaged.png' : 'artifacts/seasonal-water-map.png' });
  // Persist complete native state through real file I/O, not a mocked bridge.
  const seasonalPath = path.join(temp, 'seasonal-water-checkpoint.json');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, seasonalPath);
  await expect(page.locator('#save-seasonal')).toBeEnabled();
  await page.locator('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText(`saved at ${seasonalSeconds} elapsed seconds`);
  const savedSeasonalText = await readFile(seasonalPath, 'utf8'), savedSeasonal = JSON.parse(savedSeasonalText);
  assert.equal(savedSeasonal.schemaVersion, 3);
  assert.equal(savedSeasonal.elapsedSeconds, seasonalSeconds);
  assert.equal(savedSeasonal.surfaceKilograms.length, 10242);
  assert.equal(savedSeasonal.cumulativeSurfaceTransferRoundoff.length, 10242);
  const savedSeasonalBudget = await page.locator('#moisture-budget-details').innerText();
  await app.evaluate(({ dialog }) => { dialog.showSaveDialog = async () => ({ canceled: true, filePath: '' }); });
  await page.locator('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('saving canceled');
  assert.equal(await readFile(seasonalPath, 'utf8'), savedSeasonalText);
  // A failed rename must not replace a target directory or discard native state.
  const deniedSeasonalPath = path.join(temp, 'checkpoint-directory'); await mkdir(deniedSeasonalPath);
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, deniedSeasonalPath);
  await page.locator('#save-seasonal').click();
  await expect(page.locator('#status')).toHaveClass(/error/);
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds));
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, seasonalPath);
  await page.locator('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await page.locator('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText(`saved at ${seasonalSeconds + 86400} elapsed seconds`);
  const uninterrupted = await readFile(seasonalPath, 'utf8');
  await writeFile(seasonalPath, savedSeasonalText);
  // Failed/canceled file operations preserve both visible and native state.
  const damagedSeasonalPath = path.join(temp, 'damaged-seasonal.json');
  await writeFile(damagedSeasonalPath, JSON.stringify({ ...savedSeasonal, vaporKilograms: [] }));
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, damagedSeasonalPath);
  await page.locator('#open-seasonal').click();
  await expect(page.locator('#status')).toHaveClass(/error/);
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), uninterrupted);
  await writeFile(seasonalPath, savedSeasonalText);
  await app.evaluate(({ dialog }) => { dialog.showOpenDialog = async () => ({ canceled: true, filePaths: [] }); });
  await page.locator('#open-seasonal').click();
  await expect(page.locator('#status')).toContainText('opening canceled');
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  const oversizedSeasonalPath = path.join(temp, 'oversized-seasonal.json');
  const oversizedSeasonalFile = await open(oversizedSeasonalPath, 'w');
  try { await oversizedSeasonalFile.truncate(MAX_SEASONAL_CHECKPOINT_BYTES + 1); }
  finally { await oversizedSeasonalFile.close(); }
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, oversizedSeasonalPath);
  await page.locator('#open-seasonal').click();
  await expect(page.locator('#status')).toContainText('exceeds the 64 MiB limit');
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  // Exercise the main-process intent guard independently of disabled UI buttons:
  // a delayed old dialog cannot resurrect a canceled/replaced candidate.
  await app.evaluate(({ dialog }, destination) => {
    const holder = globalThis as unknown as { releaseCheckpointDialog?: () => void };
    dialog.showOpenDialog = () => new Promise((resolve) => {
      holder.releaseCheckpointDialog = () => {
        delete holder.releaseCheckpointDialog;
        resolve({ canceled: false, filePaths: [destination] });
      };
    });
  }, seasonalPath);
  const delayedOpen = page.evaluate(async () => {
    try { await window.desktop.openSeasonalCheckpoint(); return 'Unexpected prepared checkpoint.'; }
    catch (error) { return String(error); }
  });
  await expect.poll(() => app.evaluate(() => typeof (globalThis as unknown as { releaseCheckpointDialog?: unknown }).releaseCheckpointDialog)).toBe('function');
  await page.evaluate(async (recipe) => {
    await window.desktop.generate(recipe); await window.desktop.cancelGeneration();
  }, { ...DEFAULT_RECIPE, subdivision: 2, seed: 'replacement-during-checkpoint-dialog' });
  await app.evaluate(() => (globalThis as unknown as { releaseCheckpointDialog: () => void }).releaseCheckpointDialog());
  assert.match(await delayedOpen, /canceled or replaced/);
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  await page.locator('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), uninterrupted);
  await writeFile(seasonalPath, savedSeasonalText);
  // Restore into a different accepted world; its saved origin must win, not the
  // unsaved form values or current geography. Both GPU views remain usable.
  await page.locator('#seed').fill('world-before-seasonal-restore');
  await page.locator('#subdivision').selectOption('2');
  await page.locator('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('#region-count')).toHaveText('162');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, seasonalPath);
  await page.locator('#open-seasonal').click();
  await expect(page.locator('#status')).toContainText(`restored at ${seasonalSeconds} elapsed seconds`, { timeout: 30000 });
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds));
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await expect(page.locator('#seed')).toHaveValue(DEFAULT_RECIPE.seed);
  await expect(page.locator('#subdivision')).toHaveValue(String(DEFAULT_RECIPE.subdivision));
  await expect(page.locator('#moisture-budget-details')).toHaveText(savedSeasonalBudget, { useInnerText: true });
  await expect(page.locator('#moisture-step-note')).toContainText('no interval transfers');
  await expect(page.locator('#save-water')).toBeDisabled();
  await page.locator('[data-layer="vaporWater"]').click();
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-active-layer', 'vaporWater');
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds));
  await page.locator('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), savedSeasonalText);
  await page.locator('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await page.locator('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), uninterrupted, 'GUI save/load retains exact uninterrupted full-checkpoint continuation.');
  await page.locator('#seed').fill(DEFAULT_RECIPE.seed);
  await page.locator('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await expect(page.locator('[data-layer="vaporWater"]')).toBeDisabled();
  await expect(page.locator('#moisture-time')).toContainText('Not initialized');
  await expect(page.locator('#moisture-budget-details')).toBeEmpty();
  await page.locator('[data-layer="surface"]').click();
  await mkdir('artifacts', { recursive: true });
  const screenshot = executablePath ? 'artifacts/surface-desktop-packaged.png' : 'artifacts/surface-desktop.png';
  await page.screenshot({ path: screenshot });
  assert.deepEqual(errors, [], 'No renderer exceptions.');
  console.log(`Desktop checks passed. Desktop fingerprint: ${fingerprint}. Screenshot: ${screenshot}`);
} finally {
  await app.close();
}
