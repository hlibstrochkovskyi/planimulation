//! Vegetation with memory on a generated, static world. Each land region holds woody
//! and herbaceous cover fractions that relax yearly toward a capacity set by the
//! climate normals and that year's precipitation. Interannual precipitation anomalies
//! are spatially coherent and persistent: smooth random fields from fixed anomaly
//! centers with first-order autoregressive amplitudes, larger in dry climates. Dieback
//! is fast and regrowth slow, and woody plants re-establish mainly from wooded
//! neighbors, so bare land fills from its edges. A spin-up on its own random stream
//! starts year 0 in a typical state. Labels describe the accumulated state with
//! hysteresis, reusing the climate-normals-1 thermal rules. Net primary productivity
//! follows the Miami model, scaled by the realized share of normal cover.
//!
//! Not modeled: temperature anomalies, fire, grazing, soil depth or fertility, and
//! feedback from vegetation to water or climate. Precipitation comes from
//! climate-normals-1, not the seasonal-water budget.
use crate::{
    Random, Recipe, World,
    climate_normals::{self, Biome, Normals, aridity_threshold},
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::thread;

pub const MODEL_VERSION: &str = "vegetation-2";
/// Region histories are replayed over the whole surface, so runs stay short.
pub const MAXIMUM_YEARS: u32 = 2_000;
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
/// Window of the woody-change field, years.
pub const CHANGE_YEARS: usize = 10;
/// Normal annual precipitation (mm) at which variability takes its humid and arid
/// values; it is interpolated on a logarithmic scale between them.
pub const VARIABILITY_PRECIPITATION_MM: (f64, f64) = (2_000., 200.);
/// Normal cover below which productivity is not scaled by its realized share.
pub const MINIMUM_REFERENCE_COVER: f64 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    /// Fixed anomaly centers on a Fibonacci sphere.
    pub anomaly_centers: u32,
    /// Gaussian length scale of the anomaly field.
    pub anomaly_length_meters: f64,
    /// Year-to-year correlation of anomaly amplitudes.
    pub anomaly_persistence: f64,
    /// Standard deviation of the natural logarithm of the yearly precipitation factor
    /// in humid climates.
    pub humid_variability: f64,
    /// The same in arid climates.
    pub arid_variability: f64,
    pub woody_growth_per_year: f64,
    pub herb_growth_per_year: f64,
    /// Fraction of the excess above capacity lost per year.
    pub woody_dieback_per_year: f64,
    pub herb_dieback_per_year: f64,
    /// Woody cover added to the logistic seed term without wooded neighbors (seed
    /// bank and long-distance dispersal).
    pub background_establishment: f64,
    /// Woody seed term per unit of mean neighboring woody cover.
    pub seed_establishment: f64,
    /// Herb seed term; herbs disperse widely and are not neighbor-limited.
    pub herb_establishment: f64,
    /// Memory of the precipitation factor seen by deep-rooted woody plants (soil and
    /// groundwater storage), years. Herbs respond to the current year.
    pub root_memory_years: f64,
    /// Memory of total cover used for open-vegetation labels (perennial structure and
    /// seed banks), years.
    pub label_memory_years: f64,
    /// Years run on a separate random stream before year 0.
    pub spin_up_years: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            anomaly_centers: 480,
            anomaly_length_meters: 1_000_000.,
            anomaly_persistence: 0.4,
            humid_variability: 0.12,
            arid_variability: 0.4,
            woody_growth_per_year: 0.15,
            herb_growth_per_year: 1.5,
            woody_dieback_per_year: 0.3,
            herb_dieback_per_year: 0.8,
            background_establishment: 0.002,
            seed_establishment: 0.05,
            herb_establishment: 0.02,
            root_memory_years: 3.,
            label_memory_years: 8.,
            spin_up_years: 200,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        let rate = |v: f64| v.is_finite() && v > 0. && v <= 2.;
        let fraction = |v: f64| v.is_finite() && (0. ..=1.).contains(&v);
        let seed = |v: f64| fraction(v) && v <= 0.2;
        let memory = |v: f64| v.is_finite() && (1. ..=50.).contains(&v);
        if !(12..=5000).contains(&self.anomaly_centers)
            || !(self.anomaly_length_meters.is_finite()
                && (100_000. ..=10_000_000.).contains(&self.anomaly_length_meters))
            || !(fraction(self.anomaly_persistence) && self.anomaly_persistence < 1.)
            || !fraction(self.humid_variability)
            || !fraction(self.arid_variability)
            || !rate(self.woody_growth_per_year)
            || !rate(self.herb_growth_per_year)
            || !fraction(self.woody_dieback_per_year)
            || !fraction(self.herb_dieback_per_year)
            || !seed(self.background_establishment)
            || !seed(self.seed_establishment)
            || !seed(self.herb_establishment)
            || !memory(self.root_memory_years)
            || !memory(self.label_memory_years)
            || !(CHANGE_YEARS as u32..=1_000).contains(&self.spin_up_years)
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
    /// Land net primary productivity, petagrams of dry matter per year.
    pub productivity_petagrams: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    pub year: u32,
    pub random: u32,
    pub amplitudes: Vec<f64>,
    /// Amplitudes used in years 1..=year, year-major; enough to replay the run.
    pub amplitude_history: Vec<f64>,
    pub woody: Vec<f64>,
    pub herb: Vec<f64>,
    pub label: Vec<u8>,
    /// Precipitation factor of the most recent year (the last spin-up year at year 0).
    pub precipitation_factor: Vec<f64>,
    /// Exponential memory of the precipitation factor seen by woody roots.
    pub root_moisture: Vec<f64>,
    /// Exponential memory of total cover that decides open-vegetation labels.
    pub established_cover: Vec<f64>,
    /// Woody cover of the last `CHANGE_YEARS` years; slot `year mod CHANGE_YEARS`.
    pub woody_history: Vec<f64>,
    /// Woody cover now minus `CHANGE_YEARS` years ago.
    pub woody_change: Vec<f64>,
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

/// One year of a single region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionYear {
    pub precipitation_factor: f64,
    pub root_moisture: f64,
    pub established_cover: f64,
    pub woody: f64,
    pub herb: f64,
    pub woody_capacity: f64,
    pub cover_capacity: f64,
    /// Net primary productivity, grams of dry matter per square meter per year.
    pub productivity: f64,
    pub label: u8,
}

#[derive(Clone, Copy)]
struct LandMeta {
    index: usize,
    area: f64,
    precipitation_mm: f64,
    threshold_mm: f64,
    mean: f64,
    warmest: f64,
    coldest: f64,
    driest_mm: f64,
    variability: f64,
    half_var_sq: f64,
    miami_temp: f64,
    reference_cover: f64,
    neighbor_deg: f64,
}

