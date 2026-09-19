import type { Recipe } from '../core/recipe';

export interface DesktopAPI {
  openRecipe(): Promise<Recipe | null>;
  saveRecipe(recipe: Recipe): Promise<boolean>;
}

declare global {
  interface Window { desktop: DesktopAPI }
}
