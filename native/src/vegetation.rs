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
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::thread;

pub const MODEL_VERSION: &str = "vegetation-2";
/// The desktop experiment bounds history and checkpoint size.
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

struct Climate {
    precipitation_mm: f64,
    threshold_mm: f64,
    warmest: f64,
    variability: f64,
    half_variance: f64,
    miami_temperature: f64,
    reference_cover: f64,
    forest: u8,
    open: u8,
    desert: u8,
}

impl Climate {
    fn aridity(&self, factor: f64) -> f64 {
        if self.threshold_mm <= 0. {
            f64::INFINITY
        } else {
            self.precipitation_mm * factor / self.threshold_mm
        }
    }

    fn woody_capacity(&self, root_moisture: f64) -> f64 {
        if self.warmest < 10. {
            0.
        } else {
            smoothstep(WOODY_ARIDITY, self.aridity(root_moisture))
        }
    }

    fn capacity(&self, factor: f64, root_moisture: f64) -> (f64, f64) {
        if self.warmest < 0. {
            return (0., 0.);
        }
        let cover = smoothstep(COVER_ARIDITY, self.aridity(factor));
        if self.warmest < 10. {
            (0., cover * self.warmest / 10.)
        } else {
            (self.woody_capacity(root_moisture), cover)
        }
    }

    fn productivity(&self, factor: f64, cover: f64) -> f64 {
        let mm = self.precipitation_mm * factor;
        if cover == 0. && mm >= 0. {
            return cover;
        }
        let precipitation = miami_precipitation(mm);
        self.miami_temperature.min(precipitation) * (cover / self.reference_cover).min(1.)
    }

    fn label(&self, woody: f64, established: f64, previous: u8) -> u8 {
        if self.warmest < 10. {
            return if self.warmest < 0. {
                Biome::Ice
            } else {
                Biome::Tundra
            } as u8;
        }
        let was_forest = is_forest(previous);
        let forest_limit = if was_forest {
            FOREST_COVER.1
        } else {
            FOREST_COVER.0
        };
        let open_limit = if was_forest || is_open(previous) {
            OPEN_COVER.1
        } else {
            OPEN_COVER.0
        };
        if woody >= forest_limit {
            self.forest
        } else if established >= open_limit {
            self.open
        } else {
            self.desert
        }
    }
}

struct Land {
    index: usize,
    area: f64,
    climate: Climate,
}

#[derive(Clone, Copy)]
struct Cover {
    woody: f64,
    herb: f64,
    root_moisture: f64,
    established: f64,
    label: u8,
}

impl Cover {
    fn read(state: &State, i: usize) -> Self {
        Self {
            woody: state.woody[i],
            herb: state.herb[i],
            root_moisture: state.root_moisture[i],
            established: state.established_cover[i],
            label: state.label[i],
        }
    }

    fn write(self, state: &mut State, i: usize, factor: f64) {
        state.woody[i] = self.woody;
        state.herb[i] = self.herb;
        state.root_moisture[i] = self.root_moisture;
        state.established_cover[i] = self.established;
        state.label[i] = self.label;
        state.precipitation_factor[i] = factor;
    }
}

struct LandState {
    cover: Cover,
    factor: f64,
    woody_change: f64,
    woody_history: [f64; CHANGE_YEARS],
}

struct WeightRows {
    lengths: Vec<u32>,
    centers: Vec<u16>,
    weights: Vec<f64>,
}

struct ReplayChunk<'a> {
    roots: &'a mut [(usize, f64)],
    target_series: Option<&'a mut Vec<RegionYear>>,
}

pub struct Model {
    recipe: Recipe,
    settings: Settings,
    threads: usize,
    // Land entries are in surface order so reductions match the original serial sum.
    land: Vec<Land>,
    // Land slots come first, followed by water slots. Public state stays surface-ordered.
    region_slots: Vec<usize>,
    land_area: f64,
    water_climate: Vec<Climate>,
    neighbor_offsets: Vec<u32>,
    neighbors: Vec<u32>,
    weight_offsets: Vec<u32>,
    centers: Vec<u16>,
    weights: Vec<f64>,
    /// State at year 0, after the spin-up; region histories replay from it.
    initial: State,
}

fn default_threads(regions: usize) -> usize {
    if regions < 4_096 {
        return 1;
    }
    thread::available_parallelism().map_or(1, |threads| threads.get().min(6))
}

#[derive(Default)]
struct YearPhase {
    year: usize,
    arrived: usize,
    cancelled: bool,
}

