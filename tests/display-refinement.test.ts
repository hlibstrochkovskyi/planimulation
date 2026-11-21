import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { buildSurface } from '../src/core/surface';
import type { Water } from '../src/core/water';
import type { Drainage } from '../src/core/drainage';
import { buildRenderViewGeometry, buildViewGeometry } from '../src/renderer/view-geometry';
import { computeReliefMetrics } from '../src/renderer/natural-color';
import {
  buildLodCache,
  buildLodPatch,
  computeCoastField,
  computeValleyField,
  MAX_LOD_TRIANGLES,
  meshFrequencyForRegions,
  sampleCoastPhi,
  sampleDetailHeightMeters,
  seedOffsetFromString,
} from '../src/renderer/refinement';
import { buildDrainageLines } from '../src/renderer/rivers';
import { angularDistance, readVector } from '../src/core/vector';

function makeTestWorld(level: number) {
  const surface = buildSurface(level, 6_371_000);
  const n = surface.areasSquareMeters.length;
  const elevation = Float64Array.from({ length: n }, (_, i) => {
    const x = surface.centers[i * 3], y = surface.centers[i * 3 + 1], z = surface.centers[i * 3 + 2];
    return Math.sin(x * 3.1 + z * 2.4) * 2800 + Math.cos(y * 4.2) * 1400 - 400;
  });
  const bodyIds = Uint32Array.from({ length: n }, (_, i) => (elevation[i] < 0 ? 1 : 0));
  const depthMeters = Float64Array.from({ length: n }, (_, i) => (elevation[i] < 0 ? -elevation[i] : 0));
  const water: Water = { levelMeters: 0, resolvedVolumeCubicMeters: 0, mainOceanId: 1, bodyIds, depthMeters };
  const receivers = Uint32Array.from({ length: n }, (_, i) => {
    if (bodyIds[i]) return i;
    let best = i, bestElev = elevation[i];
    for (let k = surface.neighborOffsets[i]; k < surface.neighborOffsets[i + 1]; k++) {
      const nb = surface.neighbors[k];
      if (elevation[nb] < bestElev) { bestElev = elevation[nb]; best = nb; }
    }
    return best;
  });
  const contributingArea = Float64Array.from(surface.areasSquareMeters, (a, i) => (bodyIds[i] ? 0 : a * 20));
  const drainage: Drainage = {
    receivers,
    outlets: receivers,
    flatSteps: new Uint32Array(n),
    contributingArea,
  };
  return { surface, elevation, water, drainage };
}

test('coastline zero-contour preserves every model region center and never invents interior lakes or islands', () => {
  const { surface, elevation, water } = makeTestWorld(3);
  const n = surface.areasSquareMeters.length;
  const relief = computeReliefMetrics(surface, elevation, water.bodyIds, water.levelMeters);
  const coast = computeCoastField(surface, elevation, water.bodyIds, water.depthMeters, water.levelMeters, water.bodyIds, water.mainOceanId);
  const seedOff = seedOffsetFromString('first-light');

  for (let id = 0; id < n; id++) {
    const c = readVector(surface.centers, id);
    const wet = water.bodyIds[id] !== 0;
    // At every region center (wCenter = 1), sign must strictly match model wet/dry mask.
    const phiCenter = sampleCoastPhi(c[0], c[1], c[2], coast.phi[id], coast.strictMask[id], 1, relief.ruggedness[id], seedOff, true);
    assert.equal(phiCenter, coast.phi[id]);
    assert.ok(wet ? phiCenter < 0 : phiCenter > 0, `Center ${id} sign mismatch.`);

    // Check interior regions (all neighbors same wet/dry class): perturbation must stay 0 even at wCenter = 0.
    let allSame = true;
    for (let k = surface.neighborOffsets[id]; k < surface.neighborOffsets[id + 1]; k++) {
      if ((water.bodyIds[surface.neighbors[k]] !== 0) !== wet) { allSame = false; break; }
    }
    if (allSame) {
      const phiInterior = sampleCoastPhi(c[0], c[1], c[2], coast.phi[id], coast.strictMask[id], 0, relief.ruggedness[id], seedOff, true);
      assert.equal(phiInterior, coast.phi[id], `Interior region ${id} must not be perturbed.`);
    }
  }
});

