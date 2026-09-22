import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { DEFAULT_RECIPE, parseRecipe, serializeRecipe } from '../src/core/recipe';
import { hashString, Random, stream } from '../src/core/random';
import { buildSurface, locateRegion } from '../src/core/surface';
import { cross, dot, normalize, readVector, triangleArea } from '../src/core/vector';
import { generateWorld } from '../src/core/world';

test('recipes are strict, bounded, versioned, and round-trip without hidden defaults', () => {
  assert.deepEqual(parseRecipe(JSON.parse(serializeRecipe(DEFAULT_RECIPE))), DEFAULT_RECIPE);
  for (const value of [null, [], {}, { ...DEFAULT_RECIPE, typo: 4 }, { ...DEFAULT_RECIPE, schemaVersion: 2 },
    { ...DEFAULT_RECIPE, modelVersion: 'future' }, { ...DEFAULT_RECIPE, randomVersion: 'future' },
    { ...DEFAULT_RECIPE, seed: '' }, { ...DEFAULT_RECIPE, seed: ' '.repeat(10) },
    { ...DEFAULT_RECIPE, seed: 'x'.repeat(129) }, { ...DEFAULT_RECIPE, subdivision: 7 },
    { ...DEFAULT_RECIPE, subdivision: -1 }, { ...DEFAULT_RECIPE, subdivision: 2.5 },
    { ...DEFAULT_RECIPE, radiusMeters: NaN }, { ...DEFAULT_RECIPE, radiusMeters: Infinity },
    { ...DEFAULT_RECIPE, radiusMeters: 1 }, { ...DEFAULT_RECIPE, radiusMeters: '6371000' },
    { ...DEFAULT_RECIPE, plateCount: 1 }, { ...DEFAULT_RECIPE, plateCount: 33 }, { ...DEFAULT_RECIPE, plateCount: 2.5 },
    { ...DEFAULT_RECIPE, plateCount: 13, subdivision: 0 }, { ...DEFAULT_RECIPE, plateCount: '12' },
    { ...DEFAULT_RECIPE, maxPlateSpeedCmPerYear: -1 }, { ...DEFAULT_RECIPE, maxPlateSpeedCmPerYear: 21 },
    { ...DEFAULT_RECIPE, maxPlateSpeedCmPerYear: NaN }, { ...DEFAULT_RECIPE, maxPlateSpeedCmPerYear: '8' }]) {
    assert.throws(() => parseRecipe(value));
  }
});

test('FNV-1a known vectors and Mulberry32 reference sequence stay fixed', () => {
  assert.equal(hashString(''), 0x811c9dc5);
  assert.equal(hashString('a'), 0xe40c292c);
  assert.equal(hashString('foobar'), 0xbf9cf968);
  const random = new Random(1);
  assert.deepEqual(Array.from({ length: 5 }, () => random.nextUint32()), [2693262067, 11749833, 2265367787, 4213581821, 4159151403]);
});

test('named random streams are independent and can be resumed', () => {
  const geology = stream('seed', 'geology');
  const weather = stream('seed', 'weather');
  for (let i = 0; i < 1000; i++) weather.next();
  assert.equal(geology.nextUint32(), stream('seed', 'geology').nextUint32());
  const resumed = new Random(geology.snapshot());
  assert.deepEqual(Array.from({ length: 10 }, () => geology.nextUint32()), Array.from({ length: 10 }, () => resumed.nextUint32()));
  assert.notEqual(stream('a', 'b:c').snapshot(), stream('a:b', 'c').snapshot());
});

test('spherical octant area is independently known', () => {
  assert.ok(Math.abs(triangleArea([1, 0, 0], [0, 1, 0], [0, 0, 1]) - Math.PI / 2) < 1e-15);
});

