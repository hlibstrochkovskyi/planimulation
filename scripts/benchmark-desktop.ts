import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { _electron as electron, expect } from '@playwright/test';
import { MODEL_VERSION } from '../src/core/recipe';
import { MOISTURE_MODEL_VERSION } from '../src/shared/seasonal-moisture';

const seasonal = process.argv[2] === '--seasonal';
if (process.argv.length > 3 || (process.argv[2] && !seasonal)) throw new Error('Usage: benchmark-desktop.ts [--seasonal]');

const profile = await mkdtemp(path.join(os.tmpdir(), 'planimulation-benchmark-'));
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const app = await electron.launch({ args: ['.', `--user-data-dir=${profile}`], env, timeout: 30_000 });
app.process().on('exit', (code, signal) => { if (code || signal) console.error(`Electron exited: code=${code}, signal=${signal}`); });
try {
  const page = await app.firstWindow();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
  const results = [];
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  for (const level of [5, 6]) {
    await page.locator('#subdivision').selectOption(String(level));
    const start = performance.now();
    await page.locator('#generate').click();
    await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
    const generationAndViewsMs = performance.now() - start;
    let seasonalInitializationMs: number | null = null;
    if (seasonal) {
      const initialized = performance.now();
      await page.locator('#moisture-start').click();
      await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused', { timeout: 60_000 });
      seasonalInitializationMs = performance.now() - initialized;
      await page.locator('[data-layer="vaporWater"]').click();
      await page.locator('#map').click();
      for (let day = 1; day <= 30; day++) {
        await page.locator('#moisture-step').click();
        await expect(page.locator('#map')).toHaveAttribute('data-moisture-seconds', String(day * 86400));
        await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
      }
    }
    for (const mode of ['flat', 'globe']) {
      await page.locator(`button[data-view="${mode}"]`).click();
      await page.locator(seasonal ? '#moisture-play' : '#play').click();
      // A plain browser expression avoids tsx's injected function-name helpers
      // crossing Playwright's isolated serialization boundary.
      const sample = seasonal ? await page.evaluate<{
        rafMedianMs: number; rafP95Ms: number; rafMaxMs: number; initialStep: string | null; finalStep: string | null;
        observedRequests: number; requestMedianMs: number; requestP95Ms: number; requestMaxMs: number;
        relativeMassResidual: number; relativeLedgerResidual: number;
      }>(`(async () => {
        const intervals = [], requests = [];
        const canvas = document.querySelector('#map'), time = document.querySelector('#moisture-time');
        const initialStep = canvas.dataset.moistureSeconds;
        let last = 0, previousStep = initialStep;
        await new Promise((resolve) => {
          requestAnimationFrame(function frame(now) {
            if (last) intervals.push(now - last);
            last = now;
            document.querySelector(intervals.length % 2 ? '#zoom-in' : '#zoom-out').click();
            const step = canvas.dataset.moistureSeconds;
            if (step !== previousStep) { requests.push(Number(time.dataset.requestMilliseconds)); previousStep = step; }
            if (intervals.length < 180) requestAnimationFrame(frame); else resolve();
          });
        });
        intervals.sort((a, b) => a - b); requests.sort((a, b) => a - b);
        const budget = document.querySelector('#moisture-budget-details');
        return { rafMedianMs: intervals[90], rafP95Ms: intervals[170], rafMaxMs: intervals[179],
          initialStep, finalStep: canvas.dataset.moistureSeconds, observedRequests: requests.length,
          requestMedianMs: requests[Math.floor(requests.length * .5)] ?? null,
          requestP95Ms: requests[Math.floor(requests.length * .95)] ?? null,
          requestMaxMs: requests[requests.length - 1] ?? null,
          relativeMassResidual: Number(budget.dataset.relativeMassResidual),
          relativeLedgerResidual: Number(budget.dataset.relativeLedgerResidual) };
      })()`) : await page.evaluate<{
        rafMedianMs: number; rafP95Ms: number; rafMaxMs: number;
        initialStep: string | null; finalStep: string | null; status: string | null;
      }>(`(async () => {
        const intervals = [];
        const initialStep = document.querySelector('#diagnostic-tick').textContent;
        let last = 0;
        await new Promise((resolve) => {
          requestAnimationFrame(function frame(now) {
            if (last) intervals.push(now - last);
            last = now;
            // Continuous camera changes require fresh rendering, not an idle RAF test.
            document.querySelector(intervals.length % 2 ? '#zoom-in' : '#zoom-out').click();
            if (intervals.length < 180) requestAnimationFrame(frame); else resolve();
          });
        });
        intervals.sort((a, b) => a - b);
        return { rafMedianMs: intervals[90], rafP95Ms: intervals[170], rafMaxMs: intervals[179],
          initialStep, finalStep: document.querySelector('#diagnostic-tick').textContent,
          status: document.querySelector('#status').textContent };
      })()`);
      if (seasonal) await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'running');
      await page.locator(seasonal ? '#moisture-play' : '#play').click();
      if (seasonal) await expect(page.locator('body')).toHaveAttribute('data-seasonal-water', 'paused');
      assertAdvanced(sample.initialStep, sample.finalStep);
      results.push({ level, mode, regionCount: 10 * 4 ** level + 2,
        ...(seasonal ? { seasonalInitializationMs, frameBodyBytes: (10 * 4 ** level + 2) * 18 * 8 } : {}), generationAndViewsMs, ...sample });
    }
  }
  const gpu = await app.evaluate(async ({ app }) => ({ features: app.getGPUFeatureStatus(), info: await app.getGPUInfo('basic'), metrics: app.getAppMetrics() }));
  const layer = await page.locator('[data-layer][aria-pressed="true"]').getAttribute('data-layer');
  const exaggeration = await page.locator('#map').getAttribute('data-exaggeration');
  if (errors.length) throw new Error(`Renderer errors: ${errors.join('; ')}`);
  const report = { date: new Date().toISOString(), modelVersion: MODEL_VERSION,
    ...(seasonal ? { moistureModelVersion: MOISTURE_MODEL_VERSION, callerIntervalSeconds: 86400, warmupDays: 30 } : {}),
    layer, appliedExaggeration: exaggeration, cpu: os.cpus()[0]?.model, totalMemory: os.totalmem(),
    note: seasonal ? '180 requested-animation-frame intervals per sample under alternating zoom, vapor texture updates, a selected-region inspector, and daily native seasonal-water steps after 30 days of warmup. Request timings include IPC/decoding, not rendering; RAF observations are not GPU timers. Observed requests may omit frames between RAF callbacks. Electron process metrics exclude the native process. Not climate calibration, long-run fine-grid validation, or a future throughput guarantee.'
      : '180 requested-animation-frame intervals per sample under alternating zoom and native diffusion; not a GPU timer or a future simulation guarantee.', results, gpu };
  await mkdir('artifacts', { recursive: true });
  await writeFile(seasonal ? 'artifacts/seasonal-desktop-benchmark.json' : 'artifacts/native-desktop-benchmark.json', `${JSON.stringify(report, null, 2)}\n`);
  console.table(results);
  console.log(JSON.stringify(gpu.features));
} finally { await app.close(); }

function assertAdvanced(initial: string | null, final: string | null): void {
  if (initial === final) throw new Error('Native simulation did not advance during camera benchmark.');
}
