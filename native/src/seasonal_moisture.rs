//! Finite seasonal exchange with delayed runoff and evaporating terminal stores.
//! Version 8 optionally shares finite liquid inside fixed reference water bodies.
//! Version 9 couples minimum-leaf lake exposure below the first connection.
//! Version 10 adds bounded fast leaf spill with explicit pending/input owners.
//! Version 11 adds one-level all-dry common-sill parent owners above birth.
//! Fixed bed/thermal forcing; no general split/merge, hydraulic discharge or energy feedback.
use crate::{
    Recipe, World,
    moisture_transport::{Flow, Geometry, total_mass},
    orographic_response, runoff_transport, seasonal_temperature, seasonal_wind, surface_water,
};
use serde::{Deserialize, Serialize};

pub mod body_preparation;
pub mod closed_lake;
mod lake_exchange;
pub mod leaf_spill;
pub mod merged_lake;
pub mod preparation;
mod reference_pool;
pub mod water_return;

pub const MODEL_VERSION: &str = "seasonal-moisture-3";
pub const OROGRAPHIC_MODEL_VERSION: &str = "seasonal-moisture-4";
pub const SOIL_PRECISION_MODEL_VERSION: &str = "seasonal-moisture-5";
pub const SURFACE_PRECISION_MODEL_VERSION: &str = "seasonal-moisture-6";
pub const TERMINAL_PRECISION_MODEL_VERSION: &str = "seasonal-moisture-7";
pub const REFERENCE_POOL_MODEL_VERSION: &str = "seasonal-moisture-8";
pub const CLOSED_LAKE_MODEL_VERSION: &str = "seasonal-moisture-9";
pub const LEAF_SPILL_MODEL_VERSION: &str = "seasonal-moisture-10";
pub const MERGED_LAKE_MODEL_VERSION: &str = "seasonal-moisture-11";
pub const TERMINAL_STOCK_MODEL_VERSION: &str = "terminal-stock-compensated-1";
pub const SECONDS_PER_DAY: u64 = 86400;
pub const MAX_ELAPSED_SECONDS: u64 = 3650 * SECONDS_PER_DAY;
pub const WATER_DENSITY_KILOGRAMS_PER_CUBIC_METER: f64 = 1000.;
const VAPOR_GAS_CONSTANT: f64 = 461.5;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SoilNumerics {
    Compensated,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceNumerics {
    Compensated,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalNumerics {
    Compensated,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReferenceWaterPool {
    FastConnectedBody,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClosedLakeExchange {
    FrozenLeafExposure,
    FrozenLeafExposureWithSpill,
    FrozenLeafExposureWithSpillAndMerge,
}

// Missing means legacy; a present null is not an unrecorded/default algorithm.
fn present_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub initial_active_surface_depth_meters: f64,
    pub effective_vapor_depth_meters: f64,
    pub evaporation_response_seconds: f64,
    pub precipitation_response_seconds: f64,
    pub evaporation_enabled: bool,
    pub precipitation_enabled: bool,
    pub max_coupled_step_seconds: u32,
    pub transport: crate::moisture_transport::Settings,
    pub surface: surface_water::Settings,
    pub routing_enabled: bool,
    pub runoff: runoff_transport::Settings,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub orography: Option<orographic_response::Settings>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub soil_numerics: Option<SoilNumerics>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub surface_numerics: Option<SurfaceNumerics>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub terminal_numerics: Option<TerminalNumerics>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub reference_water_pool: Option<ReferenceWaterPool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub closed_lake_exchange: Option<ClosedLakeExchange>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            initial_active_surface_depth_meters: 1.,
            effective_vapor_depth_meters: 2000.,
            evaporation_response_seconds: 5. * SECONDS_PER_DAY as f64,
            precipitation_response_seconds: 0.25 * SECONDS_PER_DAY as f64,
            evaporation_enabled: true,
            precipitation_enabled: true,
            max_coupled_step_seconds: 3600,
            transport: crate::moisture_transport::Settings::default(),
            surface: surface_water::Settings::default(),
            routing_enabled: true,
            runoff: runoff_transport::Settings::default(),
            orography: None,
            soil_numerics: None,
            surface_numerics: None,
            terminal_numerics: None,
            reference_water_pool: None,
            closed_lake_exchange: None,
        }
    }
}

impl Settings {
    pub fn validate(self) -> Result<(), String> {
        self.transport.validate()?;
        self.surface.validate()?;
        self.runoff.validate()?;
        if let Some(orography) = self.orography {
            orography.validate()?;
        }
        if self.soil_numerics.is_some() && self.orography.is_none() {
            return Err(
                "The compensated-soil candidate currently requires the upslope model.".into(),
            );
        }
        if self.surface_numerics.is_some() && self.soil_numerics.is_none() {
            return Err("The compensated-surface candidate requires compensated soil.".into());
        }
        if self.terminal_numerics.is_some() && self.surface_numerics.is_none() {
            return Err(
                "The compensated-terminal candidate requires compensated surface stocks.".into(),
            );
        }
        if self.reference_water_pool.is_some() && self.terminal_numerics.is_none() {
            return Err("The reference-water pool requires compensated terminal stocks.".into());
        }
        if self.closed_lake_exchange.is_some() && self.reference_water_pool.is_none() {
            return Err(
                "Closed-leaf lake exchange requires the finite reference-water pool.".into(),
            );
        }
        if !(60..=21600).contains(&self.max_coupled_step_seconds) {
            return Err("Coupled moisture step limit must be 60–21600 seconds.".into());
        }
        for (value, low, high) in [
            (self.initial_active_surface_depth_meters, 0., 10.),
            (self.effective_vapor_depth_meters, 100., 5000.),
            (
                self.evaporation_response_seconds,
                3600.,
                30. * SECONDS_PER_DAY as f64,
            ),
            (
                self.precipitation_response_seconds,
                3600.,
                10. * SECONDS_PER_DAY as f64,
            ),
        ] {
            if !value.is_finite() || !(low..=high).contains(&value) {
                return Err("Invalid seasonal-moisture settings.".into());
            }
        }
        Ok(())
    }
}

/// Murphy–Koop equilibrium vapor pressure: ice below 0°C, water otherwise.
/// The supported range here is deliberately limited to -100..50°C.
pub fn saturation_pressure_pascals(temperature_celsius: f64) -> Result<f64, String> {
    if !temperature_celsius.is_finite() || !(-100. ..=50.).contains(&temperature_celsius) {
        return Err("Moisture temperature must be within -100..50 degrees Celsius.".into());
    }
    let t = temperature_celsius + 273.15;
    let log_pressure = if temperature_celsius < 0. {
        9.550426 - 5723.265 / t + 3.53068 * t.ln() - 0.00728332 * t
    } else {
        54.842763 - 6763.22 / t - 4.210 * t.ln()
            + 0.000367 * t
            + (0.0415 * (t - 218.8)).tanh()
                * (53.878 - 1331.22 / t - 9.44523 * t.ln() + 0.014025 * t)
    };
    Ok(log_pressure.exp())
}

/// An effective isothermal vapor column, not a vertically resolved atmosphere.
pub fn saturation_column_kilograms_per_square_meter(
    temperature_celsius: f64,
    effective_depth_meters: f64,
) -> Result<f64, String> {
    if !effective_depth_meters.is_finite() || !(100. ..=5000.).contains(&effective_depth_meters) {
        return Err("Invalid effective vapor depth.".into());
    }
    Ok(
        saturation_pressure_pascals(temperature_celsius)? * effective_depth_meters
            / (VAPOR_GAS_CONSTANT * (temperature_celsius + 273.15)),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exchange {
    pub surface_kilograms: f64,
    pub vapor_kilograms: f64,
    pub evaporated_kilograms: f64,
    pub precipitated_kilograms: f64,
    pub residual_kilograms: f64,
}

/// Analytic relaxation at fixed capacity, capped only by the physical donor.
/// Below freezing, evaporation is disabled; deposition remains water-equivalent.
pub fn exchange(
    surface_kilograms: f64,
    vapor_kilograms: f64,
    area_square_meters: f64,
    temperature_celsius: f64,
    seconds: f64,
    settings: Settings,
) -> Result<Exchange, String> {
    settings.validate()?;
    if [surface_kilograms, vapor_kilograms]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.)
        || !area_square_meters.is_finite()
        || area_square_meters <= 0.
        || !seconds.is_finite()
        || seconds <= 0.
        || seconds > SECONDS_PER_DAY as f64
    {
        return Err("Invalid phase-exchange stock, area, or interval.".into());
    }
    let capacity = saturation_column_kilograms_per_square_meter(
        temperature_celsius,
        settings.effective_vapor_depth_meters,
    )? * area_square_meters;
    if !capacity.is_finite() {
        return Err("Moisture saturation capacity overflow.".into());
    }
    exchange_at_capacity(
        surface_kilograms,
        vapor_kilograms,
        capacity,
        if settings.evaporation_enabled && temperature_celsius > 0. {
            -(-seconds / settings.evaporation_response_seconds).exp_m1()
        } else {
            0.
        },
        if settings.precipitation_enabled {
            -(-seconds / settings.precipitation_response_seconds).exp_m1()
        } else {
            0.
        },
    )
}

fn exchange_at_capacity(
    surface_kilograms: f64,
    vapor_kilograms: f64,
    capacity: f64,
    evaporation_fraction: f64,
    precipitation_fraction: f64,
) -> Result<Exchange, String> {
    let evaporated =
        ((capacity - vapor_kilograms).max(0.) * evaporation_fraction).min(surface_kilograms);
    let precipitated = (vapor_kilograms - capacity).max(0.) * precipitation_fraction;
    let surface = (surface_kilograms - evaporated) + precipitated;
    let vapor = (vapor_kilograms - precipitated) + evaporated;
    let residual = (surface - surface_kilograms) + (vapor - vapor_kilograms);
    let scale = surface_kilograms.max(vapor_kilograms).max(1.);
    if !surface.is_finite()
        || !vapor.is_finite()
        || !residual.is_finite()
        || residual.abs() > 16. * f64::EPSILON * scale
    {
        return Err("Local moisture exchange exceeds its arithmetic tolerance.".into());
    }
    Ok(Exchange {
        surface_kilograms: surface,
        vapor_kilograms: vapor,
        evaporated_kilograms: evaporated,
        precipitated_kilograms: precipitated,
        residual_kilograms: residual,
    })
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub schema_version: u32,
    pub model_version: String,
    pub transport_model_version: String,
    pub temperature_model_version: String,
    pub wind_model_version: String,
    pub surface_model_version: String,
    pub runoff_model_version: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub orographic_model_version: Option<String>,
    pub recipe: Recipe,
    pub settings: Settings,
    pub temperature_settings: seasonal_temperature::Settings,
    pub wind_settings: seasonal_wind::Settings,
    pub elapsed_seconds: u64,
    pub surface_kilograms: Vec<f64>,
    pub snow_kilograms: Vec<f64>,
    pub soil_kilograms: Vec<f64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub soil_low_kilograms: Option<Vec<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub snow_low_kilograms: Option<Vec<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub surface_low_kilograms: Option<Vec<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub terminal_low_kilograms: Option<Vec<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub terminal_stock_model_version: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub reference_body_model_version: Option<String>,
    /// Body stocks ordered by sorted positive IDs reconstructed from the recipe.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub reference_body_high_kilograms: Option<Vec<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub reference_body_low_kilograms: Option<Vec<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub closed_lake_model_version: Option<String>,
    /// Gross local liquid captured by the lake, not another owned water stock.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub cumulative_lake_capture_kilograms: Option<Vec<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub cumulative_lake_capture_low_kilograms: Option<Vec<f64>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub leaf_spill_state: Option<leaf_spill::Checkpoint>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_option"
    )]
    pub merged_lake_state: Option<merged_lake::Checkpoint>,
    pub pending_runoff_kilograms: Vec<f64>,
    pub terminal_water_kilograms: Vec<f64>,
    pub cumulative_runoff_transfers: Vec<runoff_transport::Transfers>,
    pub cumulative_runoff_transfer_roundoff: Vec<[f64; 4]>,
    pub cumulative_surface_transfers: Vec<surface_water::Transfers>,
    pub cumulative_surface_transfer_roundoff: Vec<[f64; 8]>,
    pub vapor_kilograms: Vec<f64>,
    pub cumulative_evaporation_kilograms: Vec<f64>,
    pub cumulative_precipitation_kilograms: Vec<f64>,
}

