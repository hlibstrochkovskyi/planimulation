/** Fixed levels shared by the map and globe. Time is interpolated in log space. */
export const ISOCHRONE_DAYS = [7, 14, 30, 60, 90, 120, 180, 270, 365, 540, 730, 1095, 1460];
export const UNREACHABLE_COLOR = [91, 96, 106] as const;

const STOPS = [
  [0, [255, 249, 194]],
  [14, [246, 211, 111]],
  [60, [222, 138, 70]],
  [120, [181, 75, 92]],
  [365, [122, 60, 137]],
  [1460, [50, 52, 110]],
] as const;

export const travelTimeCoordinate = (days: number): number => Math.log1p(days / 14);

export function travelTimeColor(days: number): readonly [number, number, number] {
  if (days < 0) return UNREACHABLE_COLOR;
  for (let i = 1; i < STOPS.length; i++) {
    const [end, high] = STOPS[i];
    if (days > end) continue;
    const [start, low] = STOPS[i - 1];
    const a = travelTimeCoordinate(start);
    const t = (travelTimeCoordinate(days) - a) / (travelTimeCoordinate(end) - a);
    return [
      low[0] + (high[0] - low[0]) * t,
      low[1] + (high[1] - low[1]) * t,
      low[2] + (high[2] - low[2]) * t,
    ];
  }
  return STOPS[STOPS.length - 1][1];
}
