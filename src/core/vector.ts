export type Vec3 = readonly [number, number, number];

export function dot(a: Vec3, b: Vec3): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

export function cross(a: Vec3, b: Vec3): Vec3 {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}

export function normalize(v: Vec3): Vec3 {
  const length = Math.hypot(...v);
  if (length === 0) throw new Error('Cannot normalize a zero vector.');
  return [v[0] / length, v[1] / length, v[2] / length];
}

export function midpoint(a: Vec3, b: Vec3): Vec3 {
  return normalize([a[0] + b[0], a[1] + b[1], a[2] + b[2]]);
}

export function readVector(array: Float64Array, index: number): Vec3 {
  return [array[index * 3], array[index * 3 + 1], array[index * 3 + 2]];
}

export function angularDistance(a: Vec3, b: Vec3): number {
  return Math.atan2(Math.hypot(...cross(a, b)), dot(a, b));
}

/** Solid angle of a minor spherical triangle, in steradians. Inputs are unit vectors. */
export function triangleArea(a: Vec3, b: Vec3, c: Vec3): number {
  return 2 * Math.atan2(Math.abs(dot(a, cross(b, c))), 1 + dot(a, b) + dot(b, c) + dot(c, a));
}
