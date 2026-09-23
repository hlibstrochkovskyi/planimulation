import type { Water } from '../core/water';
import type { BufferGeometry, Mesh, Raycaster } from 'three';

/** Whole-region caps, with the exact angular triangles of the bed. No physical fields are changed. */
export function buildWaterSurface(positions: Float32Array, regions: Float32Array, water: Water, radius: number) {
  if (positions.length !== regions.length * 3 || regions.length % 3 || !Number.isFinite(radius) || radius <= 0
    || !Number.isFinite(water.levelMeters)) throw new Error('Invalid water surface geometry.');
  let count = 0;
  for (let i = 0; i < regions.length; i += 3) {
    const id = regions[i];
    if (!Number.isInteger(id) || id < 0 || id >= water.bodyIds.length || regions[i + 1] !== id || regions[i + 2] !== id) {
      throw new Error('Invalid water surface region.');
    }
    if (water.bodyIds[id]) count += 3;
  }
  const points = new Float32Array(count * 3), ids = new Float32Array(count), lines = new Float32Array(count * 2);
  let cursor = 0;
  for (let i = 0; i < regions.length; i += 3) {
    const id = regions[i];
    if (!water.bodyIds[id]) continue;
    points.set(positions.subarray(i * 3, i * 3 + 9), cursor * 3);
    ids.fill(id, cursor, cursor + 3);
    // Each globe fan triangle's last two vertices lie on the dual-region boundary.
    for (let j = 0; j < 6; j++) lines[cursor * 2 + j] = positions[i * 3 + 3 + j] * 1.0002;
    cursor += 3;
  }
  const offset = water.levelMeters / radius;
  return { waterPositions: points, waterRegions: ids,
    waterOffsets: new Float32Array(ids.length).fill(offset), waterLines: lines,
    waterLineOffsets: new Float32Array(lines.length / 3).fill(offset) };
}

/** Intersect visible surfaces only. Water wins coincident hits at zero exaggeration. */
export function pickSurface(raycaster: Raycaster, bed: Mesh<BufferGeometry>, water?: Mesh<BufferGeometry>) {
  const candidates = water?.visible ? [water, bed] : [bed];
  const hit = raycaster.intersectObjects(candidates, false)[0];
  if (!hit?.face) return null;
  const mesh = hit.object as Mesh<BufferGeometry>;
  return { id: mesh.geometry.getAttribute('region').getX(hit.face.a), surface: mesh === water ? 'water' : 'bed' };
}
