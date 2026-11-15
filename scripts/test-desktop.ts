import { strict as assert } from 'node:assert';
import { mkdir, mkdtemp, open, readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';
import { CONTINUOUS_PLATES_MODEL_VERSION, DEFAULT_RECIPE, parseRecipe } from '../src/core/recipe';
import { NativeController } from '../src/native/client';
import { basinTree } from '../src/core/basins';
import { MAX_SEASONAL_CHECKPOINT_BYTES } from '../src/shared/seasonal-checkpoint';
import { fundedRegionalCheckpoint, regionalFlowRecipe } from '../tests/helpers/regional-water';
import { fundedSoilCheckpoint } from '../tests/helpers/soil-water';

const temp = await mkdtemp(path.join(os.tmpdir(), 'planimulation-desktop-'));
const recipePath = path.join(temp, 'recipe.json');
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const executablePath = process.argv[2] ? path.resolve(process.argv[2]) : undefined;
const app = await electron.launch({ executablePath, args: [...(executablePath ? [] : ['.']), `--user-data-dir=${temp}`], env, timeout: 30_000 });
try {
  const page = await app.firstWindow();
  page.on('dialog', () => {});
  page.setDefaultTimeout(15_000);
  await app.evaluate(({ dialog }) => { dialog.showMessageBoxSync = () => 1; });
  // Reveal controls through the same panels a user opens in the workspace.
  async function reveal(selector: string): Promise<void> {
    const target = page.locator(selector);
    if (await target.evaluate(element => !!element.closest('#recipe-form'))) {
      if (!await page.locator('#world-dialog').isVisible()) await page.locator('[data-dialog="world-dialog"]').click();
    } else if (await page.locator('#world-dialog').isVisible()) {
      await page.locator('#world-dialog [data-close-dialog]').click();
    }
    if (await target.evaluate(element => !!element.closest('#file-menu'))) {
      if (!await page.locator('#file-menu').isVisible()) await page.locator('#file-trigger').click();
    }
    if (await target.evaluate(element => !!element.closest('#display-menu'))) {
      if (!await page.locator('#display-menu').isVisible()) await page.locator('#display-trigger').click();
    } else if (await page.locator('#display-menu').isVisible()) await page.keyboard.press('Escape');
    const group = await target.evaluate(element => element.closest<HTMLDetailsElement>('.layer-group')?.dataset.group);
    if (group) {
      if (await page.locator('#toggle-layers').getAttribute('aria-expanded') === 'false') await page.locator('#toggle-layers').click();
      const details = page.locator(`[data-group="${group}"]`);
      if (!await details.evaluate(element => (element as HTMLDetailsElement).open)) await details.locator('summary').click();
    }
    const experiment = await target.evaluate(element => element.closest('.experiment-panel')?.id);
    if (experiment) await page.locator(`[aria-controls="${experiment}"]`).click();
    const initializer: Record<string, string> = { '#moisture-start': 'baseline', '#moisture-start-orographic': 'orographic',
      '#moisture-start-precise': 'precise', '#moisture-start-regional': 'regional', '#moisture-start-soil': 'soil' };
    if (initializer[selector] && await page.locator('#moisture-model').isEnabled()) await page.locator('#moisture-model').selectOption(initializer[selector]);
    if (await target.evaluate(element => !!element.closest('#inspector-pane'))) {
      if (await page.locator('#toggle-inspector').getAttribute('aria-expanded') === 'false') await page.locator('#toggle-inspector').click();
      const panel = await target.evaluate(element => element.closest('[role="tabpanel"]')?.id);
      if (panel) await page.locator(`[aria-controls="${panel}"]`).click();
    }
    if (selector === '#temperature-month' && !await target.isVisible()) {
      await reveal('[data-layer="temperature"]'); await page.locator('[data-layer="temperature"]').click();
    }
  }
  function ui(selector: string) {
    return {
      async click() {
        await reveal(selector); await page.locator(selector).click();
        if (await page.locator('#replace-dialog').isVisible()) await page.locator('#replace-world').click();
      },
      async fill(value: string) { await reveal(selector); await page.locator(selector).fill(value); },
      async selectOption(value: string) {
        const previous = await page.locator('[data-layer].active').getAttribute('data-layer');
        const restore = selector === '#temperature-month' && !await page.locator('#temperature-month').isVisible();
        await reveal(selector); await page.locator(selector).selectOption(value);
        if (restore && previous) { await reveal(`[data-layer="${previous}"]`); await page.locator(`[data-layer="${previous}"]`).click(); }
      },
    };
  }
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('console', (message) => { if (message.type() === 'error' && /THREE|WebGL|shader/i.test(message.text())) errors.push(message.text()); });
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
  await expect(page.locator('#region-count')).toHaveText('10,242');
  const fingerprint = await page.locator('#fingerprint').textContent().then(value => value ?? '');
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
  assert.deepEqual(await page.evaluate(() => Object.keys(window.desktop).sort()), ['acceptWorld', 'advance', 'cancelGeneration', 'copyText', 'exportView', 'generate', 'initializeOrographicMoisture', 'initializePreciseMoisture', 'initializeRegionalMoisture', 'initializeSoilMoisture', 'inspectWaterBudget', 'openRecipe', 'openSeasonalCheckpoint', 'openWaterCheckpoint', 'prescribeWater', 'saveRecipe', 'saveResolvedWorld', 'saveSeasonalCheckpoint', 'saveWaterCheckpoint', 'seasonalMoisture', 'seasonalSoilMoisture', 'seasonalTemperature', 'seasonalWind']);
  await expect(page.locator('[data-layer="temperature"]')).toBeEnabled();
  await expect(page.locator('#temperature-month')).toBeEnabled();
  await ui('[data-layer="temperature"]').click();
  await expect(page.locator('#legend-title')).toContainText('temperature normal');
  await expect(page.locator('#legend-low')).toContainText('°C');
  await expect(page.locator('#map')).toHaveAttribute('data-temperature-month', '3');
  await ui('#temperature-month').selectOption('9');
  await expect(page.locator('#map')).toHaveAttribute('data-temperature-month', '9');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await ui('[data-layer="surface"]').click();
  await expect(page.locator('[data-layer="windSpeed"]')).toBeEnabled();
  await ui('[data-layer="windSpeed"]').click();
  await expect(page.locator('#legend-title')).toContainText('surface-wind speed');
  await expect(page.locator('#legend-high')).toContainText('m/s');
  await mkdir('artifacts', { recursive: true });
  await page.locator('#map').screenshot({ path: executablePath ? 'artifacts/wind-map-packaged.png' : 'artifacts/wind-map.png' });
  await ui('#temperature-month').selectOption('9');
  await expect(page.locator('#map')).toHaveAttribute('data-climate-month', '9');
  await ui('[data-layer="surface"]').click();
  await ui('#temperature-month').selectOption('3');
  await ui('#water-budget-refresh').click();
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
  await ui('#save-resolved').click();
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
  await ui('#export-view').click();
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
  const selectedRegion = await page.locator('#selection-title').textContent().then(value => value ?? '');
  const selectedDetails = await page.locator('#selection-details').textContent().then(value => value ?? '');
  await expect(page.locator('#elevation-details')).toContainText('Total elevation');
  const elevationExplanation = await page.locator('#elevation-details').textContent().then(value => value ?? '');
  await expect(page.locator('#selection-details')).toContainText('Crust thickness');
  await expect(page.locator('#selection-details')).toContainText('kg/m³');
  await expect(page.locator('#selection-details')).toContainText('Initial water depth');
  await expect(page.locator('#water-note')).toContainText('Regional stock');
  const waterExplanation = await page.locator('#water-note').textContent().then(value => value ?? '');
  const drainageExplanation = await page.locator('#drainage-note').textContent().then(value => value ?? '');
  await expect(page.locator('#selection-details')).toContainText('Contributing land area');
  const crustExplanation = await page.locator('#crust-note').textContent().then(value => value ?? '');
  assert.ok(crustExplanation.includes('fitted threshold'));
  await page.setViewportSize({ width: 1280, height: 900 });
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-view', 'globe');
  await ui('[data-layer="temperature"]').click();
  await expect(canvas).toHaveAttribute('data-active-layer', 'temperature');
  await ui('[data-layer="windSpeed"]').click();
  await expect(canvas).toHaveAttribute('data-active-layer', 'windSpeed');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await ui('[data-layer="surface"]').click();
  const globeExportPath = path.join(temp, 'globe-export.png');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, globeExportPath);
  await ui('#export-view').click();
  await expect(page.locator('#status')).toContainText('exported as PNG');
  const globePng = await readFile(globeExportPath);
  assert.deepEqual(globePng.subarray(0, 8), atlasPng.subarray(0, 8));
  assert.ok(globePng.length > 10_000, 'The exported globe must contain more than a blank background.');
  assert.notDeepEqual(globePng, atlasPng);
  await expect(canvas).toHaveAttribute('data-view', 'globe');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await expect(page.locator('#selection-title')).toHaveText(selectedRegion);
  await expect(page.locator('#selection-details')).toHaveText(selectedDetails);
  await expect(page.locator('#crust-note')).toHaveText(crustExplanation);
  await expect(page.locator('#elevation-details')).toHaveText(elevationExplanation);
  await expect(page.locator('#water-note')).toHaveText(waterExplanation);
  await expect(page.locator('#drainage-note')).toHaveText(drainageExplanation);
  await ui('#exaggeration').selectOption('0');
  await expect(canvas).toHaveAttribute('data-exaggeration', '0');
  await canvas.click();
  await expect(canvas).toHaveAttribute('data-picked-surface', 'water');
  await page.setViewportSize({ width: 1600, height: 1000 });
  await ui('#exaggeration').selectOption('25');
  await expect(canvas).toHaveAttribute('data-exaggeration', '25');
  await expect(page.locator('#selection-details')).toHaveText(selectedDetails);
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await ui('#exaggeration').selectOption('10');
  await canvas.click();
  await expect(page.locator('#selection-title')).toHaveText(selectedRegion);
  await expect(page.locator('button[data-view="globe"]')).toHaveAttribute('aria-pressed', 'true');
  await expect(canvas).toHaveAttribute('data-picked-surface', 'water');
  await ui('[data-layer="depth"]').click();
  await canvas.click();
  await expect(canvas).toHaveAttribute('data-picked-surface', 'bed');
  await expect(page.locator('#selection-title')).toHaveText(selectedRegion);
  await expect(page.locator('#selection-details')).toHaveText(selectedDetails);
  await ui('[data-layer="surface"]').click();
  await page.locator('#boundaries').check();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('#boundaries').uncheck();
  await mkdir('artifacts', { recursive: true });
  await page.screenshot({ path: executablePath ? 'artifacts/globe-desktop-packaged.png' : 'artifacts/globe-desktop.png' });
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-view', 'flat');
  await ui('[data-layer="catchments"]').click();
  await expect(page.locator('#legend-title')).toContainText('Drainage catchments');
  await expect(page.locator('#legend-scale')).toBeHidden();
  await expect(page.locator('#boundary-legend')).toBeHidden();
  await ui('[data-layer="contributingArea"]').click();
  await expect(page.locator('#legend-title')).toContainText('not river discharge');
  await expect(page.locator('#legend-low')).toHaveText('0 km²');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await ui('[data-layer="waterBodies"]').click();
  await expect(page.locator('#legend-title')).toContainText('Connected water bodies');
  await expect(page.locator('#boundary-legend')).toBeHidden();
  await expect(page.locator('#legend-scale')).toBeHidden();
  await ui('[data-layer="depth"]').click();
  await expect(page.locator('#legend-low')).toHaveText('0 m');
  await expect(page.locator('#legend-gradient')).toHaveClass('water-gradient');
  await ui('[data-layer="thickness"]').click();
  await expect(page.locator('#legend-low')).toHaveText('7 km');
  await expect(page.locator('#legend-high')).toHaveText('35 km');
  await ui('[data-layer="uplift"]').click();
  await expect(page.locator('#legend-title')).toContainText('Convergence uplift');
  await expect(page.locator('#legend-high')).toHaveText('12,000 m');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await ui('[data-layer="boundaries"]').click();
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
  await ui('[data-layer="catchments"]').click();
  await clickAtlas(desktopData.drainageSample);
  await expect(page.locator('#selection-title')).toHaveText(`Region ${desktopData.drainageSample.id.toLocaleString('en')}`);
  await expect(page.locator('#drainage-note')).toContainText('Steepest bed descent');
  await page.screenshot({ path: executablePath ? 'artifacts/catchments-desktop-packaged.png' : 'artifacts/catchments-desktop.png' });
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(page.locator('#drainage-note')).toContainText('Steepest bed descent');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await ui('[data-layer="speed"]').click();
  await expect(page.locator('#legend-high')).toHaveText('8 cm/year');
  await ui('[data-layer="basins"]').click();
  await expect(page.locator('#legend-scale')).toBeHidden();
  await expect(page.locator('#legend-title')).toContainText('exclusive ownership');
  await clickAtlas(desktopData.basinSample);
  await expect(page.locator('#selection-title')).toHaveText(`Region ${desktopData.basinSample.id.toLocaleString('en')}`);
  const basinRegionTitle = await page.locator('#selection-title').textContent().then(value => value ?? '');
  const basinDetails = await page.locator('#basin-details').textContent().then(value => value ?? '');
  await expect(page.locator('#basin-title')).toContainText(`Branch ${desktopData.basinSample.node} ·`);
  await expect(page.locator('#basin-details')).toContainText('not current water level');
  await expect(page.locator('#basin-details')).toContainText('includes children');
  await expect(canvas).toHaveAttribute('data-spill-from', String(desktopData.basinSample.from));
  await expect(canvas).toHaveAttribute('data-spill-to', String(desktopData.basinSample.to));
  await page.screenshot({ path: executablePath ? 'artifacts/basins-desktop-packaged.png' : 'artifacts/basins-desktop.png' });
  await ui('#basin-parent').click();
  await expect(page.locator('#basin-title')).toContainText(`Branch ${desktopData.basinSample.parent} ·`);
  await expect(page.locator('#selection-title')).toHaveText(basinRegionTitle);
  const beforeBasinPlayback = await page.locator('#diagnostic-tick').textContent().then(value => value ?? '');
  await ui('#play').click();
  await expect(page.locator('#diagnostic-tick')).not.toHaveText(beforeBasinPlayback);
  await ui('#play').click();
  await expect(page.locator('#basin-title')).toContainText(`Branch ${desktopData.basinSample.parent} ·`);
  await ui('#basin-owner').click();
  await expect(page.locator('#basin-details')).toHaveText(basinDetails);
  await page.locator('#basin-details').scrollIntoViewIfNeeded();
  await page.screenshot({ path: executablePath ? 'artifacts/basin-inspector-packaged.png' : 'artifacts/basin-inspector.png' });
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(page.locator('#basin-details')).toHaveText(basinDetails);
  await ui('[data-layer="spill"]').click();
  await expect(page.locator('#legend-title')).toContainText('not current water level');
  await expect(page.locator('#legend-scale')).toBeVisible();
  await expect(page.locator('#boundary-legend')).toBeHidden();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.screenshot({ path: executablePath ? 'artifacts/spill-globe-packaged.png' : 'artifacts/spill-globe.png' });
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await ui('[data-layer="speed"]').click();
  await expect(page.locator('#boundary-legend')).toBeHidden();
  await page.getByRole('button', { name: 'Zoom in' }).click();
  await page.getByRole('button', { name: 'Fit map' }).click();
  await ui('[data-layer="area"]').click();
  await expect(page.locator('#legend-title')).toContainText('Region area');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await ui('[data-layer="signal"]').click();
  await ui('#play').click();
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
  await ui('#save-recipe').click();
  await expect(page.locator('#status')).toContainText('Recipe saved');
  assert.deepEqual(parseRecipe(JSON.parse(await readFile(recipePath, 'utf8'))), DEFAULT_RECIPE);

  await ui('#seed').fill('desktop-roundtrip');
  await ui('#subdivision').selectOption('3');
  const advancedSettings = page.locator('details.advanced-settings');
  if (await advancedSettings.count()) await advancedSettings.locator('summary').click();
  await ui('#plate-count').fill('7');
  await ui('#plate-speed').fill('0');
  await ui('#continental-fraction').fill('0');
  await ui('#continental-scale').fill('2');
  await ui('#relief-scale').fill('0');
  await ui('#detail-amplitude').fill('0');
  await ui('#boundary-width').fill('500');
  await ui('#water-mode').selectOption('volume');
  await expect(page.locator('#water-coverage')).toBeDisabled();
  await ui('#water-volume').fill('0');
  await ui('#generate').click();
  await expect(page.locator('#world-name')).toHaveText('desktop-roundtrip');
  await expect(page.locator('#region-count')).toHaveText('642');
  await expect(page.locator('#crust-summary')).toContainText('0.00% actual / 0.00% target');
  await expect(page.locator('#water-summary')).toContainText('0.00% actual / 0 km³ requested');
  await expect(page.locator('#drainage-summary')).toContainText('1 terminal catchments · 1 closed dry sinks');
  await expect(page.locator('#basin-summary')).toContainText('1 minima · 1 hierarchy branches');
  await expect(page.locator('#basin-title')).toHaveText('Select a region');
  await expect(page.locator('#basin-parent')).toBeDisabled();
  await ui('[data-layer="surface"]').click();
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await canvas.click();
  await expect(canvas).toHaveAttribute('data-picked-surface', 'bed');
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await expect(page.locator('#basin-title')).toContainText('global root');
  await expect(page.locator('#basin-details')).toContainText('No finite spill capacity');
  await expect(page.locator('#basin-parent')).toBeDisabled();
  await ui('#water-spill').click();
  await expect(page.locator('#status')).toContainText('no spill threshold');
  await expect(page.locator('#water-step')).toContainText('Select a source region');
  await expect(canvas).toHaveAttribute('data-spill-from', '-1');
  await ui('[data-layer="spill"]').click();
  await expect(page.locator('#legend-low')).toHaveText('No finite thresholds');
  await ui('[data-layer="plates"]').click();
  await expect(page.locator('#legend-title')).toContainText('7 connected plates');
  await ui('[data-layer="speed"]').click();
  await expect(page.locator('#legend-high')).toHaveText('0 cm/year');
  assert.notEqual(await page.locator('#fingerprint').textContent().then(value => value ?? ''), fingerprint);
  // A uniform bed with positive volume is fully wet; save/import retains volume mode.
  await ui('#water-volume').fill('1000000');
  await ui('#generate').click();
  await expect(page.locator('#water-summary')).toContainText('100.00% actual / 1,000,000 km³ requested');
  await expect(page.locator('#drainage-summary')).toContainText('1 terminal catchments · 0 closed dry sinks');
  const volumeHash = await page.locator('#fingerprint').textContent().then(value => value ?? '');
  await ui('[data-layer="surface"]').click();
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  for (const factor of ['0', '1', '10']) {
    await ui('#exaggeration').selectOption(factor);
    await canvas.click();
    await expect(canvas).toHaveAttribute('data-picked-surface', 'water');
    await expect(page.locator('#fingerprint')).toHaveText(volumeHash);
  }
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await ui('#save-recipe').click();
  await expect(page.locator('#status')).toContainText('Recipe saved');
  const volumeRecipe = parseRecipe(JSON.parse(await readFile(recipePath, 'utf8')));
  assert.deepEqual(volumeRecipe.water, { mode: 'volume', volumeCubicMeters: 1e15 });
  await ui('#water-volume').fill('0');
  await ui('#generate').click();
  await expect(page.locator('#water-summary')).toContainText('0.00% actual / 0 km³ requested');
  await ui('#open-recipe').click();
  await expect(page.locator('#water-mode')).toHaveValue('volume');
  await expect(page.locator('#water-volume')).toHaveValue('1000000');
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
  await expect(page.locator('#fingerprint')).toHaveText(volumeHash);
  await writeFile(recipePath, JSON.stringify(DEFAULT_RECIPE));
  await ui('#open-recipe').click();
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
  await ui('#open-recipe').click();
  await expect(page.locator('#fingerprint')).toHaveText(continuousHash, { timeout: 30_000 });
  await expect(page.locator('#model-label')).toContainText('CONTINUOUS-PLATES-1');
  await ui('#save-recipe').click();
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
  await ui('#open-recipe').click();
  await expect(page.locator('#fingerprint')).toHaveText(legacyHash, { timeout: 30_000 });
  await expect(page.locator('#model-label')).toContainText('BASINS-1');
  await expect(page.locator('#terrain-preparation-passes')).toHaveValue('0');
  await ui('#save-recipe').click();
  assert.deepEqual(parseRecipe(JSON.parse(await readFile(recipePath, 'utf8'))), legacyRecipe);
  await writeFile(recipePath, JSON.stringify(DEFAULT_RECIPE));
  await ui('#open-recipe').click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });
  await expect(page.locator('#terrain-preparation-passes')).toHaveValue('4');

  // Experimental small planets must limit display distortion, never physical heights.
  await ui('#radius').fill('100');
  await ui('#subdivision').selectOption('2');
  await ui('#generate').click();
  await expect(page.locator('#region-count')).toHaveText('162');
  const smallHash = await page.locator('#fingerprint').textContent().then(value => value ?? '');
  await ui('#exaggeration').selectOption('50');
  await expect(page.locator('#exaggeration-note')).toContainText('20% radius display limit');
  assert.ok(Number(await canvas.getAttribute('data-exaggeration')) < 50);
  await expect(page.locator('#fingerprint')).toHaveText(smallHash);
  await ui('#exaggeration').selectOption('10');
  await ui('#open-recipe').click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });

  await writeFile(recipePath, JSON.stringify({ ...DEFAULT_RECIPE, modelVersion: 'future' }));
  await ui('#open-recipe').click();
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
  const latestHash = await page.locator('#fingerprint').textContent().then(value => value ?? '');
  await page.evaluate(() => {
    document.querySelector<HTMLSelectElement>('#subdivision')!.value = '6';
    document.querySelector<HTMLFormElement>('#recipe-form')!.requestSubmit();
    document.querySelector<HTMLButtonElement>('#cancel')!.click();
  });
  await expect(page.locator('#status')).toContainText('canceled');
  await expect(page.locator('#fingerprint')).toHaveText(latestHash);
  await ui('#play').click();
  await expect(page.locator('#diagnostic-tick')).not.toHaveText('Step 0');
  await ui('#play').click();
  await ui('#seed').fill(DEFAULT_RECIPE.seed);
  await ui('#subdivision').selectOption(String(DEFAULT_RECIPE.subdivision));
  await ui('#generate').click();
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
  await ui('#subdivision').selectOption('2');
  await ui('#water-mode').selectOption('coverage');
  await ui('#water-coverage').fill('0');
  await ui('#generate').click();
  await expect(page.locator('#region-count')).toHaveText('162');
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await page.getByRole('button', { name: 'Fit map' }).click();
  await clickAtlas(spillSample);
  await expect(page.locator('#selection-title')).toHaveText(`Region ${spillSample.id}`);
  await ui('#water-spill').click();
  await expect(page.locator('#water-step')).toContainText('Prescribed step 1');
  await expect(page.locator('#water-budget-note')).toContainText('Prescribed step 1');
  await expect(page.locator('#water-budget-details')).toContainText('Destination branch stock');
  await expect(page.locator('#water-budget-details')).toContainText('0 units');
  await expect(page.locator('#water-summary')).toContainText('manual input');
  await expect(page.locator('#selection-details')).toContainText('Displayed prescribed-water depth');
  await expect(page.locator('#moisture-start')).toBeDisabled();
  await ui('[data-layer="surface"]').click();
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await page.screenshot({ path: executablePath ? 'artifacts/spill-desktop-packaged.png' : 'artifacts/spill-desktop.png' });
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  const waterCheckpointPath = path.join(temp, 'water-checkpoint.json');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, waterCheckpointPath);
  await ui('#save-water').click();
  await expect(page.locator('#status')).toContainText('checkpoint saved');
  const savedWater = JSON.parse(await readFile(waterCheckpointPath, 'utf8'));
  assert.equal(savedWater.step, 1);
  assert.equal(savedWater.origin.originRecipe.water.fraction, 0);
  await ui('#water-add').click();
  await expect(page.locator('#water-step')).toContainText('Prescribed step 2');
  const continuedStep = await page.locator('#water-step').textContent().then(value => value ?? '');
  const continuedBudget = await page.locator('#water-budget-details').textContent().then(value => value ?? '');
  await ui('#seed').fill('unsaved-form-edit');
  await ui('#water-coverage').fill('17');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, waterCheckpointPath);
  await ui('#open-water').click();
  await expect(page.locator('#status')).toContainText('Water checkpoint restored');
  await expect(page.locator('#water-step')).toContainText('Prescribed step 1');
  await expect(page.locator('#water-budget-note')).toContainText('Prescribed step 1');
  await expect(page.locator('#region-count')).toHaveText('162');
  await expect(page.locator('#seed')).toHaveValue(DEFAULT_RECIPE.seed);
  await expect(page.locator('#water-coverage')).toHaveValue('0');
  await page.getByRole('button', { name: 'Fit map' }).click();
  await clickAtlas(spillSample);
  await ui('#water-add').click();
  await expect(page.locator('#water-step')).toHaveText(continuedStep);
  await expect(page.locator('#water-budget-details')).toHaveText(continuedBudget);
  const damagedCheckpointPath = path.join(temp, 'damaged-water-checkpoint.json');
  await writeFile(damagedCheckpointPath, JSON.stringify({ ...savedWater, acceptedInputUnits: '0' }));
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, damagedCheckpointPath);
  await ui('#open-water').click();
  await expect(page.locator('#status')).toContainText('balance');
  await expect(page.locator('#water-step')).toHaveText(continuedStep);
  await expect(page.locator('#region-count')).toHaveText('162');
  const oversizedCheckpointPath = path.join(temp, 'oversized-water-checkpoint.json');
  await writeFile(oversizedCheckpointPath, Buffer.alloc(8 * 2 ** 20 + 1));
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, oversizedCheckpointPath);
  await ui('#open-water').click();
  await expect(page.locator('#status')).toContainText('exceeds the 8 MiB limit');
  await expect(page.locator('#water-step')).toHaveText(continuedStep);
  await ui('#water-coverage').fill('71');
  await ui('#subdivision').selectOption(String(DEFAULT_RECIPE.subdivision));
  await ui('#generate').click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });
  await canvas.click();
  // Use the real native calculation, bridge, GPU fields, and inspector. Seasonal
  // playback must not mutate the original geography or manual inventory.
  await expect(page.locator('#moisture-start')).toBeEnabled();
  await expect(page.locator('[data-layer="vaporWater"]')).toBeDisabled();
  await ui('#moisture-start').click();
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '0');
  await expect(page.locator('#moisture-budget-details')).toContainText('Initial mobile partition');
  await expect(page.locator('#moisture-selection-details')).toContainText('Soil water');
  await expect(page.locator('#water-add')).toBeDisabled();
  await expect(page.locator('#save-water')).toBeDisabled();
  await expect(page.locator('#water-budget-refresh')).toBeDisabled();
  await expect(page.locator('#play')).toBeDisabled();
  await ui('[data-layer="vaporWater"]').click();
  await expect(page.locator('#legend-title')).toContainText('fixed logarithmic scale');
  await expect(page.locator('#legend-high')).toHaveText('≥ 60 mm WE');
  const initialVaporImage = await canvas.screenshot();
  await ui('#moisture-interval').selectOption('3600');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '3600');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await expect(page.locator('#moisture-step-note')).toContainText('Last interval 1 h');
  await ui('#moisture-interval').selectOption('86400');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '90000');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  const seasonalDetails = await page.locator('#moisture-selection-details').textContent().then(value => value ?? '');
  const seasonalBudget = await page.locator('#moisture-budget-details').textContent().then(value => value ?? '');
  assert.notDeepEqual(await canvas.screenshot(), initialVaporImage, 'Vapor texture updates must change the rendered map, not just inspector values.');
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await ui('[data-layer="terminalWater"]').click();
  await expect(page.locator('#legend-title')).toContainText('not lake depth');
  await expect(page.locator('#moisture-selection-details')).toHaveText(seasonalDetails);
  await expect(page.locator('#moisture-budget-details')).toHaveText(seasonalBudget);
  await ui('#temperature-month').selectOption('9');
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '90000');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await canvas.screenshot({ path: executablePath ? 'artifacts/seasonal-water-globe-packaged.png' : 'artifacts/seasonal-water-globe.png' });
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await ui('[data-layer="precipitation"]').click();
  await expect(page.locator('#legend-high')).toHaveText('≥ 20 mm/day');
  const beforePlay = Number(await canvas.getAttribute('data-moisture-seconds'));
  await ui('#moisture-play').click();
  await expect(canvas).not.toHaveAttribute('data-moisture-seconds', String(beforePlay));
  await expect(page.locator('#save-seasonal')).toBeDisabled();
  await expect(page.locator('#open-seasonal')).toBeDisabled();
  await ui('#moisture-play').click();
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  const pausedTime = await canvas.getAttribute('data-moisture-seconds');
  await page.waitForTimeout(150);
  await expect(canvas).toHaveAttribute('data-moisture-seconds', pausedTime!);
  // A pending old-world step is kept when replacement generation is canceled.
  await page.evaluate(() => {
    document.querySelector<HTMLButtonElement>('#moisture-play')!.click();
    document.querySelector<HTMLInputElement>('#seed')!.value = 'canceled-seasonal-replacement';
    document.querySelector<HTMLFormElement>('#recipe-form')!.requestSubmit();
  });
  await page.locator('#replace-world').click();
  await page.locator('#cancel').click();
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
    const id = Number((await page.locator('#moisture-selection-title').textContent().then(value => value ?? '')).match(/Region (\d+)/)![1]);
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
  await ui('[data-layer="vaporWater"]').click();
  await canvas.screenshot({ path: executablePath ? 'artifacts/seasonal-water-map-packaged.png' : 'artifacts/seasonal-water-map.png' });
  // Persist complete native state through real file I/O, not a mocked bridge.
  const seasonalPath = path.join(temp, 'seasonal-water-checkpoint.json');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, seasonalPath);
  await expect(page.locator('#save-seasonal')).toBeEnabled();
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText(`saved at ${seasonalSeconds} elapsed seconds`);
  const savedSeasonalText = await readFile(seasonalPath, 'utf8'), savedSeasonal = JSON.parse(savedSeasonalText);
  assert.equal(savedSeasonal.schemaVersion, 3);
  assert.equal(savedSeasonal.elapsedSeconds, seasonalSeconds);
  assert.equal(savedSeasonal.surfaceKilograms.length, 10242);
  assert.equal(savedSeasonal.cumulativeSurfaceTransferRoundoff.length, 10242);
  const savedSeasonalBudget = await page.locator('#moisture-budget-details').textContent().then(value => value ?? '');
  await app.evaluate(({ dialog }) => { dialog.showSaveDialog = async () => ({ canceled: true, filePath: '' }); });
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('saving canceled');
  assert.equal(await readFile(seasonalPath, 'utf8'), savedSeasonalText);
  // A failed rename must not replace a target directory or discard native state.
  const deniedSeasonalPath = path.join(temp, 'checkpoint-directory'); await mkdir(deniedSeasonalPath);
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, deniedSeasonalPath);
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toHaveClass(/error/);
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds));
  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
  }, seasonalPath);
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText(`saved at ${seasonalSeconds + 86400} elapsed seconds`);
  const uninterrupted = await readFile(seasonalPath, 'utf8');
  await writeFile(seasonalPath, savedSeasonalText);
  // Failed/canceled file operations preserve both visible and native state.
  const damagedSeasonalPath = path.join(temp, 'damaged-seasonal.json');
  await writeFile(damagedSeasonalPath, JSON.stringify({ ...savedSeasonal, vaporKilograms: [] }));
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, damagedSeasonalPath);
  await ui('#open-seasonal').click();
  await expect(page.locator('#status')).toHaveClass(/error/);
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), uninterrupted);
  await writeFile(seasonalPath, savedSeasonalText);
  await app.evaluate(({ dialog }) => { dialog.showOpenDialog = async () => ({ canceled: true, filePaths: [] }); });
  await ui('#open-seasonal').click();
  await expect(page.locator('#status')).toContainText('opening canceled');
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  const oversizedSeasonalPath = path.join(temp, 'oversized-seasonal.json');
  const oversizedSeasonalFile = await open(oversizedSeasonalPath, 'w');
  try { await oversizedSeasonalFile.truncate(MAX_SEASONAL_CHECKPOINT_BYTES + 1); }
  finally { await oversizedSeasonalFile.close(); }
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, oversizedSeasonalPath);
  await ui('#open-seasonal').click();
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
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), uninterrupted);
  await writeFile(seasonalPath, savedSeasonalText);
  // Restore into a different accepted world; its saved origin must win, not the
  // unsaved form values or current geography. Both GPU views remain usable.
  await ui('#seed').fill('world-before-seasonal-restore');
  await ui('#subdivision').selectOption('2');
  await ui('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('#region-count')).toHaveText('162');
  await app.evaluate(({ dialog }, destination) => {
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, seasonalPath);
  await ui('#open-seasonal').click();
  await expect(page.locator('#status')).toContainText(`restored at ${seasonalSeconds} elapsed seconds`, { timeout: 30000 });
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds));
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await expect(page.locator('#seed')).toHaveValue(DEFAULT_RECIPE.seed);
  await expect(page.locator('#subdivision')).toHaveValue(String(DEFAULT_RECIPE.subdivision));
  await expect(page.locator('#moisture-budget-details')).toHaveText(savedSeasonalBudget);
  await expect(page.locator('#moisture-step-note')).toContainText('no interval transfers');
  await expect(page.locator('#save-water')).toBeDisabled();
  await ui('[data-layer="vaporWater"]').click();
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-active-layer', 'vaporWater');
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds));
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), savedSeasonalText);
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', String(seasonalSeconds + 86400));
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), uninterrupted, 'GUI save/load retains exact uninterrupted full-checkpoint continuation.');
  await ui('#seed').fill(DEFAULT_RECIPE.seed);
  await ui('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await expect(page.locator('[data-layer="vaporWater"]')).toBeDisabled();
  await expect(page.locator('#moisture-time')).toContainText('Not initialized');
  await expect(page.locator('#moisture-budget-details')).toBeEmpty();
  // Select the experimental mode before initialization. Save/load must retain
  // its version rather than silently returning to the baseline calculation.
  await expect(page.locator('#moisture-start-orographic')).toBeEnabled();
  await ui('#moisture-start-orographic').click();
  await expect(page.locator('#model-label')).toContainText('SEASONAL-MOISTURE-4');
  await expect(page.locator('#moisture-budget-note')).toContainText('not calibrated');
  await expect(page.locator('#moisture-start')).toBeDisabled();
  await expect(page.locator('#moisture-start-orographic')).toBeDisabled();
  await ui('#moisture-interval').selectOption('3600');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '3600');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  const orographicText = await readFile(seasonalPath, 'utf8');
  assert.equal(JSON.parse(orographicText).schemaVersion, 4);
  assert.equal(JSON.parse(orographicText).orographicModelVersion, 'orographic-response-1');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '7200');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  const orographicContinued = await readFile(seasonalPath, 'utf8');
  await writeFile(seasonalPath, orographicText);
  await ui('#open-seasonal').click();
  await expect(page.locator('#status')).toContainText('restored at 3600 elapsed seconds');
  await expect(page.locator('#model-label')).toContainText('SEASONAL-MOISTURE-4');
  await ui('[data-layer="vaporWater"]').click();
  await expect(canvas).toHaveAttribute('data-active-layer', 'vaporWater');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '7200');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), orographicContinued);
  await ui('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('#moisture-start-precise')).toBeEnabled();
  await ui('#moisture-start-precise').click();
  await expect(page.locator('#model-label')).toContainText('SEASONAL-MOISTURE-7');
  await expect(page.locator('#moisture-start-orographic')).toBeDisabled();
  await expect(page.locator('#moisture-start-precise')).toBeDisabled();
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await ui('[data-layer="soilWater"]').click();
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '3600');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  const preciseText = await readFile(seasonalPath, 'utf8'), precise = JSON.parse(preciseText);
  assert.equal(precise.schemaVersion, 7); assert.equal(precise.terminalStockModelVersion, 'terminal-stock-compensated-1');
  for (const key of ['surfaceLowKilograms', 'snowLowKilograms', 'soilLowKilograms', 'terminalLowKilograms']) {
    assert.equal(precise[key].length, 10242);
  }
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-active-layer', 'soilWater');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '7200');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  const preciseContinued = await readFile(seasonalPath, 'utf8');
  await writeFile(seasonalPath, preciseText);
  await ui('#open-seasonal').click();
  await expect(page.locator('#status')).toContainText('restored at 3600 elapsed seconds');
  await expect(page.locator('#model-label')).toContainText('SEASONAL-MOISTURE-7');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '7200');
  await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), preciseContinued);
  // The new ownership mode must work through the real bridge and file dialogs,
  // without relabeling the old terminal-column mode or mutating initial relief.
  await ui('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('[data-layer="regionalDepth"]')).toBeDisabled();
  await ui('#moisture-start-regional').click();
  await expect(page.locator('#model-label')).toContainText('SEASONAL-MOISTURE-15');
  await expect(canvas).toHaveAttribute('data-regional-surface', 'active');
  await expect(page.locator('[data-layer="terminalWater"]')).toBeDisabled();
  await expect(page.locator('#moisture-budget-note')).toContainText('counted once per body');
  await ui('#moisture-interval').selectOption('3600');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '3600');
  for (const mode of ['2D map', 'Globe']) {
    await page.getByRole('button', { name: mode, exact: true }).click();
    for (const layer of ['regionalDepth', 'surfaceInflow', 'surfaceOutflow', 'surface']) {
      await ui(`[data-layer="${layer}"]`).click();
      await expect(canvas).toHaveAttribute('data-active-layer', layer);
    }
  }
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  const regionalText = await readFile(seasonalPath, 'utf8');
  assert.equal(JSON.parse(regionalText).schemaVersion, 15);
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '7200');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  const regionalContinued = await readFile(seasonalPath, 'utf8');
  await writeFile(seasonalPath, regionalText);
  await ui('#open-seasonal').click();
  await expect(page.locator('#status')).toContainText('restored at 3600 elapsed seconds');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '7200');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), regionalContinued);

  // Explicit synthetic, body-funded rainfall exercises visible moving columns.
  // It is not claimed as spontaneous flooding in an ordinary generated world.
  const flowReference = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
  try {
    const generated = await flowReference.generate(await regionalFlowRecipe()); flowReference.accept(generated.epoch);
    await flowReference.initializeRegionalMoisture(generated.epoch);
    const fixture = fundedRegionalCheckpoint(await flowReference.exportSeasonalCheckpoint(generated.epoch), generated.world);
    await writeFile(seasonalPath, fixture);
    const candidate = await flowReference.loadSeasonalCheckpoint(fixture); flowReference.accept(candidate.epoch);
    const expected = await flowReference.seasonalMoisture(candidate.epoch, 3600);
    assert.ok(expected.regionalSurface!.flow.transferredKilograms > 0);
    const expectedText = await flowReference.exportSeasonalCheckpoint(candidate.epoch);
    await ui('#open-seasonal').click();
    await expect(page.locator('#status')).toContainText('restored at 900 elapsed seconds');
    await expect(page.locator('#fingerprint')).toHaveText(generated.world.checksum);
    await page.getByRole('button', { name: '2D map', exact: true }).click();
    await ui('[data-layer="surfaceInflow"]').click();
    const beforeFlowImage = await canvas.screenshot();
    await ui('#moisture-step').click();
    await expect(canvas).toHaveAttribute('data-moisture-seconds', '4500');
    assert.notDeepEqual(await canvas.screenshot(), beforeFlowImage, 'Actual surface transfers must update the cumulative inflow texture.');
    const centers = generated.world.surface.centers, region = 8;
    await clickAtlas({ x: Math.atan2(centers[region * 3 + 2], centers[region * 3]) / Math.PI,
      y: Math.asin(centers[region * 3 + 1]) / Math.PI });
    await expect(page.locator('#moisture-selection-title')).toContainText(`Region ${region}`);
    const format = new Intl.NumberFormat('en', { maximumSignificantDigits: 6 });
    await expect(page.locator('#moisture-selection-details')).toContainText(`${format.format(expected.regionalSurface!.depthMeters[region])} m`);
    const landDetails = await page.locator('#moisture-selection-details').textContent().then(value => value ?? '');
    await ui('[data-layer="surfaceInflow"]').click();
    await expect(page.locator('#legend-title')).toContainText('not a stock');
    await page.getByRole('button', { name: 'Globe', exact: true }).click();
    await ui('[data-layer="surface"]').click();
    await expect(page.locator('#moisture-selection-details')).toHaveText(landDetails);
    await expect(page.locator('#legend-title')).toContainText('initial reference heads prescribed');
    await expect(canvas).toHaveAttribute('data-regional-surface', 'active');
    await ui('#save-seasonal').click();
    await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
    assert.equal(await readFile(seasonalPath, 'utf8'), expectedText);
    await canvas.screenshot({ path: executablePath ? 'artifacts/regional-water-globe-packaged.png' : 'artifacts/regional-water-globe.png' });
  } finally { flowReference.close(); }
  await ui('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('#moisture-start')).toBeEnabled();
  await expect(page.locator('[data-layer="regionalDepth"]')).toBeDisabled();
  await expect(canvas).not.toHaveAttribute('data-regional-surface', 'active');
  await ui('#moisture-start-soil').click();
  await expect(canvas).toHaveAttribute('data-seasonal-model', 'regional-seasonal-water-1');
  await expect(page.locator('#moisture-budget-note')).toContainText('five paired regional owners');
  await expect(page.locator('#moisture-budget-details')).toContainText('Deferred numerical requests');
  await expect(page.locator('[data-layer="runoffWater"]')).toContainText('Delayed soil drainage');
  await expect(page.locator('[data-layer="liquidWater"]')).toContainText('Unified terrestrial liquid');
  await expect(page.locator('[data-layer="terminalWater"]')).toBeDisabled();
  await expect(page.locator('#moisture-start-regional')).toBeDisabled();
  await ui('#moisture-interval').selectOption('3600');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '3600');
  for (const view of ['2D map', 'Globe']) {
    await page.getByRole('button', { name: view, exact: true }).click();
    for (const layer of ['liquidWater', 'snowWater', 'soilWater', 'runoffWater', 'vaporWater', 'precipitation', 'runoffFlow', 'regionalDepth', 'surfaceInflow', 'surfaceOutflow', 'surface']) {
      await ui(`[data-layer="${layer}"]`).click();
      await expect(canvas).toHaveAttribute('data-active-layer', layer);
    }
  }
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  const soilSaved = await readFile(seasonalPath, 'utf8');
  assert.equal(JSON.parse(soilSaved).modelVersion, 'regional-seasonal-water-1');
  assert.equal(JSON.parse(soilSaved).schemaVersion, 1);
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '7200');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  const soilContinued = await readFile(seasonalPath, 'utf8');
  await writeFile(seasonalPath, soilSaved);
  await ui('#open-seasonal').click();
  await expect(page.locator('#status')).toContainText('restored at 3600 elapsed seconds');
  await ui('#moisture-step').click();
  await expect(canvas).toHaveAttribute('data-moisture-seconds', '7200');
  await ui('#save-seasonal').click();
  await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
  assert.equal(await readFile(seasonalPath, 'utf8'), soilContinued);

  // A real neighboring contact funds this supplied active-flow soil stress case.
  const soilReference = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
  try {
    const generated = await soilReference.generate(await regionalFlowRecipe()); soilReference.accept(generated.epoch);
    await soilReference.initializeSoilMoisture(generated.epoch);
    const fixture = fundedSoilCheckpoint(await soilReference.exportSeasonalCheckpoint(generated.epoch), generated.world);
    const candidate = await soilReference.loadSoilCheckpoint(fixture); soilReference.accept(candidate.epoch);
    const expected = await soilReference.seasonalSoilMoisture(candidate.epoch, 3600);
    const expectedText = await soilReference.exportSeasonalCheckpoint(candidate.epoch);
    await writeFile(seasonalPath, fixture);
    await ui('#open-seasonal').click();
    await expect(page.locator('#status')).toContainText('restored at 900 elapsed seconds');
    await page.getByRole('button', { name: '2D map', exact: true }).click();
    await ui('[data-layer="surfaceInflow"]').click();
    const before = await canvas.screenshot();
    await ui('#moisture-step').click();
    await expect(canvas).toHaveAttribute('data-moisture-seconds', '4500');
    assert.notDeepEqual(await canvas.screenshot(), before, 'Unified actual face flow must update the map texture.');
    const centers = generated.world.surface.centers;
    const region = generated.world.water.bodyIds.findIndex((id, r) => id === 0 && expected.stocks.soil.high[r] > 0
      && Math.abs(Math.atan2(centers[r * 3 + 2], centers[r * 3]) / Math.PI) < .8);
    assert.ok(region >= 0);
    await clickAtlas({ x: Math.atan2(centers[region * 3 + 2], centers[region * 3]) / Math.PI,
      y: Math.asin(centers[region * 3 + 1]) / Math.PI });
    await expect(page.locator('#moisture-selection-title')).toContainText(`Region ${region}`);
    await expect(page.locator('#moisture-selection-details')).toContainText(`${expected.stocks.soil.high[region]} kg / ${expected.stocks.soil.low[region]} kg`);
    const details = await page.locator('#moisture-selection-details').textContent().then(value => value ?? '');
    await page.getByRole('button', { name: 'Globe', exact: true }).click();
    await ui('[data-layer="surface"]').click();
    await expect(page.locator('#moisture-selection-details')).toHaveText(details);
    await expect(canvas).toHaveAttribute('data-regional-surface', 'active');
    await ui('#save-seasonal').click();
    await expect(page.locator('#status')).toContainText('Complete seasonal checkpoint saved');
    assert.equal(await readFile(seasonalPath, 'utf8'), expectedText);
    await canvas.screenshot({ path: 'artifacts/soil-water-globe.png' });
  } finally { soilReference.close(); }
  await ui('#generate').click();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready');
  await expect(page.locator('#moisture-start-soil')).toBeEnabled();
  await expect(canvas).not.toHaveAttribute('data-seasonal-model', 'regional-seasonal-water-1');
  await ui('[data-layer="surface"]').click();
  await mkdir('artifacts', { recursive: true });
  const screenshot = executablePath ? 'artifacts/surface-desktop-packaged.png' : 'artifacts/surface-desktop.png';
  await page.screenshot({ path: screenshot });
  assert.deepEqual(errors, [], 'No renderer exceptions.');
  console.log(`Desktop checks passed. Desktop fingerprint: ${fingerprint}. Screenshot: ${screenshot}`);
} finally {
  await app.close();
}
