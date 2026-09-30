import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { parseCaptureRect } from '../src/shared/capture';

const viewport = { width: 1440, height: 940 };

test('map capture uses only a bounded integer rectangle inside the viewport', () => {
  const rect = { x: 100, y: 80, width: 1200, height: 600 };
  assert.deepEqual(parseCaptureRect(rect, viewport), rect);
  assert.deepEqual(parseCaptureRect({ ...rect, x: 300 }, viewport), { x: 300, y: 80, width: 1140, height: 600 });
  assert.deepEqual(parseCaptureRect(rect, { width: 1000, height: 500 }), { x: 100, y: 80, width: 900, height: 420 });
  for (const invalid of [
    null, [], { ...rect, x: -1 }, { ...rect, width: 0 }, { ...rect, x: 1440 },
    { ...rect, width: 1.5 }, { ...rect, height: NaN }, { ...rect, height: Infinity },
    { x: 0, y: 0, width: 8192, height: 8192 },
  ]) assert.throws(() => parseCaptureRect(invalid, viewport), /Invalid map capture region/);
  assert.throws(() => parseCaptureRect(rect, { width: NaN, height: 940 }), /Invalid map capture region/);
});
