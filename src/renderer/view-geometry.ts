import { Color, ShapeUtils, Vector2 } from 'three';
import type { Surface } from '../core/surface';
import { readVector } from '../core/vector';
import { projectArc, projectRegion } from './projection';
import type { Point2 } from './projection';
import type { Tectonics } from '../core/tectonics';
import { BOUNDARY_COLORS } from '../core/tectonics';
import type { Water } from '../core/water';
import { buildWaterSurface } from './water-surface';

export interface ViewGeometry { positions: Float32Array; regions: Float32Array; lines: Float32Array;
  tectonicLines: Float32Array; tectonicColors: Float32Array;
  radialOffsets: Float32Array; lineOffsets: Float32Array; tectonicOffsets: Float32Array;
  waterPositions: Float32Array; waterRegions: Float32Array; waterOffsets: Float32Array;
  waterLines: Float32Array; waterLineOffsets: Float32Array }
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
export function buildViewGeometry(surface: Surface, tectonics?: Tectonics, elevation?: Float64Array, water?: Water): ViewPair {
  if (elevation && elevation.length !== surface.areasSquareMeters.length) throw new Error('Invalid elevation field length.');
  // Every shared dual corner uses the same mean of its incident region heights.
  // This interpolation belongs to the viewer, not to the physical model.
  const shared = new Map<string, { sum: number; count: number }>();
  const key = (v: readonly number[]): string => v.join(',');
  if (elevation) for (let id = 0; id < elevation.length; id++) {
    for (let j = surface.boundaryOffsets[id]; j < surface.boundaryOffsets[id + 1]; j++) {
      const k = key(readVector(surface.boundaryDirections, j));
      const value = shared.get(k) ?? { sum: 0, count: 0 };
      value.sum += elevation[id]; value.count++; shared.set(k, value);
    }
  }
  const corner = (v: readonly number[]): number => {
    if (!elevation) return 0;
    const value = shared.get(key(v));
    if (!value) throw new Error('Missing shared relief corner.');
    return value.sum / value.count / surface.radiusMeters;
  };
  const radialOffsets: number[] = [], lineOffsets: number[] = [], tectonicOffsets: number[] = [];
  const flat = { positions: [] as number[], regions: [] as number[], lines: [] as number[], tectonicLines: [] as number[], tectonicColors: [] as number[] };
  const globe = { positions: [] as number[], regions: [] as number[], lines: [] as number[], tectonicLines: [] as number[], tectonicColors: [] as number[] };
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
      const ha = corner(a), hb = corner(b);
      radialOffsets.push((elevation?.[id] ?? 0) / surface.radiusMeters, ha, hb);
      lineOffsets.push(ha, hb);
      globe.lines.push(...a.map((v) => v * 1.0002), ...b.map((v) => v * 1.0002));
    }
  }
  if (tectonics) for (let i = 0; i < tectonics.boundaryTypes.length; i++) {
    const a = readVector(tectonics.boundaryDirections, i * 2), b = readVector(tectonics.boundaryDirections, i * 2 + 1);
    const color = new Color(BOUNDARY_COLORS[tectonics.boundaryTypes[i]]);
    const rgb = [color.r, color.g, color.b];
    globe.tectonicLines.push(...a.map((v) => v * 1.0008), ...b.map((v) => v * 1.0008));
    tectonicOffsets.push(corner(a), corner(b));
    globe.tectonicColors.push(...rgb, ...rgb);
    for (const [p, q] of projectArc(a, b)) {
      flat.tectonicLines.push(p[0] * 2 - 1, .5 - p[1], .0003, q[0] * 2 - 1, .5 - q[1], .0003);
      flat.tectonicColors.push(...rgb, ...rgb);
    }
  }
  const pack = (data: typeof flat): ViewGeometry => ({ positions: Float32Array.from(data.positions),
    regions: Float32Array.from(data.regions), lines: Float32Array.from(data.lines),
    tectonicLines: Float32Array.from(data.tectonicLines), tectonicColors: Float32Array.from(data.tectonicColors),
    radialOffsets: new Float32Array(0), lineOffsets: new Float32Array(0), tectonicOffsets: new Float32Array(0),
    waterPositions: new Float32Array(0), waterRegions: new Float32Array(0), waterOffsets: new Float32Array(0),
    waterLines: new Float32Array(0), waterLineOffsets: new Float32Array(0) });
  const globeView = { ...pack(globe), radialOffsets: Float32Array.from(radialOffsets),
    lineOffsets: Float32Array.from(lineOffsets), tectonicOffsets: Float32Array.from(tectonicOffsets) };
  if (water) Object.assign(globeView, buildWaterSurface(globeView.positions, globeView.regions, water, surface.radiusMeters));
  return { flat: pack(flat), globe: globeView };
}
