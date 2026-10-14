import { strict as assert } from 'node:assert';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';
import type { ElectronApplication } from '@playwright/test';
import { DEFAULT_RECIPE, LEGACY_MODEL_VERSION, MULTISCALE_GEOGRAPHY_MODEL_VERSION, MULTISCALE_UPLANDS_MODEL_VERSION, parseRecipe } from '../src/core/recipe';
import { NativeController } from '../src/native/client';

// Build separately before running this bounded development/packaged smoke.
// Dialogs alone are mocked; generation, picking, validation and file handlers
// still use the real isolated desktop bridge and native implementation.
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const argumentsList = process.argv.slice(2);
const uplands = argumentsList.includes('--uplands');
const executableArguments = argumentsList.filter(arg => arg !== '--uplands');
if (executableArguments.length > 1 || executableArguments.some(arg => arg.startsWith('--'))
  || argumentsList.filter(arg => arg === '--uplands').length > 1) throw new Error('Usage: test-geography-desktop [EXECUTABLE] [--uplands]');
const executablePath = executableArguments[0] ? path.resolve(executableArguments[0]) : undefined;
const modelVersion = uplands ? MULTISCALE_UPLANDS_MODEL_VERSION : MULTISCALE_GEOGRAPHY_MODEL_VERSION;
const temporary = await mkdtemp(path.join(os.tmpdir(), 'planimulation-geography-desktop-'));
const recipePath = path.join(temporary, 'recipe.json');
const artifactPrefix = uplands ? 'geography-uplands' : 'geography-candidate';
const suffix = executablePath ? '-packaged' : '';
const flatScreenshot = `artifacts/${artifactPrefix}-map${suffix}.png`;
const globeScreenshot = `artifacts/${artifactPrefix}-globe${suffix}.png`;
const reportPath = `artifacts/${uplands ? 'geography-uplands' : 'geography'}-desktop-smoke${suffix}.json`;
const reference = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
let app: ElectronApplication | undefined;
const errors: string[] = [];
try {
  app = await electron.launch({ executablePath, args: [...(executablePath ? [] : ['.']), `--user-data-dir=${temporary}`], env, timeout: 30_000 });
  const page = await app.firstWindow();
  page.on('pageerror', error => errors.push(`Page: ${error.message}`));
  page.on('console', message => { if (message.type() === 'error') errors.push(`Console: ${message.text()}`); });
  page.on('crash', () => errors.push('Renderer crashed.'));
  const ready = () => expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
  await ready();
  await page.setViewportSize({ width: 1600, height: 1000 });
  await expect(page.locator('#generator-family')).toHaveValue(DEFAULT_RECIPE.modelVersion);
  await expect(page.locator('#terrain-preparation-passes')).toBeEnabled();
  await expect(page.locator('#region-count')).toHaveText('10,242');
  const originalFingerprint = (await reference.generate(DEFAULT_RECIPE)).world.checksum;
  await expect(page.locator('#fingerprint')).toHaveText(originalFingerprint);
  assert.equal(await page.evaluate(() => typeof (globalThis as unknown as { require?: unknown }).require), 'undefined');

  const candidate = parseRecipe({ ...DEFAULT_RECIPE, modelVersion, subdivision: 6 });
  const nativeCandidate = (await reference.generate(candidate)).world;
  await page.locator('#generator-family').selectOption(modelVersion);
  await page.locator('#subdivision').selectOption(String(candidate.subdivision));
  await expect(page.locator('#generator-note')).toContainText(uplands ? 'Experimental continental uplands' : 'Experimental multiscale');
  await page.locator('#generate').click();
  await ready();
  await expect(page.locator('#fingerprint')).toHaveText(nativeCandidate.checksum, { timeout: 30_000 });
  await expect(page.locator('#model-label')).toContainText(modelVersion.toUpperCase());
  await expect(page.locator('#region-count')).toHaveText(nativeCandidate.stats.regionCount.toLocaleString('en-US'));
  const canvas = page.locator('#map');
  await page.getByRole('button', { name: '2D map', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-view', 'flat');
  await page.getByRole('button', { name: 'Fit map', exact: true }).click();
  await canvas.click();
  await expect(page.locator('#selection-title')).toContainText('Region');
  await expect(page.locator('#crust-note')).toContainText(uplands ? 'additional initial crust thickness' : 'smooth 3D value-noise fBm');
  await expect(page.locator('#crust-note')).toContainText('fixed subdivision-5 reference sphere');
  await expect(page.locator('#elevation-note')).toContainText(uplands ? 'Variable initial continental columns' : 'tectonic-envelope-conditioned');
  await expect(page.locator('#elevation-note')).toContainText('not geological history');
  await expect(page.locator('#elevation-details')).toContainText('Conditioned multiscale detail');
  const selection = await page.locator('#selection-title').innerText();
  const selected = Number(selection.replace(/^Region\s+/, '').replaceAll(',', ''));
  assert.ok(Number.isInteger(selected) && selected >= 0 && selected < nativeCandidate.stats.regionCount);
  await expect(page.locator('#elevation-details')).toContainText(`Total elevation: ${nativeCandidate.terrain.elevation[selected].toFixed(2)} m`);
  if (!uplands) await expect(page.locator('#crust-note')).toContainText(`Threshold ${nativeCandidate.crust.threshold.toFixed(6)}`);
  if (uplands) {
    await page.locator('[data-layer="thickness"]').click();
    await expect(page.locator('#legend-high')).toContainText('≥35 km');
    await expect(page.locator('#legend-title')).toContainText('saturates above 35 km');
    await page.locator('[data-layer="surface"]').click();
  }
  const explanations = await page.locator('#elevation-details').innerText();
  await mkdir('artifacts', { recursive: true });
  const flatPng = await canvas.screenshot({ path: flatScreenshot });
  assert.ok(flatPng.length > 10_000, 'The atlas must not be a blank frame.');
  await page.getByRole('button', { name: 'Globe', exact: true }).click();
  await expect(canvas).toHaveAttribute('data-view', 'globe');
  await expect(page.locator('#selection-title')).toHaveText(selection);
  await expect(page.locator('#elevation-details')).toHaveText(explanations, { useInnerText: true });
  await expect(page.locator('#fingerprint')).toHaveText(nativeCandidate.checksum);
  const globePng = await canvas.screenshot({ path: globeScreenshot });
  assert.ok(globePng.length > 10_000, 'The globe must not be a blank frame.');
  assert.notDeepEqual(globePng, flatPng);

  await app.evaluate(({ dialog }, destination) => {
    dialog.showSaveDialog = async () => ({ canceled: false, filePath: destination });
    dialog.showOpenDialog = async () => ({ canceled: false, filePaths: [destination] });
  }, recipePath);
  await page.locator('#save-recipe').click();
  await expect(page.locator('#status')).toContainText('Recipe saved');
  assert.deepEqual(parseRecipe(JSON.parse(await readFile(recipePath, 'utf8'))), candidate);
  await page.locator('#generator-family').selectOption(DEFAULT_RECIPE.modelVersion);
  await page.locator('#seed').fill('unsaved-selector-change');
  await page.locator('#open-recipe').click();
  await expect(page.locator('#generator-family')).toHaveValue(modelVersion);
  await expect(page.locator('#seed')).toHaveValue(candidate.seed);
  await ready();
  await expect(page.locator('#fingerprint')).toHaveText(nativeCandidate.checksum);
  await page.locator('#generate').click();
  await ready();
  await expect(page.locator('#generator-family')).toHaveValue(modelVersion);
  await expect(page.locator('#fingerprint')).toHaveText(nativeCandidate.checksum);

  const { terrainPreparationPasses: _passes, ...legacyBase } = candidate;
  const legacy = parseRecipe({ ...legacyBase, modelVersion: LEGACY_MODEL_VERSION });
  const legacyFingerprint = (await reference.generate(legacy)).world.checksum;
  await page.locator('#generator-family').selectOption(LEGACY_MODEL_VERSION);
  await expect(page.locator('#terrain-preparation-passes')).toBeDisabled();
  await page.locator('#generate').click();
  await ready();
  await expect(page.locator('#model-label')).toContainText('BASINS-1');
  await expect(page.locator('#fingerprint')).toHaveText(legacyFingerprint);
  await page.locator('#save-recipe').click();
  await expect(page.locator('#status')).toContainText('Recipe saved');
  const savedLegacy = JSON.parse(await readFile(recipePath, 'utf8'));
  assert.equal(Object.hasOwn(savedLegacy, 'terrainPreparationPasses'), false);
  assert.deepEqual(parseRecipe(savedLegacy), legacy);
  await page.locator('#generator-family').selectOption(modelVersion);
  await expect(page.locator('#terrain-preparation-passes')).toBeEnabled();
  await page.locator('#open-recipe').click();
  await expect(page.locator('#generator-family')).toHaveValue(LEGACY_MODEL_VERSION);
  await expect(page.locator('#terrain-preparation-passes')).toBeDisabled();
  await expect(page.locator('#terrain-preparation-passes')).toHaveValue('0');
  await ready();
  await expect(page.locator('#fingerprint')).toHaveText(legacyFingerprint);
  await page.locator('#generate').click();
  await ready();
  await expect(page.locator('#fingerprint')).toHaveText(legacyFingerprint);
  assert.deepEqual(errors, [], 'The smoke must not emit renderer errors.');
  await writeFile(reportPath, `${JSON.stringify({
    scope: 'Bounded desktop smoke, not a performance benchmark or scientific realism validation.',
    originalFingerprint, candidateRecipe: candidate, candidateFingerprint: nativeCandidate.checksum,
    legacyRecipe: legacy, legacyFingerprint, selectedCandidateRegion: selected,
    screenshots: { flat: flatScreenshot, globe: globeScreenshot }, errors,
  }, null, 2)}\n`);
  console.log(`Geography desktop smoke passed. Candidate fingerprint: ${nativeCandidate.checksum}. Screenshots: ${flatScreenshot}, ${globeScreenshot}`);
} finally {
  reference.close();
  try { await app?.close(); }
  finally { await rm(temporary, { recursive: true, force: true }); }
}