#[derive(Clone, Copy)]
struct LandDyn {
    herb: f64,
    root_moisture: f64,
    established_cover: f64,
    woody_change: f64,
    precipitation_factor: f64,
    woody_history: [f64; CHANGE_YEARS],
    label: u8,
}

/// Sense-reversing spin barrier with yield fallback to avoid kernel futex latency
/// on sub-millisecond yearly steps.
struct SpinBarrier {
    total: usize,
    arrived: AtomicUsize,
    phase: AtomicUsize,
}

impl SpinBarrier {
    fn new(total: usize) -> Self {
        Self {
            total,
            arrived: AtomicUsize::new(0),
            phase: AtomicUsize::new(0),
        }
    }

    #[inline]
    fn wait(&self) {
        let phase = self.phase.load(Ordering::Acquire);
        if self.arrived.fetch_add(1, Ordering::AcqRel) + 1 == self.total {
            self.arrived.store(0, Ordering::Relaxed);
            self.phase.store(phase.wrapping_add(1), Ordering::Release);
        } else {
            let mut spins = 0u32;
            while self.phase.load(Ordering::Acquire) == phase {
                spins += 1;
                if spins < 2048 {
                    std::hint::spin_loop();
                } else {
                    thread::yield_now();
                    spins = 0;
                }
            }
        }
    }
}

pub struct Model {
    recipe: Recipe,
    settings: Settings,
    threads: usize,
    land: Vec<bool>,
    land_slot: Vec<u32>,
    land_area: f64,
    land_meta: Vec<LandMeta>,
    land_weight_offsets: Vec<u32>,
    land_centers: Vec<u16>,
    land_weights: Vec<f64>,
    land_neighbor_offsets: Vec<u32>,
    land_neighbors: Vec<u32>,
    neighbor_offsets: Vec<u32>,
    neighbors: Vec<u32>,
    precipitation_mm: Vec<f64>,
    threshold_mm: Vec<f64>,
    mean: Vec<f64>,
    warmest: Vec<f64>,
    coldest: Vec<f64>,
    driest_mm: Vec<f64>,
    /// Log-standard deviation of the yearly precipitation factor per region.
    variability: Vec<f64>,
    /// Precomputed Miami temperature limit per region.
    miami_temp: Vec<f64>,
    /// Total cover capacity under normal precipitation.
    normal_cover: Vec<f64>,
    /// Precomputed normal_cover.max(MINIMUM_REFERENCE_COVER).
    reference_cover: Vec<f64>,
    /// State at year 0, after the spin-up; region histories replay from it.
    initial: State,
}

