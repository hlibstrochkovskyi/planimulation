//! Vegetation with memory on a generated, static world. Each land region holds woody
//! and herbaceous cover fractions that relax yearly toward a capacity set by the
//! climate normals and that year's precipitation. Interannual precipitation anomalies
//! are spatially coherent and persistent: smooth random fields from fixed anomaly
//! centers with first-order autoregressive amplitudes. Dieback is fast and regrowth
//! slow, so a drought leaves a mark that lasts years to decades. Labels describe the
//! accumulated state with hysteresis, reusing the climate-normals-1 thermal rules.
//!
//! Not modeled: temperature anomalies, fire, grazing, seed dispersal between regions,
//! soil depth or fertility, and feedback from vegetation to water or climate.
//! Precipitation comes from climate-normals-1, not the seasonal-water budget.
use crate::{
    Random, Recipe, World,
    climate_normals::{self, Biome, Normals, aridity_threshold},
};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "vegetation-1";
pub const MAXIMUM_YEARS: u32 = 10_000;
pub const MAXIMUM_YEARS_PER_REQUEST: u32 = 1_000;
/// Aridity ratio (annual precipitation / Köppen threshold) where woody capacity rises
/// from zero to full; it is one half at the Köppen forest boundary of 2.
pub const WOODY_ARIDITY: (f64, f64) = (1.5, 2.5);
/// Aridity ratio where total cover capacity rises from zero to full; one half at the
/// Köppen desert boundary of 1.
pub const COVER_ARIDITY: (f64, f64) = (0.6, 1.4);
/// Labels switch to forest or open vegetation at the first value and keep that label
/// until cover falls below the second (hysteresis).
pub const FOREST_COVER: (f64, f64) = (0.5, 0.4);
pub const OPEN_COVER: (f64, f64) = (0.5, 0.4);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// Fixed anomaly centers on a Fibonacci sphere.
    pub anomaly_centers: u32,
    /// Gaussian length scale of the anomaly field.
    pub anomaly_length_meters: f64,
    /// Year-to-year correlation of anomaly amplitudes.
    pub anomaly_persistence: f64,
    /// Standard deviation of the natural logarithm of the yearly precipitation factor.
    pub precipitation_variability: f64,
    pub woody_growth_per_year: f64,
    pub herb_growth_per_year: f64,
    /// Fraction of the excess above capacity lost per year.
    pub woody_dieback_per_year: f64,
    pub herb_dieback_per_year: f64,
    /// Cover added to the logistic seed term, standing in for colonization from
    /// outside the region.
    pub establishment: f64,
    /// Memory of the precipitation factor seen by deep-rooted woody plants (soil and
    /// groundwater storage), years. Herbs respond to the current year.
    pub root_memory_years: f64,
    /// Memory of total cover used for open-vegetation labels (perennial structure and
    /// seed banks), years.
    pub label_memory_years: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            anomaly_centers: 240,
            anomaly_length_meters: 1_500_000.,
            anomaly_persistence: 0.4,
            precipitation_variability: 0.22,
            woody_growth_per_year: 0.15,
            herb_growth_per_year: 1.5,
            woody_dieback_per_year: 0.3,
            herb_dieback_per_year: 0.8,
            establishment: 0.02,
            root_memory_years: 3.,
            label_memory_years: 8.,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        let rate = |v: f64| v.is_finite() && v > 0. && v <= 2.;
        let fraction = |v: f64| v.is_finite() && (0. ..=1.).contains(&v);
        if !(12..=5000).contains(&self.anomaly_centers)
            || !(self.anomaly_length_meters.is_finite()
                && (100_000. ..=10_000_000.).contains(&self.anomaly_length_meters))
            || !(fraction(self.anomaly_persistence) && self.anomaly_persistence < 1.)
            || !fraction(self.precipitation_variability)
            || !rate(self.woody_growth_per_year)
            || !rate(self.herb_growth_per_year)
            || !fraction(self.woody_dieback_per_year)
            || !fraction(self.herb_dieback_per_year)
            || !(fraction(self.establishment) && self.establishment <= 0.2)
            || !(self.root_memory_years.is_finite()
                && (1. ..=50.).contains(&self.root_memory_years))
            || !(self.label_memory_years.is_finite()
                && (1. ..=50.).contains(&self.label_memory_years))
        {
            return Err("Vegetation settings are outside the supported range.".into());
        }
        Ok(())
    }
}

