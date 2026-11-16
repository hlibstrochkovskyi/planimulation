// Usage: node --import tsx scripts/bench-seasonal.ts [baseline|soil] [days]
// Times native seasonal climate on the new-world recipe: generation, initialization,
// daily steps, checkpoint export and restore. Native request time only, no rendering.
import path from 'node:path';
import { NEW_WORLD_RECIPE } from '../src/core/recipe';
import { NativeSession } from '../src/native/client';

const [model = 'baseline', daysText = '365'] = process.argv.slice(2);
if (model !== 'baseline' && model !== 'soil') throw new Error('Usage: bench-seasonal [baseline|soil] [days]');
const days = Number(daysText);
const session = new NativeSession(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
const timed = async <T>(label: string, action: () => Promise<T>): Promise<T> => {
  const started = performance.now(); const result = await action();
  console.log(`${label}: ${(performance.now() - started).toFixed(0)} ms`); return result;
};
try {
  await timed('generate', () => session.request({ command: 'generate', recipe: NEW_WORLD_RECIPE }));
  await timed('temperature normals', () => session.request({ command: 'seasonalTemperature' }));
  await timed('wind normals', () => session.request({ command: 'seasonalWind' }));
  await timed('initialize', () => session.request(model === 'soil' ? { command: 'initializeSoilMoisture' } : { command: 'seasonalMoisture', seconds: 0 }));
  const steps: number[] = [];
  for (let day = 0; day < days; day++) {
    const started = performance.now();
    await session.request({ command: 'seasonalMoisture', seconds: 86400 });
    steps.push(performance.now() - started);
  }
  steps.sort((a, b) => a - b);
  const total = steps.reduce((sum, value) => sum + value, 0);
  console.log(`${days} daily steps: ${(total / 1000).toFixed(1)} s total, median ${steps[steps.length >> 1].toFixed(0)} ms, max ${steps[steps.length - 1].toFixed(0)} ms`);
  const exported = await timed('export checkpoint', () => session.request({ command: 'exportMoisture' }));
  const json = JSON.stringify(exported.header);
  console.log(`checkpoint header ${(json.length / 1e6).toFixed(1)} MB, bytes ${(exported.bytes.length / 1e6).toFixed(1)} MB`);
} finally { session.close(); }
