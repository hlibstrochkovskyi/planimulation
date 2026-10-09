import type { MoistureFrame } from './seasonal-moisture';
import type { SoilMoistureFrame } from './soil-moisture';

/** A presentation union, not a conversion between physical state families. */
export type SeasonalDisplayFrame = MoistureFrame | SoilMoistureFrame;
export function isSoilMoistureFrame(frame: SeasonalDisplayFrame): frame is SoilMoistureFrame {
  return frame.modelVersion === 'regional-seasonal-water-1';
}
export function hasRegionalSurface(frame: SeasonalDisplayFrame | null): boolean {
  return !!frame && (isSoilMoistureFrame(frame) || !!frame.regionalSurface);
}
