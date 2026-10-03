//! Finite seasonal exchange with delayed runoff and evaporating terminal stores.
//! Fixed initial geography; no basin spill levels or energy feedback.
use crate::{
    Recipe, World,
    moisture_transport::{Flow, Geometry, total_mass},
    runoff_transport, seasonal_temperature, seasonal_wind, surface_water,
};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "seasonal-moisture-3";
pub const SECONDS_PER_DAY: u64 = 86400;
pub const MAX_ELAPSED_SECONDS: u64 = 3650 * SECONDS_PER_DAY;
pub const WATER_DENSITY_KILOGRAMS_PER_CUBIC_METER: f64 = 1000.;
const VAPOR_GAS_CONSTANT: f64 = 461.5;

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
        }
    }
}

impl Settings {
    pub fn validate(self) -> Result<(), String> {
        self.transport.validate()?;
        self.surface.validate()?;
        self.runoff.validate()?;
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
    pub recipe: Recipe,
    pub settings: Settings,
    pub temperature_settings: seasonal_temperature::Settings,
    pub wind_settings: seasonal_wind::Settings,
    pub elapsed_seconds: u64,
    pub surface_kilograms: Vec<f64>,
    pub snow_kilograms: Vec<f64>,
    pub soil_kilograms: Vec<f64>,
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
    pub fn pending_runoff_kilograms(&self) -> &[f64] {
        &self.0.pending_runoff_kilograms
    }
    pub fn terminal_water_kilograms(&self) -> &[f64] {
        &self.0.terminal_water_kilograms
    }
    /// Each owned stock exactly once, for complete-state refinement comparisons.
    pub fn owned_stocks(&self) -> impl Iterator<Item = &f64> {
        self.surface_kilograms()
            .iter()
            .chain(self.snow_kilograms())
            .chain(self.soil_kilograms())
            .chain(self.pending_runoff_kilograms())
            .chain(self.terminal_water_kilograms())
            .chain(self.vapor_kilograms())
    }
}

pub struct Model {
    origin: Checkpoint,
    areas: Vec<f64>,
    is_land: Vec<bool>,
    routing: runoff_transport::Network,
    initial_surface: Vec<f64>,
    initial_total: f64,
    temperatures: Vec<Vec<f64>>,
    capacities: Vec<Vec<f64>>,
    flows: Vec<Flow>,
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
    pub cumulative_runoff_transfers: runoff_transport::Transfers,
    pub cumulative_surface_transfers: surface_water::Transfers,
    pub vapor_kilograms: f64,
    pub residual_kilograms: f64,
    pub cumulative_evaporation_kilograms: f64,
    pub cumulative_precipitation_kilograms: f64,
    pub maximum_relative_local_surface_ledger_residual: f64,
    pub vapor_ledger_residual_kilograms: f64,
}

#[derive(Debug)]
pub struct Step {
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

impl Model {
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
        let initial_surface: Vec<_> = world
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
        let origin = Checkpoint {
            schema_version: 3,
            model_version: MODEL_VERSION.into(),
            transport_model_version: crate::moisture_transport::MODEL_VERSION.into(),
            temperature_model_version: seasonal_temperature::MODEL_VERSION.into(),
            wind_model_version: seasonal_wind::MODEL_VERSION.into(),
            surface_model_version: surface_water::MODEL_VERSION.into(),
            runoff_model_version: runoff_transport::MODEL_VERSION.into(),
            recipe: world.recipe.clone(),
            settings,
            temperature_settings,
            wind_settings,
            elapsed_seconds: 0,
            surface_kilograms: initial_surface.clone(),
            snow_kilograms: vec![0.; n],
            soil_kilograms: vec![0.; n],
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
            routing,
            initial_surface,
            initial_total,
            temperatures: normals.monthly_temperature_celsius,
            capacities,
            flows,
        })
    }

    pub fn initial_state(&self) -> State {
        State(self.origin.clone())
    }