test('fractal detail height is deterministic, zero on water, and bounded in mountains', () => {
  const seedA = seedOffsetFromString('first-light');
  const seedB = seedOffsetFromString('first-light');
  assert.deepEqual(seedA, seedB);

  const onWater = sampleDetailHeightMeters(0.5, 0.5, 0.707, 1500, -0.2, seedA, true);
  assert.equal(onWater, 0);

  const onPlain = sampleDetailHeightMeters(0.5, 0.5, 0.707, 5, 0.8, seedA, true);
  assert.equal(onPlain, 0);

  const onMountain1 = sampleDetailHeightMeters(0.5, 0.5, 0.707, 2500, 1.2, seedA, true);
  const onMountain2 = sampleDetailHeightMeters(0.5, 0.5, 0.707, 2500, 1.2, seedB, true);
  assert.equal(onMountain1, onMountain2);
  assert.ok(Math.abs(onMountain1) <= 540, `Expected bounded detail height, got ${onMountain1}`);
});

test('camera-local LOD patch is empty at overview, bounded under zoom, and deterministic', () => {
  const { surface, elevation, water } = makeTestWorld(3);
  const pair = buildViewGeometry(surface, undefined, elevation, water);
  const relief = computeReliefMetrics(surface, elevation, water.bodyIds, water.levelMeters);
  const coast = computeCoastField(surface, elevation, water.bodyIds, water.depthMeters, water.levelMeters, water.bodyIds, water.mainOceanId);
  const cache = buildLodCache(surface, elevation, pair.corners.boundaryCorners!, pair.corners.offsets, pair.corners.regions);
  const wetMask = Uint8Array.from(water.bodyIds, v => (v ? 1 : 0));
  const seedOffset = seedOffsetFromString('first-light');

  const overview = buildLodPatch(cache, {
    focus: [1, 0, 0], distance: 2.25, seedOffset, detailEnabled: true,
    waterLevelMeters: water.levelMeters, wetMask,
    phi: coast.phi, strictMask: coast.strictMask,
    localWaterLevel: coast.localWaterLevel, localReliefMeters: relief.localReliefMeters,
  });
  assert.equal(overview.triangleCount, 0);

  const closeA = buildLodPatch(cache, {
    focus: [0, 1, 0], distance: 0.4, seedOffset, detailEnabled: true,
    waterLevelMeters: water.levelMeters, wetMask,
    phi: coast.phi, strictMask: coast.strictMask,
    localWaterLevel: coast.localWaterLevel, localReliefMeters: relief.localReliefMeters,
  });
  const closeB = buildLodPatch(cache, {
    focus: [0, 1, 0], distance: 0.4, seedOffset, detailEnabled: true,
    waterLevelMeters: water.levelMeters, wetMask,
    phi: coast.phi, strictMask: coast.strictMask,
    localWaterLevel: coast.localWaterLevel, localReliefMeters: relief.localReliefMeters,
  });
  assert.ok(closeA.triangleCount > 0 && closeA.triangleCount <= MAX_LOD_TRIANGLES);
  assert.equal(closeA.subdivision, 4);
  assert.deepEqual(closeA.positions, closeB.positions);
  assert.deepEqual(closeA.radialOffsets, closeB.radialOffsets);
});

test('curved river lines are deterministic, stay within corridor bounds, and separate coastal stubs', () => {
  const { surface, elevation, water, drainage } = makeTestWorld(3);
  const relief = computeReliefMetrics(surface, elevation, water.bodyIds, water.levelMeters);
  const coast = computeCoastField(surface, elevation, water.bodyIds, water.depthMeters, water.levelMeters, water.bodyIds, water.mainOceanId);
  const linesA = buildDrainageLines(surface, drainage, elevation, water.bodyIds, undefined, {
    seed: 'first-light', waterLevelMeters: water.levelMeters, coastField: coast, reliefMetrics: relief, detailEnabled: true,
  });
  const linesB = buildDrainageLines(surface, drainage, elevation, water.bodyIds, undefined, {
    seed: 'first-light', waterLevelMeters: water.levelMeters, coastField: coast, reliefMetrics: relief, detailEnabled: true,
  });
  assert.deepEqual(linesA, linesB);
  const totalPrimary = linesA.globe.reduce((sum, g) => sum + g.positions.length / 6, 0);
  const totalDetail = linesA.globeDetail.reduce((sum, g) => sum + g.positions.length / 6, 0);
  assert.ok(totalPrimary + totalDetail > 0, 'Expected generated river segments.');
  // Every globe segment endpoint must stay finite and close to unit sphere radius before exaggeration.
  for (const bucket of [...linesA.globe, ...linesA.globeDetail]) {
    for (let i = 0; i < bucket.positions.length; i += 6) {
      const a: [number, number, number] = [bucket.positions[i], bucket.positions[i + 1], bucket.positions[i + 2]];
      const b: [number, number, number] = [bucket.positions[i + 3], bucket.positions[i + 4], bucket.positions[i + 5]];
      const lenA = Math.hypot(...a), lenB = Math.hypot(...b);
      assert.ok(Math.abs(lenA - 1.00032) < 1e-4 && Math.abs(lenB - 1.00032) < 1e-4);
      assert.ok(angularDistance([a[0] / lenA, a[1] / lenA, a[2] / lenA], [b[0] / lenB, b[1] / lenB, b[2] / lenB]) < 0.25);
    }
  }
});

