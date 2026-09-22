import { DEFAULT_RECIPE } from '../src/core/recipe';
import { generateWorld } from '../src/core/world';
import { NativeController } from '../src/native/client';
import path from 'node:path';

const results = [];
for (const subdivision of [3, 4, 5, 6]) {
  const core = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));
  try {
    const start = performance.now();
    const { world, epoch } = await core.generate({ ...DEFAULT_RECIPE, subdivision });
    const generationMs = performance.now() - start;
    core.accept(epoch);
    const times = [];
    let massError = 0;
    for (let i = 0; i < 20; i++) {
      const begin = performance.now(); const frame = await core.advance(epoch, 4);
      times.push(performance.now() - begin); massError = frame.relativeMassError;
    }
    times.sort((a, b) => a - b);
    const referenceStart = performance.now(); generateWorld({ ...DEFAULT_RECIPE, subdivision });
    results.push({ subdivision, regions: world.stats.regionCount, nativeAndIPC_ms: +generationMs.toFixed(1),
      tsSurfaceOnly_ms: +(performance.now() - referenceStart).toFixed(1),
      fourStepsAndIPC_p95_ms: +times[18].toFixed(2), massError,
      arrayMiB: +(world.stats.arrayBytes / 2 ** 20).toFixed(2), frameKiB: world.stats.regionCount * 8 / 1024, checksum: world.checksum });
  } finally { core.close(); }
}
console.table(results);
