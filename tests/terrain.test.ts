import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { BufferAttribute, BufferGeometry, DoubleSide, Mesh, MeshBasicMaterial, Raycaster, Vector3 } from 'three';
import { buildSurface } from '../src/core/surface';
import { buildViewGeometry } from '../src/renderer/view-geometry';
import { displaceDirections, effectiveExaggeration } from '../src/renderer/relief';
import { summarizeTerrain } from '../src/core/terrain';

test('display exaggeration is bounded and reversible without changing native fields', () => {
  const heights = Float64Array.of(-4500, 10000), before = heights.slice();
  assert.equal(effectiveExaggeration(1, 100000, heights), 1);
  assert.equal(effectiveExaggeration(50, 100000, heights), 2);
  assert.equal(effectiveExaggeration(10, 6371000, heights), 10);
  assert.equal(effectiveExaggeration(0, 100000, heights), 0);
  assert.throws(() => effectiveExaggeration(NaN, 1, heights));
  const base = Float32Array.of(1, 0, 0, 0, 1, 0), offsets = Float32Array.of(-0.045, 0.1);
  const displaced = displaceDirections(base, offsets, 2);
  assert.ok(Math.abs(displaced[0] - 0.91) < 1e-7);
  assert.ok(Math.abs(displaced[4] - 1.2) < 1e-7);
  displaceDirections(base, offsets, 0, displaced); assert.deepEqual(displaced, base);
  assert.deepEqual(heights, before);
});

test('shared globe corners remain watertight; flat geometry and model data remain unchanged', () => {
  const surface = buildSurface(2, 6371000);
  const elevation = Float64Array.from(surface.areasSquareMeters, (_, i) => 3000 * Math.sin(i));
  const original = structuredClone({ surface, elevation });
  const bare = buildViewGeometry(surface), views = buildViewGeometry(surface, undefined, elevation);
  assert.deepEqual(views.flat, bare.flat);
  const globe = views.globe, shared = new Map<string, number>();
  assert.equal(globe.radialOffsets.length, globe.regions.length);
  for (let i = 0; i < globe.regions.length; i++) {
    const key = Array.from(globe.positions.subarray(i * 3, i * 3 + 3)).join(',');
    if (shared.has(key)) assert.equal(globe.radialOffsets[i], shared.get(key));
    else shared.set(key, globe.radialOffsets[i]);
    if (i % 3 === 0) assert.ok(Math.abs(globe.radialOffsets[i] - elevation[globe.regions[i]] / surface.radiusMeters) < 1e-9);
  }
  const geometry = new BufferGeometry();
  geometry.setAttribute('position', new BufferAttribute(displaceDirections(globe.positions, globe.radialOffsets, 10), 3));
  geometry.computeVertexNormals();
  assert.ok((geometry.getAttribute('normal').array as Float32Array).every(Number.isFinite));
  const material = new MeshBasicMaterial({ side: DoubleSide }), mesh = new Mesh(geometry, material);
  for (const id of [0, 10, 40, 100]) {
    const p = new Vector3(...surface.centers.subarray(id * 3, id * 3 + 3));
    const hit = new Raycaster(p.clone().multiplyScalar(3), p.clone().negate()).intersectObject(mesh)[0];
    assert.ok(hit?.face); assert.equal(globe.regions[hit.face.a], id);
  }
  material.dispose(); geometry.dispose();
  assert.deepEqual({ surface, elevation }, original);
});

test('elevation summaries use physical area weighting', () => {
  const surface = buildSurface(0, 1000);
  surface.areasSquareMeters.fill(1); surface.areasSquareMeters[0] = 10;
  const elevation = new Float64Array(12); elevation[0] = 100; elevation[1] = -20;
  const zero = new Float64Array(12);
  const summary = summarizeTerrain(surface, { baseline: elevation, convergence: zero, divergence: zero, detail: zero, elevation });
  assert.equal(summary.minimumMeters, -20); assert.equal(summary.maximumMeters, 100);
  assert.equal(summary.meanMeters, 980 / 21);
});