for (const level of [0, 1, 3, 5]) {
  test(`level ${level}: closed manifold, spherical coverage, connected reciprocal adjacency`, () => {
    const surface = buildSurface(level, 1000);
    const count = surface.centers.length / 3;
    assert.equal(count, 10 * 4 ** level + 2);
    assert.equal(surface.faces.length / 3, 20 * 4 ** level);
    const positions = new Set<string>(), edgeFaces = new Map<string, number>();
    let fiveNeighbors = 0, sum = 0;
    for (let id = 0; id < count; id++) {
      const center = readVector(surface.centers, id);
      assert.ok(Math.abs(Math.hypot(...center) - 1) < 1e-14);
      positions.add(center.map((v) => v.toPrecision(14)).join(','));
      const neighbors = surface.neighbors.subarray(surface.neighborOffsets[id], surface.neighborOffsets[id + 1]);
      assert.equal(new Set(neighbors).size, neighbors.length);
      if (neighbors.length === 5) fiveNeighbors++;
      else assert.equal(neighbors.length, 6);
      for (let index = surface.neighborOffsets[id]; index < surface.neighborOffsets[id + 1]; index++) {
        const neighbor = surface.neighbors[index];
        const reverseStart = surface.neighborOffsets[neighbor], reverseEnd = surface.neighborOffsets[neighbor + 1];
        const reverse = surface.neighbors.subarray(reverseStart, reverseEnd).indexOf(id);
        assert.ok(reverse >= 0);
        assert.equal(surface.neighborDistancesMeters[index], surface.neighborDistancesMeters[reverseStart + reverse]);
        assert.ok(surface.neighborDistancesMeters[index] > 0);
      }
      assert.ok(surface.areasSquareMeters[id] > 0);
      sum += surface.areasSquareMeters[id];
    }
    assert.equal(fiveNeighbors, 12);
    assert.equal(positions.size, count);
    assert.ok(Math.abs(sum / (4 * Math.PI * 1000 ** 2) - 1) < 1e-11);
    for (let i = 0; i < surface.faces.length; i += 3) {
      const ids = [surface.faces[i], surface.faces[i + 1], surface.faces[i + 2]];
      assert.ok(dot(readVector(surface.centers, ids[0]), cross(readVector(surface.centers, ids[1]), readVector(surface.centers, ids[2]))) > 0);
      for (let j = 0; j < 3; j++) {
        const a = ids[j], b = ids[(j + 1) % 3];
        const key = a < b ? `${a}:${b}` : `${b}:${a}`;
        edgeFaces.set(key, (edgeFaces.get(key) ?? 0) + 1);
      }
    }
    assert.ok([...edgeFaces.values()].every((n) => n === 2));
    assert.equal(count - edgeFaces.size + surface.faces.length / 3, 2);
    const visited = new Set([0]), queue = [0];
    for (let i = 0; i < queue.length; i++) {
      const id = queue[i];
      for (const neighbor of surface.neighbors.subarray(surface.neighborOffsets[id], surface.neighborOffsets[id + 1])) {
        if (!visited.has(neighbor)) { visited.add(neighbor); queue.push(neighbor); }
      }
    }
    assert.equal(visited.size, count);
  });
}

test('selection respects dual regions, poles, and both sides of the map seam', () => {
  const surface = buildSurface(3, 1000);
  for (let id = 0; id < surface.areasSquareMeters.length; id++) {
    assert.equal(locateRegion(surface, readVector(surface.centers, id)), id);
  }
  assert.equal(locateRegion(surface, normalize([-1, 0.2, 1e-10])), locateRegion(surface, normalize([-1, 0.2, -1e-10])));
  assert.ok(locateRegion(surface, [0, 1, 0]) >= 0);
  assert.ok(locateRegion(surface, [0, -1, 0]) >= 0);
  const random = stream('selection', 'samples');
  for (let i = 0; i < 1000; i++) {
    const point = normalize([random.next() * 2 - 1, random.next() * 2 - 1, random.next() * 2 - 1]);
    const id = locateRegion(surface, point);
    const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
    const center = readVector(surface.centers, id);
    let contained = false;
    for (let j = start; j < end; j++) {
      const a = readVector(surface.boundaryDirections, j), b = readVector(surface.boundaryDirections, j + 1 === end ? start : j + 1);
      const partition = triangleArea(point, center, a) + triangleArea(point, a, b) + triangleArea(point, b, center);
      if (Math.abs(partition - triangleArea(center, a, b)) < 1e-11) contained = true;
    }
    assert.ok(contained, 'Selected region contains the sample by spherical area decomposition.');
  }
});

test('radius scales area quadratically and distance linearly without changing topology', () => {
  const a = buildSurface(2, 1000), b = buildSurface(2, 2000);
  assert.deepEqual(a.centers, b.centers);
  assert.deepEqual(a.neighbors, b.neighbors);
  for (let i = 0; i < a.areasSquareMeters.length; i++) assert.equal(b.areasSquareMeters[i], 4 * a.areasSquareMeters[i]);
  for (let i = 0; i < a.neighbors.length; i++) assert.equal(b.neighborDistancesMeters[i], 2 * a.neighborDistancesMeters[i]);
});

test('seed ensemble reproduces and changes the signal, not the physical mesh', () => {
  const fingerprints = new Set<string>();
  const first = generateWorld({ ...DEFAULT_RECIPE, subdivision: 2 });
  for (let seed = 0; seed < 20; seed++) {
    const recipe = { ...DEFAULT_RECIPE, subdivision: 2, seed: `ensemble-${seed}` };
    const world = generateWorld(recipe);
    assert.deepEqual(world, generateWorld(JSON.parse(serializeRecipe(recipe))));
    assert.deepEqual(world.surface, first.surface);
    assert.ok(world.diagnosticField.every((value) => Number.isFinite(value) && value >= -1 && value <= 1));
    fingerprints.add(world.checksum);
  }
  assert.equal(fingerprints.size, 20);
});
