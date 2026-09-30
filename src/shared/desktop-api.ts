import type { Recipe } from '../core/recipe';
import type { World } from '../core/world';
import type { CaptureRect } from './capture';

export interface DiagnosticFrame { epoch: number; tick: number; relativeMassError: number; field: Float64Array }

export interface DesktopAPI {
  openRecipe(): Promise<Recipe | null>;
  saveRecipe(recipe: Recipe): Promise<boolean>;
  exportView(rect: CaptureRect): Promise<boolean>;
  generate(recipe: Recipe): Promise<{ world: World; epoch: number }>;
  cancelGeneration(): Promise<void>;
  acceptWorld(epoch: number): Promise<void>;
  advance(epoch: number, steps: number): Promise<DiagnosticFrame>;
}

declare global {
  interface Window { desktop: DesktopAPI }
}
