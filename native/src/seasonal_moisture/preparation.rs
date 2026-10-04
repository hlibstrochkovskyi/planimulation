//! Read-only year-boundary diagnostics, not a warm-start state mutation.
//! Stationarity under recorded project tolerances is not physical readiness.
use super::{Model, SECONDS_PER_DAY, State};
use crate::moisture_transport::total_mass;
use serde::{Deserialize, Serialize};

pub const DIAGNOSTIC_VERSION: &str = "seasonal-preparation-analysis-1";
pub const YEAR_SECONDS: u64 = 365 * SECONDS_PER_DAY;
const STOCKS: usize = 6;
const FLOWS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Criteria {
    pub relative_inventory_component_change: f64,
    pub maximum_local_component_change_millimeters: f64,
    pub relative_annual_flow_change: f64,
    pub absolute_annual_flow_change_millimeters: f64,
    pub required_consecutive_comparisons: u32,
}
impl Default for Criteria {
    fn default() -> Self {
        Self {
            relative_inventory_component_change: 0.001,
            maximum_local_component_change_millimeters: 1.,
            relative_annual_flow_change: 0.001,
            absolute_annual_flow_change_millimeters: 0.1,
            required_consecutive_comparisons: 3,
        }
    }
}
impl Criteria {
    pub fn validate(self) -> Result<(), String> {
        if [
            self.relative_inventory_component_change,
            self.relative_annual_flow_change,
        ]
        .iter()
        .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || [
                self.maximum_local_component_change_millimeters,
                self.absolute_annual_flow_change_millimeters,
            ]
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=10_000.).contains(v))
            || !(1..=10).contains(&self.required_consecutive_comparisons)
        {
            return Err("Invalid recorded seasonal stationarity criteria.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThermalRegime {
    AlwaysCold,
    SeasonallyWarm,
    AlwaysWarm,
}
fn classify(temperatures: impl Iterator<Item = f64>) -> ThermalRegime {
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for t in temperatures {
        min = min.min(t);
        max = max.max(t);
    }
    if max <= 0. {
        ThermalRegime::AlwaysCold
    } else if min > 0. {
        ThermalRegime::AlwaysWarm
    } else {
        ThermalRegime::SeasonallyWarm
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Snapshot {
    pub(super) seconds: u64,
    // Region-major high/low pairs of the same six owned stocks.
    pub(super) stocks: Vec<[[f64; 2]; STOCKS]>,
    // Native cumulative leading totals; roundoff arrays are not physical water.
    pub(super) cumulative_flows: Vec<[f64; FLOWS]>,
}
impl Snapshot {
    pub(super) fn capture(model: &Model, state: &State) -> Result<Self, String> {
        model.budget(state)?;
        let cp = &state.0;
        let stocks = (0..model.areas.len())
            .map(|i| {
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
            })
            .collect();
        let cumulative_flows = (0..model.areas.len())
            .map(|i| {
                let f = cp.cumulative_surface_transfers[i];
                [
                    cp.cumulative_precipitation_kilograms[i],
                    cp.cumulative_evaporation_kilograms[i],
                    f.liquid_runoff + f.soil_drainage,
                    f.snowfall,
                    cp.cumulative_runoff_transfers[i].terminal_delivery,
                ]
            })
            .collect();
        Ok(Self {
            seconds: state.elapsed_seconds(),
            stocks,
            cumulative_flows,
        })
    }
    pub(super) fn annual_flows(&self, before: &Self) -> Result<Vec<[f64; FLOWS]>, String> {
        let result: Vec<_> = self
            .cumulative_flows
            .iter()
            .zip(&before.cumulative_flows)
            .map(|(a, b)| std::array::from_fn(|k| a[k] - b[k]))
            .collect();
        if result.iter().flatten().any(|v| !v.is_finite() || *v < 0.) {
            return Err("Nonfinite or decreasing cumulative annual flow diagnostic.".into());
        }
        Ok(result)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateChange {
    pub relative_inventory_component_l1_change: f64,
    pub maximum_local_component_l1_change_millimeters: f64,
    pub maximum_local_change_region: usize,
}
pub(super) fn state_change(
    before: &Snapshot,
    after: &Snapshot,
    areas: &[f64],
    initial: f64,
) -> StateChange {
    let mut regional = Vec::with_capacity(areas.len());
    let (mut maximum, mut witness) = (0., 0);
    for (i, &area) in areas.iter().enumerate() {
        let differences: [f64; STOCKS * 2] = std::array::from_fn(|k| {
            (before.stocks[i][k / 2][k % 2] - after.stocks[i][k / 2][k % 2]).abs()
        });
        let mass = total_mass(&differences);
        regional.push(mass);
        if mass / area > maximum {
            maximum = mass / area;
            witness = i;
        }
    }
    StateChange {
        relative_inventory_component_l1_change: total_mass(&regional) / initial.max(1.),
        maximum_local_component_l1_change_millimeters: maximum,
        maximum_local_change_region: witness,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowChange {
    pub maximum_local_change_millimeters: f64,
    pub maximum_local_change_region: usize,
    pub maximum_local_change_flow: usize,
    pub maximum_local_criterion_excess_millimeters: f64,
    pub maximum_criterion_excess_region: usize,
    pub maximum_criterion_excess_flow: usize,
}
pub(super) fn flow_change(
    before: &[[f64; FLOWS]],
    after: &[[f64; FLOWS]],
    areas: &[f64],
    criteria: Criteria,
) -> FlowChange {
    let mut result = FlowChange {
        maximum_local_change_millimeters: 0.,
        maximum_local_change_region: 0,
        maximum_local_change_flow: 0,
        maximum_local_criterion_excess_millimeters: 0.,
        maximum_criterion_excess_region: 0,
        maximum_criterion_excess_flow: 0,
    };
    for (i, &area) in areas.iter().enumerate() {
        for k in 0..FLOWS {
            let difference = (after[i][k] - before[i][k]).abs() / area;
            let allowed = criteria.absolute_annual_flow_change_millimeters
                + criteria.relative_annual_flow_change * after[i][k].max(before[i][k]) / area;
            if difference > result.maximum_local_change_millimeters {
                result.maximum_local_change_millimeters = difference;
                result.maximum_local_change_region = i;
                result.maximum_local_change_flow = k;
            }
            let excess = difference - allowed;
            if excess > result.maximum_local_criterion_excess_millimeters {
                result.maximum_local_criterion_excess_millimeters = excess;
                result.maximum_criterion_excess_region = i;
                result.maximum_criterion_excess_flow = k;
            }
        }
    }
    result
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThermalInventory {
    pub regime: ThermalRegime,
    pub region_count: usize,
    pub area_square_meters: f64,
    pub owned_stock_kilograms: [f64; STOCKS],
}
fn inventories(
    snapshot: &Snapshot,
    areas: &[f64],
    regimes: &[ThermalRegime],
) -> Vec<ThermalInventory> {
    [
        ThermalRegime::AlwaysCold,
        ThermalRegime::SeasonallyWarm,
        ThermalRegime::AlwaysWarm,
    ]
    .into_iter()
    .map(|regime| {
        let indices: Vec<_> = regimes
            .iter()
            .enumerate()
            .filter_map(|(i, &r)| (r == regime).then_some(i))
            .collect();
        ThermalInventory {
            regime,
            region_count: indices.len(),
            area_square_meters: total_mass(&indices.iter().map(|&i| areas[i]).collect::<Vec<_>>()),
            owned_stock_kilograms: std::array::from_fn(|k| {
                total_mass(
                    &indices
                        .iter()
                        .flat_map(|&i| snapshot.stocks[i][k])
                        .collect::<Vec<_>>(),
                )
            }),
        }
    })
    .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnualAssessment {
    pub start_seconds: u64,
    pub end_seconds: u64,
    pub state_change: StateChange,
    pub flow_change_from_previous_year: Option<FlowChange>,
    pub annual_flow_kilograms: [f64; FLOWS],
    pub annual_flow_global_millimeters: [f64; FLOWS],
    pub thermal_inventories: Vec<ThermalInventory>,
    pub cold_reference_water_liquid_kilograms: f64,
    pub structurally_cold_locked_kilograms: f64,
    pub structurally_cold_locked_fraction_of_initial_water: Option<f64>,
    pub annual_cold_snowfall_kilograms: f64,
    pub annual_cold_terminal_delivery_kilograms: f64,
    pub cold_snow_stock_change_kilograms: f64,
    pub cold_terminal_stock_change_kilograms: f64,
    pub meets_recorded_stationarity_criteria: Option<bool>,
    pub consecutive_matching_comparisons: u32,
    /// A quiet or dry cycle can satisfy this. Never label it climate readiness.
    pub stationarity_candidate: bool,
    pub positive_atmospheric_cycling_observed: bool,
}

/// Bound to one immutable model. Owns O(N) preceding boundary/flow data, not a
/// ten-year history, and receives no mutable simulation references.
pub struct Monitor<'a> {
    model: &'a Model,
    criteria: Criteria,
    regimes: Vec<ThermalRegime>,
    before: Snapshot,
    previous_annual_flows: Option<Vec<[f64; FLOWS]>>,
    consecutive: u32,
}
impl<'a> Monitor<'a> {
    pub fn new(model: &'a Model, state: &State, criteria: Criteria) -> Result<Self, String> {
        criteria.validate()?;
        // A future sublimation, lake-return, or thermal law invalidates the
        // cold-lock proof. It must opt into a separately reviewed diagnostic.
        if !matches!(
            model.model_version(),
            "seasonal-moisture-3"
                | "seasonal-moisture-4"
                | "seasonal-moisture-5"
                | "seasonal-moisture-6"
                | "seasonal-moisture-7"
        ) {
            return Err("Unsupported physical model for seasonal preparation analysis.".into());
        }
        if !state.elapsed_seconds().is_multiple_of(YEAR_SECONDS) {
            return Err("Preparation analysis must anchor at a whole-year boundary.".into());
        }
        let before = Snapshot::capture(model, state)?;
        let regimes = (0..model.areas.len())
            .map(|i| classify(model.temperatures.iter().map(|m| m[i])))
            .collect();
        Ok(Self {
            model,
            criteria,
            regimes,
            before,
            previous_annual_flows: None,
            consecutive: 0,
        })
    }
    pub fn last_observed_seconds(&self) -> u64 {
        self.before.seconds
    }
    pub fn criteria(&self) -> Criteria {
        self.criteria
    }
    pub fn thermal_regimes(&self) -> &[ThermalRegime] {
        &self.regimes
    }
    /// Only one complete next year is accepted; validation precedes observer commit.
    pub fn observe_year(&mut self, state: &State) -> Result<AnnualAssessment, String> {
        if state.elapsed_seconds() != self.before.seconds + YEAR_SECONDS {
            return Err(
                "Preparation observations require the next complete consecutive year.".into(),
            );
        }
        let after = Snapshot::capture(self.model, state)?;
        let annual = after.annual_flows(&self.before)?;
        let state_change = state_change(
            &self.before,
            &after,
            &self.model.areas,
            self.model.initial_total,
        );
        let flow_change = self
            .previous_annual_flows
            .as_ref()
            .map(|previous| flow_change(previous, &annual, &self.model.areas, self.criteria));
        let matches = flow_change.as_ref().map(|change| {
            state_change.relative_inventory_component_l1_change
                <= self.criteria.relative_inventory_component_change
                && state_change.maximum_local_component_l1_change_millimeters
                    <= self.criteria.maximum_local_component_change_millimeters
                && change.maximum_local_criterion_excess_millimeters == 0.
        });
        let consecutive = if matches == Some(true) {
            self.consecutive + 1
        } else {
            0
        };
        let groups = inventories(&after, &self.model.areas, &self.regimes);
        let previous_groups = inventories(&self.before, &self.model.areas, &self.regimes);
        let cold_liquid = total_mass(
            &(0..self.model.areas.len())
                .filter(|&i| self.regimes[i] == ThermalRegime::AlwaysCold && !self.model.is_land[i])
                .flat_map(|i| after.stocks[i][0])
                .collect::<Vec<_>>(),
        );
        // Cold land liquid can form runoff; cold wet liquid cannot. Transit and
        // vapor move, so neither belongs in this structural lower-bound set.
        let locked = total_mass(&[
            cold_liquid,
            groups[0].owned_stock_kilograms[1],
            groups[0].owned_stock_kilograms[2],
            groups[0].owned_stock_kilograms[4],
        ]);
        let cold_flows = |k| {
            total_mass(
                &(0..self.model.areas.len())
                    .filter(|&i| self.regimes[i] == ThermalRegime::AlwaysCold)
                    .map(|i| annual[i][k])
                    .collect::<Vec<_>>(),
            )
        };
        let totals: [f64; FLOWS] =
            std::array::from_fn(|k| total_mass(&annual.iter().map(|f| f[k]).collect::<Vec<_>>()));
        let area = total_mass(&self.model.areas);
        let result = AnnualAssessment {
            start_seconds: self.before.seconds,
            end_seconds: after.seconds,
            state_change,
            flow_change_from_previous_year: flow_change,
            annual_flow_kilograms: totals,
            annual_flow_global_millimeters: totals.map(|m| m / area),
            thermal_inventories: groups.clone(),
            cold_reference_water_liquid_kilograms: cold_liquid,
            structurally_cold_locked_kilograms: locked,
            structurally_cold_locked_fraction_of_initial_water: (self.model.initial_total > 0.)
                .then(|| locked / self.model.initial_total),
            annual_cold_snowfall_kilograms: cold_flows(3),
            annual_cold_terminal_delivery_kilograms: cold_flows(4),
            cold_snow_stock_change_kilograms: groups[0].owned_stock_kilograms[1]
                - previous_groups[0].owned_stock_kilograms[1],
            cold_terminal_stock_change_kilograms: groups[0].owned_stock_kilograms[4]
                - previous_groups[0].owned_stock_kilograms[4],
            meets_recorded_stationarity_criteria: matches,
            consecutive_matching_comparisons: consecutive,
            stationarity_candidate: consecutive >= self.criteria.required_consecutive_comparisons,
            positive_atmospheric_cycling_observed: totals[0] > 0. && totals[1] > 0.,
        };
        // NaN/overflow in diagnostic reductions must not turn into JSON nulls.
        if !state_change_finite(&result) {
            return Err("Nonfinite seasonal preparation diagnostic.".into());
        }
        self.before = after;
        self.previous_annual_flows = Some(annual);
        self.consecutive = consecutive;
        Ok(result)
    }
}
fn state_change_finite(report: &AnnualAssessment) -> bool {
    [
        report.state_change.relative_inventory_component_l1_change,
        report
            .state_change
            .maximum_local_component_l1_change_millimeters,
        report.cold_reference_water_liquid_kilograms,
        report.structurally_cold_locked_kilograms,
        report.annual_cold_snowfall_kilograms,
        report.annual_cold_terminal_delivery_kilograms,
        report.cold_snow_stock_change_kilograms,
        report.cold_terminal_stock_change_kilograms,
    ]
    .iter()
    .chain(report.annual_flow_kilograms.iter())
    .chain(report.annual_flow_global_millimeters.iter())
    .all(|v| v.is_finite())
        && report
            .structurally_cold_locked_fraction_of_initial_water
            .is_none_or(f64::is_finite)
        && report.thermal_inventories.iter().all(|g| {
            g.area_square_meters.is_finite()
                && g.owned_stock_kilograms.iter().all(|v| v.is_finite())
        })
        && report
            .flow_change_from_previous_year
            .as_ref()
            .is_none_or(|c| {
                c.maximum_local_change_millimeters.is_finite()
                    && c.maximum_local_criterion_excess_millimeters.is_finite()
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thermal_classes_use_operational_monthly_thresholds() {
        assert_eq!(
            classify([-5., 0., -1.].into_iter()),
            ThermalRegime::AlwaysCold
        );
        assert_eq!(
            classify([-5., 1., -1.].into_iter()),
            ThermalRegime::SeasonallyWarm
        );
        assert_eq!(
            classify([1., 2., 3.].into_iter()),
            ThermalRegime::AlwaysWarm
        );
        assert_eq!(
            classify([0., 1.].into_iter()),
            ThermalRegime::SeasonallyWarm
        );
    }
    #[test]
    fn regional_and_component_changes_do_not_cancel_at_equal_global_mass() {
        let mut a = Snapshot {
            seconds: 0,
            stocks: vec![[[0., 0.]; STOCKS]; 2],
            cumulative_flows: vec![[0.; FLOWS]; 2],
        };
        a.stocks[0][0] = [16., 0.];
        a.stocks[1][0] = [8., 0.];
        let mut b = a.clone();
        b.stocks[0][0] = [8., 0.];
        b.stocks[1][0] = [16., 0.];
        let change = state_change(&a, &b, &[2., 8.], 24.);
        assert_eq!(change.relative_inventory_component_l1_change, 16. / 24.);
        assert_eq!(change.maximum_local_component_l1_change_millimeters, 4.);
        assert_eq!(change.maximum_local_change_region, 0);
        b = a.clone();
        b.stocks[0][0][1] = 2_f64.powi(-50);
        assert!(state_change(&a, &b, &[2., 8.], 24.).relative_inventory_component_l1_change > 0.);
    }
    #[test]
    fn local_flow_changes_use_area_and_recorded_absolute_relative_floors() {
        let a = vec![[10., 0., 0., 0., 0.]; 2];
        let mut b = a.clone();
        b[0][0] = 12.;
        b[1][0] = 8.;
        let criteria = Criteria {
            relative_annual_flow_change: 0.,
            absolute_annual_flow_change_millimeters: 0.5,
            ..Default::default()
        };
        let change = flow_change(&a, &b, &[1., 10.], criteria);
        assert_eq!(change.maximum_local_change_millimeters, 2.);
        assert_eq!(change.maximum_local_criterion_excess_millimeters, 1.5);
        assert_eq!(change.maximum_criterion_excess_region, 0);
        let mut b = a.clone();
        b[0][1] = 0.25;
        assert_eq!(
            flow_change(&a, &b, &[1., 10.], criteria).maximum_local_criterion_excess_millimeters,
            0.
        );
    }
    #[test]
    fn opposing_stock_and_low_component_changes_remain_visible() {
        let mut a = Snapshot {
            seconds: 0,
            stocks: vec![[[0., 0.]; STOCKS]],
            cumulative_flows: vec![[0.; FLOWS]],
        };
        a.stocks[0][0] = [16., 0.];
        let mut b = a.clone();
        b.stocks[0][0] = [8., 0.];
        b.stocks[0][4] = [8., 0.];
        let change = state_change(&a, &b, &[2.], 16.);
        assert_eq!(change.relative_inventory_component_l1_change, 1.);
        assert_eq!(change.maximum_local_component_l1_change_millimeters, 8.);
        // Even a representation-only high/low shift is conservatively included,
        // not mistaken for a physical source or discarded by high+low rounding.
        b = a.clone();
        b.stocks[0][0] = [8., 8.];
        assert_eq!(state_change(&a, &b, &[2.], 16.), change);
    }
    #[test]
    fn annual_flow_differences_reject_nonfinite_and_decreasing_totals() {
        let a = Snapshot {
            seconds: 0,
            stocks: vec![[[0., 0.]; STOCKS]],
            cumulative_flows: vec![[1.; FLOWS]],
        };
        let mut b = a.clone();
        b.cumulative_flows[0][0] = 3.;
        assert_eq!(b.annual_flows(&a).unwrap(), [[2., 0., 0., 0., 0.]]);
        for invalid in [0., f64::NAN, f64::INFINITY] {
            b.cumulative_flows[0][0] = invalid;
            assert!(b.annual_flows(&a).is_err());
        }
    }
}
