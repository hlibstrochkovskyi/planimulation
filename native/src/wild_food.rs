//! Wild food for foragers on top of `vegetation-2`. Each land region produces
//! harvestable plant food (seeds, tubers, fruit, nuts) as a share of that year's net
//! primary productivity, distributed over the months of the growing season and lost
//! to spoilage and other consumers while it stands. Game is a stock: herbivore live
//! weight relaxes toward a capacity set by forage, grows logistically, dies back in
//! poor years and recovers with a lag. Forager capacity is the largest year-round
//! population whose monthly needs, higher in cold months, are met by standing plant
//! food and the year's sustainable game offtake, hunted when plants run short, without
//! human storage.
//!
//! A read-only output: it never changes vegetation or climate and has no random
//! stream. Not modeled: aquatic food, fire, human storage or processing, game
//! migration between regions, and competition among species.
use crate::{
    World,
    climate_normals::Normals,
    vegetation::{Model as VegetationModel, RegionYear, State as VegetationState},
};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "wild-food-1";
pub const MONTHS: usize = 12;
const DAYS_PER_MONTH: f64 = 365.25 / 12.;
/// Square meters per square kilometer.
const KM2: f64 = 1e6;
/// Woody cover over which a closed canopy reduces food on the ground.
const CANOPY_COVER: (f64, f64) = (0.5, 0.9);
/// Monthly temperatures over which growth starts, °C.
const GROWTH_CELSIUS: (f64, f64) = (0., 10.);
/// Monthly precipitation relative to twice the temperature (Bagnouls–Gaussen dry
/// month) over which moisture stops limiting growth.
const MOISTURE_RATIO: (f64, f64) = (0.5, 1.5);
/// Temperatures between which the energy need rises to its cold maximum, °C.
const COLD_NEED_CELSIUS: (f64, f64) = (10., -20.);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// Harvestable edible share of herbaceous NPP (grass seeds, tubers, greens).
    pub herb_edible_share: f64,
    /// Harvestable edible share of woody NPP (fruit, nuts) under an open canopy.
    pub woody_edible_share: f64,
    /// Loss of plant food reachable from the ground under a fully closed canopy.
    pub canopy_penalty: f64,
    /// Loss of hunting success under a fully closed canopy.
    pub canopy_hunting_penalty: f64,
    /// Energy of edible plant dry matter, kcal/g.
    pub plant_energy_kcal_per_gram: f64,
    /// Monthly loss of standing plant food to spoilage and other consumers.
    pub plant_spoilage_per_month: f64,
    /// Woody NPP available as browse, relative to herbaceous NPP.
    pub browse_share: f64,
    /// Game capacity in g live weight / m² at the reference forage.
    pub game_capacity_grams: f64,
    /// Forage, g dry matter/m²/year, at which game reaches `game_capacity_grams`.
    pub game_reference_forage: f64,
    /// Exponent of game capacity in forage; above 1, as herbivore biomass across
    /// ecosystems grows faster than productivity.
    pub game_capacity_exponent: f64,
    /// Time constant of the game capacity's memory of forage, years: herds ride out a
    /// single poor year on body reserves and movement within the region.
    pub game_memory_years: f64,
    pub game_growth_per_year: f64,
    pub game_dieback_per_year: f64,
    /// Recolonization per year as a share of capacity.
    pub game_establishment: f64,
    /// Share of the sustainable game production left to people by other predators.
    pub game_human_share: f64,
    /// Edible energy per gram of live weight, kcal/g.
    pub game_energy_kcal_per_gram: f64,
    /// Largest share of the game stock hunters can take in a year.
    pub maximum_hunting_offtake: f64,
    pub need_kcal_per_day: f64,
    /// Relative increase of the need at −20 °C and below.
    pub cold_need_increase: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            herb_edible_share: 0.000_015,
            woody_edible_share: 0.000_03,
            canopy_penalty: 0.8,
            canopy_hunting_penalty: 0.5,
            plant_energy_kcal_per_gram: 4.,
            plant_spoilage_per_month: 0.5,
            browse_share: 0.15,
            game_capacity_grams: 4.5,
            game_reference_forage: 750.,
            game_capacity_exponent: 1.5,
            game_memory_years: 3.,
            game_growth_per_year: 0.3,
            game_dieback_per_year: 0.5,
            game_establishment: 0.02,
            game_human_share: 0.2,
            game_energy_kcal_per_gram: 0.75,
            maximum_hunting_offtake: 0.3,
            need_kcal_per_day: 2_200.,
            cold_need_increase: 0.2,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        let unit = [
            self.herb_edible_share,
            self.woody_edible_share,
            self.canopy_penalty,
            self.canopy_hunting_penalty,
            self.plant_spoilage_per_month,
            self.browse_share,
            self.game_dieback_per_year,
            self.game_establishment,
            self.game_human_share,
            self.maximum_hunting_offtake,
            self.cold_need_increase,
        ];
        let positive = [
            self.plant_energy_kcal_per_gram,
            self.game_capacity_grams,
            self.game_reference_forage,
            self.game_capacity_exponent,
            self.game_growth_per_year,
            self.game_energy_kcal_per_gram,
            self.need_kcal_per_day,
        ];
        if unit.iter().any(|v| !(0. ..=1.).contains(v))
            || positive.iter().any(|&v| !(v > 0. && v.is_finite()))
            || self.plant_spoilage_per_month == 0.
            || !(self.game_memory_years >= 1. && self.game_memory_years <= 100.)
        {
            return Err("Wild-food settings are outside their supported ranges.".into());
        }
        Ok(())
    }
}

