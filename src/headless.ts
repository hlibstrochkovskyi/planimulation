import { readFile } from 'node:fs/promises';
import { DEFAULT_RECIPE, parseRecipe } from './core/recipe';
import { NativeController } from './native/client';
import path from 'node:path';

const core = new NativeController(path.resolve('dist/native', process.platform === 'win32' ? 'planimulation-core.exe' : 'planimulation-core'));

try {
  const args = process.argv.slice(2);
  if (args.length > 1 || args[0]?.startsWith('--')) throw new Error('Usage: npm run headless -- [recipe.json]');
  const recipe: unknown = args[0] ? JSON.parse(await readFile(args[0], 'utf8')) : DEFAULT_RECIPE;
  const start = performance.now();
  const { world } = await core.generate(parseRecipe(recipe));
  console.log(JSON.stringify({ recipe: world.recipe, checksum: world.checksum, stats: world.stats, generationMs: performance.now() - start }, null, 2));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
} finally { core.close(); }