test('symmetric coastline field keeps 1-cell peninsulas connected without depth asymmetry sawtooth', () => {
  const { surface, water } = makeTestWorld(3);
  const n = surface.areasSquareMeters.length;
  // Build a narrow 1-region-wide dry peninsula surrounded by deep 4,000 m ocean.
  const customBody = new Uint32Array(n).fill(1);
  const customDepth = new Float64Array(n).fill(4000);
  const customElev = new Float64Array(n).fill(-4000);
  const a = 10;
  const b = surface.neighbors[surface.neighborOffsets[a]];
  customBody[a] = 0; customDepth[a] = 0; customElev[a] = 45;
  customBody[b] = 0; customDepth[b] = 0; customElev[b] = 45;

  const coast = computeCoastField(surface, customElev, customBody, customDepth, 0, customBody, 1);
  // Shared corner between two adjacent dry peninsula cells (a, b) and one deep ocean cell must remain dry (> 0),
  // preventing deep-ocean asymmetry from severing the peninsula into sawtooth teeth.
  const pair = buildRenderViewGeometry(surface, undefined, customElev, { ...water, bodyIds: customBody, depthMeters: customDepth });
  const { offsets, regions } = pair.corners;
  let sharedDryCorners = 0;
  for (let c = 0; c + 1 < offsets.length; c++) {
    let hasA = false, hasB = false, phiSum = 0;
    const count = offsets[c + 1] - offsets[c];
    for (let j = offsets[c]; j < offsets[c + 1]; j++) {
      const rid = regions[j];
      if (rid === a) hasA = true;
      if (rid === b) hasB = true;
      phiSum += coast.phi[rid];
    }
    if (hasA && hasB) {
      sharedDryCorners++;
      assert.ok(phiSum / count > 0, `Shared peninsula corner ${c} should stay connected (>0), got ${phiSum / count}`);
    }
  }
  assert.equal(sharedDryCorners, 2, 'Expected two shared dual corners between adjacent peninsula regions.');
});

test('drainage valleys carve lower decorative detail than divides and stay bounded', () => {
  const { surface, water, drainage } = makeTestWorld(3);
  const valley = computeValleyField(surface, drainage, water.bodyIds);
  const seed = seedOffsetFromString('first-light');
  const meshFreq = meshFrequencyForRegions(surface.areasSquareMeters.length);
  let valleySum = 0, divideSum = 0;
  for (let i = 0; i < 64; i++) {
    const t = i / 64 * Math.PI * 2;
    const x = Math.cos(t) * 0.8, y = Math.sin(t) * 0.6, z = Math.sqrt(Math.max(0, 1 - x * x - y * y));
    const hDivide = sampleDetailHeightMeters(x, y, z, 1200, 0.9, seed, true, 0, meshFreq);
    const hValley = sampleDetailHeightMeters(x, y, z, 1200, 0.9, seed, true, 0.95, meshFreq);
    assert.ok(Math.abs(hDivide) <= 540 && Math.abs(hValley) <= 540);
    divideSum += hDivide;
    valleySum += hValley;
  }
  assert.ok(valleySum < divideSum - 2000, `Expected carved drainage valleys to lie lower than divides (${valleySum} vs ${divideSum}).`);
  assert.ok(valley.every((v) => v >= 0 && v <= 1));
});

test('compact indexed render geometry halves triangle count and preserves exact region indices', () => {
  const { surface, elevation, water } = makeTestWorld(3);
  const full = buildViewGeometry(surface, undefined, elevation, water);
  const compact = buildRenderViewGeometry(surface, undefined, elevation, water);
  assert.ok(compact.globe.indices && compact.flat.indices && compact.globe.waterIndices);
  assert.ok(compact.compactRing);
  assert.equal(compact.globe.indices.length / 3, (full.globe.regions.length / 3) / 2);
  assert.equal(compact.globe.waterIndices.length / 3, (full.globe.waterRegions.length / 3) / 2);
  assert.ok(compact.globe.positions.length < full.globe.positions.length * 0.25);
  assert.ok((compact.globe.landCoastIndexCount ?? 0) > 0 && (compact.globe.landCoastIndexCount ?? 0) <= compact.globe.indices.length);
  // Every indexed triangle must belong to a single model region.
  const idx = compact.globe.indices;
  const reg = compact.globe.regions;
  for (let i = 0; i < idx.length; i += 3) {
    assert.equal(reg[idx[i]], reg[idx[i + 1]]);
    assert.equal(reg[idx[i]], reg[idx[i + 2]]);
  }
});
