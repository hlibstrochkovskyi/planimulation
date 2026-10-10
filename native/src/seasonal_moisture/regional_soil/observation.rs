//! Read-only view preparation, never a replacement for a complete checkpoint.
use super::*;

pub const OBSERVATION_VERSION: &str = "regional-soil-observation-1";
pub const COTANGENT_OBSERVATION_VERSION: &str = "regional-soil-observation-2";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionalStocks {
    pub liquid: Vec<Mass>,
    pub soil: Vec<Mass>,
    pub snow: Vec<Mass>,
    pub vapor: Vec<Mass>,
    pub drainage: Vec<Mass>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceBodyStock {
    pub body_id: u32,
    /// The whole body's mobile inventory, not an allocation to each wet region.
    pub liquid: Mass,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub observation_model_version: String,
    pub model_version: String,
    pub soil_model_version: String,
    pub transport_model_version: String,
    pub surface_flow_model_version: String,
    pub runoff_model_version: String,
    pub settings: Settings,
    pub elapsed_seconds: u64,
    pub budget: Budget,
    pub stocks: RegionalStocks,
    pub reference_bodies: Vec<ReferenceBodyStock>,
    pub resolution: Option<ResolutionBudget>,
    pub cumulative_local_transfers: Vec<Transfers>,
    /// Initial land's liquid divided by its whole physical column area.
    pub regional_liquid_depth_meters: Vec<f64>,
    /// Live land columns plus prescribed initial wet-body geometry.
    pub visible_water_depth_meters: Vec<f64>,
    /// Zero is a placeholder where the derived depth is not representably positive.
    pub visible_water_level_meters: Vec<f64>,
    /// Rounded gross histories: neither instantaneous discharge nor extra stocks.
    /// In observation v2 these are numerical weak-form adjacency exchanges.
    pub cumulative_surface_incoming_kilograms: Vec<f64>,
    pub cumulative_surface_outgoing_kilograms: Vec<f64>,
}

impl Model {
    pub fn observe(&self, state: &State) -> Result<Observation, String> {
        let cp = &state.0;
        let budget = self.validate(cp)?;
        let layout = self.surface_layout();
        let pool = self.forcing.reference_pool.as_ref().unwrap();
        let n = layout.areas.len();
        let mut depth = vec![0.; n];
        let mut visible_depth = vec![0.; n];
        let mut level = vec![0.; n];
        for r in 0..n {
            if layout.land[r] {
                depth[r] = cp.liquid[r].high / (1000. * layout.areas[r]);
                visible_depth[r] = depth[r];
                if depth[r] > 0. {
                    level[r] = layout.beds[r] + depth[r];
                }
            } else {
                visible_depth[r] = (layout.reference_level - layout.beds[r]).max(0.);
                if visible_depth[r] > 0. {
                    level[r] = layout.reference_level;
                }
            }
        }
        let mut incoming = vec![Vec::new(); n];
        let mut outgoing = vec![Vec::new(); n];
        for (&[source, target], mass) in self.forcing.flows[0]
            .directed_contacts()
            .iter()
            .zip(&cp.surface_transfers)
        {
            incoming[target].extend([mass.high, mass.low]);
            outgoing[source].extend([mass.high, mass.low]);
        }
        let rounded = |terms: Vec<Vec<f64>>| -> Vec<f64> {
            terms
                .iter()
                .map(|v| moisture_transport::total_mass(v))
                .collect()
        };
        let incoming = rounded(incoming);
        let outgoing = rounded(outgoing);
        if depth
            .iter()
            .chain(&visible_depth)
            .chain(&incoming)
            .chain(&outgoing)
            .any(|v| !v.is_finite() || *v < 0.)
            || level.iter().any(|v| !v.is_finite())
        {
            return Err("Regional-soil observation has an unrepresentable derived field.".into());
        }
        Ok(Observation {
            observation_model_version: if cp.settings.surface_operator.is_legacy() {
                OBSERVATION_VERSION
            } else {
                COTANGENT_OBSERVATION_VERSION
            }
            .into(),
            model_version: cp.model_version.clone(),
            soil_model_version: cp.soil_model_version.clone(),
            transport_model_version: cp.transport_model_version.clone(),
            surface_flow_model_version: cp.surface_flow_model_version.clone(),
            runoff_model_version: cp.runoff_model_version.clone(),
            settings: cp.settings,
            elapsed_seconds: cp.elapsed_seconds,
            budget,
            stocks: RegionalStocks {
                liquid: cp.liquid.clone(),
                soil: cp.soil.clone(),
                snow: cp.snow.clone(),
                vapor: cp.vapor.clone(),
                drainage: cp.drainage.clone(),
            },
            reference_bodies: pool
                .ids
                .iter()
                .zip(&cp.reference_bodies)
                .map(|(&body_id, &liquid)| ReferenceBodyStock { body_id, liquid })
                .collect(),
            resolution: cp.resolution,
            cumulative_local_transfers: cp.local_transfers.clone(),
            regional_liquid_depth_meters: depth,
            visible_water_depth_meters: visible_depth,
            visible_water_level_meters: level,
            cumulative_surface_incoming_kilograms: incoming,
            cumulative_surface_outgoing_kilograms: outgoing,
        })
    }
}
