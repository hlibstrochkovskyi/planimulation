export const WILD_FOOD_MODEL_VERSION = 'wild-food-1';
export const MONTHS = 12;

/** Native wild-food-1 defaults; the core rejects any other desktop settings. */
export const DEFAULT_WILD_FOOD_SETTINGS = {
  herbEdibleShare: 0.000015,
  woodyEdibleShare: 0.00003,
  canopyPenalty: 0.8,
  canopyHuntingPenalty: 0.5,
  plantEnergyKcalPerGram: 4,
  plantSpoilagePerMonth: 0.5,
  browseShare: 0.15,
  gameCapacityGrams: 4.5,
  gameReferenceForage: 750,
  gameCapacityExponent: 1.5,
  gameMemoryYears: 3,
  gameGrowthPerYear: 0.3,
  gameDiebackPerYear: 0.5,
  gameEstablishment: 0.02,
  gameHumanShare: 0.2,
  gameEnergyKcalPerGram: 0.75,
  maximumHuntingOfftake: 0.3,
  needKcalPerDay: 2200,
  coldNeedIncrease: 0.2,
} as const;
export type WildFoodSettings = typeof DEFAULT_WILD_FOOD_SETTINGS;
/** Temperatures (°C) between which the need rises from normal to its cold maximum. */
export const COLD_NEED_CELSIUS = [10, -20] as const;

/** Land totals of one simulated year. */
export interface WildFoodYear {
  /** Year-round forager capacity of all land, people. */
  capacityPeople: number;
  /** Land plant food and sustainable game offtake left to people, kcal/year. */
  plantKcal: number;
  gameKcal: number;
  /** Land game stock relative to its capacity. */
  gameRatio: number;
}

/** Wild food on the vegetation state; energies kcal/m²/year, capacities people/km². */
export interface WildFoodFrame {
  epoch: number;
  modelVersion: typeof WILD_FOOD_MODEL_VERSION;
  year: number;
  /** Year-round capacity without human storage. */
  capacity: Float64Array;
  /** Capacity if a whole year's food could be stored without loss. */
  storedCapacity: Float64Array;
  plantKcal: Float64Array;
  gameKcal: Float64Array;
  /** Game live weight and its capacity, g/m². */
  game: Float64Array;
  gameCapacity: Float64Array;
  /** Share of hunting success that remains under the canopy. */
  access: Float64Array;
  /** Month (0–11) with the least new plant food relative to need. */
  leanMonth: Uint32Array;
  summaries: WildFoodYear[];
}

export interface WildFoodRegionYear {
  capacity: number;
  storedCapacity: number;
  plantKcal: number;
  gameKcal: number;
  game: number;
  gameCapacity: number;
  access: number;
  leanMonth: number;
  /** Share of the year's plant food produced in each month. */
  season: number[];
}

export interface WildFoodSeries {
  epoch: number;
  region: number;
  years: WildFoodRegionYear[];
}

/** A group forages in one region from year 1 for `years` years, then leaves. */
export interface ForageExperiment {
  epoch: number;
  region: number;
  people: number;
  years: number;
  /** Game live weight with the group (g/m²) and the unmet share of its need, from year 0. */
  game: number[];
  shortfall: number[];
}
