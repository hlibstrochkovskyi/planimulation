import { readFile } from 'node:fs/promises';
import { DEFAULT_RECIPE } from './core/recipe';
import { generateWorld } from './core/world';

try {
  const args = process.argv.slice(2);
  if (args.length > 1 || args[0]?.startsWith('--')) throw new Error('Usage: npm run headless -- [recipe.json]');
  const recipe: unknown = args[0] ? JSON.parse(await readFile(args[0], 'utf8')) : DEFAULT_RECIPE;
  const start = performance.now();
  const world = generateWorld(recipe);
  console.log(JSON.stringify({ recipe: world.recipe, checksum: world.checksum, stats: world.stats, generationMs: performance.now() - start }, null, 2));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
