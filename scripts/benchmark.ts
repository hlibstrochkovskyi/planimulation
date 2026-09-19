import { DEFAULT_RECIPE } from '../src/core/recipe';
import { generateWorld } from '../src/core/world';

const results = [];
for (const subdivision of [3, 4, 5, 6]) {
  const start = performance.now();
  const world = generateWorld({ ...DEFAULT_RECIPE, subdivision });
  results.push({ subdivision, regions: world.stats.regionCount, milliseconds: +(performance.now() - start).toFixed(1),
    arrayMiB: +(world.stats.arrayBytes / 2 ** 20).toFixed(2), areaError: world.stats.relativeAreaError, checksum: world.checksum });
}
console.table(results);
