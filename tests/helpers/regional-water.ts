import { strict as assert } from 'node:assert';
import { readFile } from 'node:fs/promises';
import { parseRecipe } from '../../src/core/recipe';
import type { World } from '../../src/core/world';

export async function regionalFlowRecipe() {
  return parseRecipe({ ...JSON.parse(await readFile('docs/scenarios/spill-connections.json', 'utf8')),
    subdivision: 2, water: { mode: 'coverage', fraction: 0.3 } });
}

interface FundingCheckpoint {
  elapsedSeconds: number;
  referenceBodyHighKilograms: number[]; referenceBodyLowKilograms: number[];
  terminalWaterKilograms: number[]; terminalLowKilograms: number[];
  cumulativeLakeCaptureKilograms: number[]; cumulativeLakeCaptureLowKilograms: number[];
  cumulativeSurfaceTransfers: Array<{ liquidEvaporation: number; rain: number }>;
  cumulativeEvaporationKilograms: number[]; cumulativePrecipitationKilograms: number[];
}

function credit(highs: number[], lows: number[], index: number, amount: number): void {
  const before = highs[index], s = before + amount, virtualAmount = s - before;
  const error = (before - (s - virtualAmount)) + (amount - virtualAmount);
  const tail = lows[index] + error, high = s + tail, virtualTail = high - s;
  highs[index] = high;
  lows[index] = (s - (high - virtualTail)) + (tail - virtualTail);
}

/** Directed stress fixture, NOT natural rainfall or a product initialization.
 * Every synthetic rain credit is funded by evaporation from a real finite body.
 * Rust validates the resulting full checkpoint, including regional ledgers. */
export function fundedRegionalCheckpoint(contents: string, world: World): string {
  const cp = JSON.parse(contents) as FundingCheckpoint;
  cp.elapsedSeconds = Math.max(cp.elapsedSeconds, 900);
  const ids = [...new Set(world.water.bodyIds)].filter((id) => id > 0).sort((a, b) => a - b);
  const used = new Set<number>();
  for (const [region, amount] of [[8, 1.4e17], [156, 2.4e17]]) {
    assert.equal(world.water.bodyIds[region], 0);
    const body = cp.referenceBodyHighKilograms.reduce((best, mass, i, masses) => mass > masses[best] ? i : best, 0);
    const contact = world.water.bodyIds.findIndex((id, i) => id === ids[body] && !used.has(i));
    assert.ok(contact >= 0 && cp.referenceBodyHighKilograms[body] > amount);
    used.add(contact);
    credit(cp.referenceBodyHighKilograms, cp.referenceBodyLowKilograms, body, -amount);
    cp.cumulativeSurfaceTransfers[contact].liquidEvaporation += amount;
    cp.cumulativeEvaporationKilograms[contact] += amount;
    cp.cumulativeSurfaceTransfers[region].rain += amount;
    cp.cumulativePrecipitationKilograms[region] += amount;
    credit(cp.cumulativeLakeCaptureKilograms, cp.cumulativeLakeCaptureLowKilograms, region, amount);
    credit(cp.terminalWaterKilograms, cp.terminalLowKilograms, region, amount);
  }
  return JSON.stringify(cp);
}