fn default_threads(regions: usize) -> usize {
    if regions < 4_096 {
        return 1;
    }
    thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .min(4)
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

/// Miami model (Lieth 1975): net primary productivity in g dry matter / m² / year,
/// the smaller of the temperature and precipitation limits.
pub fn miami(annual_mean_celsius: f64, precipitation_mm: f64) -> (f64, f64) {
    (
        3000. / (1. + (1.315 - 0.119 * annual_mean_celsius).exp()),
        3000. * (1. - (-0.000664 * precipitation_mm).exp()),
    )
}

/// Advance an AR(1) amplitude vector one year.
fn next_amplitudes(amplitudes: &mut [f64], persistence: f64, random: &mut Random) {
    let innovation = (1. - persistence * persistence).sqrt();
    for a in amplitudes {
        *a = persistence * *a + innovation * normal(random);
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
        Self::new_with_threads(
            world,
            normals,
            settings,
            default_threads(world.surface.centers.len()),
        )
    }

    pub fn new_with_threads(
        world: &World,
        normals: &Normals,
        settings: Settings,
        threads: usize,
    ) -> Result<(Self, State), String> {
        settings.validate()?;
        let s = &world.surface;
        let n = s.centers.len();
        if normals.biome.len() != n || world.water.body_ids.len() != n {
            return Err("Vegetation input arrays do not match the surface.".into());
        }
        let threads = threads.max(1);
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
        let land: Vec<bool> = world.water.body_ids.iter().map(|&b| b == 0).collect();
        let mut land_slot = vec![u32::MAX; n];
        let mut land_indices = Vec::new();
        for (i, &is_land) in land.iter().enumerate() {
            if is_land {
                land_slot[i] = land_indices.len() as u32;
                land_indices.push(i);
            }
        }
        let land_count = land_indices.len();
        let length = settings.anomaly_length_meters / world.recipe.radius_meters;
        let cutoff = 3. * length;
        let cos_cutoff = if cutoff < std::f64::consts::PI {
            cutoff.cos() - 1e-12
        } else {
            -2.
        };
        let build_chunk =
            |indices: std::ops::Range<usize>| -> Result<(Vec<u32>, Vec<u16>, Vec<f64>), String> {
                let mut lens = Vec::new();
                let mut local_centers = Vec::new();
                let mut local_weights = Vec::new();
                for i in indices {
                    let c = s.centers[i];
                    if !land[i] {
                        let covered = points.iter().any(|p| {
                            let dot = c[0] * p[0] + c[1] * p[1] + c[2] * p[2];
                            dot >= cos_cutoff && dot.clamp(-1., 1.).acos() < cutoff
                        });
                        if !covered {
                            return Err(
                                "Vegetation anomaly centers do not cover the surface.".into(),
                            );
                        }
                        continue;
                    }
                    let start = local_weights.len();
                    for (k, p) in points.iter().enumerate() {
                        let dot = c[0] * p[0] + c[1] * p[1] + c[2] * p[2];
                        if dot >= cos_cutoff {
                            let angle = dot.clamp(-1., 1.).acos();
                            if angle < cutoff {
                                local_centers.push(k as u16);
                                local_weights.push((-0.5 * (angle / length).powi(2)).exp());
                            }
                        }
                    }
                    let scale = local_weights[start..]
                        .iter()
                        .map(|w| w * w)
                        .sum::<f64>()
                        .sqrt();
                    if scale <= 0. {
                        return Err("Vegetation anomaly centers do not cover the surface.".into());
                    }
                    for w in &mut local_weights[start..] {
                        *w /= scale;
                    }
                    lens.push((local_weights.len() - start) as u32);
                }
                Ok((lens, local_centers, local_weights))
            };
        let chunk_size = n.div_ceil(threads).max(1);
        let parts: Vec<_> = if threads <= 1 || n <= chunk_size {
            vec![build_chunk(0..n)?]
        } else {
            let ranges: Vec<_> = (0..n)
                .step_by(chunk_size)
                .map(|start| start..(start + chunk_size).min(n))
                .collect();
            let mut results = Vec::with_capacity(ranges.len());
            let build_ref = &build_chunk;
            thread::scope(|scope| {
                let handles: Vec<_> = ranges[..ranges.len() - 1]
                    .iter()
                    .cloned()
                    .map(|range| scope.spawn(move || build_ref(range)))
                    .collect();
                let last = build_chunk(ranges[ranges.len() - 1].clone());
                for handle in handles {
                    results.push(handle.join().unwrap());
                }
                results.push(last);
            });
            results.into_iter().collect::<Result<Vec<_>, _>>()?
        };
        let total_weights: usize = parts.iter().map(|(_, _, w)| w.len()).sum();
        let mut land_weight_offsets = Vec::with_capacity(land_count + 1);
        land_weight_offsets.push(0);
        let mut land_centers = Vec::with_capacity(total_weights);
        let mut land_weights = Vec::with_capacity(total_weights);
        for (lens, c_part, w_part) in parts {
            land_centers.extend_from_slice(&c_part);
            land_weights.extend_from_slice(&w_part);
            for len in lens {
                land_weight_offsets.push(*land_weight_offsets.last().unwrap() + len);
            }
        }
        let (wet, dry) = VARIABILITY_PRECIPITATION_MM;
        let variability: Vec<f64> = normals
            .annual_precipitation_mm
            .iter()
            .map(|&p| {
                let t =
                    ((wet / p.max(f64::MIN_POSITIVE)).log10() / (wet / dry).log10()).clamp(0., 1.);
                settings.humid_variability
                    + t * (settings.arid_variability - settings.humid_variability)
            })
            .collect();
        let miami_temp: Vec<f64> = normals
            .annual_mean_celsius
            .iter()
            .map(|&t| 3000. / (1. + (1.315 - 0.119 * t).exp()))
            .collect();
        let threshold_mm: Vec<f64> = normals
            .annual_mean_celsius
            .iter()
            .map(|&t| aridity_threshold(t))
            .collect();
        let land_area: f64 = land_indices.iter().map(|&i| s.areas[i]).sum();
        let mut land_neighbor_offsets = Vec::with_capacity(land_count + 1);
        land_neighbor_offsets.push(0);
        let mut land_neighbors = Vec::with_capacity(land_count * 6);
        let mut land_meta = Vec::with_capacity(land_count);
        for &i in &land_indices {
            let (a, b) = (s.offsets[i] as usize, s.offsets[i + 1] as usize);
            for &j in &s.neighbors[a..b] {
                let slot = land_slot[j as usize];
                if slot != u32::MAX {
                    land_neighbors.push(slot);
                }
            }
            land_neighbor_offsets.push(land_neighbors.len() as u32);
            let var = variability[i];
            land_meta.push(LandMeta {
                index: i,
                area: s.areas[i],
                precipitation_mm: normals.annual_precipitation_mm[i],
                threshold_mm: threshold_mm[i],
                mean: normals.annual_mean_celsius[i],
                warmest: normals.warmest_month_celsius[i],
                coldest: normals.coldest_month_celsius[i],
                driest_mm: normals.driest_month_mm[i],
                variability: var,
                half_var_sq: 0.5 * var * var,
                miami_temp: miami_temp[i],
                reference_cover: MINIMUM_REFERENCE_COVER,
                neighbor_deg: (b - a).max(1) as f64,
            });
        }
        let mut model = Self {
            recipe: world.recipe.clone(),
            settings,
            threads,
            land,
            land_slot,
            land_area,
            land_meta,
            land_weight_offsets,
            land_centers,
            land_weights,
            land_neighbor_offsets,
            land_neighbors,
            neighbor_offsets: s.offsets.clone(),
            neighbors: s.neighbors.clone(),
            precipitation_mm: normals.annual_precipitation_mm.clone(),
            threshold_mm,
            mean: normals.annual_mean_celsius.clone(),
            warmest: normals.warmest_month_celsius.clone(),
            coldest: normals.coldest_month_celsius.clone(),
            driest_mm: normals.driest_month_mm.clone(),
            variability,
            miami_temp,
            normal_cover: Vec::new(),
            reference_cover: Vec::new(),
            initial: State::default(),
        };
        model.normal_cover = (0..n).map(|i| model.capacity(i, 1., 1.).1).collect();
        model.reference_cover = model
            .normal_cover
            .iter()
            .map(|&c| c.max(MINIMUM_REFERENCE_COVER))
            .collect();
        for m in &mut model.land_meta {
            m.reference_cover = model.reference_cover[m.index];
        }
        // Start from the capacity of normal precipitation, then spin up on a separate
        // stream so year 0 is a typical state and the run's stream is unaffected.
        let initial: Vec<RegionYear> = (0..n).map(|i| model.initial_region(i)).collect();
        let woody: Vec<f64> = initial.iter().map(|r| r.woody).collect();
        let mut state = State {
            woody_history: woody.repeat(CHANGE_YEARS),
            woody,
            herb: initial.iter().map(|r| r.herb).collect(),
            label: initial.iter().map(|r| r.label).collect(),
            precipitation_factor: vec![1.; n],
            root_moisture: vec![1.; n],
            established_cover: initial.iter().map(|r| r.established_cover).collect(),
            woody_change: vec![0.; n],
            ..State::default()
        };
        let mut spin = Random::stream(&world.recipe.seed, "vegetation-2/spin-up");
        let mut amplitudes: Vec<f64> = (0..count).map(|_| normal(&mut spin)).collect();
        let spin_years = settings.spin_up_years as usize;
        let mut spin_history = Vec::with_capacity(spin_years * count);
        for _ in 0..spin_years {
            next_amplitudes(&mut amplitudes, settings.anomaly_persistence, &mut spin);
            spin_history.extend_from_slice(&amplitudes);
        }
        model.step_years(&mut state, &spin_history, 1 - i64::from(settings.spin_up_years), false);
        let mut random = Random::stream(&world.recipe.seed, MODEL_VERSION);
        state.amplitudes = (0..count).map(|_| normal(&mut random)).collect();
        state.random = random.0;
        model.initial = state.clone();
        Ok((model, state))
    }

    pub fn threads(&self) -> usize {
        self.threads
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
                &state.woody_change,
            ]
            .iter()
            .any(|v| v.len() != n)
            || state.woody_history.len() != n * CHANGE_YEARS
            || state.label.len() != n
            || !bounded(&state.woody)
            || !bounded(&state.herb)
            || !bounded(&state.established_cover)
            || !bounded(&state.woody_history)
            || state
                .woody_change
                .iter()
                .any(|c| !(c.is_finite() && (-1. ..=1.).contains(c)))
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

    /// Log-standard deviation of the region's yearly precipitation factor.
    pub fn variability(&self, i: usize) -> f64 {
        self.variability[i]
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

    /// Miami productivity of the year's climate, scaled by the realized share of the
    /// cover that normal precipitation supports.
    pub fn productivity(&self, i: usize, factor: f64, cover: f64) -> f64 {
        if !self.land[i] {
            return 0.;
        }
        self.productivity_land(i, factor, cover)
    }

    #[inline]
    fn productivity_land(&self, i: usize, factor: f64, cover: f64) -> f64 {
        let precipitation = 3000. * (1. - (-0.000664 * (self.precipitation_mm[i] * factor)).exp());
        self.miami_temp[i].min(precipitation) * (cover / self.reference_cover[i]).min(1.)
    }

    /// Woody seed term from the mean woody cover of the neighbors last year.
    pub fn woody_establishment(&self, i: usize, woody: &[f64]) -> f64 {
        let (a, b) = (
            self.neighbor_offsets[i] as usize,
            self.neighbor_offsets[i + 1] as usize,
        );
        let mean = self.neighbors[a..b]
            .iter()
            .map(|&j| woody[j as usize])
            .sum::<f64>()
            / (b - a).max(1) as f64;
        self.settings.background_establishment + self.settings.seed_establishment * mean
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
            productivity: self.productivity(i, 1., woody + herb),
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

    #[inline]
    fn factor_land(&self, p: usize, m: &LandMeta, amplitudes: &[f64]) -> f64 {
        let (a, b) = (
            self.land_weight_offsets[p] as usize,
            self.land_weight_offsets[p + 1] as usize,
        );
        let mut z = 0.;
        for (&w, &c) in self.land_weights[a..b].iter().zip(&self.land_centers[a..b]) {
            z += w * amplitudes[c as usize];
        }
        (m.variability * z - m.half_var_sq).exp()
    }

    #[inline]
    fn woody_establishment_land(&self, p: usize, m: &LandMeta, woody: &[f64]) -> f64 {
        let (a, b) = (
            self.land_neighbor_offsets[p] as usize,
            self.land_neighbor_offsets[p + 1] as usize,
        );
        let mut sum = 0.;
        for &nb in &self.land_neighbors[a..b] {
            sum += woody[nb as usize];
        }
        let mean = sum / m.neighbor_deg;
        self.settings.background_establishment + self.settings.seed_establishment * mean
    }

    #[inline]
    fn woody_establishment_land_atomic(&self, p: usize, m: &LandMeta, woody: &[AtomicU64]) -> f64 {
        let (a, b) = (
            self.land_neighbor_offsets[p] as usize,
            self.land_neighbor_offsets[p + 1] as usize,
        );
        let mut sum = 0.;
        for &nb in &self.land_neighbors[a..b] {
            sum += f64::from_bits(woody[nb as usize].load(Ordering::Relaxed));
        }
        let mean = sum / m.neighbor_deg;
        self.settings.background_establishment + self.settings.seed_establishment * mean
    }

    #[inline]
    fn step_land_meta(
        &self,
        m: &LandMeta,
        prev_woody: f64,
        dyn_state: &mut LandDyn,
        factor: f64,
        woody_establishment: f64,
        slot: usize,
    ) -> (f64, bool) {
        let s = &self.settings;
        let root_moisture =
            dyn_state.root_moisture + (factor - dyn_state.root_moisture) / s.root_memory_years;
        let (woody_capacity, cover_capacity) = if m.warmest < 0. {
            (0., 0.)
        } else {
            let ratio_f = if m.threshold_mm <= 0. {
                f64::INFINITY
            } else {
                m.precipitation_mm * factor / m.threshold_mm
            };
            let cover = smoothstep(COVER_ARIDITY, ratio_f);
            if m.warmest < 10. {
                (0., cover * m.warmest / 10.)
            } else {
                let ratio_rm = if m.threshold_mm <= 0. {
                    f64::INFINITY
                } else {
                    m.precipitation_mm * root_moisture / m.threshold_mm
                };
                (smoothstep(WOODY_ARIDITY, ratio_rm), cover)
            }
        };
        let woody = relax(
            prev_woody,
            woody_capacity,
            s.woody_growth_per_year,
            s.woody_dieback_per_year,
            woody_establishment,
        );
        // Canopy displaces herbs immediately; herbs fill the remaining cover capacity.
        let herb = relax(
            dyn_state.herb,
            (cover_capacity - woody).max(0.),
            s.herb_growth_per_year,
            s.herb_dieback_per_year,
            s.herb_establishment,
        )
        .min(1. - woody);
        let established_cover = dyn_state.established_cover
            + (woody + herb - dyn_state.established_cover) / s.label_memory_years;
        let prev_label = dyn_state.label;
        let next_label = if m.warmest < 0. {
            Biome::Ice as u8
        } else if m.warmest < 10. {
            Biome::Tundra as u8
        } else {
            let was_forest = is_forest(prev_label);
            let was_vegetated = was_forest || is_open(prev_label);
            let forest = woody
                >= if was_forest {
                    FOREST_COVER.1
                } else {
                    FOREST_COVER.0
                };
            let open = established_cover
                >= if was_vegetated {
                    OPEN_COVER.1
                } else {
                    OPEN_COVER.0
                };
            (if forest {
                if m.coldest >= 18. {
                    if m.driest_mm >= 60. {
                        Biome::TropicalRainforest
                    } else {
                        Biome::TropicalSeasonalForest
                    }
                } else if m.coldest > -3. || m.warmest >= 22. {
                    Biome::TemperateForest
                } else {
                    Biome::BorealForest
                }
            } else if open {
                if m.mean >= 18. {
                    Biome::Savanna
                } else {
                    Biome::TemperateGrassland
                }
            } else if m.mean >= 18. {
                Biome::HotDesert
            } else {
                Biome::ColdDesert
            }) as u8
        };
        dyn_state.woody_change = woody - dyn_state.woody_history[slot];
        dyn_state.woody_history[slot] = woody;
        dyn_state.herb = herb;
        dyn_state.label = next_label;
        dyn_state.precipitation_factor = factor;
        dyn_state.root_moisture = root_moisture;
        dyn_state.established_cover = established_cover;
        (woody, next_label != prev_label)
    }

    #[inline]
    fn productivity_meta(m: &LandMeta, factor: f64, cover: f64) -> f64 {
        let precipitation = 3000. * (1. - (-0.000664 * (m.precipitation_mm * factor)).exp());
        m.miami_temp.min(precipitation) * (cover / m.reference_cover).min(1.)
    }

    fn step_region(
        &self,
        i: usize,
        previous: &RegionYear,
        factor: f64,
        woody_establishment: f64,
    ) -> RegionYear {
        let s = &self.settings;
        let root_moisture =
            previous.root_moisture + (factor - previous.root_moisture) / s.root_memory_years;
        let (woody_capacity, cover_capacity) = self.capacity(i, factor, root_moisture);
        let woody = relax(
            previous.woody,
            woody_capacity,
            s.woody_growth_per_year,
            s.woody_dieback_per_year,
            woody_establishment,
        );
        // Canopy displaces herbs immediately; herbs fill the remaining cover capacity.
        let herb = relax(
            previous.herb,
            (cover_capacity - woody).max(0.),
            s.herb_growth_per_year,
            s.herb_dieback_per_year,
            s.herb_establishment,
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
            productivity: self.productivity(i, factor, woody + herb),
            label: self.label(i, woody, established_cover, Some(previous.label)),
        }
    }

    pub fn region(&self, state: &State, i: usize) -> RegionYear {
        let factor = state.precipitation_factor[i];
        let (woody_capacity, cover_capacity) = self.capacity(i, factor, state.root_moisture[i]);
        RegionYear {
            precipitation_factor: factor,
            root_moisture: state.root_moisture[i],
            established_cover: state.established_cover[i],
            woody: state.woody[i],
            herb: state.herb[i],
            woody_capacity,
            cover_capacity,
            productivity: self.productivity(i, factor, state.woody[i] + state.herb[i]),
            label: state.label[i],
        }
    }

    #[cfg(test)]
    fn step_year(&self, state: &mut State, amplitudes: &[f64], year: i64) -> YearSummary {
        let before = state.summaries.len();
        self.step_years(state, amplitudes, year, true);
        state.summaries.drain(before..).next().unwrap()
    }

    fn pack_land_dyn(&self, state: &State) -> Vec<LandDyn> {
        let n = self.land.len();
        self.land_meta
            .iter()
            .map(|m| {
                let i = m.index;
                let mut woody_history = [0.; CHANGE_YEARS];
                for (slot, dst) in woody_history.iter_mut().enumerate() {
                    *dst = state.woody_history[slot * n + i];
                }
                LandDyn {
                    herb: state.herb[i],
                    root_moisture: state.root_moisture[i],
                    established_cover: state.established_cover[i],
                    woody_change: state.woody_change[i],
                    precipitation_factor: state.precipitation_factor[i],
                    woody_history,
                    label: state.label[i],
                }
            })
            .collect()
    }

    fn unpack_land_dyn(&self, state: &mut State, dyn_states: &[LandDyn]) {
        let n = self.land.len();
        for (m, d) in self.land_meta.iter().zip(dyn_states) {
            let i = m.index;
            state.herb[i] = d.herb;
            state.root_moisture[i] = d.root_moisture;
            state.established_cover[i] = d.established_cover;
            state.woody_change[i] = d.woody_change;
            state.precipitation_factor[i] = d.precipitation_factor;
            state.label[i] = d.label;
            for (slot, &val) in d.woody_history.iter().enumerate() {
                state.woody_history[slot * n + i] = val;
            }
        }
    }

    fn step_years(
        &self,
        state: &mut State,
        amplitudes: &[f64],
        start_year: i64,
        record_summaries: bool,
    ) {
        let k = self.settings.anomaly_centers as usize;
        let years = amplitudes.len() / k;
        if years == 0 {
            return;
        }
        let land_count = self.land_meta.len();
        if land_count == 0 {
            if record_summaries {
                state.summaries.extend((0..years).map(|_| YearSummary {
                    precipitation_factor: 0.,
                    woody_cover: 0.,
                    total_cover: 0.,
                    forest_share: 0.,
                    label_changes: 0,
                    productivity_petagrams: 0.,
                }));
            }
            return;
        }
        let chunk_land = land_count.div_ceil(self.threads.max(1)).max(1);
        let t_count = land_count.div_ceil(chunk_land);
        if t_count <= 1 {
            self.step_years_serial(state, amplitudes, start_year, record_summaries);
        } else {
            self.step_years_parallel(
                state,
                amplitudes,
                start_year,
                record_summaries,
                chunk_land,
                t_count,
            );
        }
    }

    fn step_years_serial(
        &self,
        state: &mut State,
        amplitudes: &[f64],
        start_year: i64,
        record_summaries: bool,
    ) {
        let k = self.settings.anomaly_centers as usize;
        let mut dyn_states = self.pack_land_dyn(state);
        let mut woody_prev: Vec<f64> = self.land_meta.iter().map(|m| state.woody[m.index]).collect();
        let mut woody_next = vec![0.; woody_prev.len()];
        let land = self.land_area.max(f64::MIN_POSITIVE);
        for (y, amps) in amplitudes.chunks_exact(k).enumerate() {
            let slot = (start_year + y as i64).rem_euclid(CHANGE_YEARS as i64) as usize;
            let mut sums = [0.; 5];
            let mut changes = 0u32;
            for p in 0..self.land_meta.len() {
                let m = &self.land_meta[p];
                let factor = self.factor_land(p, m, amps);
                let seed = self.woody_establishment_land(p, m, &woody_prev);
                let d = &mut dyn_states[p];
                let (next_woody, changed) =
                    self.step_land_meta(m, woody_prev[p], d, factor, seed, slot);
                woody_next[p] = next_woody;
                if record_summaries {
                    let prod = Self::productivity_meta(m, factor, next_woody + d.herb);
                    changes += u32::from(changed);
                    let area = m.area;
                    sums[0] += area * factor;
                    sums[1] += area * next_woody;
                    sums[2] += area * (next_woody + d.herb);
                    sums[3] += if is_forest(d.label) { area } else { 0. };
                    sums[4] += area * prod;
                }
            }
            std::mem::swap(&mut woody_prev, &mut woody_next);
            if record_summaries {
                state.summaries.push(YearSummary {
                    precipitation_factor: sums[0] / land,
                    woody_cover: sums[1] / land,
                    total_cover: sums[2] / land,
                    forest_share: sums[3] / land,
                    label_changes: changes,
                    productivity_petagrams: sums[4] / 1e15,
                });
            }
        }
        for (m, &w) in self.land_meta.iter().zip(&woody_prev) {
            state.woody[m.index] = w;
        }
        self.unpack_land_dyn(state, &dyn_states);
    }

    fn step_years_parallel(
        &self,
        state: &mut State,
        amplitudes: &[f64],
        start_year: i64,
        record_summaries: bool,
        chunk_land: usize,
        t_count: usize,
    ) {
        let k = self.settings.anomaly_centers as usize;
        let years = amplitudes.len() / k;
        let land_count = self.land_meta.len();
        let land_area = self.land_area.max(f64::MIN_POSITIVE);
        let mut dyn_states = self.pack_land_dyn(state);
        let woody_bufs: [Vec<AtomicU64>; 2] = [
            self.land_meta
                .iter()
                .map(|m| AtomicU64::new(state.woody[m.index].to_bits()))
                .collect(),
            (0..land_count).map(|_| AtomicU64::new(0)).collect(),
        ];
        let contrib_bufs: [Vec<[AtomicU64; 5]>; 2] = if record_summaries {
            std::array::from_fn(|_| {
                (0..land_count)
                    .map(|_| std::array::from_fn(|_| AtomicU64::new(0)))
                    .collect()
            })
        } else {
            [Vec::new(), Vec::new()]
        };
        let changes_bufs: [Vec<AtomicU32>; 2] = if record_summaries {
            std::array::from_fn(|_| (0..t_count).map(|_| AtomicU32::new(0)).collect())
        } else {
            [Vec::new(), Vec::new()]
        };
        let reduce = |buf: usize| -> YearSummary {
            let mut sums = [0.; 5];
            for entry in &contrib_bufs[buf] {
                sums[0] += f64::from_bits(entry[0].load(Ordering::Relaxed));
                sums[1] += f64::from_bits(entry[1].load(Ordering::Relaxed));
                sums[2] += f64::from_bits(entry[2].load(Ordering::Relaxed));
                sums[3] += f64::from_bits(entry[3].load(Ordering::Relaxed));
                sums[4] += f64::from_bits(entry[4].load(Ordering::Relaxed));
            }
            let changes: u32 = changes_bufs[buf]
                .iter()
                .map(|c| c.load(Ordering::Relaxed))
                .sum();
            YearSummary {
                precipitation_factor: sums[0] / land_area,
                woody_cover: sums[1] / land_area,
                total_cover: sums[2] / land_area,
                forest_share: sums[3] / land_area,
                label_changes: changes,
                productivity_petagrams: sums[4] / 1e15,
            }
        };

        let barrier = SpinBarrier::new(t_count);
        let mut summaries = Vec::with_capacity(if record_summaries { years } else { 0 });
        let woody_ref = &woody_bufs;
        let contrib_ref = &contrib_bufs;
        let changes_ref = &changes_bufs;
        let barrier_ref = &barrier;
        let reduce_ref = &reduce;
        thread::scope(|scope| {
            let mut chunks = dyn_states.chunks_mut(chunk_land).enumerate();
            let first = chunks.next().unwrap();
            let run_worker = |c: usize,
                              dyn_chunk: &mut [LandDyn],
                              mut out: Option<&mut Vec<YearSummary>>| {
                let land_start = c * chunk_land;
                let meta_chunk = &self.land_meta[land_start..land_start + dyn_chunk.len()];
                for y in 0..years {
                    if let Some(ref mut list) = out
                        && y > 0
                    {
                        list.push(reduce_ref((y - 1) & 1));
                    }
                    let amps = &amplitudes[y * k..(y + 1) * k];
                    let read_buf = &woody_ref[y & 1];
                    let write_buf = &woody_ref[(y + 1) & 1];
                    let slot = (start_year + y as i64).rem_euclid(CHANGE_YEARS as i64) as usize;
                    let mut local_changes = 0u32;
                    let cb = if record_summaries {
                        Some(&contrib_ref[y & 1][land_start..land_start + dyn_chunk.len()])
                    } else {
                        None
                    };
                    for (offset, (m, d)) in meta_chunk.iter().zip(dyn_chunk.iter_mut()).enumerate()
                    {
                        let p = land_start + offset;
                        let factor = self.factor_land(p, m, amps);
                        let seed = self.woody_establishment_land_atomic(p, m, read_buf);
                        let prev_woody = f64::from_bits(read_buf[p].load(Ordering::Relaxed));
                        let (next_woody, changed) =
                            self.step_land_meta(m, prev_woody, d, factor, seed, slot);
                        write_buf[p].store(next_woody.to_bits(), Ordering::Relaxed);
                        if let Some(contrib) = cb {
                            let prod = Self::productivity_meta(m, factor, next_woody + d.herb);
                            let area = m.area;
                            let entry = &contrib[offset];
                            entry[0].store((area * factor).to_bits(), Ordering::Relaxed);
                            entry[1].store((area * next_woody).to_bits(), Ordering::Relaxed);
                            entry[2].store(
                                (area * (next_woody + d.herb)).to_bits(),
                                Ordering::Relaxed,
                            );
                            entry[3].store(
                                (if is_forest(d.label) { area } else { 0. }).to_bits(),
                                Ordering::Relaxed,
                            );
                            entry[4].store((area * prod).to_bits(), Ordering::Relaxed);
                            local_changes += u32::from(changed);
                        }
                    }
                    if record_summaries {
                        changes_ref[y & 1][c].store(local_changes, Ordering::Relaxed);
                    }
                    barrier_ref.wait();
                }
                if let Some(list) = out {
                    list.push(reduce_ref((years - 1) & 1));
                }
            };
            let run_ref = &run_worker;
            let handles: Vec<_> = chunks
                .map(|(c, dyn_chunk)| scope.spawn(move || run_ref(c, dyn_chunk, None)))
                .collect();
            run_worker(
                first.0,
                first.1,
                if record_summaries {
                    Some(&mut summaries)
                } else {
                    None
                },
            );
            for handle in handles {
                handle.join().unwrap();
            }
        });
        let final_buf = &woody_bufs[years & 1];
        for (p, m) in self.land_meta.iter().enumerate() {
            state.woody[m.index] = f64::from_bits(final_buf[p].load(Ordering::Relaxed));
        }
        self.unpack_land_dyn(state, &dyn_states);
        if record_summaries {
            state.summaries.extend(summaries);
        }
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
        let history_start = state.amplitude_history.len();
        let mut random = Random(state.random);
        for _ in 0..years {
            next_amplitudes(
                &mut state.amplitudes,
                self.settings.anomaly_persistence,
                &mut random,
            );
            state.amplitude_history.extend_from_slice(&state.amplitudes);
        }
        state.random = random.0;
        let run_amplitudes = state.amplitude_history[history_start..].to_vec();
        let start_year = i64::from(state.year) + 1;
        self.step_years(state, &run_amplitudes, start_year, true);
        state.year += years;
        Ok(())
    }

    /// Capacities of the most recent year for every region.
    pub fn capacities(&self, state: &State) -> (Vec<f64>, Vec<f64>) {
        (0..self.land.len())
            .map(|i| self.capacity(i, state.precipitation_factor[i], state.root_moisture[i]))
            .unzip()
    }

    /// Productivity of the most recent year for every region.
    pub fn productivities(&self, state: &State) -> Vec<f64> {
        (0..self.land.len())
            .map(|i| {
                self.productivity(
                    i,
                    state.precipitation_factor[i],
                    state.woody[i] + state.herb[i],
                )
            })
            .collect()
    }

    /// One region from year 0 to the current year. Seed dispersal couples neighbors,
    /// so the coupled woody component is replayed from the recorded amplitudes.
    pub fn region_series(&self, state: &State, i: usize) -> Result<Vec<RegionYear>, String> {
        if i >= self.land.len() {
            return Err("Vegetation region is outside the generated world.".into());
        }
        let k = self.settings.anomaly_centers as usize;
        let years = state.amplitude_history.chunks_exact(k).len();
        let initial_target = self.region(&self.initial, i);
        let mut series = Vec::with_capacity(years + 1);
        series.push(initial_target);
        if years == 0 {
            return Ok(series);
        }
        let target_p = self.land_slot[i] as usize;
        if !self.land[i] {
            series.resize(years + 1, initial_target);
            return Ok(series);
        }
        if self.warmest[i] < 10. || self.settings.seed_establishment == 0. {
            let mut current = initial_target;
            let seed = self.settings.background_establishment;
            let m = &self.land_meta[target_p];
            for amplitudes in state.amplitude_history.chunks_exact(k) {
                let factor = self.factor_land(target_p, m, amplitudes);
                current = self.step_region(i, &current, factor, seed);
                series.push(current);
            }
            return Ok(series);
        }
        // Only land regions with warmest >= 10 °C can hold nonzero woody cover and
        // transmit woody seed changes; regions at graph distance >= years cannot reach i.
        let land_count = self.land_meta.len();
        let max_dist = years - 1;
        let mut visited = vec![false; land_count];
        let mut queue = VecDeque::new();
        visited[target_p] = true;
        queue.push_back((target_p, 0usize));
        let mut others = Vec::new();
        while let Some((u, dist)) = queue.pop_front() {
            if u != target_p {
                others.push(u);
            }
            if dist == max_dist {
                continue;
            }
            let (a, b) = (
                self.land_neighbor_offsets[u] as usize,
                self.land_neighbor_offsets[u + 1] as usize,
            );
            for &v_u32 in &self.land_neighbors[a..b] {
                let v = v_u32 as usize;
                if !visited[v] && self.land_meta[v].warmest >= 10. {
                    visited[v] = true;
                    queue.push_back((v, dist + 1));
                }
            }
        }
        others.sort_unstable();
        let s = &self.settings;
        let woody_bufs: [Vec<AtomicU64>; 2] = [
            self.land_meta
                .iter()
                .map(|m| AtomicU64::new(self.initial.woody[m.index].to_bits()))
                .collect(),
            self.land_meta
                .iter()
                .map(|m| AtomicU64::new(self.initial.woody[m.index].to_bits()))
                .collect(),
        ];
        let mut root_moisture_others: Vec<f64> = others
            .iter()
            .map(|&p| self.initial.root_moisture[self.land_meta[p].index])
            .collect();
        let t_count = if self.threads <= 1 || others.len() < 256 {
            1
        } else {
            let chunk = others.len().div_ceil(self.threads).max(1);
            others.len().div_ceil(chunk)
        };
        let target_meta = &self.land_meta[target_p];
        let mut current = initial_target;
        let step_others = |indices: &[usize],
                           rm_chunk: &mut [f64],
                           amplitudes: &[f64],
                           read_buf: &[AtomicU64],
                           write_buf: &[AtomicU64]| {
            for (&p, rm) in indices.iter().zip(rm_chunk.iter_mut()) {
                let m = &self.land_meta[p];
                let factor = self.factor_land(p, m, amplitudes);
                let next_rm = *rm + (factor - *rm) / s.root_memory_years;
                *rm = next_rm;
                let ratio = if m.threshold_mm <= 0. {
                    f64::INFINITY
                } else {
                    m.precipitation_mm * next_rm / m.threshold_mm
                };
                let cap = smoothstep(WOODY_ARIDITY, ratio);
                let seed = self.woody_establishment_land_atomic(p, m, read_buf);
                let prev_woody = f64::from_bits(read_buf[p].load(Ordering::Relaxed));
                let next_woody = relax(
                    prev_woody,
                    cap,
                    s.woody_growth_per_year,
                    s.woody_dieback_per_year,
                    seed,
                );
                write_buf[p].store(next_woody.to_bits(), Ordering::Relaxed);
            }
        };
        if t_count <= 1 {
            for (y, amplitudes) in state.amplitude_history.chunks_exact(k).enumerate() {
                let read_buf = &woody_bufs[y & 1];
                let write_buf = &woody_bufs[(y + 1) & 1];
                let target_factor = self.factor_land(target_p, target_meta, amplitudes);
                let target_seed = self.woody_establishment_land_atomic(target_p, target_meta, read_buf);
                current = self.step_region(i, &current, target_factor, target_seed);
                write_buf[target_p].store(current.woody.to_bits(), Ordering::Relaxed);
                series.push(current);
                step_others(&others, &mut root_moisture_others, amplitudes, read_buf, write_buf);
            }
            return Ok(series);
        }
        let chunk_size = others.len().div_ceil(t_count);
        let barrier = SpinBarrier::new(t_count);
        let amps_all = &state.amplitude_history[..years * k];
        let mut rm_rest = root_moisture_others.as_mut_slice();
        let mut chunks = Vec::with_capacity(t_count);
        for c in 0..t_count {
            let start = c * chunk_size;
            let end = ((c + 1) * chunk_size).min(others.len());
            let (rm_chunk, tail) = rm_rest.split_at_mut(end - start);
            rm_rest = tail;
            chunks.push((&others[start..end], rm_chunk));
        }
        let woody_ref = &woody_bufs;
        let barrier_ref = &barrier;
        let step_ref = &step_others;
        thread::scope(|scope| {
            let mut iter = chunks.into_iter();
            let first = iter.next().unwrap();
            let handles: Vec<_> = iter
                .map(|(indices, rm_chunk)| {
                    scope.spawn(move || {
                        for (y, amplitudes) in amps_all.chunks_exact(k).enumerate() {
                            step_ref(
                                indices,
                                rm_chunk,
                                amplitudes,
                                &woody_ref[y & 1],
                                &woody_ref[(y + 1) & 1],
                            );
                            barrier_ref.wait();
                        }
                    })
                })
                .collect();
            let (first_indices, first_rm) = first;
            for (y, amplitudes) in amps_all.chunks_exact(k).enumerate() {
                let read_buf = &woody_bufs[y & 1];
                let write_buf = &woody_bufs[(y + 1) & 1];
                let target_factor = self.factor_land(target_p, target_meta, amplitudes);
                let target_seed = self.woody_establishment_land_atomic(target_p, target_meta, read_buf);
                current = self.step_region(i, &current, target_factor, target_seed);
                write_buf[target_p].store(current.woody.to_bits(), Ordering::Relaxed);
                series.push(current);
                step_others(first_indices, first_rm, amplitudes, read_buf, write_buf);
                barrier.wait();
            }
            for handle in handles {
                handle.join().unwrap();
            }
        });
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

    fn steady() -> Settings {
        Settings {
            humid_variability: 0.,
            arid_variability: 0.,
            ..Default::default()
        }
    }

    #[test]
    fn relaxation_is_bounded_and_asymmetric() {
        let s = Settings::default();
        let grow = relax(
            0.5,
            1.,
            s.woody_growth_per_year,
            s.woody_dieback_per_year,
            s.background_establishment,
        );
        let die = relax(
            0.5,
            0.,
            s.woody_growth_per_year,
            s.woody_dieback_per_year,
            s.background_establishment,
        );
        assert!(grow > 0.5 && grow < 0.6 && die < 0.4 && die > 0.);
        assert_eq!(relax(0.9, 0.95, 2., 1., 0.2), 0.95);
        assert!((relax(0.9, 0.2, 2., 1., 0.2) - 0.2).abs() < 1e-12);
    }

    #[test]
    fn miami_productivity_matches_published_values() {
        // Lieth (1975): about 2,230 g/m² allowed by 20 °C and 1,460 g/m² by 1,000 mm.
        let (t, p) = miami(20., 1000.);
        assert!((t - 2231.).abs() < 2. && (p - 1456.).abs() < 2., "{t} {p}");
        assert!(miami(-20., 3000.).0 < 100. && miami(30., 0.).1 == 0.);
    }

    #[test]
    fn without_variability_the_state_is_the_climate_classification_forever() {
        let world = test_world("vegetation");
        let normals = Normals::from_world(&world).unwrap();
        let (model, mut state) = Model::new(&world, &normals, steady()).unwrap();
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
        assert!(state.woody_change.iter().all(|&c| c == 0.));
        assert!(state.summaries.iter().all(|s| s.label_changes == 0));
    }

    #[test]
    fn variability_is_larger_in_dry_climates() {
        let world = test_world("vegetation");
        let normals = Normals::from_world(&world).unwrap();
        let (model, _) = Model::new(&world, &normals, Settings::default()).unwrap();
        for (i, &p) in normals.annual_precipitation_mm.iter().enumerate() {
            let sigma = model.variability(i);
            assert!((0.12 - 1e-12..=0.4 + 1e-12).contains(&sigma));
            if p >= 2000. {
                assert!((sigma - 0.12).abs() < 1e-12);
            }
            if p <= 200. {
                assert!((sigma - 0.4).abs() < 1e-12);
            }
        }
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
        let changes: u32 = state.summaries.iter().map(|s| s.label_changes).sum();
        let rate = f64::from(changes) / 200. / land.len() as f64;
        assert!(rate < 0.015, "yearly label change rate {rate}");
    }

    #[test]
    fn the_spin_up_starts_year_zero_in_the_long_run_state() {
        let world = test_world("vegetation");
        let (model, mut state) = Model::from_world(&world, Settings::default()).unwrap();
        model.advance(&mut state, 300).unwrap();
        let mean = |years: &[YearSummary]| {
            years.iter().map(|s| s.forest_share).sum::<f64>() / years.len() as f64
        };
        let (early, late) = (mean(&state.summaries[..50]), mean(&state.summaries[250..]));
        assert!((early - late).abs() < 0.02, "forest {early} then {late}");
        // The spin-up leaves forest below the capacity of normal precipitation.
        let (_, steady) = Model::from_world(&world, steady()).unwrap();
        let forest = |labels: &[u8]| labels.iter().filter(|&&l| is_forest(l)).count();
        assert!(forest(&model.initial.label) < forest(&steady.label));
    }

    #[test]
    fn cleared_forest_regrows_from_wooded_neighbors() {
        let world = test_world("vegetation");
        let s = &world.surface;
        let ring = |i: usize| {
            s.neighbors[s.offsets[i] as usize..s.offsets[i + 1] as usize]
                .iter()
                .map(|&j| j as usize)
        };
        let regrowth = |seed_establishment: f64| {
            let settings = Settings {
                seed_establishment,
                ..steady()
            };
            let (model, mut state) = Model::from_world(&world, settings).unwrap();
            let wooded = |j: usize| model.capacity(j, 1., 1.).0 > 0.8;
            let center = (0..state.woody.len())
                .find(|&c| wooded(c) && ring(c).all(wooded))
                .expect("a region inside a forest");
            state.woody[center] = 0.;
            let amplitudes = vec![0.; model.settings.anomaly_centers as usize];
            for year in 1..=20 {
                model.step_year(&mut state, &amplitudes, year);
            }
            (state.woody[center], state.woody_change[center])
        };
        let ((seeded, change), (isolated, _)) = (regrowth(0.05), regrowth(0.));
        assert!(
            seeded > 2. * isolated && seeded > 0.3,
            "seeded {seeded} isolated {isolated}"
        );
        assert!(change > 0.);
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
            assert_eq!(series[30], model.region(&whole, i));
        }
        assert!(Model::restore(&test_world("another"), model.checkpoint(&whole)).is_err());
    }

    #[test]
    fn a_drought_cuts_forest_quickly_and_recovery_takes_years() {
        let world = test_world("vegetation");
        let (model, state) = Model::from_world(&world, Settings::default()).unwrap();
        // A forest region near its aridity margin, where a dry year lowers capacity.
        let i = (0..state.woody.len())
            .find(|&i| {
                state.woody[i] > 0.9
                    && model.capacity(i, 1., 1.).0 > 0.95
                    && model.capacity(i, 1., 0.75).0 < 0.3
            })
            .expect("a marginal forest region");
        let seed = model.woody_establishment(i, &state.woody);
        let mut current = model.region(&state, i);
        let before = current.woody;
        for _ in 0..3 {
            current = model.step_region(i, &current, 0.6, seed);
        }
        let after = current.woody;
        assert!(after < before - 0.2, "drought effect {before} → {after}");
        assert!(current.productivity < model.region(&state, i).productivity);
        let mut years = 0;
        while current.woody < before - 0.05 {
            current = model.step_region(i, &current, 1., seed);
            years += 1;
            assert!(years < 200);
        }
        assert!(years >= 5, "recovery took {years} years");
    }

    #[test]
    fn multithreaded_run_is_bitwise_identical_to_single_threaded() {
        let world = test_world("vegetation");
        let normals = Normals::from_world(&world).unwrap();
        let (m1, mut s1) = Model::new_with_threads(&world, &normals, Settings::default(), 1).unwrap();
        let (m4, mut s4) = Model::new_with_threads(&world, &normals, Settings::default(), 4).unwrap();
        assert_eq!(s1, s4, "spin-up state differs between 1 and 4 threads");
        m1.advance(&mut s1, 50).unwrap();
        m4.advance(&mut s4, 50).unwrap();
        assert_eq!(s1, s4, "advanced state differs between 1 and 4 threads");
        // Check region_series across water, tundra/ice, and temperate/tropical land regions.
        let mut samples = Vec::new();
        for kind in [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
            if let Some(idx) = normals.biome.iter().position(|&b| b == kind) {
                samples.push(idx);
            }
        }
        assert!(samples.len() >= 5);
        for idx in samples {
            let r1 = m1.region_series(&s1, idx).unwrap();
            let r4 = m4.region_series(&s4, idx).unwrap();
            assert_eq!(r1, r4, "region_series differs for region {idx}");
            assert_eq!(r4[50], m4.region(&s4, idx), "region_series end differs for region {idx}");
        }
    }
}
