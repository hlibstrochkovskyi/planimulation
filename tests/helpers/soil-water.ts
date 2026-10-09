import { strict as assert } from 'node:assert';
import type { World } from '../../src/core/world';

interface Mass { high: number; low: number }
interface FundingCheckpoint {
  elapsedSeconds: number; referenceBodies: Mass[]; liquid: Mass[];
  localTransfers: Array<{ liquidEvaporation: Mass; rain: Mass }>;
  atmosphericTransfers: Mass[];
}
function credit(m: Mass, amount: number): void {
  const sum = m.high + amount, v = sum - m.high;
  const tail = m.low + ((m.high - (sum - v)) + (amount - v));
  const high = sum + tail, w = high - sum;
  m.high = high; m.low = (sum - (high - w)) + (tail - w);
}
/** Synthetic directed stress input, not naturally reached rainfall. One finite
 * body funds evaporation, a real adjacent atmospheric crossing and land rain. */
export function fundedSoilCheckpoint(contents: string, world: World): string {
  const cp = JSON.parse(contents) as FundingCheckpoint, faces: number[][] = [];
  for (let a = 0; a < world.stats.regionCount; a++) {
    for (const b of world.surface.neighbors.subarray(world.surface.neighborOffsets[a], world.surface.neighborOffsets[a + 1])) {
      if (a < b) faces.push([a, b]);
    }
  }
  faces.sort(([a, b], [c, d]) => a - c || b - d);
  const face = faces.findIndex(([a, b]) => Number(world.water.bodyIds[a] > 0) !== Number(world.water.bodyIds[b] > 0));
  assert.ok(face >= 0);
  const [a, b] = faces[face], source = world.water.bodyIds[a] > 0 ? a : b, target = source === a ? b : a;
  const ids = [...new Set(world.water.bodyIds)].filter(id => id > 0).sort((a, b) => a - b);
  const body = ids.indexOf(world.water.bodyIds[source]), amount = world.surface.areasSquareMeters[target] * 50000;
  assert.ok(cp.referenceBodies[body].high > amount);
  credit(cp.referenceBodies[body], -amount);
  credit(cp.localTransfers[source].liquidEvaporation, amount);
  credit(cp.atmosphericTransfers[2 * face + Number(source > target)], amount);
  credit(cp.localTransfers[target].rain, amount); credit(cp.liquid[target], amount);
  cp.elapsedSeconds = 900;
  return JSON.stringify(cp);
}
