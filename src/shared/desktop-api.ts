import type { Recipe } from '../core/recipe';
import type { World } from '../core/world';
import type { CaptureRect } from './capture';
import type { TemperatureNormals } from '../core/seasonal-temperature';
import type { WindNormals } from '../core/seasonal-wind';
import type { MoistureFrame } from './seasonal-moisture';

export interface DiagnosticFrame { epoch: number; tick: number; relativeMassError: number; field: Float64Array }
export type PrescribedWaterMode = 'oneCubicKilometer' | 'fillToSpill';
export interface WaterFrame { epoch: number; step: number; inputUnits: string; acceptedInputUnits: string;
  depthMeters: Float64Array; surfaceLevelsMeters: Float64Array; bodyIds: Uint32Array; mainOceanId: number }
export interface PreparedWaterWorld { world: World; waterFrame: WaterFrame; epoch: number }
export interface PreparedMoistureWorld { world: World; moistureFrame: MoistureFrame; epoch: number }
export interface WaterBudget { epoch: number; step: number; initialTotalUnits: string; acceptedInputUnits: string;
  storedTotalUnits: string; stocks: Array<{ branch: number; volumeUnits: string }> }

export interface DesktopAPI {
  openRecipe(): Promise<Recipe | null>;
  saveRecipe(recipe: Recipe): Promise<boolean>;
  exportView(rect: CaptureRect): Promise<boolean>;
  generate(recipe: Recipe): Promise<{ world: World; epoch: number }>;
  cancelGeneration(): Promise<void>;
  acceptWorld(epoch: number): Promise<void>;
  advance(epoch: number, steps: number): Promise<DiagnosticFrame>;
  seasonalTemperature(epoch: number): Promise<TemperatureNormals>;
  seasonalWind(epoch: number): Promise<WindNormals>;
  seasonalMoisture(epoch: number, seconds: number): Promise<MoistureFrame>;
  initializeOrographicMoisture(epoch: number): Promise<MoistureFrame>;
  initializePreciseMoisture(epoch: number): Promise<MoistureFrame>;
  initializeRegionalMoisture(epoch: number): Promise<MoistureFrame>;
  openSeasonalCheckpoint(): Promise<PreparedMoistureWorld | null>;
  saveSeasonalCheckpoint(epoch: number): Promise<boolean>;
  prescribeWater(epoch: number, region: number, mode: PrescribedWaterMode): Promise<WaterFrame>;
  openWaterCheckpoint(): Promise<PreparedWaterWorld | null>;
  saveWaterCheckpoint(epoch: number): Promise<boolean>;
  inspectWaterBudget(epoch: number): Promise<WaterBudget>;
  saveResolvedWorld(epoch: number): Promise<boolean>;
}

declare global {
  interface Window { desktop: DesktopAPI }
}