/// Validated state; only explicit model operations can change its stocks or clock.
#[derive(Clone, Debug, PartialEq)]
pub struct State(Checkpoint);
impl State {
    pub fn checkpoint(&self) -> Checkpoint {
        self.0.clone()
    }
    pub fn elapsed_seconds(&self) -> u64 {
        self.0.elapsed_seconds
    }
    pub fn surface_kilograms(&self) -> &[f64] {
        &self.0.surface_kilograms
    }
    pub fn vapor_kilograms(&self) -> &[f64] {
        &self.0.vapor_kilograms
    }
    pub fn snow_kilograms(&self) -> &[f64] {
        &self.0.snow_kilograms
    }
    pub fn soil_kilograms(&self) -> &[f64] {
        &self.0.soil_kilograms
    }
    /// Signed low component of the same soil stock in the headless candidate.
    pub fn soil_low_kilograms(&self) -> Option<&[f64]> {
        self.0.soil_low_kilograms.as_deref()
    }
    pub fn snow_low_kilograms(&self) -> Option<&[f64]> {
        self.0.snow_low_kilograms.as_deref()
    }
    pub fn surface_low_kilograms(&self) -> Option<&[f64]> {
        self.0.surface_low_kilograms.as_deref()
    }
    pub fn terminal_low_kilograms(&self) -> Option<&[f64]> {
        self.0.terminal_low_kilograms.as_deref()
    }
    pub fn reference_body_high_kilograms(&self) -> Option<&[f64]> {
        self.0.reference_body_high_kilograms.as_deref()
    }
    pub fn reference_body_low_kilograms(&self) -> Option<&[f64]> {
        self.0.reference_body_low_kilograms.as_deref()
    }
    pub fn cumulative_lake_capture_kilograms(&self) -> Option<&[f64]> {
        self.0.cumulative_lake_capture_kilograms.as_deref()
    }
    pub fn cumulative_lake_capture_low_kilograms(&self) -> Option<&[f64]> {
        self.0.cumulative_lake_capture_low_kilograms.as_deref()
    }
    pub fn pending_runoff_kilograms(&self) -> &[f64] {
        &self.0.pending_runoff_kilograms
    }
    pub fn terminal_water_kilograms(&self) -> &[f64] {
        &self.0.terminal_water_kilograms
    }
    /// Rounded leading regional stocks and optional body-owned liquid.
    /// Compensated candidates' signed low parts are available separately.
    pub fn owned_stocks(&self) -> impl Iterator<Item = &f64> {
        self.surface_kilograms()
            .iter()
            .chain(self.snow_kilograms())
            .chain(self.soil_kilograms())
            .chain(self.pending_runoff_kilograms())
            .chain(self.terminal_water_kilograms())
            .chain(self.vapor_kilograms())
            .chain(self.reference_body_high_kilograms().into_iter().flatten())
            .chain(
                self.0
                    .leaf_spill_state
                    .iter()
                    .flat_map(|s| &s.pending_input.high_kilograms),
            )
            .chain(
                self.0
                    .merged_lake_state
                    .iter()
                    .flat_map(|s| &s.parents)
                    .flat_map(|p| [&p.birth_high_kilograms, &p.surplus_high_kilograms]),
            )
    }
    /// All representation components, each owned once; lows are not extra stores.
    pub fn owned_stock_components(&self) -> impl Iterator<Item = &f64> {
        self.owned_stocks()
            .chain(self.soil_low_kilograms().into_iter().flatten())
            .chain(self.snow_low_kilograms().into_iter().flatten())
            .chain(self.surface_low_kilograms().into_iter().flatten())
            .chain(self.terminal_low_kilograms().into_iter().flatten())
            .chain(self.reference_body_low_kilograms().into_iter().flatten())
            .chain(
                self.0
                    .leaf_spill_state
                    .iter()
                    .flat_map(|s| &s.pending_input.low_kilograms),
            )
            .chain(
                self.0
                    .merged_lake_state
                    .iter()
                    .flat_map(|s| &s.parents)
                    .flat_map(|p| [&p.birth_low_kilograms, &p.surplus_low_kilograms]),
            )
    }
}

pub struct Model {
    origin: Checkpoint,
    areas: Vec<f64>,
    is_land: Vec<bool>,
    // Immutable connectivity; versions 3–7 use this only for diagnostics.
    reference_body_ids: Vec<u32>,
    reference_pool: Option<reference_pool::Layout>,
    lake_exchange: Option<lake_exchange::Layout>,
    routing: runoff_transport::Network,
    initial_surface: Vec<f64>,
    initial_total: f64,
    temperatures: Vec<Vec<f64>>,
    capacities: Vec<Vec<f64>>,
    flows: Vec<Flow>,
    orographic_forcing: Option<OrographicForcing>,
}

