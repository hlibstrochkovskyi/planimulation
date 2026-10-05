/** Shared display checks, not replacements for authoritative native ledgers. */
export function sum(values: Iterable<number>): number {
  let total = 0, correction = 0;
  for (const value of values) {
    const adjusted = value - correction, next = total + adjusted;
    correction = (next - total) - adjusted; total = next;
  }
  return total;
}
export function matches(value: unknown, expected: unknown): boolean {
  if (typeof expected !== 'object' || expected === null) return value === expected;
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const object = value as Record<string, unknown>, reference = expected as Record<string, unknown>;
  return Object.keys(object).length === Object.keys(reference).length
    && Object.entries(reference).every(([key, item]) => matches(object[key], item));
}
export function numericRecord(value: unknown, keys: readonly string[], signed: readonly string[] = []): Record<string, number> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid seasonal-water budget.');
  const object = value as Record<string, unknown>;
  if (Object.keys(object).length !== keys.length || keys.some((key) => typeof object[key] !== 'number'
    || !Number.isFinite(object[key]) || (!signed.includes(key) && (object[key] as number) < 0))) {
    throw new Error('Invalid seasonal-water budget quantities.');
  }
  return object as Record<string, number>;
}
export function close(a: number, b: number, scale = Math.max(Math.abs(a), Math.abs(b), 1)): void {
  if (!Number.isFinite(a) || !Number.isFinite(b) || Math.abs(a - b) > 1e-12 * scale) {
    throw new Error('Seasonal-water display budget does not reconcile.');
  }
}