fn smoothstep((low, high): (f64, f64), x: f64) -> f64 {
    let t = ((x - low) / (high - low)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// Logistic growth toward `capacity` from below, proportional dieback from above.
fn relax(value: f64, capacity: f64, growth: f64, dieback: f64, establishment: f64) -> f64 {
    if value < capacity {
        (value + growth * (value + establishment) * (capacity - value) / capacity).min(capacity)
    } else {
        value - dieback * (value - capacity)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YearSummary {
    /// Year-round forager capacity of all land, people.
    pub capacity_people: f64,
    /// Land plant food, kcal/year.
    pub plant_kcal: f64,
    /// Land sustainable game offtake left to people, kcal/year.
    pub game_kcal: f64,
    /// Land game stock relative to its capacity.
    pub game_ratio: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub year: u32,
    /// Game live weight and its capacity, g/m².
    pub game: Vec<f64>,
    pub game_capacity: Vec<f64>,
    pub summaries: Vec<YearSummary>,
}

/// Food of one region in one year. Energies are kcal/m²/year, capacities people/km².
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RegionFood {
    pub plant_kcal: f64,
    pub game_kcal: f64,
    /// Game live weight and its capacity, g/m².
    pub game: f64,
    pub game_capacity: f64,
    /// Year-round capacity without human storage.
    pub capacity: f64,
    /// Capacity if a whole year's food could be stored without loss.
    pub stored_capacity: f64,
    /// Month (0–11) with the least new plant food relative to need.
    pub lean_month: u8,
    /// Share of hunting success that remains under the canopy.
    pub access: f64,
    /// Share of the plant food produced in each month.
    pub season: [f64; MONTHS],
}

/// One year of a foraging experiment in a single region.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ForageYear {
    /// Game live weight with the group, g/m².
    pub game: f64,
    /// Share of the group's need that was not met.
    pub shortfall: f64,
}

pub struct Model {
    settings: Settings,
    land: Vec<bool>,
    areas: Vec<f64>,
    temperature: Vec<[f64; MONTHS]>,
    precipitation: Vec<[f64; MONTHS]>,
    /// Per-person need in each month of each region, kcal.
    need: Vec<[f64; MONTHS]>,
}

impl Model {
    pub fn new(
        world: &World,
        normals: &Normals,
        vegetation: &VegetationModel,
        vegetation_state: &VegetationState,
        settings: Settings,
    ) -> Result<(Self, State), String> {
        settings.validate()?;
        let n = world.surface.centers.len();
        let months = |field: &[Vec<f64>]| -> Result<Vec<[f64; MONTHS]>, String> {
            if field.len() != MONTHS || field.iter().any(|m| m.len() != n) {
                return Err("Wild-food climate arrays do not match the surface.".into());
            }
            Ok((0..n)
                .map(|i| std::array::from_fn(|m| field[m][i]))
                .collect())
        };
        let temperature = months(&normals.monthly_temperature_celsius)?;
        let precipitation = months(&normals.monthly_precipitation_mm)?;
        if vegetation_state.woody.len() != n || world.water.body_ids.len() != n {
            return Err("Wild-food vegetation arrays do not match the surface.".into());
        }
        let need = temperature
            .iter()
            .map(|t| {
                t.map(|c| {
                    settings.need_kcal_per_day
                        * DAYS_PER_MONTH
                        * (1. + settings.cold_need_increase * smoothstep(COLD_NEED_CELSIUS, c))
                })
            })
            .collect();
        let model = Self {
            settings,
            land: world.water.body_ids.iter().map(|&b| b == 0).collect(),
            areas: world.surface.areas.clone(),
            temperature,
            precipitation,
            need,
        };
        let capacity: Vec<f64> = (0..n)
            .map(|i| model.forage_capacity(&vegetation.region(vegetation_state, i)))
            .collect();
        let mut state = State {
            game: capacity.clone(),
            game_capacity: capacity,
            ..State::default()
        };
        let summary = model.summary(vegetation, vegetation_state, &state);
        state.summaries.push(summary);
        Ok((model, state))
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    /// Share of the year's plant growth in each month.
    pub fn season(&self, i: usize, factor: f64) -> [f64; MONTHS] {
        let (t, p) = (&self.temperature[i], &self.precipitation[i]);
        let thermal = t.map(|c| smoothstep(GROWTH_CELSIUS, c));
        let weights: [f64; MONTHS] = std::array::from_fn(|m| {
            thermal[m] * smoothstep(MOISTURE_RATIO, p[m] * factor / (2. * t[m]).max(10.))
        });
        let total: f64 = weights.iter().sum();
        let thermal_total: f64 = thermal.iter().sum();
        if total > 0. {
            weights.map(|w| w / total)
        } else if thermal_total > 0. {
            thermal.map(|w| w / thermal_total)
        } else {
            [1. / MONTHS as f64; MONTHS]
        }
    }

    fn woody_share(year: &RegionYear) -> f64 {
        let cover = year.woody + year.herb;
        if cover > 0. { year.woody / cover } else { 0. }
    }

    /// Shares of tree food and of hunting success that remain under the canopy.
    pub fn access(&self, year: &RegionYear) -> (f64, f64) {
        let closure = smoothstep(CANOPY_COVER, year.woody);
        (
            1. - self.settings.canopy_penalty * closure,
            1. - self.settings.canopy_hunting_penalty * closure,
        )
    }

    /// Harvestable plant food, kcal/m²/year.
    pub fn plant_food(&self, year: &RegionYear) -> f64 {
        let s = &self.settings;
        let woody = Self::woody_share(year);
        year.productivity
            * ((1. - woody) * s.herb_edible_share
                + woody * s.woody_edible_share * self.access(year).0)
            * s.plant_energy_kcal_per_gram
    }

    /// Forage from herbaceous growth and browse, g dry matter/m²/year.
    pub fn forage_supply(&self, year: &RegionYear) -> f64 {
        let woody = Self::woody_share(year);
        year.productivity * ((1. - woody) + woody * self.settings.browse_share)
    }

    /// Game capacity of this year's forage alone, g live weight/m².
    pub fn forage_capacity(&self, year: &RegionYear) -> f64 {
        let s = &self.settings;
        s.game_capacity_grams
            * (self.forage_supply(year) / s.game_reference_forage).powf(s.game_capacity_exponent)
    }

    /// Sustainable offtake left to people, kcal/m²/year: logistic production at the
    /// stock, or at half the capacity when the stock is larger.
    pub fn game_food(&self, game: f64, capacity: f64, access: f64) -> f64 {
        if capacity <= 0. {
            return 0.;
        }
        let s = &self.settings;
        let stock = game.min(capacity / 2.);
        s.game_growth_per_year
            * stock
            * (1. - stock / capacity)
            * s.game_human_share
            * access
            * s.game_energy_kcal_per_gram
    }

    /// Game a population of `people` per km² must hunt in a repeating year, kcal/m²,
    /// eating standing plant food (lost at the spoilage rate) first and hunting in
    /// whichever months plant food runs short.
    fn hunted(&self, i: usize, people: f64, plant: &[f64; MONTHS]) -> f64 {
        let keep = 1. - self.settings.plant_spoilage_per_month;
        // The second cycle starts from the first one's end, close to the repeating year.
        let (mut stock, mut hunted) = (0., 0.);
        for _ in 0..2 {
            hunted = 0.;
            for m in 0..MONTHS {
                let demand = people / KM2 * self.need[i][m];
                stock = stock * keep + plant[m];
                let eaten = demand.min(stock);
                stock -= eaten;
                hunted += demand - eaten;
            }
        }
        hunted
    }

    #[cfg(test)]
    fn feasible(&self, i: usize, people: f64, plant: &[f64; MONTHS], game: f64) -> bool {
        self.hunted(i, people, plant) <= game
    }

    /// Year-round capacity and perfect-storage capacity, people/km². The hunting need
    /// is continuous, piecewise linear and increasing in population, so the capacity is
    /// found by false position (Illinois variant) from the month-by-month
    /// self-sufficient population, which is always feasible.
    pub fn capacity(&self, i: usize, plant: &[f64; MONTHS], game: f64) -> (f64, f64) {
        let need: f64 = self.need[i].iter().sum();
        let stored = (plant.iter().sum::<f64>() + game) / need * KM2;
        let mut low = (0..MONTHS)
            .map(|m| (plant[m] + game / MONTHS as f64) / self.need[i][m] * KM2)
            .fold(f64::INFINITY, f64::min)
            .min(stored);
        let mut high = stored;
        let mut f_high = self.hunted(i, high, plant) - game;
        if f_high <= 0. {
            return (high, stored);
        }
        let mut f_low = self.hunted(i, low, plant) - game;
        let mut side = 0;
        for _ in 0..60 {
            if high - low <= 1e-3 * high {
                break;
            }
            let mid = (low * f_high - high * f_low) / (f_high - f_low);
            let mid = if mid > low && mid < high {
                mid
            } else {
                0.5 * (low + high)
            };
            let f = self.hunted(i, mid, plant) - game;
            if f <= 0. {
                low = mid;
                f_low = f;
                if side == -1 {
                    f_high *= 0.5;
                }
                side = -1;
            } else {
                high = mid;
                f_high = f;
                if side == 1 {
                    f_low *= 0.5;
                }
                side = 1;
            }
        }
        (low, stored)
    }

    /// Food of region `i` in a year with the given game stock and capacity, g/m².
    pub fn region(
        &self,
        year: &RegionYear,
        (game, game_capacity): (f64, f64),
        i: usize,
    ) -> RegionFood {
        if !self.land[i] {
            return RegionFood::default();
        }
        let season = self.season(i, year.precipitation_factor);
        let plant_kcal = self.plant_food(year);
        let access = self.access(year).1;
        let game_kcal = self.game_food(game, game_capacity, access);
        let plant = season.map(|s| s * plant_kcal);
        let (capacity, stored_capacity) = self.capacity(i, &plant, game_kcal);
        let lean_month = (0..MONTHS)
            .min_by(|&a, &b| (plant[a] / self.need[i][a]).total_cmp(&(plant[b] / self.need[i][b])))
            .unwrap_or(0) as u8;
        RegionFood {
            plant_kcal,
            game_kcal,
            game,
            game_capacity,
            capacity,
            stored_capacity,
            lean_month,
            access,
            season,
        }
    }

    /// Food of every region for the vegetation state the food state belongs to.
    pub fn regions(
        &self,
        vegetation: &VegetationModel,
        v: &VegetationState,
        state: &State,
    ) -> Vec<RegionFood> {
        // Regions are independent; threads take blocks from a shared counter, so the
        // result does not depend on the thread count.
        const BLOCK: usize = 512;
        let n = self.land.len();
        let threads = std::thread::available_parallelism().map_or(1, |t| t.get().min(8));
        let next = std::sync::atomic::AtomicUsize::new(0);
        let mut blocks: Vec<(usize, Vec<RegionFood>)> = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..threads)
                .map(|_| {
                    scope.spawn(|| {
                        let mut done = Vec::new();
                        loop {
                            let start = next.fetch_add(BLOCK, std::sync::atomic::Ordering::Relaxed);
                            if start >= n {
                                break done;
                            }
                            let foods = (start..(start + BLOCK).min(n))
                                .map(|i| {
                                    let herd = (state.game[i], state.game_capacity[i]);
                                    self.region(&vegetation.region(v, i), herd, i)
                                })
                                .collect();
                            done.push((start, foods));
                        }
                    })
                })
                .collect();
            workers
                .into_iter()
                .flat_map(|w| w.join().expect("wild-food worker panicked"))
                .collect()
        });
        blocks.sort_unstable_by_key(|(start, _)| *start);
        blocks.into_iter().flat_map(|(_, foods)| foods).collect()
    }

    fn summary(
        &self,
        vegetation: &VegetationModel,
        v: &VegetationState,
        state: &State,
    ) -> YearSummary {
        self.summarize(&self.regions(vegetation, v, state))
    }

    fn summarize(&self, regions: &[RegionFood]) -> YearSummary {
        let mut summary = YearSummary::default();
        let (mut game, mut capacity) = (0., 0.);
        for (&a, food) in self.areas.iter().zip(regions) {
            summary.capacity_people += food.capacity * a / KM2;
            summary.plant_kcal += food.plant_kcal * a;
            summary.game_kcal += food.game_kcal * a;
            game += a * food.game;
            capacity += a * food.game_capacity;
        }
        summary.game_ratio = if capacity > 0. { game / capacity } else { 0. };
        summary
    }

    /// Next year's game stock and capacity: the capacity remembers forage over a few
    /// years, and the stock grows toward it logistically or dies back above it.
    fn next_herd(&self, (game, capacity): (f64, f64), year: &RegionYear) -> (f64, f64) {
        let s = &self.settings;
        let capacity = capacity + (self.forage_capacity(year) - capacity) / s.game_memory_years;
        if capacity <= 0. {
            return (game * (1. - s.game_dieback_per_year), 0.);
        }
        let game = relax(
            game,
            capacity,
            s.game_growth_per_year,
            s.game_dieback_per_year,
            s.game_establishment * capacity,
        );
        (game, capacity)
    }

    fn first_herd(&self, series: &[RegionYear]) -> (f64, f64) {
        let capacity = series.first().map_or(0., |y| self.forage_capacity(y));
        (capacity, capacity)
    }

    /// Advance one year after the vegetation state advanced one year.
    pub fn step(
        &self,
        vegetation: &VegetationModel,
        v: &VegetationState,
        state: &mut State,
    ) -> Result<(), String> {
        Self::check_next_year(v, state)?;
        for i in (0..self.land.len()).filter(|&i| self.land[i]) {
            self.advance_herd(i, &vegetation.region(v, i), state);
        }
        state.year = v.year;
        let summary = self.summary(vegetation, v, state);
        state.summaries.push(summary);
        Ok(())
    }

    /// Advance one year and return the regional values used for the summary.
    /// This path runs on the caller's thread so an ensemble can parallelize across seeds.
    pub fn step_with_regions(
        &self,
        vegetation: &VegetationModel,
        v: &VegetationState,
        state: &mut State,
        regions: &mut Vec<RegionFood>,
    ) -> Result<(), String> {
        Self::check_next_year(v, state)?;
        regions.clear();
        regions.extend(self.land.iter().enumerate().map(|(i, &land)| {
            if !land {
                return RegionFood::default();
            }
            let year = vegetation.region(v, i);
            let herd = self.advance_herd(i, &year, state);
            self.region(&year, herd, i)
        }));
        state.year = v.year;
        state.summaries.push(self.summarize(regions));
        Ok(())
    }

    fn check_next_year(v: &VegetationState, state: &State) -> Result<(), String> {
        if v.year != state.year + 1 {
            return Err("Wild food must advance with vegetation one year at a time.".into());
        }
        Ok(())
    }

    fn advance_herd(&self, i: usize, year: &RegionYear, state: &mut State) -> (f64, f64) {
        let herd = self.next_herd((state.game[i], state.game_capacity[i]), year);
        (state.game[i], state.game_capacity[i]) = herd;
        herd
    }

    /// One region's food from its vegetation history since year 0.
    pub fn region_series(
        &self,
        i: usize,
        series: &[RegionYear],
    ) -> Result<Vec<RegionFood>, String> {
        if i >= self.land.len() {
            return Err("Wild-food region is outside the generated world.".into());
        }
        let mut herd = self.first_herd(series);
        Ok(series
            .iter()
            .enumerate()
            .map(|(k, year)| {
                if k > 0 {
                    herd = self.next_herd(herd, year);
                }
                self.region(year, herd, i)
            })
            .collect())
    }

    /// A group of `people` forages in one region from year 1 for `years` years, then
    /// leaves. Plants are used first, up to the plant-only year-round capacity; game
    /// covers the rest up to the hunting offtake limit. Returns every year to the end
    /// of the vegetation history, so recovery after the group leaves is visible.
    pub fn forage(
        &self,
        i: usize,
        series: &[RegionYear],
        people: f64,
        years: u32,
    ) -> Result<Vec<ForageYear>, String> {
        if i >= self.land.len() || !self.land[i] {
            return Err("Foraging needs a land region of the generated world.".into());
        }
        if !(people > 0. && people <= 1e7) {
            return Err("Foraging accepts 1 to 10,000,000 people.".into());
        }
        let s = &self.settings;
        let area = self.areas[i];
        let need: f64 = self.need[i].iter().sum();
        let (mut game, mut capacity) = self.first_herd(series);
        let mut out = vec![ForageYear {
            game,
            shortfall: 0.,
        }];
        for (k, year) in series.iter().enumerate().skip(1) {
            (game, capacity) = self.next_herd((game, capacity), year);
            let mut shortfall = 0.;
            if k as u32 <= years {
                let season = self.season(i, year.precipitation_factor);
                let plant_kcal = self.plant_food(year);
                let (plant_people, _) = self.capacity(i, &season.map(|f| f * plant_kcal), 0.);
                let demand = people * need;
                let remaining = (demand - plant_people * area / KM2 * need).max(0.);
                let available = s.maximum_hunting_offtake
                    * self.access(year).1
                    * game
                    * area
                    * s.game_energy_kcal_per_gram;
                let hunted = remaining.min(available);
                game -= hunted / (area * s.game_energy_kcal_per_gram);
                shortfall = (remaining - hunted) / demand;
            }
            out.push(ForageYear { game, shortfall });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Recipe, vegetation::Settings as VegetationSettings};

    fn test_world(seed: &str) -> World {
        let recipe: Recipe = serde_json::from_value(serde_json::json!({"schemaVersion":1,
            "modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,
            "subdivision":4,"radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,
            "continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,
            "detailAmplitudeMeters":300.,"terrainPreparationPasses":4,"water":{"mode":"coverage","fraction":0.71}}))
            .unwrap();
        World::generate(recipe).unwrap()
    }

    fn setup(
        seed: &str,
    ) -> (
        World,
        Normals,
        VegetationModel,
        VegetationState,
        Model,
        State,
    ) {
        let world = test_world(seed);
        let normals = Normals::from_world(&world).unwrap();
        let (vegetation, v) =
            VegetationModel::new(&world, &normals, VegetationSettings::default()).unwrap();
        let (food, state) =
            Model::new(&world, &normals, &vegetation, &v, Settings::default()).unwrap();
        (world, normals, vegetation, v, food, state)
    }

    fn year(productivity: f64, woody: f64, herb: f64) -> RegionYear {
        RegionYear {
            precipitation_factor: 1.,
            root_moisture: 1.,
            established_cover: woody + herb,
            woody,
            herb,
            woody_capacity: woody,
            cover_capacity: woody + herb,
            productivity,
            label: 0,
        }
    }

    #[test]
    fn seasons_sum_to_one_and_follow_warmth() {
        let (_, normals, _, _, food, _) = setup("first-light");
        for i in (0..food.land.len()).filter(|&i| food.land[i]) {
            let season = food.season(i, 1.);
            assert!((season.iter().sum::<f64>() - 1.).abs() < 1e-12);
            let t = &normals.monthly_temperature_celsius;
            if normals.warmest_month_celsius[i] > 5. && normals.coldest_month_celsius[i] < -5. {
                let coldest = (0..MONTHS)
                    .min_by(|&a, &b| t[a][i].total_cmp(&t[b][i]))
                    .unwrap();
                let peak = (0..MONTHS)
                    .max_by(|&a, &b| season[a].total_cmp(&season[b]))
                    .unwrap();
                assert!(season[coldest] < 1e-12 && t[peak][i] > 0.);
            }
        }
    }

    #[test]
    fn capacity_lies_between_self_sufficient_and_stored_bounds() {
        let (_, _, _, _, food, _) = setup("first-light");
        let i = (0..food.land.len()).find(|&i| food.land[i]).unwrap();
        let mut plant = [0.; MONTHS];
        plant[5] = 1.;
        let (seasonal, stored) = food.capacity(i, &plant, 0.);
        let need: f64 = food.need[i].iter().sum();
        assert!((stored - KM2 / need).abs() < 1e-9 * stored);
        assert!(
            seasonal > 0. && seasonal < 0.3 * stored,
            "{seasonal} {stored}"
        );
        let (even, even_stored) = food.capacity(i, &[1. / MONTHS as f64; MONTHS], 0.);
        assert!(even > 0.9 * even_stored && even <= even_stored);
        let (with_game, _) = food.capacity(i, &plant, 0.5);
        assert!(with_game > seasonal);
        assert!(
            food.feasible(i, seasonal, &plant, 0.)
                && !food.feasible(i, 1.01 * seasonal, &plant, 0.)
        );
    }

    #[test]
    fn closed_canopy_and_drought_lower_food() {
        let (_, _, _, _, food, _) = setup("first-light");
        let open = food.plant_food(&year(1000., 0.4, 0.4));
        let closed = food.plant_food(&year(1000., 0.95, 0.05));
        assert!(closed < open);
        let grass = food.forage_capacity(&year(800., 0., 0.8));
        assert!(food.forage_capacity(&year(800., 0.8, 0.)) < 0.3 * grass);
        assert!(food.forage_capacity(&year(400., 0., 0.8)) < grass);
        assert_eq!(food.game_food(0., grass, 1.), 0.);
        assert!(food.game_food(grass, grass, 1.) == food.game_food(grass / 2., grass, 1.));
    }

    #[test]
    fn game_dies_back_in_drought_and_recovers_with_lag() {
        let (_, _, _, _, food, _) = setup("first-light");
        let normal = year(800., 0., 0.8);
        let dry = year(250., 0., 0.4);
        let mut series = vec![normal; 31];
        series[1..4].fill(dry);
        let i = (0..food.land.len()).find(|&i| food.land[i]).unwrap();
        let foods = food.region_series(i, &series).unwrap();
        let k = foods[0].game_capacity;
        assert!((foods[0].game - k).abs() < 1e-12);
        assert!(foods[1].game > 0.8 * k && foods[3].game < 0.65 * k);
        let recovered = foods
            .iter()
            .skip(4)
            .position(|f| f.game > 0.95 * k)
            .unwrap();
        assert!(recovered >= 5, "{recovered}");
        assert!(foods.iter().all(|f| f.game >= 0. && f.game <= k + 1e-12));
    }

    #[test]
    fn foraging_depletes_game_and_it_recovers_after_the_group_leaves() {
        let (_, _, _, _, food, _) = setup("first-light");
        let i = (0..food.land.len())
            .filter(|&i| food.land[i])
            .max_by(|&a, &b| food.areas[a].total_cmp(&food.areas[b]))
            .unwrap();
        let series = vec![year(800., 0.1, 0.7); 61];
        let base = food.region_series(i, &series).unwrap();
        let sustainable = base[0].capacity * food.areas[i] / KM2;
        let light = food.forage(i, &series, 0.8 * sustainable, 30).unwrap();
        assert!(light.iter().all(|y| y.shortfall == 0.));
        assert!(light[30].game > 0.4 * base[30].game);
        let heavy = food.forage(i, &series, 4. * sustainable, 30).unwrap();
        assert!(heavy[30].game < 0.1 * base[30].game && heavy[30].shortfall > 0.3);
        assert!(heavy[60].game > 0.9 * base[60].game);
        assert!(food.forage(i, &series, 0., 30).is_err());
    }

    #[test]
    fn run_steps_with_vegetation_and_series_replays_it() {
        let (_, _, vegetation, mut v, food, mut state) = setup("5c151f46");
        for _ in 0..20 {
            vegetation.advance(&mut v, 1).unwrap();
            food.step(&vegetation, &v, &mut state).unwrap();
        }
        assert_eq!(state.summaries.len(), 21);
        assert!(food.step(&vegetation, &v, &mut state).is_err());
        let regions = food.regions(&vegetation, &v, &state);
        for (i, r) in regions.iter().enumerate() {
            assert!(r.capacity >= 0. && r.capacity <= r.stored_capacity + 1e-12);
            assert!(r.game >= 0. && r.plant_kcal >= 0. && r.game_kcal >= 0.);
            assert!(food.land[i] || r.capacity == 0.);
        }
        let i = (0..food.land.len())
            .filter(|&i| food.land[i])
            .nth(40)
            .unwrap();
        let replay = food
            .region_series(i, &vegetation.region_series(&v, i).unwrap())
            .unwrap();
        assert_eq!(replay.len(), 21);
        assert_eq!(*replay.last().unwrap(), regions[i]);
        let total: f64 = state.summaries.last().unwrap().capacity_people;
        assert!(total > 0. && total.is_finite());
    }

    #[test]
    fn regional_step_matches_the_regular_food_step() {
        let (_, _, vegetation, mut v, food, mut standard) = setup("5c151f46");
        let mut observed = standard.clone();
        let mut regions = Vec::new();
        assert!(
            food.step_with_regions(&vegetation, &v, &mut observed, &mut regions)
                .is_err()
        );
        for _ in 0..20 {
            vegetation.advance(&mut v, 1).unwrap();
            food.step(&vegetation, &v, &mut standard).unwrap();
            food.step_with_regions(&vegetation, &v, &mut observed, &mut regions)
                .unwrap();
        }
        assert_eq!(
            serde_json::to_vec(&standard).unwrap(),
            serde_json::to_vec(&observed).unwrap()
        );
        assert_eq!(regions, food.regions(&vegetation, &v, &standard));
    }
}
