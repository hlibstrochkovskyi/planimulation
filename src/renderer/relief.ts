/** Display-only safety limit: keep radial excursions within 20% of the reference radius. */
export function effectiveExaggeration(requested: number, radiusMeters: number, heights: Float64Array, waterLevel?: number): number {
  if (!Number.isFinite(requested) || requested < 0 || requested > 50 || !Number.isFinite(radiusMeters) || radiusMeters <= 0) throw new Error('Invalid display exaggeration.');
  if (waterLevel !== undefined && !Number.isFinite(waterLevel)) throw new Error('Invalid water display level.');
  let max = Math.abs(waterLevel ?? 0);
  for (const height of heights) max = Math.max(max, Math.abs(height));
  return max === 0 ? requested : Math.min(requested, 0.2 * radiusMeters / max);
}

/** Rebuild from immutable directions, never from already displaced vertices. */
export function displaceDirections(base: Float32Array, offsets: Float32Array, factor: number, out: Float32Array = new Float32Array(base.length)): Float32Array {
  if (base.length !== offsets.length * 3 || out.length !== base.length) throw new Error('Invalid relief geometry lengths.');
  for (let i = 0; i < offsets.length; i++) {
    const scale = 1 + factor * offsets[i];
    for (let axis = 0; axis < 3; axis++) out[i * 3 + axis] = base[i * 3 + axis] * scale;
  }
  return out;
}
