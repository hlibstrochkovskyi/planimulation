import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { buildSurface, locateRegion } from '../src/core/surface';
import { directionAt, projectRegion } from '../src/renderer/projection';
import type { Point2 } from '../src/renderer/projection';

function clipX(polygon: Point2[], limit: number, keepGreater: boolean): Point2[] {
  const result: Point2[] = [];
  for (let i = 0; i < polygon.length; i++) {
    const a = polygon[i], b = polygon[(i + 1) % polygon.length];
    const insideA = keepGreater ? a[0] >= limit : a[0] <= limit;
    const insideB = keepGreater ? b[0] >= limit : b[0] <= limit;
    if (insideA) result.push(a);
    if (insideA !== insideB) result.push([limit, a[1] + (b[1] - a[1]) * (limit - a[0]) / (b[0] - a[0])]);
  }
  return result;
}

for (const level of [0, 1, 3]) {
  test(`projection level ${level} covers the rectangle including poles and seam`, () => {
    const surface = buildSurface(level, 1000);
    let area = 0;
    for (let id = 0; id < surface.areasSquareMeters.length; id++) {
      const polygon = projectRegion(surface, id);
      assert.ok(polygon.every(([u, v]) => Number.isFinite(u) && v >= 0 && v <= 1));
      for (const shift of [-1, 0, 1]) {
        const clipped = clipX(clipX(polygon.map(([x, y]) => [x + shift, y]), 0, true), 1, false);
        let sum = 0;
        for (let j = 0; j < clipped.length; j++) {
          const a = clipped[j], b = clipped[(j + 1) % clipped.length];
          sum += a[0] * b[1] - b[0] * a[1];
        }
        area += Math.abs(sum) / 2;
      }
    }
    assert.ok(Math.abs(area - 1) < 1e-8, `Projected area was ${area}`);
    assert.equal(locateRegion(surface, directionAt(0, 0.4)), locateRegion(surface, directionAt(1, 0.4)));
  });
}