/// Land-area-weighted global record of one simulated year.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YearSummary {
    pub precipitation_factor: f64,
    pub woody_cover: f64,
    pub total_cover: f64,
    pub forest_share: f64,
    pub label_changes: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    pub year: u32,
    pub random: u32,
    pub amplitudes: Vec<f64>,
    /// Amplitudes used in years 1..=year, year-major; enough to replay any region.
    pub amplitude_history: Vec<f64>,
    pub woody: Vec<f64>,
    pub herb: Vec<f64>,
    pub label: Vec<u8>,
    /// Precipitation factor of the most recent year (1 at year 0).
    pub precipitation_factor: Vec<f64>,
    /// Exponential memory of the precipitation factor seen by woody roots.
    pub root_moisture: Vec<f64>,
    /// Exponential memory of total cover that decides open-vegetation labels.
    pub established_cover: Vec<f64>,
    pub summaries: Vec<YearSummary>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub model_version: String,
    pub climate_model_version: String,
    pub recipe: Recipe,
    pub settings: Settings,
    pub state: State,
}

/// One year of a single region, replayed from the recorded anomaly amplitudes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionYear {
    pub precipitation_factor: f64,
    pub root_moisture: f64,
    pub established_cover: f64,
    pub woody: f64,
    pub herb: f64,
    pub woody_capacity: f64,
    pub cover_capacity: f64,
    pub label: u8,
}

pub struct Model {
    recipe: Recipe,
    settings: Settings,
    land: Vec<bool>,
    areas: Vec<f64>,
    precipitation_mm: Vec<f64>,
    threshold_mm: Vec<f64>,
    mean: Vec<f64>,
    warmest: Vec<f64>,
    coldest: Vec<f64>,
    driest_mm: Vec<f64>,
    offsets: Vec<u32>,
    centers: Vec<u32>,
    weights: Vec<f64>,
}