    pub fn restore(checkpoint: Checkpoint) -> Result<(Self, State), String> {
        if checkpoint.schema_version != 3
            || checkpoint.model_version != MODEL_VERSION
            || checkpoint.transport_model_version != crate::moisture_transport::MODEL_VERSION
            || checkpoint.temperature_model_version != seasonal_temperature::MODEL_VERSION
            || checkpoint.wind_model_version != seasonal_wind::MODEL_VERSION
            || checkpoint.surface_model_version != surface_water::MODEL_VERSION
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
            || cp.runoff_model_version != origin.runoff_model_version
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
        if (cp.elapsed_seconds == 0 || !cp.settings.routing_enabled)
            && (cp
                .cumulative_runoff_transfers
                .iter()
                .any(|f| f.values().iter().any(|v| *v != 0.))
                || cp
                    .cumulative_runoff_transfer_roundoff
                    .iter()
                    .flatten()
                    .any(|v| *v != 0.)
                || cp.terminal_water_kilograms.iter().any(|v| *v != 0.))
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
            if (!self.routing.is_terminal(i)
                && (terminal != 0.
                    || route.terminal_delivery != 0.
                    || route.terminal_evaporation != 0.))
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
                .max(1.);
            for (ledger, residual) in [
                (stocks.liquid - self.initial_surface[i]) - f.rain - f.melt
                    + f.liquid_evaporation
                    + f.infiltration
                    + f.liquid_runoff,
                stocks.snow - (f.snowfall - f.melt),
                stocks.soil - f.infiltration + f.soil_evaporation + f.soil_drainage,
                stocks.pending_runoff - f.liquid_runoff - f.soil_drainage - route.received_transit
                    + route.sent,
                cp.cumulative_evaporation_kilograms[i]
                    - (f.liquid_evaporation + f.soil_evaporation + route.terminal_evaporation),
                cp.cumulative_precipitation_kilograms[i] - (f.rain + f.snowfall),
                terminal - (route.terminal_delivery - route.terminal_evaporation),
                route.received_transit - expected_received[i],
                route.terminal_delivery - expected_delivery[i],
            ]
            .into_iter()
            .enumerate()
            {
                if !residual.is_finite() {
                    return Err("Nonfinite typed water ledger.".into());
                }
                if residual.abs() / scale > local_residual {
                    local_residual = residual.abs() / scale;
                    local_witness = (i, ledger);
                }
            }
        }
        let surface = total_mass(&cp.surface_kilograms);
        let snow = total_mass(&cp.snow_kilograms);
        let soil = total_mass(&cp.soil_kilograms);
        let runoff = total_mass(&cp.pending_runoff_kilograms);
        let terminal = total_mass(&cp.terminal_water_kilograms);
        let vapor = total_mass(&cp.vapor_kilograms);
        let evaporation = total_mass(&cp.cumulative_evaporation_kilograms);
        let precipitation = total_mass(&cp.cumulative_precipitation_kilograms);
        let residual =
            total_mass(&[surface, snow, soil, runoff, terminal, vapor]) - self.initial_total;
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

    pub fn advance(&self, state: &mut State, seconds: u32) -> Result<Step, String> {
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
                    let vapor = next.vapor_kilograms[i];
                    let capacity = self.capacities[month][i];
                    let potential_evaporation = (capacity - vapor).max(0.) * evaporation_fraction;
                    let deposited = (vapor - capacity).max(0.) * precipitation_fraction;
                    let terminal_evaporation = if self.temperatures[month][i] > 0. {
                        potential_evaporation.min(next.terminal_water_kilograms[i])
                    } else {
                        0.
                    };
                    let old_terminal = next.terminal_water_kilograms[i];
                    next.terminal_water_kilograms[i] -= terminal_evaporation;
                    let terminal_residual =
                        (next.terminal_water_kilograms[i] - old_terminal) + terminal_evaporation;
                    if !terminal_residual.is_finite()
                        || terminal_residual.abs() > 16. * f64::EPSILON * old_terminal.max(1.)
                    {
                        return Err("Terminal evaporation exceeds its arithmetic tolerance.".into());
                    }
                    max_local_residual = max_local_residual.max(terminal_residual.abs());
                    let terminal_flux = runoff_transport::Transfers {
                        terminal_evaporation,
                        ..Default::default()
                    };
                    next.cumulative_runoff_transfers[i].accumulate(
                        terminal_flux,
                        &mut next.cumulative_runoff_transfer_roundoff[i],
                    );
                    runoff_transfers[i].accumulate(terminal_flux, &mut runoff_step_roundoff[i]);
                    let result = surface_water::advance_prepared(
                        surface_water::Stocks {
                            liquid: next.surface_kilograms[i],
                            snow: next.snow_kilograms[i],
                            soil: next.soil_kilograms[i],
                            pending_runoff: next.pending_runoff_kilograms[i],
                        },
                        self.areas[i],
                        self.is_land[i],
                        self.temperatures[month][i],
                        deposited,
                        potential_evaporation - terminal_evaporation,
                        &surface_response,
                    )?;
                    let evaporated = result.transfers.liquid_evaporation
                        + result.transfers.soil_evaporation
                        + terminal_evaporation;
                    next.surface_kilograms[i] = result.stocks.liquid;
                    next.snow_kilograms[i] = result.stocks.snow;
                    next.soil_kilograms[i] = result.stocks.soil;
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
                if let Some((_, response)) = &prepared_routing {
                    let routed = self
                        .routing
                        .advance_prepared(&next.pending_runoff_kilograms, response)?;
                    max_routing_residual =
                        max_routing_residual.max(routed.residual_kilograms.abs());
                    next.pending_runoff_kilograms = routed.transit_kilograms;
                    for i in 0..n {
                        next.terminal_water_kilograms[i] += routed.terminal_delivery_kilograms[i];
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
            }
            next.elapsed_seconds += interval;
            coupled_substeps += 1;
        }
        let budget = self.validate_checkpoint(&next)?;
        *state = State(next);
        Ok(Step {
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
