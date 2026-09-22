import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { _electron as electron, expect } from '@playwright/test';
import { MODEL_VERSION } from '../src/core/recipe';

const profile = await mkdtemp(path.join(os.tmpdir(), 'planimulation-benchmark-'));
const env = Object.fromEntries(Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined));
delete env.ELECTRON_RUN_AS_NODE;
const app = await electron.launch({ args: ['.', `--user-data-dir=${profile}`], env, timeout: 30_000 });
app.process().on('exit', (code, signal) => { if (code || signal) console.error(`Electron exited: code=${code}, signal=${signal}`); });
try {
  const page = await app.firstWindow();
  await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
  const results = [];
  for (const level of [5, 6]) {
    await page.locator('#subdivision').selectOption(String(level));
    const start = performance.now();
    await page.locator('#generate').click();
    await expect(page.locator('body')).toHaveAttribute('data-state', 'ready', { timeout: 30_000 });
    const generationAndViewsMs = performance.now() - start;
    for (const mode of ['flat', 'globe']) {
      await page.locator(`button[data-view="${mode}"]`).click();
      await page.locator('#play').click();
      // A plain browser expression avoids tsx's injected function-name helpers
      // crossing Playwright's isolated serialization boundary.
      const sample = await page.evaluate<{
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
      await page.locator('#play').click();
      assertAdvanced(sample.initialStep, sample.finalStep);
      results.push({ level, mode, generationAndViewsMs, ...sample });
    }
  }
  const gpu = await app.evaluate(async ({ app }) => ({ features: app.getGPUFeatureStatus(), info: await app.getGPUInfo('basic'), metrics: app.getAppMetrics() }));
  const report = { date: new Date().toISOString(), modelVersion: MODEL_VERSION, layer: 'plates', cpu: os.cpus()[0]?.model, totalMemory: os.totalmem(),
    note: '180 requested-animation-frame intervals per sample under alternating zoom and native diffusion; not a GPU timer or a future simulation guarantee.', results, gpu };
  await mkdir('artifacts', { recursive: true });
  await writeFile('artifacts/native-desktop-benchmark.json', `${JSON.stringify(report, null, 2)}\n`);
  console.table(results);
  console.log(JSON.stringify(gpu.features));
} finally { await app.close(); }

function assertAdvanced(initial: string | null, final: string | null): void {
  if (initial === final) throw new Error('Native simulation did not advance during camera benchmark.');
}
