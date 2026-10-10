//! Separately pinned regional seasonal cycle with one terrestrial liquid owner.
//! Old seasonal schemas are neither migrated nor advanced by this model.
use super::{
    ClosedLakeExchange, ReferenceWaterPool, SoilNumerics, SurfaceNumerics, TerminalNumerics,
    surface_flow,
};
use crate::{
    Recipe, World, moisture_transport, orographic_response, runoff_transport, seasonal_temperature,
    seasonal_wind,
    surface_water::{
        self,
        ponded_soil::{self, Mass, ResolutionBudget},
    },
};
use serde::{Deserialize, Serialize};
mod accounting;
mod cycle;
mod observation;
mod surface;
mod surface_cotan;
pub mod surface_verification;
pub use observation::{
    COTANGENT_OBSERVATION_VERSION, OBSERVATION_VERSION, Observation, ReferenceBodyStock,
    RegionalStocks,
};

pub const MODEL_VERSION: &str = "regional-seasonal-water-1";
pub const COTANGENT_MODEL_VERSION: &str = "regional-seasonal-water-2";
pub const COTANGENT_SURFACE_VERSION: &str = "regional-surface-cotan-paired-1";
pub const COTANGENT_RETAINING_SURFACE_VERSION: &str = "regional-surface-cotan-paired-2";
pub const SURFACE_FLOW_VERSION: &str = "regional-surface-flow-paired-1";
pub const DRAINAGE_VERSION: &str = "runoff-transport-paired-1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NumericalPolicy {
    #[default]
    RejectUnrepresentable,
    RetainDonor,
}
impl NumericalPolicy {
    fn is_reject(&self) -> bool {
        *self == Self::RejectUnrepresentable
    }
}

