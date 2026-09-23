import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { BufferAttribute, BufferGeometry, DoubleSide, Mesh, MeshBasicMaterial, Raycaster, Vector3 } from 'three';
import { buildSurface } from '../src/core/surface';
import type { Water } from '../src/core/water';
import { buildViewGeometry } from '../src/renderer/view-geometry';
import { buildWaterSurface, pickSurface } from '../src/renderer/water-surface';
import { displaceDirections, effectiveExaggeration } from '../src/renderer/relief';

function state(n: number, level: number, wet: (id: number) => boolean): Water {
  return { levelMeters: level, resolvedVolumeCubicMeters: 0, mainOceanId: 1,
    bodyIds: Uint32Array.from({ length: n }, (_, id) => wet(id) ? 1 : 0),
    depthMeters: Float64Array.from({ length: n }, (_, id) => wet(id) ? 1 : 0) };
}

function mesh(positions: Float32Array, regions: Float32Array) {
  const geometry = new BufferGeometry();
  geometry.setAttribute('position', new BufferAttribute(positions, 3));
  geometry.setAttribute('region', new BufferAttribute(regions, 1));
  geometry.computeVertexNormals();
  assert.ok((geometry.getAttribute('normal').array as Float32Array).every(Number.isFinite));
  return new Mesh(geometry, new MeshBasicMaterial({ side: DoubleSide }));
}

test('water caps use only wet regions and preserve the flat atlas, geological mesh, and model fields', () => {
  for (const level of [0, 2, 3]) {
    const surface = buildSurface(level, 100_000), n = surface.areasSquareMeters.length;
    const elevation = Float64Array.from({ length: n }, (_, id) => Math.sin(id) * 2000);
    const water = state(n, 500, (id) => id % 3 === 0), original = structuredClone({ surface, elevation, water });
    const bare = buildViewGeometry(surface, undefined, elevation), pair = buildViewGeometry(surface, undefined, elevation, water);
    assert.deepEqual(pair.flat, bare.flat);
    assert.deepEqual(pair.globe.positions, bare.globe.positions);
    assert.deepEqual(pair.globe.radialOffsets, bare.globe.radialOffsets);
    assert.deepEqual(new Set(pair.globe.waterRegions), new Set(Array.from(water.bodyIds).flatMap((v, id) => v ? [id] : [])));
    let triangles = 0;
    for (let id = 0; id < n; id++) if (water.bodyIds[id]) triangles += surface.boundaryOffsets[id + 1] - surface.boundaryOffsets[id];
    assert.equal(pair.globe.waterRegions.length, triangles * 3);
    assert.equal(pair.globe.waterLines.length, triangles * 6);
    assert.ok(pair.globe.waterOffsets.every((h) => Math.abs(h - 0.005) < 1e-9));
    for (let i = 0; i < pair.globe.waterRegions.length; i += 3) {
      assert.equal(pair.globe.waterRegions[i], pair.globe.waterRegions[i + 1]);
      assert.equal(pair.globe.waterRegions[i], pair.globe.waterRegions[i + 2]);
    }
    assert.deepEqual({ surface, elevation, water }, original);
    const dry = buildViewGeometry(surface, undefined, elevation, state(n, -3000, () => false));
    assert.equal(dry.globe.waterPositions.length, 0); assert.equal(dry.globe.waterLines.length, 0);
  }
});

test('fully flooded caps share corners, have a uniform vertex level, and obey the joint exaggeration bound', () => {
  const s = buildSurface(2, 100_000), n = s.areasSquareMeters.length, heights = new Float64Array(n).fill(-1000);
  for (const level of [-500, 25000]) {
    const water = state(n, level, () => true), g = buildViewGeometry(s, undefined, heights, water).globe;
    const factor = effectiveExaggeration(50, s.radiusMeters, heights, level);
    assert.ok(factor <= 20);
    if (level === 25000) assert.equal(factor, 0.8);
    const points = displaceDirections(g.waterPositions, g.waterOffsets, factor), shared = new Map<string, string>();
    for (let i = 0; i < g.waterRegions.length; i++) {
      const key = g.waterPositions.subarray(i * 3, i * 3 + 3).join(',');
      const p = points.subarray(i * 3, i * 3 + 3), radius = Math.hypot(...p);
      assert.ok(Math.abs(radius - (1 + factor * level / s.radiusMeters)) < 2e-7);
      if (shared.has(key)) assert.equal(shared.get(key), p.join(',')); else shared.set(key, p.join(','));
    }
    displaceDirections(g.waterPositions, g.waterOffsets, 0, points);
    assert.deepEqual(points, g.waterPositions);
  }
  assert.throws(() => effectiveExaggeration(10, 100000, heights, NaN));
});

test('picking selects the visible water or occluding bed, ignores hidden water, and handles coincident surfaces', () => {
  const base = Float32Array.from([1, 0, 0, ...new Vector3(1, 0.2, 0).normalize().toArray(), ...new Vector3(1, 0, 0.2).normalize().toArray()]);
  const regions = Float32Array.of(7, 7, 7);
  const data = buildWaterSurface(base, regions, state(8, 0, () => true), 1000);
  const water = mesh(displaceDirections(data.waterPositions, data.waterOffsets, 1), data.waterRegions);
  const bed = mesh(displaceDirections(base, Float32Array.of(-0.05, 0.05, 0.05), 1), regions);
  const ray = (weights: number[]) => {
    const p = new Vector3();
    weights.forEach((weight, id) => p.addScaledVector(new Vector3(...base.subarray(id * 3, id * 3 + 3)), weight));
    p.normalize(); return new Raycaster(p.clone().multiplyScalar(3), p.clone().negate());
  };
  assert.deepEqual(pickSurface(ray([0.8, 0.1, 0.1]), bed, water), { id: 7, surface: 'water' });
  assert.deepEqual(pickSurface(ray([0.1, 0.8, 0.1]), bed, water), { id: 7, surface: 'bed' });
  water.visible = false;
  assert.deepEqual(pickSurface(ray([0.8, 0.1, 0.1]), bed, water), { id: 7, surface: 'bed' });
  water.visible = true;
  const coincident = mesh(base, regions);
  assert.deepEqual(pickSurface(ray([0.8, 0.1, 0.1]), coincident, water), { id: 7, surface: 'water' });
  assert.equal(pickSurface(new Raycaster(new Vector3(3, 0, 0), new Vector3(1, 0, 0)), bed, water), null);
  for (const item of [bed, water, coincident]) { item.geometry.dispose(); item.material.dispose(); }
});

test('globe water picking retains region identity at poles and both sides of the atlas seam', () => {
  const s = buildSurface(2, 6371000), n = s.areasSquareMeters.length;
  const g = buildViewGeometry(s, undefined, new Float64Array(n).fill(-4500), state(n, 100, () => true)).globe;
  const bed = mesh(displaceDirections(g.positions, g.radialOffsets, 10), g.regions);
  const water = mesh(displaceDirections(g.waterPositions, g.waterOffsets, 10), g.waterRegions);
  // Sampling every center includes poles and seam-adjacent regions, not just a camera-facing patch.
  for (let id = 0; id < n; id++) {
    const p = new Vector3(...s.centers.subarray(id * 3, id * 3 + 3));
    assert.deepEqual(pickSurface(new Raycaster(p.clone().multiplyScalar(3), p.clone().negate()), bed, water), { id, surface: 'water' });
  }
  for (const item of [bed, water]) { item.geometry.dispose(); item.material.dispose(); }
});
