//! Read-only ownership and frozen-demand controls, not an ocean mixing law.
use super::{Model, State};
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock};
use serde::Serialize;
use std::collections::BTreeMap;

pub const DIAGNOSTIC_VERSION: &str = "water-return-analysis-1";

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub region_count: usize,
    pub area_square_meters: f64,
    /// Liquid, snow, soil, transit, terminal, vapor; low parts belong to stocks.
    pub owned_stock_kilograms: [f64; 6],
    /// Terminal delivery, terminal evaporation, liquid evaporation, soil evaporation.
    pub cumulative_transfer_kilograms: [f64; 4],
    pub terminal_bearing_area_square_meters: f64,
    pub maximum_terminal_region: Option<usize>,
    pub maximum_terminal_kilograms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Body {
    pub reference_body_id: u32,
    pub inventory: Inventory,
    /// Twelve independent frozen-state probes, not annual flow observations.
    pub frozen_monthly_liquid_probes: Vec<FrozenProbe>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrozenProbe {
    pub month: usize,
    pub probe_seconds: u32,
    pub warm_demand_area_square_meters: f64,
    pub locally_supply_limited_area_square_meters: f64,
    pub potential_demand_kilograms: f64,
    pub local_liquid_grant_kilograms: f64,
    /// Approximate ideal sharing cap within this one existing connected body.
    /// No grants are allocated, withdrawn, or transported by this diagnostic.
    pub perfect_body_sharing_cap_kilograms: f64,
    pub additional_sharing_cap_kilograms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Audit {
    pub diagnostic_version: String,
    pub elapsed_seconds: u64,
    pub reference_bodies: Vec<Body>,
    pub closed_dry_terminals: Inventory,
    pub other_dry_land: Inventory,
}

fn stock_pairs(state: &State, i: usize) -> [[f64; 2]; 6] {
    let cp = &state.0;
    [
        [
            cp.surface_kilograms[i],
            cp.surface_low_kilograms.as_ref().map_or(0., |v| v[i]),
        ],
        [
            cp.snow_kilograms[i],
            cp.snow_low_kilograms.as_ref().map_or(0., |v| v[i]),
        ],
        [
            cp.soil_kilograms[i],
            cp.soil_low_kilograms.as_ref().map_or(0., |v| v[i]),
        ],
        [cp.pending_runoff_kilograms[i], 0.],
        [
            cp.terminal_water_kilograms[i],
            cp.terminal_low_kilograms.as_ref().map_or(0., |v| v[i]),
        ],
        [cp.vapor_kilograms[i], 0.],
    ]
}
fn inventory(model: &Model, state: &State, regions: &[usize]) -> Inventory {
    let areas: Vec<_> = regions.iter().map(|&i| model.areas[i]).collect();
    let stocks: Vec<_> = regions.iter().map(|&i| stock_pairs(state, i)).collect();
    let transfers: Vec<[f64; 4]> = regions
        .iter()
        .map(|&i| {
            let cp = &state.0;
            [
                cp.cumulative_runoff_transfers[i].terminal_delivery,
                cp.cumulative_runoff_transfers[i].terminal_evaporation,
                cp.cumulative_surface_transfers[i].liquid_evaporation,
                cp.cumulative_surface_transfers[i].soil_evaporation,
            ]
        })
        .collect();
    let mut maximum_terminal_region = None;
    let mut maximum_terminal_kilograms = 0.;
    let mut terminal_areas = Vec::new();
    for (j, &i) in regions.iter().enumerate() {
        let terminal = total_mass(&stocks[j][4]);
        if terminal > 0. {
            terminal_areas.push(model.areas[i]);
        }
        if terminal > maximum_terminal_kilograms {
            maximum_terminal_kilograms = terminal;
            maximum_terminal_region = Some(i);
        }
    }
    Inventory {
        region_count: regions.len(),
        area_square_meters: total_mass(&areas),
        owned_stock_kilograms: std::array::from_fn(|k| {
            total_mass(&stocks.iter().flat_map(|s| s[k]).collect::<Vec<_>>())
        }),
        cumulative_transfer_kilograms: std::array::from_fn(|k| {
            total_mass(&transfers.iter().map(|s| s[k]).collect::<Vec<_>>())
        }),
        terminal_bearing_area_square_meters: total_mass(&terminal_areas),
        maximum_terminal_region,
        maximum_terminal_kilograms,
    }
}

/// Two actual donor copies, preserving conservative signed-low grant limits.
fn local_grant(demand: f64, liquid: [f64; 2], terminal: [f64; 2]) -> Result<f64, String> {
    let mut terminal = CompensatedStock::new(terminal[0], terminal[1], f64::MAX)?;
    let mut liquid = CompensatedStock::new(liquid[0], liquid[1], f64::MAX)?;
    let first = terminal.withdraw(demand);
    Ok(first + liquid.withdraw(demand - first))
}

fn frozen_probe(
    model: &Model,
    state: &State,
    regions: &[usize],
    month: usize,
    seconds: u32,
) -> Result<FrozenProbe, String> {
    let fraction = if model.origin.settings.evaporation_enabled {
        -(-(f64::from(seconds)) / model.origin.settings.evaporation_response_seconds).exp_m1()
    } else {
        0.
    };
    let mut demands = Vec::with_capacity(regions.len());
    let mut grants = Vec::with_capacity(regions.len());
    let mut liquid_components = Vec::with_capacity(regions.len() * 4);
    let mut warm_areas = Vec::new();
    let mut limited_areas = Vec::new();
    for &i in regions {
        let stocks = stock_pairs(state, i);
        liquid_components.extend_from_slice(&stocks[0]);
        liquid_components.extend_from_slice(&stocks[4]);
        let demand = if model.temperatures[month][i] > 0. {
            (model.capacities[month][i] - state.0.vapor_kilograms[i]).max(0.) * fraction
        } else {
            0.
        };
        let grant = local_grant(demand, stocks[0], stocks[4])?;
        if demand > 0. {
            warm_areas.push(model.areas[i]);
        }
        if demand > grant {
            limited_areas.push(model.areas[i]);
        }
        demands.push(demand);
        grants.push(grant);
    }
    let potential = total_mass(&demands);
    let local = total_mass(&grants);
    // This is a diagnostic bound, not a certified directed-rounded mass grant.
    let sharing = potential.min(total_mass(&liquid_components));
    let result = FrozenProbe {
        month,
        probe_seconds: seconds,
        warm_demand_area_square_meters: total_mass(&warm_areas),
        locally_supply_limited_area_square_meters: total_mass(&limited_areas),
        potential_demand_kilograms: potential,
        local_liquid_grant_kilograms: local,
        perfect_body_sharing_cap_kilograms: sharing,
        // Roundoff-scale cancellation can make two equal approximate caps differ.
        // Report the signed difference instead of repairing it into positive gain.
        additional_sharing_cap_kilograms: sharing - local,
    };
    if !probe_finite(&result) {
        return Err("Nonfinite frozen water-return probe.".into());
    }
    Ok(result)
}

fn probe_finite(p: &FrozenProbe) -> bool {
    [
        p.warm_demand_area_square_meters,
        p.locally_supply_limited_area_square_meters,
        p.potential_demand_kilograms,
        p.local_liquid_grant_kilograms,
        p.perfect_body_sharing_cap_kilograms,
        p.additional_sharing_cap_kilograms,
    ]
    .iter()
    .all(|v| v.is_finite())
}
fn inventory_finite(i: &Inventory) -> bool {
    [
        i.area_square_meters,
        i.terminal_bearing_area_square_meters,
        i.maximum_terminal_kilograms,
    ]
    .iter()
    .chain(i.owned_stock_kilograms.iter())
    .chain(i.cumulative_transfer_kilograms.iter())
    .all(|v| v.is_finite())
}

impl Audit {
    /// No simulation step, remapping, source replacement, or mutable reference.
    pub fn capture(model: &Model, state: &State, probe_seconds: u32) -> Result<Self, String> {
        if !(1..=3600).contains(&probe_seconds) {
            return Err("Frozen return probe must be 1–3600 seconds.".into());
        }
        if !matches!(
            model.model_version(),
            "seasonal-moisture-3"
                | "seasonal-moisture-4"
                | "seasonal-moisture-5"
                | "seasonal-moisture-6"
                | "seasonal-moisture-7"
        ) {
            return Err("Unsupported physical law for water-return analysis.".into());
        }
        model.budget(state)?;
        if model.reference_body_ids.len() != model.areas.len()
            || model
                .reference_body_ids
                .iter()
                .enumerate()
                .any(|(i, &id)| (id == 0) != model.is_land[i])
        {
            return Err("Inconsistent immutable reference-water labels.".into());
        }
        let mut bodies = BTreeMap::<u32, Vec<usize>>::new();
        let mut closed = Vec::new();
        let mut land = Vec::new();
        for (i, &id) in model.reference_body_ids.iter().enumerate() {
            if id != 0 {
                bodies.entry(id).or_default().push(i);
            } else if model.routing.is_terminal(i) {
                closed.push(i);
            } else {
                land.push(i);
            }
        }
        let reference_bodies = bodies
            .into_iter()
            .map(|(id, regions)| {
                Ok(Body {
                    reference_body_id: id,
                    inventory: inventory(model, state, &regions),
                    frozen_monthly_liquid_probes: (0..12)
                        .map(|m| frozen_probe(model, state, &regions, m, probe_seconds))
                        .collect::<Result<Vec<_>, String>>()?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let result = Self {
            diagnostic_version: DIAGNOSTIC_VERSION.into(),
            elapsed_seconds: state.elapsed_seconds(),
            reference_bodies,
            closed_dry_terminals: inventory(model, state, &closed),
            other_dry_land: inventory(model, state, &land),
        };
        if !result
            .reference_bodies
            .iter()
            .all(|b| inventory_finite(&b.inventory))
            || !inventory_finite(&result.closed_dry_terminals)
            || !inventory_finite(&result.other_dry_land)
        {
            return Err("Nonfinite water-return ownership inventory.".into());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Recipe, World, water::WaterSettings};

    fn fixture() -> (Model, State) {
        let mut recipe: Recipe = serde_json::from_str(include_str!(
            "../../../docs/scenarios/seasonal-temperature.json"
        ))
        .unwrap();
        recipe.subdivision = 0;
        recipe.plate_count = 4;
        recipe.water = WaterSettings::Coverage { fraction: 1. };
        let world = World::generate(recipe).unwrap();
        let mut model = Model::from_world(
            &world,
            super::super::Settings::default(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut state = model.initial_state();
        state.0.surface_kilograms.fill(0.);
        // Deliberate local-operator inputs, not a fabricated resumable checkpoint.
        // Audit::capture would reject their altered native ownership ledgers.
        let fraction = -(-300. / model.origin.settings.evaporation_response_seconds).exp_m1();
        for m in 0..12 {
            model.temperatures[m].fill(0.);
            model.capacities[m].fill(0.);
            for i in 0..3 {
                model.temperatures[m][i] = 15.;
                model.capacities[m][i] = 10. / fraction;
            }
        }
        (model, state)
    }
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() <= 1e-13 * a.abs().max(b.abs()).max(1.));
    }

    #[test]
    fn identical_body_mass_but_different_localization_changes_only_the_local_cap() {
        let (model, mut state) = fixture();
        state.0.terminal_water_kilograms[0] = 30.;
        let localized = frozen_probe(&model, &state, &[0, 1, 2], 0, 300).unwrap();
        near(localized.potential_demand_kilograms, 30.);
        near(localized.local_liquid_grant_kilograms, 10.);
        near(localized.perfect_body_sharing_cap_kilograms, 30.);
        near(localized.additional_sharing_cap_kilograms, 20.);
        state.0.terminal_water_kilograms[0] = 0.;
        state.0.surface_kilograms[..3].fill(10.);
        let distributed = frozen_probe(&model, &state, &[0, 1, 2], 0, 300).unwrap();
        near(distributed.local_liquid_grant_kilograms, 30.);
        near(
            distributed.perfect_body_sharing_cap_kilograms,
            localized.perfect_body_sharing_cap_kilograms,
        );
        near(distributed.additional_sharing_cap_kilograms, 0.);
    }

    #[test]
    fn mass_limited_cold_and_disconnected_probes_do_not_borrow_other_sources() {
        let (mut model, mut state) = fixture();
        state.0.terminal_water_kilograms[0] = 5.;
        // Region 3 is not in the probed body; arbitrary outside mass is irrelevant.
        state.0.surface_kilograms[3] = 1e12;
        let p = frozen_probe(&model, &state, &[0, 1, 2], 0, 300).unwrap();
        near(p.perfect_body_sharing_cap_kilograms, 5.);
        near(p.local_liquid_grant_kilograms, 5.);
        model.temperatures[0][..3].fill(0.);
        let cold = frozen_probe(&model, &state, &[0, 1, 2], 0, 300).unwrap();
        assert_eq!(cold.potential_demand_kilograms, 0.);
        assert_eq!(cold.local_liquid_grant_kilograms, 0.);
        assert_eq!(cold.perfect_body_sharing_cap_kilograms, 0.);
    }

    #[test]
    fn terminal_priority_and_signed_low_donors_match_independent_dyadic_values() {
        assert_eq!(local_grant(10., [8., 0.], [5., 0.]).unwrap(), 10.);
        assert_eq!(local_grant(10., [3., 0.], [5., 0.]).unwrap(), 8.);
        assert_eq!(
            local_grant(1., [0., 0.], [1., -2_f64.powi(-54)]).unwrap(),
            1_f64.next_down()
        );
        assert_eq!(
            local_grant(1., [1., -2_f64.powi(-54)], [0., 0.]).unwrap(),
            1_f64.next_down()
        );
        assert!(local_grant(1., [1., 0.1], [0., 0.]).is_err());
    }

    #[test]
    fn region_relabeling_preserves_scalar_probe_outcomes() {
        let (mut model, mut state) = fixture();
        state.0.surface_kilograms[0] = 5.;
        state.0.terminal_water_kilograms[2] = 30.;
        model.capacities[0][0] *= 2.;
        model.capacities[0][1] *= 0.5;
        let before = frozen_probe(&model, &state, &[0, 1, 2], 0, 300).unwrap();
        model.areas.swap(0, 2);
        model.capacities[0].swap(0, 2);
        model.temperatures[0].swap(0, 2);
        state.0.surface_kilograms.swap(0, 2);
        state.0.terminal_water_kilograms.swap(0, 2);
        let after = frozen_probe(&model, &state, &[0, 1, 2], 0, 300).unwrap();
        assert_eq!(before, after);
    }
}