/// A numerical operator selection, not a change to physical cell areas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfaceOperator {
    #[default]
    BarycentricTwoPoint,
    /// Conservative weak-form adjacency exchange, not literal dual-face discharge.
    CotangentWeakForm,
}
impl SurfaceOperator {
    fn is_legacy(&self) -> bool {
        *self == Self::BarycentricTwoPoint
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    // Omission retains old checkpoint bytes and resolves only to the old operator.
    #[serde(default, skip_serializing_if = "SurfaceOperator::is_legacy")]
    pub surface_operator: SurfaceOperator,
    #[serde(default, skip_serializing_if = "NumericalPolicy::is_reject")]
    pub numerical_policy: NumericalPolicy,
    pub initial_active_surface_depth_meters: f64,
    pub effective_vapor_depth_meters: f64,
    pub evaporation_response_seconds: f64,
    pub precipitation_response_seconds: f64,
    pub evaporation_enabled: bool,
    pub precipitation_enabled: bool,
    pub routing_enabled: bool,
    pub max_coupled_step_seconds: u32,
    pub melt_kilograms_per_square_meter_degree_day: f64,
    pub soil: ponded_soil::Settings,
    pub transport: moisture_transport::Settings,
    pub runoff: runoff_transport::Settings,
    pub surface_flow: surface_flow::Settings,
    pub orography: orographic_response::Settings,
}
impl Default for Settings {
    fn default() -> Self {
        let old = super::Settings::default();
        Self {
            surface_operator: SurfaceOperator::default(),
            numerical_policy: NumericalPolicy::default(),
            initial_active_surface_depth_meters: 10.,
            effective_vapor_depth_meters: old.effective_vapor_depth_meters,
            evaporation_response_seconds: old.evaporation_response_seconds,
            precipitation_response_seconds: old.precipitation_response_seconds,
            evaporation_enabled: true,
            precipitation_enabled: true,
            routing_enabled: true,
            max_coupled_step_seconds: 900,
            melt_kilograms_per_square_meter_degree_day: old
                .surface
                .melt_kilograms_per_square_meter_degree_day,
            soil: Default::default(),
            transport: old.transport,
            runoff: old.runoff,
            surface_flow: Default::default(),
            orography: Default::default(),
        }
    }
}
impl Settings {
    // Reuse the existing forcing/geometry preparation, not its state machine.
    fn preparation(self) -> super::Settings {
        super::Settings {
            initial_active_surface_depth_meters: self.initial_active_surface_depth_meters,
            effective_vapor_depth_meters: self.effective_vapor_depth_meters,
            evaporation_response_seconds: self.evaporation_response_seconds,
            precipitation_response_seconds: self.precipitation_response_seconds,
            evaporation_enabled: self.evaporation_enabled,
            precipitation_enabled: self.precipitation_enabled,
            routing_enabled: self.routing_enabled,
            max_coupled_step_seconds: self.max_coupled_step_seconds,
            surface: surface_water::Settings {
                melt_kilograms_per_square_meter_degree_day: self
                    .melt_kilograms_per_square_meter_degree_day,
                soil_capacity_kilograms_per_square_meter: self
                    .soil
                    .soil_capacity_kilograms_per_square_meter,
                soil_retained_fraction: self.soil.soil_retained_fraction,
                infiltration_response_seconds: self.soil.infiltration_response_seconds,
                soil_drainage_response_seconds: self.soil.soil_drainage_response_seconds,
                // No liquid-runoff generation is executed here. Keep this legacy
                // preparation-only bound above every supported coupled ceiling.
                liquid_runoff_response_seconds: 30. * 86400.,
            },
            transport: self.transport,
            runoff: self.runoff,
            orography: Some(self.orography),
            soil_numerics: Some(SoilNumerics::Compensated),
            surface_numerics: Some(SurfaceNumerics::Compensated),
            terminal_numerics: Some(TerminalNumerics::Compensated),
            reference_water_pool: Some(ReferenceWaterPool::FastConnectedBody),
            closed_lake_exchange: Some(ClosedLakeExchange::FrozenRegionalSurfaceFlow(
                self.surface_flow,
            )),
        }
    }
    pub fn validate(self) -> Result<(), String> {
        self.soil.validate()?;
        self.preparation().validate()
    }
}

/// Cumulative gross local transfers, not additional owned reservoirs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transfers {
    pub rain: Mass,
    pub snowfall: Mass,
    pub melt: Mass,
    pub liquid_evaporation: Mass,
    pub soil_evaporation: Mass,
    pub infiltration: Mass,
    pub soil_drainage: Mass,
}
impl Transfers {
    pub fn values(self) -> [Mass; 7] {
        [
            self.rain,
            self.snowfall,
            self.melt,
            self.liquid_evaporation,
            self.soil_evaporation,
            self.infiltration,
            self.soil_drainage,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::present_option"
    )]
    pub resolution: Option<ResolutionBudget>,
    pub schema_version: u32,
    pub model_version: String,
    pub soil_model_version: String,
    pub transport_model_version: String,
    pub surface_flow_model_version: String,
    pub runoff_model_version: String,
    pub temperature_model_version: String,
    pub wind_model_version: String,
    pub orographic_model_version: String,
    pub reference_body_model_version: String,
    pub recipe: Recipe,
    pub settings: Settings,
    pub temperature_settings: seasonal_temperature::Settings,
    pub wind_settings: seasonal_wind::Settings,
    pub elapsed_seconds: u64,
    pub liquid: Vec<Mass>,
    pub soil: Vec<Mass>,
    pub snow: Vec<Mass>,
    pub vapor: Vec<Mass>,
    pub drainage: Vec<Mass>,
    /// Sorted positive initial body IDs, reconstructed from the resolved recipe.
    pub reference_bodies: Vec<Mass>,
    pub local_transfers: Vec<Transfers>,
    /// Two directions of each canonical physical adjacency, including inactive directions.
    pub atmospheric_transfers: Vec<Mass>,
    /// Weak-form edge exchanges for cotangent; not literal barycentric-face discharge.
    pub surface_transfers: Vec<Mass>,
    /// One outgoing directed receiver edge per region, including terminal self edges.
    pub drainage_sent: Vec<Mass>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct State(Checkpoint);
impl State {
    pub fn checkpoint(&self) -> Checkpoint {
        self.0.clone()
    }
    pub fn elapsed_seconds(&self) -> u64 {
        self.0.elapsed_seconds
    }
    pub fn local_transfers(&self) -> &[Transfers] {
        &self.0.local_transfers
    }
    pub fn drainage_sent(&self) -> &[Mass] {
        &self.0.drainage_sent
    }
}
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Budget {
    pub initial_kilograms: f64,
    pub stored_kilograms: f64,
    pub relative_global_residual: f64,
    pub maximum_relative_local_residual: f64,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub budget: Budget,
    pub coupled_substeps: usize,
    pub atmospheric_substeps: usize,
    pub surface_substeps: usize,
}

pub struct Model {
    // Only prepared fixed geography and prescribed forcing are reused.
    forcing: super::Model,
    // Prepared once and regenerated on restore. The old forcing layout is never
    // mutated, and no renderer geometry or checkpoint-supplied weight is accepted.
    cotangent_layout: Option<surface_flow::Layout>,
    origin: Checkpoint,
}
impl Model {
    pub fn settings(&self) -> Settings {
        self.origin.settings
    }
    pub fn temperature_settings(&self) -> seasonal_temperature::Settings {
        self.origin.temperature_settings
    }
    pub fn wind_settings(&self) -> seasonal_wind::Settings {
        self.origin.wind_settings
    }
    pub fn from_world(
        world: &World,
        settings: Settings,
        temperature_settings: seasonal_temperature::Settings,
        wind_settings: seasonal_wind::Settings,
    ) -> Result<Self, String> {
        settings.validate()?;
        let forcing = super::Model::from_world(
            world,
            settings.preparation(),
            temperature_settings,
            wind_settings,
        )?;
        let legacy_initial = forcing.initial_state().checkpoint();
        let n = world.surface.areas.len();
        let faces = forcing.flows[0].directed_contacts().len();
        let contacts = forcing.flows[0].directed_contacts();
        let surface_contacts: Vec<_> = forcing
            .lake_exchange
            .as_ref()
            .unwrap()
            .regional
            .as_ref()
            .unwrap()
            .faces
            .iter()
            .flat_map(|face| {
                let [a, b] = face.regions;
                [[a, b], [b, a]]
            })
            .collect();
        if surface_contacts != contacts
            || forcing
                .flows
                .iter()
                .any(|f| f.directed_contacts() != contacts)
        {
            return Err("Regional seasonal physical-face order is inconsistent.".into());
        }
        let cotangent_layout = if settings.surface_operator == SurfaceOperator::CotangentWeakForm {
            Some(surface_cotan::prepare(
                &world.surface,
                forcing
                    .lake_exchange
                    .as_ref()
                    .unwrap()
                    .regional
                    .as_ref()
                    .unwrap()
                    .clone(),
            )?)
        } else {
            None
        };
        let reference_bodies = legacy_initial
            .reference_body_high_kilograms
            .as_ref()
            .unwrap()
            .iter()
            .zip(
                legacy_initial
                    .reference_body_low_kilograms
                    .as_ref()
                    .unwrap(),
            )
            .map(|(&high, &low)| Mass { high, low })
            .collect();
        let origin = Checkpoint {
            resolution: (settings.numerical_policy == NumericalPolicy::RetainDonor)
                .then(ResolutionBudget::default),
            schema_version: if settings.surface_operator.is_legacy() {
                1
            } else {
                2
            },
            model_version: if settings.surface_operator.is_legacy() {
                MODEL_VERSION
            } else {
                COTANGENT_MODEL_VERSION
            }
            .into(),
            soil_model_version: if settings.numerical_policy.is_reject() {
                ponded_soil::MODEL_VERSION
            } else {
                ponded_soil::RECORDED_MODEL_VERSION
            }
            .into(),
            transport_model_version: if settings.numerical_policy.is_reject() {
                moisture_transport::PAIRED_MODEL_VERSION
            } else {
                moisture_transport::PAIRED_RETAINING_MODEL_VERSION
            }
            .into(),
            surface_flow_model_version: if settings.surface_operator
                == SurfaceOperator::CotangentWeakForm
            {
                if settings.numerical_policy.is_reject() {
                    COTANGENT_SURFACE_VERSION
                } else {
                    COTANGENT_RETAINING_SURFACE_VERSION
                }
            } else if settings.numerical_policy.is_reject() {
                SURFACE_FLOW_VERSION
            } else {
                "regional-surface-flow-paired-2"
            }
            .into(),
            runoff_model_version: if settings.numerical_policy.is_reject() {
                DRAINAGE_VERSION
            } else {
                "runoff-transport-paired-2"
            }
            .into(),
            temperature_model_version: seasonal_temperature::MODEL_VERSION.into(),
            wind_model_version: seasonal_wind::MODEL_VERSION.into(),
            orographic_model_version: orographic_response::MODEL_VERSION.into(),
            reference_body_model_version: super::reference_pool::MODEL_VERSION.into(),
            recipe: world.recipe.clone(),
            settings,
            temperature_settings,
            wind_settings,
            elapsed_seconds: 0,
            liquid: vec![Mass::default(); n],
            soil: vec![Mass::default(); n],
            snow: vec![Mass::default(); n],
            vapor: vec![Mass::default(); n],
            drainage: vec![Mass::default(); n],
            reference_bodies,
            local_transfers: vec![Transfers::default(); n],
            atmospheric_transfers: vec![Mass::default(); faces],
            surface_transfers: vec![Mass::default(); faces],
            drainage_sent: vec![Mass::default(); n],
        };
        let model = Self {
            forcing,
            cotangent_layout,
            origin,
        };
        model.validate(&model.origin)?;
        Ok(model)
    }
    pub fn initial_state(&self) -> State {
        State(self.origin.clone())
    }
    fn surface_layout(&self) -> &surface_flow::Layout {
        self.cotangent_layout.as_ref().unwrap_or_else(|| {
            self.forcing
                .lake_exchange
                .as_ref()
                .unwrap()
                .regional
                .as_ref()
                .unwrap()
        })
    }
    pub fn restore(checkpoint: Checkpoint) -> Result<(Self, State), String> {
        let world = World::generate(checkpoint.recipe.clone())?;
        let model = Self::from_world(
            &world,
            checkpoint.settings,
            checkpoint.temperature_settings,
            checkpoint.wind_settings,
        )?;
        model.validate(&checkpoint)?;
        Ok((model, State(checkpoint)))
    }
    pub fn budget(&self, state: &State) -> Result<Budget, String> {
        self.validate(&state.0)
    }
    pub fn advance(&self, state: &mut State, seconds: u32) -> Result<Step, String> {
        self.validate(&state.0)?;
        if seconds == 0
            || seconds > 86400
            || state.elapsed_seconds() + u64::from(seconds) > super::MAX_ELAPSED_SECONDS
        {
            return Err("Invalid regional seasonal interval or elapsed limit.".into());
        }
        let mut candidate = state.0.clone();
        let counts = self.advance_candidate(&mut candidate, seconds)?;
        let budget = self.validate(&candidate)?;
        state.0 = candidate;
        Ok(Step {
            budget,
            coupled_substeps: counts[0],
            atmospheric_substeps: counts[1],
            surface_substeps: counts[2],
        })
    }
}
