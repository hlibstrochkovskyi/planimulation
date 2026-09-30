export interface CaptureRect { x: number; y: number; width: number; height: number }

/** Validate and clip an untrusted region to the actual native viewport. */
export function parseCaptureRect(value: unknown, viewport: { width: number; height: number }): CaptureRect {
  if (!Number.isSafeInteger(viewport.width) || !Number.isSafeInteger(viewport.height)
    || viewport.width < 1 || viewport.height < 1) throw new Error('Invalid map capture region.');
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid map capture region.');
  const input = value as Record<string, unknown>;
  const { x, y, width, height } = input;
  if (![x, y, width, height].every((part) => typeof part === 'number' && Number.isSafeInteger(part))
    || typeof x !== 'number' || typeof y !== 'number' || typeof width !== 'number' || typeof height !== 'number'
    || x < 0 || y < 0 || width < 1 || height < 1 || width > 8192 || height > 8192
    || width * height > 16_777_216 || x >= viewport.width || y >= viewport.height) {
    throw new Error('Invalid map capture region.');
  }
  return { x, y, width: Math.min(width, viewport.width - x), height: Math.min(height, viewport.height - y) };
}