struct YearBarrier {
    workers: usize,
    phase: Mutex<YearPhase>,
    changed: Condvar,
}

impl YearBarrier {
    fn wait(&self) -> bool {
        let mut phase = self.phase.lock().unwrap();
        if phase.cancelled {
            return false;
        }
        let year = phase.year;
        phase.arrived += 1;
        if phase.arrived == self.workers {
            phase.arrived = 0;
            phase.year += 1;
            self.changed.notify_all();
        } else {
            phase = self
                .changed
                .wait_while(phase, |p| p.year == year && !p.cancelled)
                .unwrap();
        }
        !phase.cancelled
    }
}

/// Release the other workers if setup, a worker, or the ordered reduction panics.
struct CancelYearOnPanic<'a>(&'a YearBarrier);

impl Drop for CancelYearOnPanic<'_> {
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.phase.lock().unwrap().cancelled = true;
            self.0.changed.notify_all();
        }
    }
}

/// Workers enter the next year while the calling thread reduces the completed one.
/// Alternating buffers keep both sets of per-region contributions separate.
fn run_years<T: Send>(
    chunks: impl IntoIterator<Item = T>,
    years: usize,
    step: impl Fn(usize, usize, &mut T) -> u32 + Sync,
    mut completed: impl FnMut(usize, u32),
) {
    let chunks: Vec<_> = chunks.into_iter().collect();
    let barrier = YearBarrier {
        workers: chunks.len(),
        phase: Mutex::default(),
        changed: Condvar::new(),
    };
    let changes: Vec<_> = (0..chunks.len())
        .map(|_| [AtomicU32::new(0), AtomicU32::new(0)])
        .collect();
    let (barrier, changes, step) = (&barrier, &changes, &step);
    thread::scope(|scope| {
        let _cancel = CancelYearOnPanic(barrier);
        let mut chunks = chunks.into_iter().enumerate();
        let (_, mut first) = chunks.next().expect("at least one vegetation chunk");
        for (i, mut chunk) in chunks {
            scope.spawn(move || {
                let _cancel = CancelYearOnPanic(barrier);
                for year in 0..years {
                    changes[i][year % 2].store(step(i, year, &mut chunk), Ordering::Relaxed);
                    if !barrier.wait() {
                        return;
                    }
                }
            });
        }
        for year in 0..years {
            let local_changes = step(0, year, &mut first);
            if !barrier.wait() {
                break;
            }
            let changes = local_changes
                + changes[1..]
                    .iter()
                    .map(|counts| counts[year % 2].load(Ordering::Relaxed))
                    .sum::<u32>();
            completed(year, changes);
        }
    });
}

fn add_terms(sums: &mut [f64; 5], terms: [f64; 5]) {
    for (sum, term) in sums.iter_mut().zip(terms) {
        *sum += term;
    }
}

fn woody_buffers(woody: &[f64]) -> [Vec<AtomicU64>; 2] {
    std::array::from_fn(|_| woody.iter().map(|w| AtomicU64::new(w.to_bits())).collect())
}

fn load_woody(buffer: &[AtomicU64], index: usize) -> f64 {
    f64::from_bits(buffer[index].load(Ordering::Relaxed))
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
        miami_temperature(annual_mean_celsius),
        miami_precipitation(precipitation_mm),
    )
}

fn miami_temperature(celsius: f64) -> f64 {
    3000. / (1. + (1.315 - 0.119 * celsius).exp())
}

