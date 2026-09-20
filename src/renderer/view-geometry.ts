import { ShapeUtils, Vector2 } from 'three';
import type { Surface } from '../core/surface';
import { readVector } from '../core/vector';
import { projectRegion } from './projection';
import type { Point2 } from './projection';

export interface ViewGeometry { positions: Float32Array; regions: Float32Array; lines: Float32Array }
export interface ViewPair { flat: ViewGeometry; globe: ViewGeometry }

function clip(points: Point2[], bound: number, greater: boolean): Point2[] {
  const result: Point2[] = [];
  for (let i = 0; i < points.length; i++) {
    const a = points[i], b = points[(i + 1) % points.length];
    const insideA = greater ? a[0] >= bound : a[0] <= bound;
    const insideB = greater ? b[0] >= bound : b[0] <= bound;
    if (insideA) result.push(a);
    if (insideA !== insideB) result.push([bound, a[1] + (b[1] - a[1]) * (bound - a[0]) / (b[0] - a[0])]);
  }
  return result;
}

/** Display-only triangulation. Clipping must not create new simulation regions. */
export function buildViewGeometry(surface: Surface): ViewPair {
  const flat = { positions: [] as number[], regions: [] as number[], lines: [] as number[] };
  const globe = { positions: [] as number[], regions: [] as number[], lines: [] as number[] };
  for (let id = 0; id < surface.areasSquareMeters.length; id++) {
    const polygon = projectRegion(surface, id);
    for (const shift of [-1, 0, 1]) {
      const clipped = clip(clip(polygon.map(([x, y]) => [x + shift, y]), 0, true), 1, false);
      if (clipped.length < 3) continue;
      const contour = clipped.map(([x, y]) => new Vector2(x * 2 - 1, .5 - y));
      for (const triangle of ShapeUtils.triangulateShape(contour, [])) {
        for (const vertex of triangle) { flat.positions.push(contour[vertex].x, contour[vertex].y, 0); flat.regions.push(id); }
      }
      for (let j = 0; j < contour.length; j++) {
        const a = contour[j], b = contour[(j + 1) % contour.length];
        flat.lines.push(a.x, a.y, .0001, b.x, b.y, .0001);
      }
    }
    const center = readVector(surface.centers, id);
    const start = surface.boundaryOffsets[id], end = surface.boundaryOffsets[id + 1];
    for (let j = start; j < end; j++) {
      const a = readVector(surface.boundaryDirections, j);
      const b = readVector(surface.boundaryDirections, j + 1 === end ? start : j + 1);
      globe.positions.push(...center, ...a, ...b); globe.regions.push(id, id, id);
      globe.lines.push(...a.map((v) => v * 1.0002), ...b.map((v) => v * 1.0002));
    }
  }
  const pack = (data: typeof flat): ViewGeometry => ({ positions: Float32Array.from(data.positions),
    regions: Float32Array.from(data.regions), lines: Float32Array.from(data.lines) });
  return { flat: pack(flat), globe: pack(globe) };
}
