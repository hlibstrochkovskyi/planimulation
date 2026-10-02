import type { World } from '../core/world';
import type { WaterBudget } from '../shared/desktop-api';

/** Display-only conversion. Exact accounting stays in native integer units. */
export function approximateCubicKilometers(units: string): number {
  return Number(BigInt(units)) / 2 ** 56 / 1e9;
}

/** The frozen initial receiver graph determines this source's current stock owner. */
export function runoffDestination(world: World, budget: WaterBudget, region: number):
  { terminal: number; branch: number; volumeUnits: string } {
  if (!Number.isSafeInteger(region) || region < 0 || region >= world.stats.regionCount) {
    throw new Error('Water-budget source is outside the generated world.');
  }
  const stocks = new Map(budget.stocks.map((stock) => [stock.branch, stock.volumeUnits]));
  const terminal = world.drainage.outlets[region];
  let branch = world.basins.regionNodes[terminal];
  for (;;) {
    const volumeUnits = stocks.get(branch);
    if (volumeUnits !== undefined) return { terminal, branch, volumeUnits };
    const parent = world.basins.parents[branch];
    if (parent === branch) throw new Error('No active stock owns this runoff destination.');
    branch = parent;
  }
}
