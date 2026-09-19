import type { Surface } from '../core/surface';
import { angularDistance, normalize, readVector } from '../core/vector';
import type { Vec3 } from '../core/vector';

export type Point2 = readonly [number, number];
const TAU = Math.PI * 2;

export function directionAt(u: number, v: number): Vec3 {
  const longitude = (((u % 1) + 1) % 1 - 0.5) * TAU;
  const latitude = (0.5 - v) * Math.PI;
  return [Math.cos(latitude) * Math.cos(longitude), Math.sin(latitude), Math.cos(latitude) * Math.sin(longitude)];
}

function longitudeNear(longitude: number, previous: number): number {
  while (longitude - previous > Math.PI) longitude -= TAU;
  while (longitude - previous < -Math.PI) longitude += TAU;
  return longitude;
}

/** Project geodesic boundaries; cap polar regions and retain seam-crossing coordinates. */
export function projectRegion(surface: Surface, id: number): Point2[] {
  const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
  const points: Vec3[] = [];
  for (let i = start; i < end; i++) {
    const a = readVector(surface.boundaryDirections, i);
    const b = readVector(surface.boundaryDirections, i + 1 === end ? start : i + 1);
    const steps = Math.max(1, Math.ceil(angularDistance(a, b) / 0.04));
    for (let step = 0; step < steps; step++) {
      const t = step / steps;
      points.push(normalize([a[0] * (1 - t) + b[0] * t, a[1] * (1 - t) + b[1] * t, a[2] * (1 - t) + b[2] * t]));
    }
  }
  const isPole = (p: Vec3): boolean => Math.hypot(p[0], p[2]) < 1e-10;
  while (isPole(points[0])) points.push(points.shift()!);
  const projected: [number, number][] = [];
  for (let i = 0; i < points.length; i++) {
    const p = points[i];
    const previous = projected.length ? projected[projected.length - 1][0] : Math.atan2(p[2], p[0]);
    if (isPole(p)) {
      const next = points[(i + 1) % points.length];
      const nextLongitude = longitudeNear(Math.atan2(next[2], next[0]), previous);
      projected.push([previous, Math.sign(p[1]) * Math.PI / 2], [nextLongitude, Math.sign(p[1]) * Math.PI / 2]);
    } else {
      projected.push([longitudeNear(Math.atan2(p[2], p[0]), previous), Math.asin(Math.max(-1, Math.min(1, p[1])))]);
    }
  }
  const first = projected[0];
  const closingLongitude = longitudeNear(first[0], projected[projected.length - 1][0]);
  if (Math.abs(closingLongitude - first[0]) > Math.PI) {
    const pole = Math.sign(surface.centers[id * 3 + 1]) * Math.PI / 2;
    projected.push([closingLongitude, first[1]], [closingLongitude, pole], [first[0], pole]);
  }
  return projected.map(([longitude, latitude]) => [longitude / TAU + 0.5, 0.5 - latitude / Math.PI]);
}