fn smoothstep((low, high): (f64, f64), x: f64) -> f64 {
    let t = ((x - low) / (high - low)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn is_forest(label: u8) -> bool {
    [
        Biome::BorealForest,
        Biome::TemperateForest,
        Biome::TropicalSeasonalForest,
        Biome::TropicalRainforest,
    ]
    .iter()
    .any(|&b| b as u8 == label)
}

fn is_open(label: u8) -> bool {
    label == Biome::TemperateGrassland as u8 || label == Biome::Savanna as u8
}

/// Unit-variance normal deviate from two uniform draws (Box–Muller, cosine branch).
fn normal(random: &mut Random) -> f64 {
    let u = (f64::from(random.next_u32()) + 1.) / 4_294_967_296.;
    let v = f64::from(random.next_u32()) / 4_294_967_296.;
    (-2. * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
}

/// Logistic growth toward `capacity` from below, proportional decay toward it from above.
fn relax(value: f64, capacity: f64, growth: f64, dieback: f64, establishment: f64) -> f64 {
    if value < capacity {
        (value + growth * (value + establishment) * (capacity - value) / capacity).min(capacity)
    } else {
        value - dieback * (value - capacity)
    }
}

impl Model {
    pub fn from_world(world: &World, settings: Settings) -> Result<(Self, State), String> {
        let normals = Normals::from_world(world)?;
        Self::new(world, &normals, settings)
    }

    pub fn new(
        world: &World,
        normals: &Normals,
        settings: Settings,
    ) -> Result<(Self, State), String> {
        settings.validate()?;
        let s = &world.surface;
        let n = s.centers.len();
        if normals.biome.len() != n || world.water.body_ids.len() != n {
            return Err("Vegetation input arrays do not match the surface.".into());
        }
        let count = settings.anomaly_centers as usize;
        let golden = std::f64::consts::PI * (3. - 5f64.sqrt());
        let points: Vec<[f64; 3]> = (0..count)
            .map(|k| {
                let y = 1. - 2. * (k as f64 + 0.5) / count as f64;
                let r = (1. - y * y).sqrt();
                let phi = golden * k as f64;
                [r * phi.cos(), y, r * phi.sin()]
            })
            .collect();
        let length = settings.anomaly_length_meters / world.recipe.radius_meters;
        let (mut offsets, mut centers, mut weights) = (vec![0], Vec::new(), Vec::new());
        for c in &s.centers {
            let start = weights.len();
            for (k, p) in points.iter().enumerate() {
                let angle = (c[0] * p[0] + c[1] * p[1] + c[2] * p[2])
                    .clamp(-1., 1.)
                    .acos();
                if angle < 3. * length {
                    centers.push(k as u32);
                    weights.push((-0.5 * (angle / length).powi(2)).exp());
                }
            }
            let scale = weights[start..].iter().map(|w| w * w).sum::<f64>().sqrt();
            if scale <= 0. {
                return Err("Vegetation anomaly centers do not cover the surface.".into());
            }
            for w in &mut weights[start..] {
                *w /= scale;
            }
            offsets.push(weights.len() as u32);
        }
        let model = Self {
            recipe: world.recipe.clone(),
            settings,
            land: world.water.body_ids.iter().map(|&b| b == 0).collect(),
            areas: s.areas.clone(),
            precipitation_mm: normals.annual_precipitation_mm.clone(),
            threshold_mm: normals
                .annual_mean_celsius
                .iter()
                .map(|&t| aridity_threshold(t))
                .collect(),
            mean: normals.annual_mean_celsius.clone(),
            warmest: normals.warmest_month_celsius.clone(),
            coldest: normals.coldest_month_celsius.clone(),
            driest_mm: normals.driest_month_mm.clone(),
            offsets,
            centers,
            weights,
        };
        let mut random = Random::stream(&world.recipe.seed, MODEL_VERSION);
        let amplitudes = (0..count).map(|_| normal(&mut random)).collect();
        let initial: Vec<RegionYear> = (0..n).map(|i| model.initial_region(i)).collect();
        let state = State {
            year: 0,
            random: random.0,
            amplitudes,
            amplitude_history: Vec::new(),
            woody: initial.iter().map(|r| r.woody).collect(),
            herb: initial.iter().map(|r| r.herb).collect(),
            label: initial.iter().map(|r| r.label).collect(),
            precipitation_factor: vec![1.; n],
            root_moisture: vec![1.; n],
            established_cover: initial.iter().map(|r| r.established_cover).collect(),
            summaries: Vec::new(),
        };
        Ok((model, state))
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    pub fn checkpoint(&self, state: &State) -> Checkpoint {
        Checkpoint {
            model_version: MODEL_VERSION.into(),
            climate_model_version: climate_normals::MODEL_VERSION.into(),
            recipe: self.recipe.clone(),
            settings: self.settings,
            state: state.clone(),
        }
    }

    /// Rebuild the model for `world` and resume a checkpoint taken from it.
    pub fn restore(world: &World, checkpoint: Checkpoint) -> Result<(Self, State), String> {
        if checkpoint.model_version != MODEL_VERSION
            || checkpoint.climate_model_version != climate_normals::MODEL_VERSION
            || checkpoint.recipe != world.recipe
        {
            return Err("Vegetation checkpoint does not match this world or model version.".into());
        }
        let (model, initial) = Self::from_world(world, checkpoint.settings)?;
        let state = checkpoint.state;
        let (n, k, years) = (
            initial.woody.len(),
            initial.amplitudes.len(),
            state.year as usize,
        );
        let bounded = |v: &[f64]| v.iter().all(|x| x.is_finite() && (0. ..=1.).contains(x));
        if state.year > MAXIMUM_YEARS
            || state.amplitudes.len() != k
            || state.amplitude_history.len() != k * years
            || state.summaries.len() != years
            || [
                &state.woody,
                &state.herb,
                &state.precipitation_factor,
                &state.root_moisture,
                &state.established_cover,
            ]
            .iter()
            .any(|v| v.len() != n)
            || state.label.len() != n
            || !bounded(&state.woody)
            || !bounded(&state.herb)
            || !bounded(&state.established_cover)
            || state
                .precipitation_factor
                .iter()
                .chain(&state.root_moisture)
                .any(|f| !(f.is_finite() && *f > 0.))
            || state.amplitudes.iter().any(|a| !a.is_finite())
            || state
                .label
                .iter()
                .any(|&l| l > Biome::TropicalRainforest as u8)
        {
            return Err("Vegetation checkpoint state is invalid.".into());
        }
        Ok((model, state))
    }

    /// Aridity ratio (precipitation / Köppen threshold) for a precipitation factor.
    pub fn aridity_ratio(&self, i: usize, factor: f64) -> f64 {
        if self.threshold_mm[i] <= 0. {
            f64::INFINITY
        } else {
            self.precipitation_mm[i] * factor / self.threshold_mm[i]
        }
    }

    /// Woody capacity from the root-zone factor and total cover capacity from the
    /// current year's factor.
    pub fn capacity(&self, i: usize, factor: f64, root_moisture: f64) -> (f64, f64) {
        if !self.land[i] || self.warmest[i] < 0. {
            return (0., 0.);
        }
        let cover = smoothstep(COVER_ARIDITY, self.aridity_ratio(i, factor));
        if self.warmest[i] < 10. {
            (0., cover * self.warmest[i] / 10.)
        } else {
            (
                smoothstep(WOODY_ARIDITY, self.aridity_ratio(i, root_moisture)),
                cover,
            )
        }
    }

    fn initial_region(&self, i: usize) -> RegionYear {
        let (woody, cover) = self.capacity(i, 1., 1.);
        let herb = (cover - woody).max(0.);
        RegionYear {
            precipitation_factor: 1.,
            root_moisture: 1.,
            established_cover: woody + herb,
            woody,
            herb,
            woody_capacity: woody,
            cover_capacity: cover,
            label: self.label(i, woody, woody + herb, None),
        }
    }

    /// Structural class from woody cover and established total cover (forest, open or
    /// barren) with the thermal subtypes of climate-normals-1. `previous` enables
    /// hysteresis.
    pub fn label(&self, i: usize, woody: f64, established: f64, previous: Option<u8>) -> u8 {
        if !self.land[i] {
            return Biome::Water as u8;
        }
        let (mean, warmest, coldest) = (self.mean[i], self.warmest[i], self.coldest[i]);
        if warmest < 0. {
            return Biome::Ice as u8;
        }
        if warmest < 10. {
            return Biome::Tundra as u8;
        }
        let was_forest = previous.is_some_and(is_forest);
        let was_vegetated = was_forest || previous.is_some_and(is_open);
        let forest = woody
            >= if was_forest {
                FOREST_COVER.1
            } else {
                FOREST_COVER.0
            };
        let open = established
            >= if was_vegetated {
                OPEN_COVER.1
            } else {
                OPEN_COVER.0
            };
        (if forest {
            if coldest >= 18. {
                if self.driest_mm[i] >= 60. {
                    Biome::TropicalRainforest
                } else {
                    Biome::TropicalSeasonalForest
                }
            } else if coldest > -3. || warmest >= 22. {
                Biome::TemperateForest
            } else {
                Biome::BorealForest
            }
        } else if open {
            if mean >= 18. {
                Biome::Savanna
            } else {
                Biome::TemperateGrassland
            }
        } else if mean >= 18. {
            Biome::HotDesert
        } else {
            Biome::ColdDesert
        }) as u8
    }

    fn factor(&self, i: usize, amplitudes: &[f64]) -> f64 {
        let (a, b) = (self.offsets[i] as usize, self.offsets[i + 1] as usize);
        let z: f64 = (a..b)
            .map(|k| self.weights[k] * amplitudes[self.centers[k] as usize])
            .sum();
        let sigma = self.settings.precipitation_variability;
        (sigma * z - 0.5 * sigma * sigma).exp()
    }

    fn step_region(&self, i: usize, previous: &RegionYear, factor: f64) -> RegionYear {
        let s = &self.settings;
        let root_moisture =
            previous.root_moisture + (factor - previous.root_moisture) / s.root_memory_years;
        let (woody_capacity, cover_capacity) = self.capacity(i, factor, root_moisture);
        let woody = relax(
            previous.woody,
            woody_capacity,
            s.woody_growth_per_year,
            s.woody_dieback_per_year,
            s.establishment,
        );
        // Canopy displaces herbs immediately; herbs fill the remaining cover capacity.
        let herb = relax(
            previous.herb,
            (cover_capacity - woody).max(0.),
            s.herb_growth_per_year,
            s.herb_dieback_per_year,
            s.establishment,
        )
        .min(1. - woody);
        let established_cover = previous.established_cover
            + (woody + herb - previous.established_cover) / s.label_memory_years;
        RegionYear {
            precipitation_factor: factor,
            root_moisture,
            established_cover,
            woody,
            herb,
            woody_capacity,
            cover_capacity,
            label: self.label(i, woody, established_cover, Some(previous.label)),
        }
    }

    fn region(&self, state: &State, i: usize) -> RegionYear {
        let (woody_capacity, cover_capacity) =
            self.capacity(i, state.precipitation_factor[i], state.root_moisture[i]);
        RegionYear {
            precipitation_factor: state.precipitation_factor[i],
            root_moisture: state.root_moisture[i],
            established_cover: state.established_cover[i],
            woody: state.woody[i],
            herb: state.herb[i],
            woody_capacity,
            cover_capacity,
            label: state.label[i],
        }
    }

    fn next_amplitudes(&self, state: &mut State) {
        let rho = self.settings.anomaly_persistence;
        let innovation = (1. - rho * rho).sqrt();
        let mut random = Random(state.random);
        for a in &mut state.amplitudes {
            *a = rho * *a + innovation * normal(&mut random);
        }
        state.random = random.0;
        state.amplitude_history.extend_from_slice(&state.amplitudes);
    }

    pub fn advance(&self, state: &mut State, years: u32) -> Result<(), String> {
        if years == 0 || years > MAXIMUM_YEARS_PER_REQUEST {
            return Err(format!(
                "Vegetation accepts 1–{MAXIMUM_YEARS_PER_REQUEST} years per request."
            ));
        }
        if state.year + years > MAXIMUM_YEARS {
            return Err(format!(
                "Vegetation runs are limited to {MAXIMUM_YEARS} years."
            ));
        }
        for _ in 0..years {
            self.next_amplitudes(state);
            let mut sums = [0.; 5];
            let mut changes = 0;
            for i in (0..self.land.len()).filter(|&i| self.land[i]) {
                let factor = self.factor(i, &state.amplitudes);
                let next = self.step_region(i, &self.region(state, i), factor);
                changes += u32::from(next.label != state.label[i]);
                let area = self.areas[i];
                sums[0] += area;
                sums[1] += area * factor;
                sums[2] += area * next.woody;
                sums[3] += area * (next.woody + next.herb);
                sums[4] += if is_forest(next.label) { area } else { 0. };
                state.woody[i] = next.woody;
                state.herb[i] = next.herb;
                state.label[i] = next.label;
                state.precipitation_factor[i] = factor;
                state.root_moisture[i] = next.root_moisture;
                state.established_cover[i] = next.established_cover;
            }
            let land = sums[0].max(f64::MIN_POSITIVE);
            state.summaries.push(YearSummary {
                precipitation_factor: sums[1] / land,
                woody_cover: sums[2] / land,
                total_cover: sums[3] / land,
                forest_share: sums[4] / land,
                label_changes: changes,
            });
            state.year += 1;
        }
        Ok(())
    }

    /// Capacities of the most recent year for every region (initial normals at year 0).
    pub fn capacities(&self, state: &State) -> (Vec<f64>, Vec<f64>) {
        (0..self.land.len())
            .map(|i| self.capacity(i, state.precipitation_factor[i], state.root_moisture[i]))
            .unzip()
    }

    /// Replay one region from year 0; vegetation is local, so this reproduces the
    /// global run exactly.
    pub fn region_series(&self, state: &State, i: usize) -> Result<Vec<RegionYear>, String> {
        if i >= self.land.len() {
            return Err("Vegetation region is outside the generated world.".into());
        }
        let k = self.settings.anomaly_centers as usize;
        let mut current = self.initial_region(i);
        let mut series = vec![current];
        if !self.land[i] {
            series.resize(state.year as usize + 1, current);
            return Ok(series);
        }
        for amplitudes in state.amplitude_history.chunks_exact(k) {
            let factor = self.factor(i, amplitudes);
            current = self.step_region(i, &current, factor);
            series.push(current);
        }
        Ok(series)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_world(seed: &str) -> World {
        let recipe: Recipe = serde_json::from_value(serde_json::json!({"schemaVersion":1,
            "modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,
            "subdivision":4,"radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,
            "continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,
            "detailAmplitudeMeters":300.,"terrainPreparationPasses":4,"water":{"mode":"coverage","fraction":0.71}}))
            .unwrap();
        World::generate(recipe).unwrap()
    }

    #[test]
    fn relaxation_is_bounded_and_asymmetric() {
        let s = Settings::default();
        let grow = relax(
            0.5,
            1.,
            s.woody_growth_per_year,
            s.woody_dieback_per_year,
            s.establishment,
        );
        let die = relax(
            0.5,
            0.,
            s.woody_growth_per_year,
            s.woody_dieback_per_year,
            s.establishment,
        );
        assert!(grow > 0.5 && grow < 0.6 && die < 0.4 && die > 0.);
        assert_eq!(relax(0.9, 0.95, 2., 1., 0.2), 0.95);
        assert!((relax(0.9, 0.2, 2., 1., 0.2) - 0.2).abs() < 1e-12);
    }

    #[test]
    fn without_variability_the_state_is_the_climate_classification_forever() {
        let world = test_world("vegetation");
        let normals = Normals::from_world(&world).unwrap();
        let settings = Settings {
            precipitation_variability: 0.,
            ..Default::default()
        };
        let (model, mut state) = Model::new(&world, &normals, settings).unwrap();
        let initial = state.clone();
        for i in 0..normals.biome.len() {
            if world.water.body_ids[i] == 0 {
                assert_eq!(state.label[i], normals.biome[i], "region {i}");
            }
        }
        model.advance(&mut state, 50).unwrap();
        assert_eq!(state.woody, initial.woody);
        assert_eq!(state.herb, initial.herb);
        assert_eq!(state.label, initial.label);
        assert!(state.summaries.iter().all(|s| s.label_changes == 0));
    }

    #[test]
    fn variable_years_stay_bounded_coherent_and_stable() {
        let world = test_world("vegetation");
        let (model, mut state) = Model::from_world(&world, Settings::default()).unwrap();
        model.advance(&mut state, 200).unwrap();
        let land: Vec<usize> = (0..state.woody.len())
            .filter(|&i| world.water.body_ids[i] == 0)
            .collect();
        for &i in &land {
            assert!((0. ..=1.).contains(&state.woody[i]) && state.herb[i] >= 0.);
            assert!(state.woody[i] + state.herb[i] <= 1. + 1e-12);
        }
        let factors: Vec<f64> = state
            .summaries
            .iter()
            .map(|s| s.precipitation_factor)
            .collect();
        let mean = factors.iter().sum::<f64>() / factors.len() as f64;
        assert!((mean - 1.).abs() < 0.05, "global mean factor {mean}");
        // Neighbors share most of their anomaly; distant regions do not.
        let s = &world.surface;
        let ln = |i: usize| state.precipitation_factor[i].ln();
        let (mut near, mut far) = (0., 0.);
        for (index, &i) in land.iter().enumerate() {
            near += (ln(i) - ln(s.neighbors[s.offsets[i] as usize] as usize)).abs();
            far += (ln(i) - ln(land[(index + land.len() / 2) % land.len()])).abs();
        }
        assert!(near < 0.4 * far, "neighbor {near} vs distant {far}");
        // Labels do not flicker: under 1.5% of land changes class in an average year.
        let changes: u32 = state
            .summaries
            .iter()
            .skip(50)
            .map(|s| s.label_changes)
            .sum();
        let rate = f64::from(changes) / 150. / land.len() as f64;
        assert!(rate < 0.015, "yearly label change rate {rate}");
    }

    #[test]
    fn checkpoint_continuation_and_region_replay_match_an_uninterrupted_run() {
        let world = test_world("vegetation");
        let (model, mut whole) = Model::from_world(&world, Settings::default()).unwrap();
        model.advance(&mut whole, 30).unwrap();
        let (first, mut part) = Model::from_world(&world, Settings::default()).unwrap();
        first.advance(&mut part, 10).unwrap();
        let json = serde_json::to_string(&first.checkpoint(&part)).unwrap();
        let (resumed, mut part) =
            Model::restore(&world, serde_json::from_str(&json).unwrap()).unwrap();
        resumed.advance(&mut part, 20).unwrap();
        assert_eq!(part, whole);
        for i in [0, 100, 1000, 2000] {
            let series = model.region_series(&whole, i).unwrap();
            assert_eq!(series.len(), 31);
            let last = series[30];
            assert_eq!(
                (last.woody, last.herb, last.label),
                (whole.woody[i], whole.herb[i], whole.label[i])
            );
        }
        assert!(Model::restore(&test_world("another"), model.checkpoint(&whole)).is_err());
    }

    #[test]
    fn a_drought_cuts_forest_quickly_and_recovery_takes_years() {
        let world = test_world("vegetation");
        let (model, state) = Model::from_world(&world, Settings::default()).unwrap();
        // A forest region near its aridity margin, where a dry year lowers capacity.
        let i = (0..state.woody.len())
            .find(|&i| model.capacity(i, 1., 1.).0 > 0.95 && model.capacity(i, 1., 0.75).0 < 0.3)
            .expect("a marginal forest region");
        let mut current = model.region(&state, i);
        let before = current.woody;
        for _ in 0..3 {
            current = model.step_region(i, &current, 0.6);
        }
        let after = current.woody;
        assert!(after < before - 0.2, "drought effect {before} → {after}");
        let mut years = 0;
        while current.woody < before - 0.05 {
            current = model.step_region(i, &current, 1.);
            years += 1;
            assert!(years < 200);
        }
        assert!(years >= 5, "recovery took {years} years");
    }
}