fn miami_precipitation(mm: f64) -> f64 {
    3000. * (1. - (-0.000664 * mm).exp())
}

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

    fn new_with_threads(
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
        let mut region_slots = vec![usize::MAX; n];
        let mut land_indices = Vec::new();
        for (i, &is_land) in land.iter().enumerate() {
            if is_land {
                region_slots[i] = land_indices.len();
                land_indices.push(i);
            }
        }
        let land_count = land_indices.len();
        let length = settings.anomaly_length_meters / world.recipe.radius_meters;
        let cutoff = 3. * length;
        // Reject distant centers cheaply. The margin sends borderline candidates
        // through the original acos comparison and weight calculation.
        let cos_cutoff = if cutoff < std::f64::consts::PI {
            cutoff.cos() - 1e-12
        } else {
            -2.
        };
        let build_chunk = |indices: std::ops::Range<usize>| -> Result<WeightRows, String> {
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
                        return Err("Vegetation anomaly centers do not cover the surface.".into());
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
            Ok(WeightRows {
                lengths: lens,
                centers: local_centers,
                weights: local_weights,
            })
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
        let total_weights: usize = parts.iter().map(|part| part.weights.len()).sum();
        let mut land_weight_offsets = Vec::with_capacity(land_count + 1);
        land_weight_offsets.push(0);
        let mut land_centers = Vec::with_capacity(total_weights);
        let mut land_weights = Vec::with_capacity(total_weights);
        for part in parts {
            land_centers.extend(part.centers);
            land_weights.extend(part.weights);
            for len in part.lengths {
                land_weight_offsets.push(*land_weight_offsets.last().unwrap() + len);
            }
        }
        let make_climate = |i: usize| {
            let precipitation_mm = normals.annual_precipitation_mm[i];
            let mean = normals.annual_mean_celsius[i];
            let warmest = normals.warmest_month_celsius[i];
            let coldest = normals.coldest_month_celsius[i];
            let (wet, dry) = VARIABILITY_PRECIPITATION_MM;
            let t = ((wet / precipitation_mm.max(f64::MIN_POSITIVE)).log10() / (wet / dry).log10())
                .clamp(0., 1.);
            let variability = settings.humid_variability
                + t * (settings.arid_variability - settings.humid_variability);
            let forest = if coldest >= 18. {
                if normals.driest_month_mm[i] >= 60. {
                    Biome::TropicalRainforest
                } else {
                    Biome::TropicalSeasonalForest
                }
            } else if coldest > -3. || warmest >= 22. {
                Biome::TemperateForest
            } else {
                Biome::BorealForest
            };
            let (open, desert) = if mean >= 18. {
                (Biome::Savanna, Biome::HotDesert)
            } else {
                (Biome::TemperateGrassland, Biome::ColdDesert)
            };
            let mut climate = Climate {
                precipitation_mm,
                threshold_mm: aridity_threshold(mean),
                warmest,
                variability,
                half_variance: 0.5 * variability * variability,
                miami_temperature: miami_temperature(mean),
                reference_cover: 0.,
                forest: forest as u8,
                open: open as u8,
                desert: desert as u8,
            };
            climate.reference_cover = climate.capacity(1., 1.).1.max(MINIMUM_REFERENCE_COVER);
            climate
        };
        let land_area = land_indices.iter().map(|&i| s.areas[i]).sum::<f64>();
        let model_land = land_indices
            .into_iter()
            .map(|i| Land {
                index: i,
                area: s.areas[i],
                climate: make_climate(i),
            })
            .collect();
        let mut water_climate = Vec::with_capacity(n - land_count);
        for (i, &is_land) in land.iter().enumerate() {
            if !is_land {
                region_slots[i] = land_count + water_climate.len();
                water_climate.push(make_climate(i));
            }
        }
        let mut model = Self {
            recipe: world.recipe.clone(),
            settings,
            threads,
            land: model_land,
            region_slots,
            land_area,
            water_climate,
            neighbor_offsets: s.offsets.clone(),
            neighbors: s.neighbors.clone(),
            weight_offsets: land_weight_offsets,
            centers: land_centers,
            weights: land_weights,
            initial: State::default(),
        };
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
        model.step_years(
            &mut state,
            &spin_history,
            1 - i64::from(settings.spin_up_years),
            false,
        );
        let mut random = Random::stream(&world.recipe.seed, MODEL_VERSION);
        state.amplitudes = (0..count).map(|_| normal(&mut random)).collect();
        state.random = random.0;
        model.initial = state.clone();
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

    fn climate(&self, i: usize) -> &Climate {
        let slot = self.region_slots[i];
        if let Some(land) = self.land.get(slot) {
            &land.climate
        } else {
            &self.water_climate[slot - self.land.len()]
        }
    }

    /// Aridity ratio (precipitation / Köppen threshold) for a precipitation factor.
    pub fn aridity_ratio(&self, i: usize, factor: f64) -> f64 {
        self.climate(i).aridity(factor)
    }

    /// Log-standard deviation of the region's yearly precipitation factor.
    pub fn variability(&self, i: usize) -> f64 {
        self.climate(i).variability
    }

    /// Woody capacity from the root-zone factor and total cover capacity from the
    /// current year's factor.
    pub fn capacity(&self, i: usize, factor: f64, root_moisture: f64) -> (f64, f64) {
        self.land
            .get(self.region_slots[i])
            .map_or((0., 0.), |land| {
                land.climate.capacity(factor, root_moisture)
            })
    }

    /// Miami productivity of the year's climate, scaled by the realized share of the
    /// cover that normal precipitation supports.
    pub fn productivity(&self, i: usize, factor: f64, cover: f64) -> f64 {
        self.land
            .get(self.region_slots[i])
            .map_or(0., |land| land.climate.productivity(factor, cover))
    }

    fn neighbors(&self, i: usize) -> &[u32] {
        &self.neighbors[self.neighbor_offsets[i] as usize..self.neighbor_offsets[i + 1] as usize]
    }

    /// Woody seed term from the mean woody cover of the neighbors last year.
    pub fn woody_establishment(&self, i: usize, woody: &[f64]) -> f64 {
        self.seed_establishment(i, |j| woody[j])
    }

    fn seed_establishment(&self, i: usize, woody: impl Fn(usize) -> f64) -> f64 {
        let neighbors = self.neighbors(i);
        let sum = neighbors.iter().map(|&j| woody(j as usize)).sum::<f64>();
        let mean = sum / neighbors.len().max(1) as f64;
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

    /// Structural class from woody and established cover with climate-normal
    /// thermal subtypes. `previous` enables hysteresis.
    pub fn label(&self, i: usize, woody: f64, established: f64, previous: Option<u8>) -> u8 {
        self.land
            .get(self.region_slots[i])
            .map_or(Biome::Water as u8, |land| {
                land.climate
                    .label(woody, established, previous.unwrap_or(0))
            })
    }

    #[inline]
    fn factor(&self, slot: usize, climate: &Climate, amplitudes: &[f64]) -> f64 {
        let range = self.weight_offsets[slot] as usize..self.weight_offsets[slot + 1] as usize;
        let z: f64 = self.weights[range.clone()]
            .iter()
            .zip(&self.centers[range])
            .map(|(&weight, &center)| weight * amplitudes[center as usize])
            .sum();
        (climate.variability * z - climate.half_variance).exp()
    }

    /// Evaluate four independent years together so the weight loads are shared.
    /// Each lane adds centers in their original order.
    fn factor_block(&self, slot: usize, climate: &Climate, amplitudes: &[[f64; 4]]) -> [f64; 4] {
        let range = self.weight_offsets[slot] as usize..self.weight_offsets[slot + 1] as usize;
        let mut z = [0.; 4];
        for (&weight, &center) in self.weights[range.clone()].iter().zip(&self.centers[range]) {
            let amplitude = amplitudes[center as usize];
            for (total, amplitude) in z.iter_mut().zip(amplitude) {
                *total += weight * amplitude;
            }
        }
        z.map(|z| (climate.variability * z - climate.half_variance).exp())
    }

    fn root_moisture(&self, previous: f64, factor: f64) -> f64 {
        previous + (factor - previous) / self.settings.root_memory_years
    }

    fn next_woody(&self, previous: f64, capacity: f64, seed: impl FnOnce() -> f64) -> f64 {
        let establishment = if previous < capacity { seed() } else { 0. };
        relax(
            previous,
            capacity,
            self.settings.woody_growth_per_year,
            self.settings.woody_dieback_per_year,
            establishment,
        )
    }

    fn grow(
        &self,
        climate: &Climate,
        previous: Cover,
        factor: f64,
        seed: impl FnOnce() -> f64,
    ) -> (Cover, (f64, f64)) {
        let s = &self.settings;
        let root_moisture = self.root_moisture(previous.root_moisture, factor);
        let (woody_capacity, cover_capacity) = climate.capacity(factor, root_moisture);
        let woody = self.next_woody(previous.woody, woody_capacity, seed);
        // Canopy displaces herbs immediately; herbs fill the remaining cover capacity.
        let herb = relax(
            previous.herb,
            (cover_capacity - woody).max(0.),
            s.herb_growth_per_year,
            s.herb_dieback_per_year,
            s.herb_establishment,
        )
        .min(1. - woody);
        let established =
            previous.established + (woody + herb - previous.established) / s.label_memory_years;
        let label = climate.label(woody, established, previous.label);
        (
            Cover {
                woody,
                herb,
                root_moisture,
                established,
                label,
            },
            (woody_capacity, cover_capacity),
        )
    }

    fn step_region(&self, i: usize, previous: &RegionYear, factor: f64, seed: f64) -> RegionYear {
        let climate = &self.land[self.region_slots[i]].climate;
        let cover = Cover {
            woody: previous.woody,
            herb: previous.herb,
            root_moisture: previous.root_moisture,
            established: previous.established_cover,
            label: previous.label,
        };
        let (next, (woody_capacity, cover_capacity)) = self.grow(climate, cover, factor, || seed);
        RegionYear {
            precipitation_factor: factor,
            root_moisture: next.root_moisture,
            established_cover: next.established,
            woody: next.woody,
            herb: next.herb,
            woody_capacity,
            cover_capacity,
            productivity: climate.productivity(factor, next.woody + next.herb),
            label: next.label,
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

    fn terms(land: &Land, next: Cover, factor: f64) -> [f64; 5] {
        let area = land.area;
        let cover = next.woody + next.herb;
        [
            area * factor,
            area * next.woody,
            area * cover,
            if is_forest(next.label) { area } else { 0. },
            area * land.climate.productivity(factor, cover),
        ]
    }

    fn summary(
        &self,
        [factor, woody, cover, forest, productivity]: [f64; 5],
        changes: u32,
    ) -> YearSummary {
        let area = self.land_area.max(f64::MIN_POSITIVE);
        YearSummary {
            precipitation_factor: factor / area,
            woody_cover: woody / area,
            total_cover: cover / area,
            forest_share: forest / area,
            label_changes: changes,
            productivity_petagrams: productivity / 1e15,
        }
    }

    /// Inline one-year requests, which are also used by coupled annual consumers.
    fn step_year(&self, state: &mut State, amplitudes: &[f64], year: i64) -> YearSummary {
        self.step_one::<true>(state, amplitudes, year)
    }

    fn step_one<const SUMMARIZE: bool>(
        &self,
        state: &mut State,
        amplitudes: &[f64],
        year: i64,
    ) -> YearSummary {
        let n = self.region_slots.len();
        let history = year.rem_euclid(CHANGE_YEARS as i64) as usize * n;
        let woody = state.woody.clone();
        let mut sums = [0.; 5];
        let mut changes = 0;
        for (slot, land) in self.land.iter().enumerate() {
            let i = land.index;
            let previous = Cover::read(state, i);
            let factor = self.factor(slot, &land.climate, amplitudes);
            let seed = || self.woody_establishment(i, &woody);
            let (next, _) = self.grow(&land.climate, previous, factor, seed);
            if SUMMARIZE {
                changes += u32::from(previous.label != next.label);
                add_terms(&mut sums, Self::terms(land, next, factor));
            }
            state.woody_change[i] = next.woody - state.woody_history[history + i];
            state.woody_history[history + i] = next.woody;
            next.write(state, i, factor);
        }
        self.summary(sums, changes)
    }

    fn step_years(&self, state: &mut State, amplitudes: &[f64], start_year: i64, summarize: bool) {
        let k = self.settings.anomaly_centers as usize;
        let years = amplitudes.len() / k;
        let threads = self.threads.min(self.land.len());
        if threads < 2 || years == 1 {
            for (y, amps) in amplitudes.chunks_exact(k).enumerate() {
                let year = start_year + y as i64;
                if summarize {
                    let summary = self.step_year(state, amps, year);
                    state.summaries.push(summary);
                } else {
                    self.step_one::<false>(state, amps, year);
                }
            }
            return;
        }

        let n = self.region_slots.len();
        let mut dynamic: Vec<_> = self
            .land
            .iter()
            .map(|land| {
                let i = land.index;
                LandState {
                    cover: Cover::read(state, i),
                    factor: state.precipitation_factor[i],
                    woody_change: state.woody_change[i],
                    woody_history: std::array::from_fn(|y| state.woody_history[y * n + i]),
                }
            })
            .collect();
        let woody = woody_buffers(&state.woody);
        let chunk_size = dynamic.len().div_ceil(threads);
        let transposed: Vec<[f64; 4]> = amplitudes
            .chunks(4 * k)
            .flat_map(|block| {
                (0..k).map(move |center| {
                    std::array::from_fn(|lane| block.get(lane * k + center).copied().unwrap_or(0.))
                })
            })
            .collect();
        let terms: [Vec<_>; 2] = std::array::from_fn(|_| {
            if summarize {
                dynamic
                    .chunks(chunk_size)
                    .map(|chunk| Mutex::new(vec![[0.; 5]; chunk.len()]))
                    .collect()
            } else {
                Vec::new()
            }
        });
        let chunks = dynamic.chunks_mut(chunk_size).map(|cells| {
            let factors = vec![[0.; 4]; cells.len()];
            (cells, factors)
        });
        run_years(
            chunks,
            years,
            |chunk, year, (cells, factors)| {
                let read = &woody[year % 2];
                let write = &woody[(year + 1) % 2];
                let lane = year % 4;
                let start = chunk * chunk_size;
                let land = &self.land[start..start + cells.len()];
                if lane == 0 {
                    let block = year / 4;
                    let amps = &transposed[block * k..(block + 1) * k];
                    for (offset, (land, value)) in land.iter().zip(factors.iter_mut()).enumerate() {
                        *value = self.factor_block(start + offset, &land.climate, amps);
                    }
                }
                let history = (start_year + year as i64).rem_euclid(CHANGE_YEARS as i64) as usize;
                let mut contributions = summarize.then(|| terms[year % 2][chunk].lock().unwrap());
                let mut changes = 0;
                for (offset, (cell, land)) in cells.iter_mut().zip(land).enumerate() {
                    let i = land.index;
                    let factor = factors[offset][lane];
                    let seed = || self.seed_establishment(i, |j| load_woody(read, j));
                    let (next, _) = self.grow(&land.climate, cell.cover, factor, seed);
                    if let Some(terms) = &mut contributions {
                        terms[offset] = Self::terms(land, next, factor);
                        changes += u32::from(cell.cover.label != next.label);
                    }
                    write[i].store(next.woody.to_bits(), Ordering::Relaxed);
                    cell.woody_change = next.woody - cell.woody_history[history];
                    cell.woody_history[history] = next.woody;
                    cell.cover = next;
                    cell.factor = factor;
                }
                changes
            },
            |year, changes| {
                if summarize {
                    let mut sums = [0.; 5];
                    for chunk in &terms[year % 2] {
                        for &term in chunk.lock().unwrap().iter() {
                            add_terms(&mut sums, term);
                        }
                    }
                    state.summaries.push(self.summary(sums, changes));
                }
            },
        );
        for (land, cell) in self.land.iter().zip(dynamic) {
            let i = land.index;
            cell.cover.write(state, i, cell.factor);
            state.woody_change[i] = cell.woody_change;
            for (y, value) in cell.woody_history.into_iter().enumerate() {
                state.woody_history[y * n + i] = value;
            }
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
        (0..self.region_slots.len())
            .map(|i| self.capacity(i, state.precipitation_factor[i], state.root_moisture[i]))
            .unzip()
    }

    /// Productivity of the most recent year for every region.
    pub fn productivities(&self, state: &State) -> Vec<f64> {
        (0..self.region_slots.len())
            .map(|i| {
                self.productivity(
                    i,
                    state.precipitation_factor[i],
                    state.woody[i] + state.herb[i],
                )
            })
            .collect()
    }

    /// One region from year 0 to the current year. Replay the woody component
    /// that can reach this region, then derive its full history from that component.
    pub fn region_series(&self, state: &State, i: usize) -> Result<Vec<RegionYear>, String> {
        if i >= self.region_slots.len() {
            return Err("Vegetation region is outside the generated world.".into());
        }
        let k = self.settings.anomaly_centers as usize;
        let years = state.amplitude_history.len() / k;
        let initial = self.region(&self.initial, i);
        let mut series = Vec::with_capacity(years + 1);
        series.push(initial);
        let target = self.region_slots[i];
        if years == 0 {
            return Ok(series);
        }
        if target >= self.land.len() {
            series.resize(years + 1, initial);
            return Ok(series);
        }

        // Water and cold land start without woody cover and can never acquire it.
        // A change travels at most one edge per year. The unchanged buffers already
        // hold year-zero cover for neighbors just outside the replay radius.
        let mut visited = vec![false; self.land.len()];
        let mut queue = VecDeque::from([(target, 0)]);
        let mut roots = Vec::new();
        visited[target] = true;
        if self.land[target].climate.warmest >= 10. {
            while let Some((slot, distance)) = queue.pop_front() {
                let land = &self.land[slot];
                if slot != target {
                    roots.push((slot, self.initial.root_moisture[land.index]));
                }
                if distance + 1 == years {
                    continue;
                }
                for &neighbor in self.neighbors(land.index) {
                    let slot = self.region_slots[neighbor as usize];
                    if slot < self.land.len()
                        && !visited[slot]
                        && self.land[slot].climate.warmest >= 10.
                    {
                        visited[slot] = true;
                        queue.push_back((slot, distance + 1));
                    }
                }
            }
        }
        roots.sort_unstable_by_key(|&(slot, _)| slot);
        let woody = woody_buffers(&self.initial.woody);
        let target_climate = &self.land[target].climate;
        let step = |_: usize, year: usize, chunk: &mut ReplayChunk<'_>| {
            let read = &woody[year % 2];
            let write = &woody[(year + 1) % 2];
            let amps = &state.amplitude_history[year * k..(year + 1) * k];
            for (slot, root) in chunk.roots.iter_mut() {
                let land = &self.land[*slot];
                let j = land.index;
                let factor = self.factor(*slot, &land.climate, amps);
                *root = self.root_moisture(*root, factor);
                let capacity = land.climate.woody_capacity(*root);
                let seed = || self.seed_establishment(j, |p| load_woody(read, p));
                let next = self.next_woody(load_woody(read, j), capacity, seed);
                write[j].store(next.to_bits(), Ordering::Relaxed);
            }
            if let Some(series) = &mut chunk.target_series {
                let factor = self.factor(target, target_climate, amps);
                let seed = self.seed_establishment(i, |j| load_woody(read, j));
                let next = self.step_region(i, series.last().unwrap(), factor, seed);
                write[i].store(next.woody.to_bits(), Ordering::Relaxed);
                series.push(next);
            }
            0
        };
        let threads = self.threads.min(roots.len().div_ceil(768));
        if threads < 2 {
            let mut chunk = ReplayChunk {
                roots: &mut roots,
                target_series: Some(&mut series),
            };
            for year in 0..years {
                step(0, year, &mut chunk);
            }
        } else {
            let size = roots.len().div_ceil(threads);
            let target_series =
                std::iter::once(Some(&mut series)).chain(std::iter::repeat_with(|| None));
            let chunks = roots
                .chunks_mut(size)
                .zip(target_series)
                .map(|(roots, target_series)| ReplayChunk {
                    roots,
                    target_series,
                });
            run_years(chunks, years, step, |_, _| {});
        }
        Ok(series)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_world(seed: &str) -> World {
        World::generate(test_recipe(seed)).unwrap()
    }

    fn test_recipe(seed: &str) -> Recipe {
        serde_json::from_value(serde_json::json!({"schemaVersion":1,
            "modelVersion":"multiscale-geography-4","randomVersion":"fnv1a-utf8-mulberry32-1","seed":seed,
            "subdivision":4,"radiusMeters":6371000.,"plateCount":12,"maxPlateSpeedCmPerYear":8.,
            "continentalFraction":0.38,"continentalScale":1.,"reliefScale":1.,"boundaryWidthKm":300.,
            "detailAmplitudeMeters":300.,"terrainPreparationPasses":4,"water":{"mode":"coverage","fraction":0.71}}))
            .unwrap()
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

    fn fnv_update(h: &mut u64, bytes: &[u8]) {
        for &b in bytes {
            *h ^= u64::from(b);
            *h = h.wrapping_mul(0x100000001b3);
        }
    }

    fn hash_f64_slice(h: &mut u64, s: &[f64]) {
        for &x in s {
            fnv_update(h, &x.to_bits().to_le_bytes());
        }
    }

    fn hash_state(state: &State) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        fnv_update(&mut h, &state.year.to_le_bytes());
        fnv_update(&mut h, &state.random.to_le_bytes());
        hash_f64_slice(&mut h, &state.amplitudes);
        hash_f64_slice(&mut h, &state.amplitude_history);
        hash_f64_slice(&mut h, &state.woody);
        hash_f64_slice(&mut h, &state.herb);
        fnv_update(&mut h, &state.label);
        hash_f64_slice(&mut h, &state.precipitation_factor);
        hash_f64_slice(&mut h, &state.root_moisture);
        hash_f64_slice(&mut h, &state.established_cover);
        hash_f64_slice(&mut h, &state.woody_history);
        hash_f64_slice(&mut h, &state.woody_change);
        for s in &state.summaries {
            fnv_update(&mut h, &s.precipitation_factor.to_bits().to_le_bytes());
            fnv_update(&mut h, &s.woody_cover.to_bits().to_le_bytes());
            fnv_update(&mut h, &s.total_cover.to_bits().to_le_bytes());
            fnv_update(&mut h, &s.forest_share.to_bits().to_le_bytes());
            fnv_update(&mut h, &s.label_changes.to_le_bytes());
            fnv_update(&mut h, &s.productivity_petagrams.to_bits().to_le_bytes());
        }
        h
    }

    fn hash_series(series: &[RegionYear]) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        for r in series {
            fnv_update(&mut h, &r.precipitation_factor.to_bits().to_le_bytes());
            fnv_update(&mut h, &r.root_moisture.to_bits().to_le_bytes());
            fnv_update(&mut h, &r.established_cover.to_bits().to_le_bytes());
            fnv_update(&mut h, &r.woody.to_bits().to_le_bytes());
            fnv_update(&mut h, &r.herb.to_bits().to_le_bytes());
            fnv_update(&mut h, &r.woody_capacity.to_bits().to_le_bytes());
            fnv_update(&mut h, &r.cover_capacity.to_bits().to_le_bytes());
            fnv_update(&mut h, &r.productivity.to_bits().to_le_bytes());
            fnv_update(&mut h, &[r.label]);
        }
        h
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    #[test]
    fn state_and_replay_match_serial_vegetation_2() {
        // Captured by running these hash functions against unmodified a655880, using
        // first-light, the default geography recipe at subdivision 4 and default
        // vegetation settings. Integers and f64 bit patterns are fed little-endian.
        let world = test_world("first-light");
        let normals = Normals::from_world(&world).unwrap();
        let (model, mut state) =
            Model::new_with_threads(&world, &normals, Settings::default(), 4).unwrap();
        assert_eq!(hash_state(&state), 0x625bbd214b17de76);
        model.advance(&mut state, 300).unwrap();
        assert_eq!(hash_state(&state), 0xbd25e437844301ca);
        for (region, expected) in [
            (1, 0x9b22fb069a6bceef),  // Water
            (36, 0x94cb06ad7b82e690), // Ice
            (44, 0x68035a2689a38030), // Tundra
            (0, 0x94c69ed7fc664c24),  // Boreal
            (117, 0x1263dc8b4bfcea78),
            (5, 0x188ee27239db3e74),
            (4, 0x90db9f5618dc7861),
            (321, 0x09d28fa80a107564),
            (13, 0x9aafd303bbd343e4),
            (89, 0xe19d4cadee2a55c6),
            (325, 0x3e19f9afebedcd14),
        ] {
            let series = model.region_series(&state, region).unwrap();
            assert_eq!(hash_series(&series), expected, "region {region}");
            assert_eq!(series[300], model.region(&state, region));
        }
    }

    #[test]
    fn multithreaded_run_is_bitwise_identical_to_single_threaded() {
        let mut recipe = test_recipe("first-light");
        recipe.subdivision = 6;
        let world = World::generate(recipe).unwrap();
        let normals = Normals::from_world(&world).unwrap();
        let (m1, mut s1) =
            Model::new_with_threads(&world, &normals, Settings::default(), 1).unwrap();
        let (m4, mut s4) =
            Model::new_with_threads(&world, &normals, Settings::default(), 4).unwrap();
        assert_eq!(hash_state(&s1), hash_state(&s4), "spin-up");
        m4.advance(&mut s4, 301).unwrap();
        for _ in 0..301 {
            m1.advance(&mut s1, 1).unwrap();
        }
        assert_eq!(
            hash_state(&s1),
            hash_state(&s4),
            "301-year state and summaries"
        );

        // Region 4 is in the largest warm component (3,105 land regions on this
        // recipe), so its replay exercises parallel chunks as well as reduction-free work.
        let climate_samples =
            [Biome::Water, Biome::Tundra, Biome::TropicalRainforest].map(|class| {
                normals
                    .biome
                    .iter()
                    .position(|&label| label == class as u8)
                    .unwrap()
            });
        for i in climate_samples.into_iter().chain([4]) {
            let serial = m1.region_series(&s1, i).unwrap();
            let parallel = m4.region_series(&s4, i).unwrap();
            assert_eq!(hash_series(&serial), hash_series(&parallel), "region {i}");
            assert_eq!(parallel[301], m4.region(&s4, i));
        }

        // Existing checkpoints allow nonzero values in water cells. Neighbor seed
        // supply reads the public state in surface order, including those values.
        let coast = m1
            .land
            .iter()
            .flat_map(|land| m1.neighbors(land.index))
            .map(|&i| i as usize)
            .find(|&i| world.water.body_ids[i] != 0)
            .unwrap();
        s1.woody[coast] = 0.25;
        s4.woody[coast] = 0.25;
        m1.advance(&mut s1, 7).unwrap();
        m4.advance(&mut s4, 7).unwrap();
        assert_eq!(
            hash_state(&s1),
            hash_state(&s4),
            "public state with a water neighbor"
        );
    }
}