struct OrographicForcing {
    uplift_meters_per_second: Vec<Vec<f64>>,
    additional_rates_per_second: Vec<Vec<f64>>,
    maximum_rate_per_second: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Budget {
    pub initial_mobile_water_kilograms: f64,
    pub surface_kilograms: f64,
    pub snow_kilograms: f64,
    pub soil_kilograms: f64,
    pub pending_runoff_kilograms: f64,
    pub terminal_water_kilograms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_body_water_kilograms: Option<f64>,
    /// Gross transfer only; excluded from the inventory total.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cumulative_lake_capture_kilograms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_lake_input_kilograms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_lake_water_kilograms: Option<f64>,
    pub cumulative_runoff_transfers: runoff_transport::Transfers,
    pub cumulative_surface_transfers: surface_water::Transfers,
    pub vapor_kilograms: f64,
    pub residual_kilograms: f64,
    pub cumulative_evaporation_kilograms: f64,
    pub cumulative_precipitation_kilograms: f64,
    /// Includes body identities in versions 8–11, leaf identities in 9–11,
    /// direct spill identities in 10/11 and parent identities in 11.
    pub maximum_relative_local_surface_ledger_residual: f64,
    pub vapor_ledger_residual_kilograms: f64,
}

#[derive(Debug)]
pub struct Step {
    /// Present in versions 10/11, and returned only after atomic acceptance.
    pub leaf_spill_events: Option<Vec<leaf_spill::Event>>,
    pub evaporation_kilograms: Vec<f64>,
    pub precipitation_kilograms: Vec<f64>,
    pub surface_transfers: Vec<surface_water::Transfers>,
    pub runoff_transfers: Vec<runoff_transport::Transfers>,
    pub transport_substeps: usize,
    pub coupled_substeps: usize,
    pub maximum_observed_vapor_column_kilograms_per_square_meter: f64,
    pub maximum_absolute_local_exchange_residual_kilograms: f64,
    pub maximum_absolute_routing_residual_kilograms: f64,
    pub budget: Budget,
}

/// Read-only diagnostic of a provisional local operation. Observations from a
/// rejected caller interval must not be mistaken for committed transfers.
#[derive(Clone, Copy, Debug)]
pub struct SurfaceObservation {
    pub coupled_start_seconds: u64,
    pub phase: usize,
    pub region: usize,
    pub local_seconds: f64,
    pub before: surface_water::Stocks,
    pub after: surface_water::Step,
    pub before_soil_low_kilograms: f64,
    pub after_soil_low_kilograms: f64,
    pub before_snow_low_kilograms: f64,
    pub after_snow_low_kilograms: f64,
    pub before_liquid_low_kilograms: f64,
    pub after_liquid_low_kilograms: f64,
}

/// Copied provisional diagnostics, not committed transfers in a rejected call.
#[derive(Clone, Copy, Debug)]
pub struct TerminalObservation {
    pub region: usize,
    pub before_kilograms: f64,
    pub before_low_kilograms: f64,
    pub after_kilograms: f64,
    pub after_low_kilograms: f64,
    pub received_kilograms: f64,
    pub evaporated_kilograms: f64,
}

impl Model {
    pub fn model_version(&self) -> &str {
        &self.origin.model_version
    }
    pub fn checkpoint_schema_version(&self) -> u32 {
        self.origin.schema_version
    }
    pub fn surface_model_version(&self) -> &str {
        &self.origin.surface_model_version
    }
    pub fn orographic_uplift(&self) -> Option<&[Vec<f64>]> {
        self.orographic_forcing
            .as_ref()
            .map(|f| f.uplift_meters_per_second.as_slice())
    }
    /// Read-only resolved configuration for adapters; display frames are not checkpoints.
    pub fn settings(&self) -> Settings {
        self.origin.settings
    }
    pub fn temperature_settings(&self) -> seasonal_temperature::Settings {
        self.origin.temperature_settings
    }
    pub fn wind_settings(&self) -> seasonal_wind::Settings {
        self.origin.wind_settings
    }
    /// Requires an unmodified generated world. Custom forcing/initial fields
    /// are not encoded by the recipe-based checkpoint and are unsupported.
    pub fn from_world(
        world: &World,
        settings: Settings,
        temperature_settings: seasonal_temperature::Settings,
        wind_settings: seasonal_wind::Settings,
    ) -> Result<Self, String> {
        settings.validate()?;
        let routing = runoff_transport::Network::from_surface(
            &world.surface,
            &world.drainage,
            settings.runoff,
        )?;
        if world.water.depth_meters.len() != world.surface.areas.len()
            || world
                .water
                .depth_meters
                .iter()
                .any(|v| !v.is_finite() || *v < 0.)
        {
            return Err("Invalid initial water for the mobile partition.".into());
        }
        let normals = seasonal_temperature::Normals::from_world(world, temperature_settings)?;
        let capacities = normals
            .monthly_temperature_celsius
            .iter()
            .map(|month| {
                month
                    .iter()
                    .zip(&world.surface.areas)
                    .map(|(&t, &area)| {
                        saturation_column_kilograms_per_square_meter(
                            t,
                            settings.effective_vapor_depth_meters,
                        )
                        .map(|q| q * area)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let geometry = Geometry::from_surface(&world.surface, world.recipe.radius_meters)?;
        let flows = (0..12)
            .map(|month| {
                Flow::seasonal_month(
                    &geometry,
                    month,
                    wind_settings,
                    temperature_settings.axial_tilt_degrees,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let orographic_forcing = if let Some(orography) = settings.orography {
            let heights: Vec<_> = world
                .terrain
                .elevation
                .iter()
                .enumerate()
                .map(|(i, &bed)| {
                    if world.water.depth_meters[i] > 0. {
                        world.water.level_meters
                    } else {
                        bed
                    }
                })
                .collect();
            let gradients = orographic_response::terrain_gradients(
                &world.surface,
                world.recipe.radius_meters,
                &heights,
            )?;
            let winds = seasonal_wind::Normals::from_world(
                world,
                wind_settings,
                temperature_settings.axial_tilt_degrees,
            )?;
            let mut uplifts = Vec::with_capacity(12);
            let mut rates = Vec::with_capacity(12);
            let mut maximum: f64 = 0.;
            for month in 0..12 {
                let mut uplift = Vec::with_capacity(gradients.len());
                let mut rate = Vec::with_capacity(gradients.len());
                for (i, &gradient) in gradients.iter().enumerate() {
                    let point = world.surface.centers[i];
                    let wind = seasonal_wind::tangent_vector(
                        point,
                        winds.monthly_east_meters_per_second[month][i],
                        winds.monthly_north_meters_per_second[month][i],
                    );
                    let w = orographic_response::uplift(point, gradient, wind)?;
                    let k = orography.additional_rate_per_second(w)?;
                    maximum = maximum.max(k);
                    uplift.push(w);
                    rate.push(k);
                }
                uplifts.push(uplift);
                rates.push(rate);
            }
            Some(OrographicForcing {
                uplift_meters_per_second: uplifts,
                additional_rates_per_second: rates,
                maximum_rate_per_second: maximum,
            })
        } else {
            None
        };
        let mut initial_surface: Vec<_> = world
            .surface
            .areas
            .iter()
            .zip(&world.water.depth_meters)
            .map(|(a, depth)| {
                a * depth.min(settings.initial_active_surface_depth_meters)
                    * WATER_DENSITY_KILOGRAMS_PER_CUBIC_METER
            })
            .collect();
        let n = initial_surface.len();
        let initial_total = total_mass(&initial_surface);
        if !initial_total.is_finite() {
            return Err("Initial mobile-water overflow.".into());
        }
        let reference_pool = settings
            .reference_water_pool
            .map(|_| reference_pool::Layout::new(&world.water.body_ids));
        let lake_exchange = settings
            .closed_lake_exchange
            .map(|mode| lake_exchange::Layout::from_world(world, mode))
            .transpose()?;
        let with_merge = settings.closed_lake_exchange
            == Some(ClosedLakeExchange::FrozenLeafExposureWithSpillAndMerge);
        let with_spill = with_merge
            || settings.closed_lake_exchange
                == Some(ClosedLakeExchange::FrozenLeafExposureWithSpill);
        let mut body_high = Vec::new();
        let mut body_low = Vec::new();
        if let Some(layout) = &reference_pool {
            for members in &layout.members {
                let mut stock = surface_water::CompensatedStock::new(0., 0., f64::MAX)?;
                for &i in members {
                    if world.water.depth_meters[i] <= 0. {
                        return Err("Reference-body membership disagrees with the wet mask.".into());
                    }
                    stock.credit(initial_surface[i])?;
                    initial_surface[i] = 0.;
                }
                body_high.push(stock.high);
                body_low.push(stock.low);
            }
            if world
                .water
                .depth_meters
                .iter()
                .enumerate()
                .any(|(i, d)| (*d > 0.) != layout.by_region[i].is_some())
            {
                return Err("Reference-body membership does not partition reference water.".into());
            }
        }
        let origin = Checkpoint {
            schema_version: if with_merge {
                11
            } else if with_spill {
                10
            } else if lake_exchange.is_some() {
                9
            } else if reference_pool.is_some() {
                8
            } else if settings.terminal_numerics.is_some() {
                7
            } else if settings.surface_numerics.is_some() {
                6
            } else if settings.soil_numerics.is_some() {
                5
            } else if settings.orography.is_some() {
                4
            } else {
                3
            },
            model_version: if with_merge {
                MERGED_LAKE_MODEL_VERSION
            } else if with_spill {
                LEAF_SPILL_MODEL_VERSION
            } else if lake_exchange.is_some() {
                CLOSED_LAKE_MODEL_VERSION
            } else if reference_pool.is_some() {
                REFERENCE_POOL_MODEL_VERSION
            } else if settings.terminal_numerics.is_some() {
                TERMINAL_PRECISION_MODEL_VERSION
            } else if settings.surface_numerics.is_some() {
                SURFACE_PRECISION_MODEL_VERSION
            } else if settings.soil_numerics.is_some() {
                SOIL_PRECISION_MODEL_VERSION
            } else if settings.orography.is_some() {
                OROGRAPHIC_MODEL_VERSION
            } else {
                MODEL_VERSION
            }
            .into(),
            transport_model_version: crate::moisture_transport::MODEL_VERSION.into(),
            temperature_model_version: seasonal_temperature::MODEL_VERSION.into(),
            wind_model_version: seasonal_wind::MODEL_VERSION.into(),
            surface_model_version: if settings.surface_numerics.is_some() {
                surface_water::PRECISE_SURFACE_MODEL_VERSION
            } else if settings.soil_numerics.is_some() {
                surface_water::COMPENSATED_MODEL_VERSION
            } else {
                surface_water::MODEL_VERSION
            }
            .into(),
            runoff_model_version: runoff_transport::MODEL_VERSION.into(),
            orographic_model_version: settings
                .orography
                .map(|_| orographic_response::MODEL_VERSION.into()),
            recipe: world.recipe.clone(),
            settings,
            temperature_settings,
            wind_settings,
            elapsed_seconds: 0,
            surface_kilograms: initial_surface.clone(),
            snow_kilograms: vec![0.; n],
            soil_kilograms: vec![0.; n],
            soil_low_kilograms: settings.soil_numerics.map(|_| vec![0.; n]),
            snow_low_kilograms: settings.surface_numerics.map(|_| vec![0.; n]),
            surface_low_kilograms: settings.surface_numerics.map(|_| vec![0.; n]),
            terminal_low_kilograms: settings.terminal_numerics.map(|_| vec![0.; n]),
            terminal_stock_model_version: settings
                .terminal_numerics
                .map(|_| TERMINAL_STOCK_MODEL_VERSION.into()),
            reference_body_model_version: reference_pool
                .as_ref()
                .map(|_| reference_pool::MODEL_VERSION.into()),
            reference_body_high_kilograms: reference_pool.as_ref().map(|_| body_high),
            reference_body_low_kilograms: reference_pool.as_ref().map(|_| body_low),
            closed_lake_model_version: lake_exchange.as_ref().map(|_| {
                if with_merge {
                    lake_exchange::MERGE_MODEL_VERSION
                } else if with_spill {
                    lake_exchange::SPILL_MODEL_VERSION
                } else {
                    lake_exchange::MODEL_VERSION
                }
                .into()
            }),
            cumulative_lake_capture_kilograms: lake_exchange.as_ref().map(|_| vec![0.; n]),
            cumulative_lake_capture_low_kilograms: lake_exchange.as_ref().map(|_| vec![0.; n]),
            leaf_spill_state: with_spill.then(|| leaf_spill::Checkpoint::zero(n)),
            merged_lake_state: with_merge.then(merged_lake::Checkpoint::empty),
            pending_runoff_kilograms: vec![0.; n],
            terminal_water_kilograms: vec![0.; n],
            cumulative_runoff_transfers: vec![runoff_transport::Transfers::default(); n],
            cumulative_runoff_transfer_roundoff: vec![[0.; 4]; n],
            cumulative_surface_transfers: vec![surface_water::Transfers::default(); n],
            cumulative_surface_transfer_roundoff: vec![[0.; 8]; n],
            vapor_kilograms: vec![0.; n],
            cumulative_evaporation_kilograms: vec![0.; n],
            cumulative_precipitation_kilograms: vec![0.; n],
        };
        Ok(Self {
            origin,
            areas: world.surface.areas.clone(),
            is_land: world.water.depth_meters.iter().map(|d| *d == 0.).collect(),
            reference_body_ids: world.water.body_ids.clone(),
            reference_pool,
            lake_exchange,
            routing,
            initial_surface,
            initial_total,
            temperatures: normals.monthly_temperature_celsius,
            capacities,
            flows,
            orographic_forcing,
        })
    }

    pub fn initial_state(&self) -> State {
        State(self.origin.clone())
    }

    pub fn restore(checkpoint: Checkpoint) -> Result<(Self, State), String> {
        let supported = matches!(
            (
                checkpoint.schema_version,
                checkpoint.model_version.as_str(),
                checkpoint.settings.orography,
                checkpoint.orographic_model_version.as_deref(),
            ),
            (3, MODEL_VERSION, None, None)
                | (
                    4,
                    OROGRAPHIC_MODEL_VERSION,
                    Some(_),
                    Some(orographic_response::MODEL_VERSION)
                )
        );
        let compensated = checkpoint.schema_version == 5
            && checkpoint.model_version == SOIL_PRECISION_MODEL_VERSION
            && checkpoint.settings.orography.is_some()
            && checkpoint.settings.soil_numerics == Some(SoilNumerics::Compensated)
            && checkpoint.orographic_model_version.as_deref()
                == Some(orographic_response::MODEL_VERSION)
            && checkpoint.soil_low_kilograms.is_some()
            && checkpoint.settings.surface_numerics.is_none()
            && checkpoint.snow_low_kilograms.is_none()
            && checkpoint.surface_low_kilograms.is_none();
        let precise_surface = checkpoint.schema_version == 6
            && checkpoint.model_version == SURFACE_PRECISION_MODEL_VERSION
            && checkpoint.settings.orography.is_some()
            && checkpoint.settings.soil_numerics == Some(SoilNumerics::Compensated)
            && checkpoint.settings.surface_numerics == Some(SurfaceNumerics::Compensated)
            && checkpoint.orographic_model_version.as_deref()
                == Some(orographic_response::MODEL_VERSION)
            && checkpoint.soil_low_kilograms.is_some()
            && checkpoint.snow_low_kilograms.is_some()
            && checkpoint.surface_low_kilograms.is_some();
        let legacy = supported
            && checkpoint.settings.soil_numerics.is_none()
            && checkpoint.soil_low_kilograms.is_none()
            && checkpoint.settings.surface_numerics.is_none()
            && checkpoint.snow_low_kilograms.is_none()
            && checkpoint.surface_low_kilograms.is_none();
        let legacy_lake = checkpoint.settings.closed_lake_exchange.is_none()
            && checkpoint.merged_lake_state.is_none()
            && checkpoint.leaf_spill_state.is_none()
            && checkpoint.closed_lake_model_version.is_none()
            && checkpoint.cumulative_lake_capture_kilograms.is_none()
            && checkpoint.cumulative_lake_capture_low_kilograms.is_none();
        let coupled_lake = checkpoint.schema_version == 9
            && checkpoint.merged_lake_state.is_none()
            && checkpoint.leaf_spill_state.is_none()
            && checkpoint.model_version == CLOSED_LAKE_MODEL_VERSION
            && checkpoint.settings.closed_lake_exchange
                == Some(ClosedLakeExchange::FrozenLeafExposure)
            && checkpoint.closed_lake_model_version.as_deref()
                == Some(lake_exchange::MODEL_VERSION)
            && checkpoint.cumulative_lake_capture_kilograms.is_some()
            && checkpoint.cumulative_lake_capture_low_kilograms.is_some();
        let spilling_lake = checkpoint.schema_version == 10
            && checkpoint.merged_lake_state.is_none()
            && checkpoint.model_version == LEAF_SPILL_MODEL_VERSION
            && checkpoint.settings.closed_lake_exchange
                == Some(ClosedLakeExchange::FrozenLeafExposureWithSpill)
            && checkpoint.closed_lake_model_version.as_deref()
                == Some(lake_exchange::SPILL_MODEL_VERSION)
            && checkpoint
                .leaf_spill_state
                .as_ref()
                .is_some_and(|s| s.model_version == leaf_spill::MODEL_VERSION)
            && checkpoint.cumulative_lake_capture_kilograms.is_some()
            && checkpoint.cumulative_lake_capture_low_kilograms.is_some();
        let merging_lake = checkpoint.schema_version == 11
            && checkpoint.model_version == MERGED_LAKE_MODEL_VERSION
            && checkpoint.settings.closed_lake_exchange
                == Some(ClosedLakeExchange::FrozenLeafExposureWithSpillAndMerge)
            && checkpoint.closed_lake_model_version.as_deref()
                == Some(lake_exchange::MERGE_MODEL_VERSION)
            && checkpoint
                .leaf_spill_state
                .as_ref()
                .is_some_and(|s| s.model_version == leaf_spill::MODEL_VERSION)
            && checkpoint
                .merged_lake_state
                .as_ref()
                .is_some_and(|s| s.model_version == merged_lake::MODEL_VERSION)
            && checkpoint.cumulative_lake_capture_kilograms.is_some()
            && checkpoint.cumulative_lake_capture_low_kilograms.is_some();
        let pooled = ((checkpoint.schema_version == 8
            && checkpoint.model_version == REFERENCE_POOL_MODEL_VERSION
            && legacy_lake)
            || coupled_lake
            || spilling_lake
            || merging_lake)
            && checkpoint.settings.reference_water_pool
                == Some(ReferenceWaterPool::FastConnectedBody)
            && checkpoint.reference_body_model_version.as_deref()
                == Some(reference_pool::MODEL_VERSION)
            && checkpoint.reference_body_high_kilograms.is_some()
            && checkpoint.reference_body_low_kilograms.is_some();
        let legacy_pool = checkpoint.settings.reference_water_pool.is_none()
            && checkpoint.reference_body_model_version.is_none()
            && checkpoint.reference_body_high_kilograms.is_none()
            && checkpoint.reference_body_low_kilograms.is_none();
        let precise_terminal = ((checkpoint.schema_version == 7
            && checkpoint.model_version == TERMINAL_PRECISION_MODEL_VERSION
            && legacy_pool)
            || pooled)
            && checkpoint.settings.orography.is_some()
            && checkpoint.settings.soil_numerics == Some(SoilNumerics::Compensated)
            && checkpoint.settings.surface_numerics == Some(SurfaceNumerics::Compensated)
            && checkpoint.settings.terminal_numerics == Some(TerminalNumerics::Compensated)
            && checkpoint.orographic_model_version.as_deref()
                == Some(orographic_response::MODEL_VERSION)
            && checkpoint.soil_low_kilograms.is_some()
            && checkpoint.snow_low_kilograms.is_some()
            && checkpoint.surface_low_kilograms.is_some()
            && checkpoint.terminal_low_kilograms.is_some()
            && checkpoint.terminal_stock_model_version.as_deref()
                == Some(TERMINAL_STOCK_MODEL_VERSION);
        let legacy_terminal = checkpoint.settings.terminal_numerics.is_none()
            && checkpoint.terminal_low_kilograms.is_none()
            && checkpoint.terminal_stock_model_version.is_none();
        let surface_version = if precise_surface || precise_terminal {
            surface_water::PRECISE_SURFACE_MODEL_VERSION
        } else if compensated {
            surface_water::COMPENSATED_MODEL_VERSION
        } else {
            surface_water::MODEL_VERSION
        };
        if !(((legacy || compensated || precise_surface) && legacy_terminal) || precise_terminal)
            || (!pooled && !legacy_pool)
            || (!coupled_lake && !spilling_lake && !merging_lake && !legacy_lake)
            || checkpoint.transport_model_version != crate::moisture_transport::MODEL_VERSION
            || checkpoint.temperature_model_version != seasonal_temperature::MODEL_VERSION
            || checkpoint.wind_model_version != seasonal_wind::MODEL_VERSION
            || checkpoint.surface_model_version != surface_version
            || checkpoint.runoff_model_version != runoff_transport::MODEL_VERSION
        {
            return Err("Unsupported seasonal-moisture checkpoint version.".into());
        }
        let world = World::generate(checkpoint.recipe.clone())?;
        let model = Self::from_world(
            &world,
            checkpoint.settings,
            checkpoint.temperature_settings,
            checkpoint.wind_settings,
        )?;
        model.validate_checkpoint(&checkpoint)?;
        Ok((model, State(checkpoint)))
    }

    fn validate_checkpoint(&self, cp: &Checkpoint) -> Result<Budget, String> {
        let origin = &self.origin;
        if cp.schema_version != origin.schema_version
            || cp.model_version != origin.model_version
            || cp.transport_model_version != origin.transport_model_version
            || cp.temperature_model_version != origin.temperature_model_version
            || cp.wind_model_version != origin.wind_model_version
            || cp.surface_model_version != origin.surface_model_version
            || cp.terminal_stock_model_version != origin.terminal_stock_model_version
            || cp.reference_body_model_version != origin.reference_body_model_version
            || cp.closed_lake_model_version != origin.closed_lake_model_version
            || cp.runoff_model_version != origin.runoff_model_version
            || cp.orographic_model_version != origin.orographic_model_version
            || cp.recipe != origin.recipe
            || cp.settings != origin.settings
            || cp.temperature_settings != origin.temperature_settings
            || cp.wind_settings != origin.wind_settings
            || cp.elapsed_seconds > MAX_ELAPSED_SECONDS
        {
            return Err("Moisture state does not match the model or clock range.".into());
        }
        for values in [
            &cp.surface_kilograms,
            &cp.snow_kilograms,
            &cp.soil_kilograms,
            &cp.pending_runoff_kilograms,
            &cp.terminal_water_kilograms,
            &cp.vapor_kilograms,
            &cp.cumulative_evaporation_kilograms,
            &cp.cumulative_precipitation_kilograms,
        ] {
            if values.len() != self.areas.len() || values.iter().any(|v| !v.is_finite() || *v < 0.)
            {
                return Err("Invalid seasonal-moisture checkpoint stock or ledger.".into());
            }
        }
        match (&cp.soil_low_kilograms, cp.settings.soil_numerics) {
            (None, None) => {}
            (Some(low), Some(SoilNumerics::Compensated)) if low.len() == self.areas.len() => {
                for (i, &value) in low.iter().enumerate() {
                    surface_water::validate_stock_precision(
                        cp.soil_kilograms[i],
                        value,
                        self.areas[i]
                            * cp.settings.surface.soil_capacity_kilograms_per_square_meter,
                    )?;
                    if !self.is_land[i] && value != 0. {
                        return Err("Soil low component on reference water.".into());
                    }
                }
            }
            _ => return Err("Invalid compensated-soil checkpoint shape or mode.".into()),
        }
        match (&cp.snow_low_kilograms, cp.settings.surface_numerics) {
            (None, None) => {}
            (Some(low), Some(SurfaceNumerics::Compensated)) if low.len() == self.areas.len() => {
                for (i, &value) in low.iter().enumerate() {
                    surface_water::validate_stock_precision(cp.snow_kilograms[i], value, f64::MAX)?;
                }
            }
            _ => return Err("Invalid compensated-snow checkpoint shape or mode.".into()),
        }
        match (&cp.surface_low_kilograms, cp.settings.surface_numerics) {
            (None, None) => {}
            (Some(low), Some(SurfaceNumerics::Compensated)) if low.len() == self.areas.len() => {
                for (i, &value) in low.iter().enumerate() {
                    surface_water::validate_stock_precision(
                        cp.surface_kilograms[i],
                        value,
                        f64::MAX,
                    )?;
                    if cp.elapsed_seconds == 0 && value != 0. {
                        return Err("Noninitial liquid low component at day zero.".into());
                    }
                }
            }
            _ => return Err("Invalid compensated-liquid checkpoint shape or mode.".into()),
        }
        match (&cp.terminal_low_kilograms, cp.settings.terminal_numerics) {
            (None, None) => {}
            (Some(low), Some(TerminalNumerics::Compensated)) if low.len() == self.areas.len() => {
                for (i, &value) in low.iter().enumerate() {
                    surface_water::validate_stock_precision(
                        cp.terminal_water_kilograms[i],
                        value,
                        f64::MAX,
                    )?;
                }
            }
            _ => return Err("Invalid compensated-terminal checkpoint shape or mode.".into()),
        }
        match (
            &self.reference_pool,
            &cp.reference_body_high_kilograms,
            &cp.reference_body_low_kilograms,
        ) {
            (None, None, None) => {}
            (Some(layout), Some(high), Some(low))
                if high.len() == layout.ids.len() && low.len() == layout.ids.len() =>
            {
                for (&high, &low) in high.iter().zip(low) {
                    surface_water::CompensatedStock::new(high, low, f64::MAX)?;
                }
                if cp.elapsed_seconds == 0
                    && (cp.reference_body_high_kilograms != origin.reference_body_high_kilograms
                        || cp.reference_body_low_kilograms != origin.reference_body_low_kilograms)
                {
                    return Err("Noninitial reference-body stock at day zero.".into());
                }
                for i in 0..self.areas.len() {
                    if !self.is_land[i]
                        && (cp.surface_kilograms[i] != 0.
                            || cp.surface_low_kilograms.as_ref().unwrap()[i] != 0.
                            || cp.terminal_water_kilograms[i] != 0.
                            || cp.terminal_low_kilograms.as_ref().unwrap()[i] != 0.)
                    {
                        return Err("Reference-body water has a duplicate regional owner.".into());
                    }
                }
            }
            _ => return Err("Invalid reference-body checkpoint shape or mode.".into()),
        }
        if cp.cumulative_surface_transfers.len() != self.areas.len()
            || cp.cumulative_surface_transfer_roundoff.len() != self.areas.len()
            || cp
                .cumulative_surface_transfers
                .iter()
                .any(|f| f.values().iter().any(|v| !v.is_finite() || *v < 0.))
        {
            return Err("Invalid surface-transfer ledger.".into());
        }
        if cp.cumulative_runoff_transfers.len() != self.areas.len()
            || cp.cumulative_runoff_transfer_roundoff.len() != self.areas.len()
        {
            return Err("Invalid runoff-transfer ledger length.".into());
        }
        let lake_ledger = match &self.lake_exchange {
            Some(layout) => {
                let graph = match (&layout.spill, &cp.leaf_spill_state) {
                    (Some(spill), Some(_)) => {
                        spill.validate(cp, self.reference_pool.as_ref().unwrap())?
                    }
                    (None, None) => (0., 0),
                    _ => return Err("Leaf spill accounting outside its pinned mode.".into()),
                };
                let lake = layout.validate(cp)?;
                Some(if graph.0 > lake.0 { graph } else { lake })
            }
            None if cp.cumulative_lake_capture_kilograms.is_none()
                && cp.cumulative_lake_capture_low_kilograms.is_none()
                && cp.leaf_spill_state.is_none() =>
            {
                None
            }
            None => return Err("Lake capture ledger outside the coupled lake model.".into()),
        };
        for (totals, corrections) in cp
            .cumulative_runoff_transfers
            .iter()
            .zip(&cp.cumulative_runoff_transfer_roundoff)
        {
            if totals.values().iter().any(|v| !v.is_finite() || *v < 0.)
                || totals
                    .values()
                    .into_iter()
                    .zip(corrections)
                    .any(|(total, c)| !c.is_finite() || c.abs() > 4. * f64::EPSILON * total.max(1.))
            {
                return Err("Invalid runoff-transfer stock or summation roundoff.".into());
            }
        }
        for (totals, corrections) in cp
            .cumulative_surface_transfers
            .iter()
            .zip(&cp.cumulative_surface_transfer_roundoff)
        {
            if totals
                .values()
                .into_iter()
                .zip(corrections)
                .any(|(total, c)| !c.is_finite() || c.abs() > 4. * f64::EPSILON * total.max(1.))
            {
                return Err("Invalid surface-transfer summation roundoff.".into());
            }
        }
        if cp.elapsed_seconds == 0
            && (cp.surface_kilograms != self.initial_surface
                || cp
                    .vapor_kilograms
                    .iter()
                    .chain(&cp.cumulative_evaporation_kilograms)
                    .chain(&cp.cumulative_precipitation_kilograms)
                    .chain(&cp.snow_kilograms)
                    .chain(&cp.soil_kilograms)
                    .chain(&cp.pending_runoff_kilograms)
                    .chain(&cp.terminal_water_kilograms)
                    .any(|v| *v != 0.)
                || cp
                    .cumulative_surface_transfers
                    .iter()
                    .any(|f| f.values().iter().any(|v| *v != 0.)))
        {
            return Err("Noninitial moisture stock at day zero.".into());
        }
        if cp.elapsed_seconds == 0
            && cp
                .cumulative_surface_transfer_roundoff
                .iter()
                .flatten()
                .any(|v| *v != 0.)
        {
            return Err("Noninitial surface-transfer roundoff at day zero.".into());
        }
        let lake_exchange_active = self.lake_exchange.is_some() && cp.elapsed_seconds > 0;
        if (cp.elapsed_seconds == 0 || !cp.settings.routing_enabled)
            && (cp.cumulative_runoff_transfers.iter().any(|f| {
                f.values()[..if lake_exchange_active { 3 } else { 4 }]
                    .iter()
                    .any(|v| *v != 0.)
            }) || cp
                .cumulative_runoff_transfer_roundoff
                .iter()
                .flat_map(|c| &c[..if lake_exchange_active { 3 } else { 4 }])
                .any(|v| *v != 0.)
                || (!lake_exchange_active && cp.terminal_water_kilograms.iter().any(|v| *v != 0.)))
        {
            return Err(
                "Runoff transfers exist before initialization or while routing is disabled.".into(),
            );
        }
        let mut expected_received = vec![0.; self.areas.len()];
        let mut expected_delivery = vec![0.; self.areas.len()];
        for (i, f) in cp.cumulative_runoff_transfers.iter().enumerate() {
            let r = self.routing.receivers()[i] as usize;
            if self.routing.is_terminal(r) {
                expected_delivery[r] += f.sent;
            } else {
                expected_received[r] += f.sent;
            }
        }
        let mut local_residual: f64 = 0.;
        let mut local_witness = (0, 0);
        for i in 0..self.areas.len() {
            let stocks = surface_water::Stocks {
                liquid: cp.surface_kilograms[i],
                snow: cp.snow_kilograms[i],
                soil: cp.soil_kilograms[i],
                pending_runoff: cp.pending_runoff_kilograms[i],
            };
            stocks.validate(self.areas[i], self.is_land[i], cp.settings.surface)?;
            let f = cp.cumulative_surface_transfers[i];
            let route = cp.cumulative_runoff_transfers[i];
            let terminal = cp.terminal_water_kilograms[i];
            let pooled_region = self.reference_pool.is_some() && !self.is_land[i];
            let lake_region = self
                .lake_exchange
                .as_ref()
                .is_some_and(|layout| layout.owns(cp, i));
            if pooled_region && route.terminal_evaporation != 0. {
                return Err(
                    "Body-owned evaporation recorded as regional terminal evaporation.".into(),
                );
            }
            if (!self.routing.is_terminal(i)
                && (terminal != 0.
                    || route.terminal_delivery != 0.
                    || (!lake_region && route.terminal_evaporation != 0.)))
                || (self.routing.is_terminal(i) && route.received_transit != 0.)
            {
                return Err("Runoff terminal stock or transfer has the wrong recipient.".into());
            }
            if cp.settings.routing_enabled
                && self.routing.is_terminal(i)
                && stocks.pending_runoff != 0.
            {
                return Err(
                    "Terminal runoff has not been transferred to its recipient store.".into(),
                );
            }
            if !self.is_land[i]
                && (f.infiltration != 0.
                    || f.soil_evaporation != 0.
                    || f.soil_drainage != 0.
                    || f.liquid_runoff != 0.)
            {
                return Err("Land transfers recorded on a reference-water region.".into());
            }
            let scale = self.initial_surface[i]
                .max(stocks.total())
                .max(cp.cumulative_evaporation_kilograms[i])
                .max(cp.cumulative_precipitation_kilograms[i])
                .max(f.values().into_iter().fold(0., f64::max))
                .max(terminal)
                .max(route.values().into_iter().fold(0., f64::max))
                .max(
                    cp.cumulative_lake_capture_kilograms
                        .as_ref()
                        .map_or(0., |v| v[i]),
                )
                .max(1.);
            for (ledger, residual) in [
                cp.surface_low_kilograms.as_ref().map_or(
                    (stocks.liquid - self.initial_surface[i]) - f.rain - f.melt
                        + f.liquid_evaporation
                        + f.infiltration
                        + f.liquid_runoff,
                    |low| {
                        let terms = [
                            stocks.liquid,
                            low[i],
                            -self.initial_surface[i],
                            -f.rain,
                            -f.melt,
                            f.liquid_evaporation,
                            f.infiltration,
                            f.liquid_runoff,
                        ];
                        if let Some(capture) = &cp.cumulative_lake_capture_kilograms {
                            let mut captured = terms.to_vec();
                            captured.extend([
                                capture[i],
                                cp.cumulative_lake_capture_low_kilograms.as_ref().unwrap()[i],
                            ]);
                            total_mass(&captured)
                        } else {
                            total_mass(&terms)
                        }
                    },
                ),
                cp.snow_low_kilograms
                    .as_ref()
                    .map_or(stocks.snow - (f.snowfall - f.melt), |low| {
                        total_mass(&[stocks.snow, low[i], -f.snowfall, f.melt])
                    }),
                cp.soil_low_kilograms.as_ref().map_or(
                    stocks.soil - f.infiltration + f.soil_evaporation + f.soil_drainage,
                    |low| {
                        total_mass(&[
                            stocks.soil,
                            low[i],
                            -f.infiltration,
                            f.soil_evaporation,
                            f.soil_drainage,
                        ])
                    },
                ),
                stocks.pending_runoff - f.liquid_runoff - f.soil_drainage - route.received_transit
                    + route.sent,
                cp.cumulative_evaporation_kilograms[i]
                    - (f.liquid_evaporation + f.soil_evaporation + route.terminal_evaporation),
                cp.cumulative_precipitation_kilograms[i] - (f.rain + f.snowfall),
                cp.terminal_low_kilograms.as_ref().map_or(
                    terminal - (route.terminal_delivery - route.terminal_evaporation),
                    |low| {
                        total_mass(&[
                            terminal,
                            low[i],
                            -route.terminal_delivery,
                            route.terminal_evaporation,
                        ])
                    },
                ),
                route.received_transit - expected_received[i],
                route.terminal_delivery - expected_delivery[i],
            ]
            .into_iter()
            .enumerate()
            {
                if (pooled_region && (ledger == 0 || ledger == 6)) || (lake_region && ledger == 6) {
                    // Body identity replaces only the two displaced local owners.
                    continue;
                }
                if !residual.is_finite() {
                    return Err("Nonfinite typed water ledger.".into());
                }
                if residual.abs() / scale > local_residual {
                    local_residual = residual.abs() / scale;
                    local_witness = (i, ledger);
                }
            }
        }
        if let Some((residual, region)) = lake_ledger
            && residual > local_residual
        {
            local_residual = residual;
            local_witness = (region, 10);
        }
        let body_water = if let Some(layout) = &self.reference_pool {
            let high = cp.reference_body_high_kilograms.as_ref().unwrap();
            let low = cp.reference_body_low_kilograms.as_ref().unwrap();
            for (b, members) in layout.members.iter().enumerate() {
                let initial_high = origin.reference_body_high_kilograms.as_ref().unwrap()[b];
                let initial_low = origin.reference_body_low_kilograms.as_ref().unwrap()[b];
                let mut terms = vec![high[b], low[b], -initial_high, -initial_low];
                let mut flows = Vec::with_capacity(4 * members.len());
                for &i in members {
                    let f = cp.cumulative_surface_transfers[i];
                    let arrival = cp.cumulative_runoff_transfers[i].terminal_delivery;
                    terms.extend([-f.rain, -f.melt, -arrival, f.liquid_evaporation]);
                    flows.extend([f.rain, f.melt, arrival, f.liquid_evaporation]);
                    if let Some(spill) = &cp.leaf_spill_state {
                        let incoming = spill.cumulative_incoming.stock(i)?;
                        terms.extend([-incoming.high, -incoming.low]);
                        flows.extend([incoming.high, incoming.low]);
                    }
                }
                let residual = total_mass(&terms);
                let scale = initial_high.max(high[b]).max(total_mass(&flows)).max(1.);
                if !residual.is_finite() || !scale.is_finite() {
                    return Err("Nonfinite reference-body ledger.".into());
                }
                if residual.abs() / scale > local_residual {
                    local_residual = residual.abs() / scale;
                    local_witness = (members[0], 9);
                }
            }
            Some(total_mass(
                &high.iter().chain(low).copied().collect::<Vec<_>>(),
            ))
        } else {
            None
        };
        let surface = if let Some(low) = &cp.surface_low_kilograms {
            total_mass(
                &cp.surface_kilograms
                    .iter()
                    .chain(low)
                    .copied()
                    .collect::<Vec<_>>(),
            )
        } else {
            total_mass(&cp.surface_kilograms)
        };
        let snow = if let Some(low) = &cp.snow_low_kilograms {
            total_mass(
                &cp.snow_kilograms
                    .iter()
                    .chain(low)
                    .copied()
                    .collect::<Vec<_>>(),
            )
        } else {
            total_mass(&cp.snow_kilograms)
        };
        let soil = if let Some(low) = &cp.soil_low_kilograms {
            total_mass(
                &cp.soil_kilograms
                    .iter()
                    .chain(low)
                    .copied()
                    .collect::<Vec<_>>(),
            )
        } else {
            total_mass(&cp.soil_kilograms)
        };
        let runoff = total_mass(&cp.pending_runoff_kilograms);
        let terminal = if let Some(low) = &cp.terminal_low_kilograms {
            total_mass(
                &cp.terminal_water_kilograms
                    .iter()
                    .chain(low)
                    .copied()
                    .collect::<Vec<_>>(),
            )
        } else {
            total_mass(&cp.terminal_water_kilograms)
        };
        let vapor = total_mass(&cp.vapor_kilograms);
        let evaporation = total_mass(&cp.cumulative_evaporation_kilograms);
        let precipitation = total_mass(&cp.cumulative_precipitation_kilograms);
        let pending_lake = cp
            .leaf_spill_state
            .as_ref()
            .map(|s| s.pending_input.total());
        let merged_water = cp
            .merged_lake_state
            .as_ref()
            .map(merged_lake::Checkpoint::total);
        let residual = if let Some(merged) = merged_water {
            total_mass(&[
                surface,
                snow,
                soil,
                runoff,
                terminal,
                vapor,
                body_water.unwrap(),
                pending_lake.unwrap(),
                merged,
            ]) - self.initial_total
        } else if let Some(pending) = pending_lake {
            total_mass(&[
                surface,
                snow,
                soil,
                runoff,
                terminal,
                vapor,
                body_water.unwrap(),
                pending,
            ]) - self.initial_total
        } else if let Some(body) = body_water {
            total_mass(&[surface, snow, soil, runoff, terminal, vapor, body]) - self.initial_total
        } else {
            total_mass(&[surface, snow, soil, runoff, terminal, vapor]) - self.initial_total
        };
        let vapor_residual = vapor - (evaporation - precipitation);
        if [
            surface,
            snow,
            soil,
            runoff,
            terminal,
            vapor,
            evaporation,
            precipitation,
            residual,
            vapor_residual,
            local_residual,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || residual.abs() > 1e-12 * self.initial_total.max(1.)
            || vapor_residual.abs() > 1e-12 * evaporation.max(precipitation).max(vapor).max(1.)
            || local_residual > 1e-12
        {
            return Err(format!(
                "Seasonal-moisture stock or exchange ledger is inconsistent at second {}: global relative {}, local relative {} (region {}, ledger {}), vapor relative {}.",
                cp.elapsed_seconds,
                residual.abs() / self.initial_total.max(1.),
                local_residual,
                local_witness.0,
                local_witness.1,
                vapor_residual.abs() / evaporation.max(precipitation).max(vapor).max(1.)
            ));
        }
        Ok(Budget {
            initial_mobile_water_kilograms: self.initial_total,
            surface_kilograms: surface,
            snow_kilograms: snow,
            soil_kilograms: soil,
            pending_runoff_kilograms: runoff,
            terminal_water_kilograms: terminal,
            reference_body_water_kilograms: body_water,
            pending_lake_input_kilograms: pending_lake,
            merged_lake_water_kilograms: merged_water,
            cumulative_lake_capture_kilograms: cp.cumulative_lake_capture_kilograms.as_ref().map(
                |high| {
                    total_mass(
                        &high
                            .iter()
                            .chain(cp.cumulative_lake_capture_low_kilograms.as_ref().unwrap())
                            .copied()
                            .collect::<Vec<_>>(),
                    )
                },
            ),
            cumulative_runoff_transfers: total_runoff_transfers(&cp.cumulative_runoff_transfers),
            cumulative_surface_transfers: total_transfers(&cp.cumulative_surface_transfers),
            vapor_kilograms: vapor,
            residual_kilograms: residual,
            cumulative_evaporation_kilograms: evaporation,
            cumulative_precipitation_kilograms: precipitation,
            maximum_relative_local_surface_ledger_residual: local_residual,
            vapor_ledger_residual_kilograms: vapor_residual,
        })
    }

    pub fn budget(&self, state: &State) -> Result<Budget, String> {
        self.validate_checkpoint(&state.0)
    }

    /// Eligible geometry only, not a claim that these parents are active lakes.
    pub fn merge_candidates(&self) -> Option<Vec<merged_lake::Candidate>> {
        self.lake_exchange
            .as_ref()?
            .merge
            .as_ref()
            .map(|m| m.candidates())
    }

    /// Read-only active parent surfaces under this exact model and checkpoint.
    pub fn merged_lake_surfaces(&self, state: &State) -> Result<Vec<merged_lake::Surface>, String> {
        self.budget(state)?;
        self.lake_exchange
            .as_ref()
            .and_then(|l| l.merge.as_ref())
            .ok_or("Merged lake inspection requires seasonal model 11.")?
            .surfaces(&state.0)
    }

    /// Actual response/routing bound, including any pinned upslope response.
    /// Caller intervals shorter than this bound provide temporal refinement.
    pub fn maximum_coupled_step_seconds(&self) -> Result<u64, String> {
        let settings = self.origin.settings;
        let mut coupled_limit = u64::from(settings.max_coupled_step_seconds);
        for (enabled, tau) in [
            (
                settings.evaporation_enabled,
                settings.evaporation_response_seconds,
            ),
            (
                settings.precipitation_enabled,
                settings.precipitation_response_seconds,
            ),
            (true, settings.surface.infiltration_response_seconds),
            (true, settings.surface.liquid_runoff_response_seconds),
            (true, settings.surface.soil_drainage_response_seconds),
        ] {
            if enabled {
                coupled_limit = coupled_limit.min((tau / 6.).floor() as u64);
            }
        }
        if settings.routing_enabled {
            coupled_limit = coupled_limit.min(self.routing.maximum_coupled_step_seconds());
        }
        if settings.precipitation_enabled
            && let Some(forcing) = &self.orographic_forcing
            && forcing.maximum_rate_per_second > 0.
        {
            let limit = (1.
                / (6.
                    * (1. / settings.precipitation_response_seconds
                        + forcing.maximum_rate_per_second)))
                .floor();
            coupled_limit = coupled_limit.min(limit as u64);
            // Canonical hour divisors preserve default hourly/daily batching.
            coupled_limit = (1..=3600)
                .rev()
                .find(|&dt| 3600 % dt == 0 && dt <= coupled_limit)
                .ok_or("Orographic response requires unsupported sub-second coupling.")?;
        }
        Ok(coupled_limit)
    }

    pub fn advance(&self, state: &mut State, seconds: u32) -> Result<Step, String> {
        self.advance_with_observers(state, seconds, |_| {}, |_| {})
    }

    /// Versions 3–7 use the same transactional path as `advance`; the observer
    /// receives copied provisional diagnostics. Body-owned versions 8/9 reject.
    pub fn advance_observed(
        &self,
        state: &mut State,
        seconds: u32,
        observe: impl FnMut(SurfaceObservation),
    ) -> Result<Step, String> {
        if self.reference_pool.is_some() {
            return Err("Regional surface observers do not describe body-owned exchange.".into());
        }
        self.advance_with_observers(state, seconds, observe, |_| {})
    }

    pub fn advance_terminal_observed(
        &self,
        state: &mut State,
        seconds: u32,
        observe: impl FnMut(TerminalObservation),
    ) -> Result<Step, String> {
        if self.reference_pool.is_some() {
            return Err("Regional terminal observers do not describe body-owned exchange.".into());
        }
        self.advance_with_observers(state, seconds, |_| {}, observe)
    }

    fn credit_reference_body(
        &self,
        cp: &mut Checkpoint,
        region: usize,
        high: f64,
        low: f64,
    ) -> Result<(), String> {
        let b = self.reference_pool.as_ref().unwrap().by_region[region].unwrap();
        let body_high = cp.reference_body_high_kilograms.as_mut().unwrap();
        let body_low = cp.reference_body_low_kilograms.as_mut().unwrap();
        let mut stock = surface_water::CompensatedStock::new(body_high[b], body_low[b], f64::MAX)?;
        stock.credit(high)?;
        stock.credit(low)?;
        body_high[b] = stock.high;
        body_low[b] = stock.low;
        Ok(())
    }

    fn advance_with_observers(
        &self,
        state: &mut State,
        seconds: u32,
        mut observe: impl FnMut(SurfaceObservation),
        mut observe_terminal: impl FnMut(TerminalObservation),
    ) -> Result<Step, String> {
        self.budget(state)?;
        if seconds == 0
            || u64::from(seconds) > SECONDS_PER_DAY
            || state.elapsed_seconds() + u64::from(seconds) > MAX_ELAPSED_SECONDS
        {
            return Err("Moisture step must be 1–86400 seconds within the ten-year bound.".into());
        }
        let mut next = state.0.clone();
        let end = next.elapsed_seconds + u64::from(seconds);
        let n = self.areas.len();
        let mut evaporation = vec![0.; n];
        let mut precipitation = vec![0.; n];
        let mut surface_transfers = vec![surface_water::Transfers::default(); n];
        let mut runoff_transfers = vec![runoff_transport::Transfers::default(); n];
        let mut runoff_step_roundoff = vec![[0.; 4]; n];
        let mut max_local_residual: f64 = 0.;
        let mut max_routing_residual: f64 = 0.;
        let mut transport_substeps = 0;
        let mut coupled_substeps = 0;
        let mut maximum_vapor_column: f64 = 0.;
        let mut pool_demand = if self.reference_pool.is_some() {
            vec![0.; n]
        } else {
            Vec::new()
        };
        let mut lake_demand = self.lake_exchange.as_ref().map(|_| vec![0.; n]);
        let mut leaf_spill_events = next.leaf_spill_state.as_ref().map(|_| Vec::new());
        let settings = self.origin.settings;
        let coupled_limit = self.maximum_coupled_step_seconds()?;
        if settings.orography.is_some()
            && (state.elapsed_seconds() % coupled_limit + u64::from(seconds))
                .div_ceil(coupled_limit)
                > 4096
        {
            return Err("Orographic interval exceeds the 4096 coupled-step work limit; use a shorter caller interval.".into());
        }
        let mut prepared_routing = None;
        while next.elapsed_seconds < end {
            let day = next.elapsed_seconds / SECONDS_PER_DAY;
            let month = (day as usize % seasonal_temperature::DAYS_PER_YEAR) * 12
                / seasonal_temperature::DAYS_PER_YEAR;
            let interval = (end - next.elapsed_seconds)
                .min(SECONDS_PER_DAY - next.elapsed_seconds % SECONDS_PER_DAY)
                .min(coupled_limit - next.elapsed_seconds % coupled_limit);
            let evaporation_fraction = if settings.evaporation_enabled {
                -(-(interval as f64 * 0.5) / settings.evaporation_response_seconds).exp_m1()
            } else {
                0.
            };
            let precipitation_fraction = if settings.precipitation_enabled {
                -(-(interval as f64 * 0.5) / settings.precipitation_response_seconds).exp_m1()
            } else {
                0.
            };
            let surface_response =
                surface_water::Response::new(interval as f64 * 0.5, settings.surface)?;
            if settings.routing_enabled
                && prepared_routing
                    .as_ref()
                    .is_none_or(|(last, _)| *last != interval)
            {
                prepared_routing = Some((interval, self.routing.prepare(interval as f64 * 0.5)?));
            }
            for phase in 0..2 {
                pool_demand.fill(0.);
                if let Some(demand) = &mut lake_demand {
                    demand.fill(0.);
                }
                let lake_wet = self
                    .lake_exchange
                    .as_ref()
                    .map(|layout| layout.exposure(&next))
                    .transpose()?;
                if phase == 1 {
                    let transported = self.flows[month].advance(
                        &next.vapor_kilograms,
                        interval as f64,
                        self.origin.settings.transport,
                    )?;
                    next.vapor_kilograms = transported.stock_kilograms;
                    transport_substeps += transported.budget.substeps;
                }
                for i in 0..n {
                    let pooled_region = self.reference_pool.is_some() && !self.is_land[i];
                    let lake_region = self
                        .lake_exchange
                        .as_ref()
                        .is_some_and(|layout| layout.owns(&next, i));
                    let lake_exposed = lake_wet.as_ref().is_some_and(|wet| wet[i]);
                    let vapor = next.vapor_kilograms[i];
                    let capacity = self.capacities[month][i];
                    let potential_evaporation = (capacity - vapor).max(0.) * evaporation_fraction;
                    if pooled_region && self.temperatures[month][i] > 0. {
                        pool_demand[i] = potential_evaporation;
                    }
                    if lake_exposed && self.temperatures[month][i] > 0. {
                        lake_demand.as_mut().unwrap()[i] = potential_evaporation;
                    }
                    let extra_rate = self
                        .orographic_forcing
                        .as_ref()
                        .map_or(0., |f| f.additional_rates_per_second[month][i]);
                    let deposited = if settings.precipitation_enabled && extra_rate > 0. {
                        orographic_response::deposition(
                            vapor,
                            capacity,
                            1. / settings.precipitation_response_seconds,
                            extra_rate,
                            interval as f64 * 0.5,
                        )?
                    } else {
                        (vapor - capacity).max(0.) * precipitation_fraction
                    };
                    let mut terminal_evaporation =
                        if !lake_region && self.temperatures[month][i] > 0. {
                            potential_evaporation.min(next.terminal_water_kilograms[i])
                        } else {
                            0.
                        };
                    let old_terminal = next.terminal_water_kilograms[i];
                    let old_terminal_low = next
                        .terminal_low_kilograms
                        .as_ref()
                        .map_or(0., |low| low[i]);
                    if let Some(low) = &mut next.terminal_low_kilograms {
                        let mut terminal =
                            surface_water::CompensatedStock::new(old_terminal, low[i], f64::MAX)?;
                        terminal_evaporation = terminal.withdraw(terminal_evaporation);
                        next.terminal_water_kilograms[i] = terminal.high;
                        low[i] = terminal.low;
                    } else {
                        next.terminal_water_kilograms[i] -= terminal_evaporation;
                    }
                    let legacy_terminal_residual =
                        (next.terminal_water_kilograms[i] - old_terminal) + terminal_evaporation;
                    let terminal_residual = next
                        .terminal_low_kilograms
                        .as_ref()
                        .map_or(legacy_terminal_residual, |low| {
                            legacy_terminal_residual + (low[i] - old_terminal_low)
                        });
                    if !terminal_residual.is_finite()
                        || terminal_residual.abs() > 16. * f64::EPSILON * old_terminal.max(1.)
                    {
                        return Err("Terminal evaporation exceeds its arithmetic tolerance.".into());
                    }
                    max_local_residual = max_local_residual.max(terminal_residual.abs());
                    observe_terminal(TerminalObservation {
                        region: i,
                        before_kilograms: old_terminal,
                        before_low_kilograms: old_terminal_low,
                        after_kilograms: next.terminal_water_kilograms[i],
                        after_low_kilograms: next
                            .terminal_low_kilograms
                            .as_ref()
                            .map_or(0., |low| low[i]),
                        received_kilograms: 0.,
                        evaporated_kilograms: terminal_evaporation,
                    });
                    let terminal_flux = runoff_transport::Transfers {
                        terminal_evaporation,
                        ..Default::default()
                    };
                    next.cumulative_runoff_transfers[i].accumulate(
                        terminal_flux,
                        &mut next.cumulative_runoff_transfer_roundoff[i],
                    );
                    runoff_transfers[i].accumulate(terminal_flux, &mut runoff_step_roundoff[i]);
                    let before = surface_water::Stocks {
                        liquid: next.surface_kilograms[i],
                        snow: next.snow_kilograms[i],
                        soil: next.soil_kilograms[i],
                        pending_runoff: next.pending_runoff_kilograms[i],
                    };
                    let before_low = next.soil_low_kilograms.as_ref().map_or(0., |low| low[i]);
                    let before_snow_low = next.snow_low_kilograms.as_ref().map_or(0., |low| low[i]);
                    let before_liquid_low =
                        next.surface_low_kilograms.as_ref().map_or(0., |low| low[i]);
                    let (mut result, mut after_low, after_snow_low, after_liquid_low) =
                        if next.snow_low_kilograms.is_some() {
                            // Submerged soil remains owned/inactive; in-flight runoff
                            // remains in the original delayed receiver network.
                            let exchange_before = if lake_exposed {
                                surface_water::Stocks {
                                    soil: 0.,
                                    pending_runoff: 0.,
                                    ..before
                                }
                            } else {
                                before
                            };
                            let precise = surface_water::advance_precise_surface_prepared(
                                exchange_before,
                                if lake_exposed { 0. } else { before_low },
                                before_snow_low,
                                before_liquid_low,
                                self.areas[i],
                                self.is_land[i] && !lake_exposed,
                                self.temperatures[month][i],
                                deposited,
                                if pooled_region || lake_exposed {
                                    0.
                                } else {
                                    potential_evaporation - terminal_evaporation
                                },
                                &surface_response,
                            )?;
                            (
                                precise.step,
                                precise.soil_low_kilograms,
                                precise.snow_low_kilograms,
                                precise.liquid_low_kilograms,
                            )
                        } else if next.soil_low_kilograms.is_some() {
                            let precise = surface_water::advance_compensated_prepared(
                                before,
                                before_low,
                                self.areas[i],
                                self.is_land[i],
                                self.temperatures[month][i],
                                deposited,
                                potential_evaporation - terminal_evaporation,
                                &surface_response,
                            )?;
                            (precise.step, precise.soil_low_kilograms, 0., 0.)
                        } else {
                            (
                                surface_water::advance_prepared(
                                    before,
                                    self.areas[i],
                                    self.is_land[i],
                                    self.temperatures[month][i],
                                    deposited,
                                    potential_evaporation - terminal_evaporation,
                                    &surface_response,
                                )?,
                                0.,
                                0.,
                                0.,
                            )
                        };
                    if lake_exposed {
                        result.stocks.soil = before.soil;
                        result.stocks.pending_runoff = before.pending_runoff;
                        after_low = before_low;
                    }
                    observe(SurfaceObservation {
                        coupled_start_seconds: next.elapsed_seconds,
                        phase,
                        region: i,
                        local_seconds: interval as f64 * 0.5,
                        before,
                        after: result,
                        before_soil_low_kilograms: before_low,
                        after_soil_low_kilograms: after_low,
                        before_snow_low_kilograms: before_snow_low,
                        after_snow_low_kilograms: after_snow_low,
                        before_liquid_low_kilograms: before_liquid_low,
                        after_liquid_low_kilograms: after_liquid_low,
                    });
                    let evaporated = result.transfers.liquid_evaporation
                        + result.transfers.soil_evaporation
                        + terminal_evaporation;
                    next.surface_kilograms[i] = result.stocks.liquid;
                    if pooled_region {
                        self.credit_reference_body(
                            &mut next,
                            i,
                            result.stocks.liquid,
                            after_liquid_low,
                        )?;
                        next.surface_kilograms[i] = 0.;
                    }
                    if lake_exposed {
                        self.lake_exchange.as_ref().unwrap().capture(
                            &mut next,
                            i,
                            result.stocks.liquid,
                            after_liquid_low,
                        )?;
                        next.surface_kilograms[i] = 0.;
                    }
                    next.snow_kilograms[i] = result.stocks.snow;
                    next.soil_kilograms[i] = result.stocks.soil;
                    if let Some(low) = &mut next.soil_low_kilograms {
                        low[i] = after_low;
                    }
                    if let Some(low) = &mut next.snow_low_kilograms {
                        low[i] = after_snow_low;
                    }
                    if let Some(low) = &mut next.surface_low_kilograms {
                        low[i] = if pooled_region || lake_exposed {
                            0.
                        } else {
                            after_liquid_low
                        };
                    }
                    next.pending_runoff_kilograms[i] = result.stocks.pending_runoff;
                    next.vapor_kilograms[i] = (vapor - deposited) + evaporated;
                    next.cumulative_surface_transfers[i].accumulate_compensated(
                        result.transfers,
                        &mut next.cumulative_surface_transfer_roundoff[i],
                    );
                    surface_transfers[i].accumulate(result.transfers);
                    let totals = next.cumulative_surface_transfers[i];
                    next.cumulative_evaporation_kilograms[i] = totals.liquid_evaporation
                        + totals.soil_evaporation
                        + next.cumulative_runoff_transfers[i].terminal_evaporation;
                    next.cumulative_precipitation_kilograms[i] = totals.rain + totals.snowfall;
                    evaporation[i] += evaporated;
                    precipitation[i] += deposited;
                    max_local_residual = max_local_residual.max(result.residual_kilograms.abs());
                    if phase == 1 {
                        maximum_vapor_column =
                            maximum_vapor_column.max(next.vapor_kilograms[i] / self.areas[i]);
                    }
                }
                if let Some(layout) = &self.lake_exchange {
                    if let Some(spill) = &layout.spill {
                        leaf_spill::append_events(
                            leaf_spill_events.as_mut().unwrap(),
                            spill.resolve(
                                &mut next,
                                self.reference_pool.as_ref().unwrap(),
                                layout.merge.as_ref(),
                            )?,
                        )?;
                    }
                    let (grants, residual) =
                        layout.evaporate(&mut next, lake_demand.as_ref().unwrap())?;
                    max_local_residual = max_local_residual.max(residual);
                    for (i, grant) in grants.into_iter().enumerate() {
                        if grant == 0. {
                            continue;
                        }
                        next.vapor_kilograms[i] += grant;
                        let transfer = runoff_transport::Transfers {
                            terminal_evaporation: grant,
                            ..Default::default()
                        };
                        next.cumulative_runoff_transfers[i]
                            .accumulate(transfer, &mut next.cumulative_runoff_transfer_roundoff[i]);
                        runoff_transfers[i].accumulate(transfer, &mut runoff_step_roundoff[i]);
                        let f = next.cumulative_surface_transfers[i];
                        next.cumulative_evaporation_kilograms[i] = f.liquid_evaporation
                            + f.soil_evaporation
                            + next.cumulative_runoff_transfers[i].terminal_evaporation;
                        evaporation[i] += grant;
                        if phase == 1 {
                            maximum_vapor_column =
                                maximum_vapor_column.max(next.vapor_kilograms[i] / self.areas[i]);
                        }
                    }
                }
                if let Some(layout) = &self.reference_pool {
                    for (b, members) in layout.members.iter().enumerate() {
                        let donor = surface_water::CompensatedStock::new(
                            next.reference_body_high_kilograms.as_ref().unwrap()[b],
                            next.reference_body_low_kilograms.as_ref().unwrap()[b],
                            f64::MAX,
                        )?;
                        let demand: Vec<_> = members.iter().map(|&i| pool_demand[i]).collect();
                        let (after, grants, residual) = reference_pool::allocate(donor, &demand)?;
                        next.reference_body_high_kilograms.as_mut().unwrap()[b] = after.high;
                        next.reference_body_low_kilograms.as_mut().unwrap()[b] = after.low;
                        max_local_residual = max_local_residual.max(residual.abs());
                        for (&i, grant) in members.iter().zip(grants) {
                            next.vapor_kilograms[i] += grant;
                            let transfer = surface_water::Transfers {
                                liquid_evaporation: grant,
                                ..Default::default()
                            };
                            next.cumulative_surface_transfers[i].accumulate_compensated(
                                transfer,
                                &mut next.cumulative_surface_transfer_roundoff[i],
                            );
                            surface_transfers[i].accumulate(transfer);
                            next.cumulative_evaporation_kilograms[i] =
                                next.cumulative_surface_transfers[i].liquid_evaporation;
                            evaporation[i] += grant;
                            if phase == 1 {
                                maximum_vapor_column = maximum_vapor_column
                                    .max(next.vapor_kilograms[i] / self.areas[i]);
                            }
                        }
                    }
                }
                if let Some((_, response)) = &prepared_routing {
                    let routed = self
                        .routing
                        .advance_prepared(&next.pending_runoff_kilograms, response)?;
                    max_routing_residual =
                        max_routing_residual.max(routed.residual_kilograms.abs());
                    next.pending_runoff_kilograms = routed.transit_kilograms;
                    for i in 0..n {
                        let old_terminal = next.terminal_water_kilograms[i];
                        let old_low = next
                            .terminal_low_kilograms
                            .as_ref()
                            .map_or(0., |low| low[i]);
                        if self.reference_pool.is_some() && !self.is_land[i] {
                            self.credit_reference_body(
                                &mut next,
                                i,
                                routed.terminal_delivery_kilograms[i],
                                0.,
                            )?;
                        } else if let Some(spill) = &mut next.leaf_spill_state {
                            spill
                                .pending_input
                                .credit(i, routed.terminal_delivery_kilograms[i])?;
                        } else if let Some(low) = &mut next.terminal_low_kilograms {
                            let mut terminal = surface_water::CompensatedStock::new(
                                next.terminal_water_kilograms[i],
                                low[i],
                                f64::MAX,
                            )?;
                            terminal.credit(routed.terminal_delivery_kilograms[i])?;
                            next.terminal_water_kilograms[i] = terminal.high;
                            low[i] = terminal.low;
                        } else {
                            next.terminal_water_kilograms[i] +=
                                routed.terminal_delivery_kilograms[i];
                        }
                        observe_terminal(TerminalObservation {
                            region: i,
                            before_kilograms: old_terminal,
                            before_low_kilograms: old_low,
                            after_kilograms: next.terminal_water_kilograms[i],
                            after_low_kilograms: next
                                .terminal_low_kilograms
                                .as_ref()
                                .map_or(0., |low| low[i]),
                            received_kilograms: routed.terminal_delivery_kilograms[i],
                            evaporated_kilograms: 0.,
                        });
                        let transfers = runoff_transport::Transfers {
                            sent: routed.sent_kilograms[i],
                            received_transit: routed.received_transit_kilograms[i],
                            terminal_delivery: routed.terminal_delivery_kilograms[i],
                            terminal_evaporation: 0.,
                        };
                        next.cumulative_runoff_transfers[i].accumulate(
                            transfers,
                            &mut next.cumulative_runoff_transfer_roundoff[i],
                        );
                        runoff_transfers[i].accumulate(transfers, &mut runoff_step_roundoff[i]);
                    }
                }
                if let Some(layout) = &self.lake_exchange {
                    if let Some(spill) = &layout.spill {
                        leaf_spill::append_events(
                            leaf_spill_events.as_mut().unwrap(),
                            spill.resolve(
                                &mut next,
                                self.reference_pool.as_ref().unwrap(),
                                layout.merge.as_ref(),
                            )?,
                        )?;
                    }
                    layout.exposure(&next)?;
                }
            }
            next.elapsed_seconds += interval;
            coupled_substeps += 1;
        }
        let budget = self.validate_checkpoint(&next)?;
        *state = State(next);
        Ok(Step {
            leaf_spill_events,
            evaporation_kilograms: evaporation,
            precipitation_kilograms: precipitation,
            surface_transfers,
            runoff_transfers,
            transport_substeps,
            coupled_substeps,
            maximum_observed_vapor_column_kilograms_per_square_meter: maximum_vapor_column,
            maximum_absolute_local_exchange_residual_kilograms: max_local_residual,
            maximum_absolute_routing_residual_kilograms: max_routing_residual,
            budget,
        })
    }
}

fn total_runoff_transfers(
    transfers: &[runoff_transport::Transfers],
) -> runoff_transport::Transfers {
    let mut sums = [0.; 4];
    for (i, sum) in sums.iter_mut().enumerate() {
        *sum = total_mass(&transfers.iter().map(|f| f.values()[i]).collect::<Vec<_>>());
    }
    runoff_transport::Transfers::from_values(sums)
}

fn total_transfers(transfers: &[surface_water::Transfers]) -> surface_water::Transfers {
    let mut sums = [0.; 8];
    for (i, sum) in sums.iter_mut().enumerate() {
        *sum = total_mass(&transfers.iter().map(|f| f.values()[i]).collect::<Vec<_>>());
    }
    surface_water::Transfers::from_values(sums)
}
