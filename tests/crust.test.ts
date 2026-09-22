import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { summarizeCrust } from '../src/core/crust';
import { buildSurface } from '../src/core/surface';

test('crust summaries distinguish disconnected patches, fractional values, and a connecting path', () => {
  const surface = buildSurface(0, 1000);
  const n = surface.areasSquareMeters.length;
  const crust = { threshold: 0, potential: new Float64Array(n), continentality: new Float64Array(n),
    thicknessMeters: new Float64Array(n), densityKgPerCubicMeter: new Float64Array(n) };
  assert.ok(!surface.neighbors.subarray(surface.neighborOffsets[0], surface.neighborOffsets[1]).includes(3));
  crust.continentality[0] = 0.7; crust.continentality[3] = 0.8;
  const total = surface.areasSquareMeters.reduce((sum, area) => sum + area, 0);
  let summary = summarizeCrust(surface, crust);
  assert.equal(summary.continentalPatchCount, 2);
  assert.equal(summary.largestContinentalPatchAreaSquareMeters, Math.max(surface.areasSquareMeters[0], surface.areasSquareMeters[3]));
  assert.equal(summary.continentalAreaFraction, (surface.areasSquareMeters[0] + surface.areasSquareMeters[3]) / total);
  assert.equal(summary.meanContinentality, (0.7 * surface.areasSquareMeters[0] + 0.8 * surface.areasSquareMeters[3]) / total);
  // The graph path 0–5–4–3 joins both patches without using map coordinates.
  crust.continentality[5] = 0.6; crust.continentality[4] = 0.6;
  summary = summarizeCrust(surface, crust);
  assert.equal(summary.continentalPatchCount, 1);
  assert.ok(Math.abs(summary.largestContinentalPatchAreaSquareMeters / total - summary.continentalAreaFraction) < 1e-14);
  crust.continentality.fill(0.5);
  summary = summarizeCrust(surface, crust);
  assert.equal(summary.continentalPatchCount, 0, 'Exactly 0.5 is not continental-dominant.');
  assert.equal(summary.continentalAreaFraction, 0);
  assert.equal(summary.meanContinentality, 0.5);
});
