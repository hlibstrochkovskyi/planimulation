/** Independent scalar decoder/reference, not a second geography generator. */
export function continentalUplandsThickness(continentality: number, potential: number, threshold: number): number {
  if (!Number.isFinite(continentality) || continentality < 0 || continentality > 1
    || !Number.isFinite(potential) || Math.abs(potential) > 1
    || !Number.isFinite(threshold) || Math.abs(threshold) > 1.12) {
    throw new Error('Invalid continental-uplands material inputs.');
  }
  const interior = Math.max(potential - threshold, 0);
  return 7000 + 28000 * continentality + 15000 * continentality * (interior / (interior + 0.25));
}

export function columnBuoyancyBaseline(thickness: number, density: number): number {
  return -4500 + thickness * (1 - density / 3300) - 7000 * (1 - 3000 / 3300);
}
