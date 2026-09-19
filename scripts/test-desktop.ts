import { strict as assert } from 'node:assert';
import { mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { _electron as electron, expect } from '@playwright/test';
import { DEFAULT_RECIPE, parseRecipe } from '../src/core/recipe';
import { generateWorld } from '../src/core/world';

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
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
  await expect(page.locator('#region-count')).toHaveText('10,242');
  const fingerprint = await page.locator('#fingerprint').innerText();
  const workerFile = (await readdir('dist/renderer/assets')).find((name) => name.startsWith('world.worker-') && name.endsWith('.js'));
  assert.ok(workerFile);
  const desktopData = await page.evaluate(async ({ filename, recipe }) => {
    return await new Promise<{ checksum: string; fields: Record<string, number[]> }>((resolve, reject) => {
      const worker = new Worker(new URL(`./assets/${filename}`, location.href), { type: 'module' });
      worker.onerror = (event) => { worker.terminate(); reject(new Error(event.message)); };
      worker.onmessage = (event) => {
        if (event.data.type === 'error') { worker.terminate(); reject(new Error(event.data.message)); }
        if (event.data.type !== 'complete') return;
        const world = event.data.world;
        const fields: Record<string, number[]> = { diagnosticField: Array.from(world.diagnosticField) };
        for (const [key, value] of Object.entries(world.surface)) {
          if (ArrayBuffer.isView(value)) fields[key] = Array.from(value as unknown as ArrayLike<number>);
        }
        worker.terminate(); resolve({ checksum: world.checksum, fields });
      };
      worker.postMessage(recipe);
    });
  }, { filename: workerFile, recipe: { ...DEFAULT_RECIPE } });
  assert.equal(desktopData.checksum, fingerprint, 'Independent desktop workers reproduce bit for bit.');
  const reference = generateWorld(DEFAULT_RECIPE);
  let largestRelativeError = 0;
  for (const [key, expected] of Object.entries({ ...reference.surface, diagnosticField: reference.diagnosticField })) {
    if (!ArrayBuffer.isView(expected)) continue;
    const actual = desktopData.fields[key], values = expected as Float64Array | Uint32Array;
    assert.equal(actual.length, values.length);
    for (let i = 0; i < actual.length; i++) {
      if (values instanceof Uint32Array) assert.equal(actual[i], values[i], `${key}[${i}]`);
      else {
        const relative = Math.abs(actual[i] - values[i]) / Math.max(1, Math.abs(values[i]));
        largestRelativeError = Math.max(largestRelativeError, relative);
        assert.ok(relative < 1e-12, `${key}[${i}] differs by ${relative}`);
      }
    }
  }
  console.log(`Cross-runtime comparison: Node ${reference.checksum}, Electron ${fingerprint}, maximum scaled error ${largestRelativeError}.`);
  assert.equal(await page.evaluate(() => typeof (globalThis as unknown as { require?: unknown }).require), 'undefined');
  assert.deepEqual(await page.evaluate(() => Object.keys(window.desktop).sort()), ['openRecipe', 'saveRecipe']);

  const canvas = page.locator('#map');
  const bounds = await canvas.boundingBox();
  assert.ok(bounds);
  await canvas.click({ position: { x: bounds.width / 2, y: bounds.height / 2 } });
  await expect(page.locator('#selection-title')).toContainText('Region');
  await page.getByRole('button', { name: 'Zoom in' }).click();
  await page.getByRole('button', { name: 'Fit map' }).click();
  await page.locator('[data-layer="area"]').click();
  await expect(page.locator('#legend-title')).toContainText('Region area');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);
  await page.locator('[data-layer="signal"]').click();

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
  await page.locator('#generate').click();
  await expect(page.locator('#world-name')).toHaveText('desktop-roundtrip');
  await expect(page.locator('#region-count')).toHaveText('642');
  assert.notEqual(await page.locator('#fingerprint').innerText(), fingerprint);
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });

  await writeFile(recipePath, JSON.stringify({ ...DEFAULT_RECIPE, modelVersion: 'future' }));
  await page.getByRole('button', { name: 'Open recipe' }).click();
  await expect(page.locator('#status')).toContainText('Unsupported recipe version');
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint);

  // Queue replacement jobs synchronously; stale workers cannot publish results.
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
  await page.locator('#seed').fill(DEFAULT_RECIPE.seed);
  await page.locator('#subdivision').selectOption(String(DEFAULT_RECIPE.subdivision));
  await page.locator('#generate').click();
  await expect(page.locator('#fingerprint')).toHaveText(fingerprint, { timeout: 30_000 });
  await canvas.click({ position: { x: bounds.width * .6, y: bounds.height * .5 } });
  await mkdir('artifacts', { recursive: true });
  const screenshot = executablePath ? 'artifacts/surface-desktop-packaged.png' : 'artifacts/surface-desktop.png';
  await page.screenshot({ path: screenshot });
  assert.deepEqual(errors, [], 'No renderer exceptions.');
  console.log(`Desktop checks passed. Desktop fingerprint: ${fingerprint}. Screenshot: ${screenshot}`);
} finally {
  await app.close();
}
