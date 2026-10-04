//! Read-only whole-year diagnostics for the fixed connected-body physical mode.
//! No virtual regional liquid allocation, cold-lock inference, or warm-start reset.
use super::{Model, REFERENCE_POOL_MODEL_VERSION, State, preparation};
use crate::moisture_transport::total_mass;
use serde::Serialize;

pub use preparation::{Criteria, YEAR_SECONDS};
pub const DIAGNOSTIC_VERSION: &str = "body-seasonal-preparation-analysis-1";

struct Snapshot {
    regional: preparation::Snapshot,
    body_stocks: Vec<[f64; 2]>,
    // Rain, melt, actual routed arrivals, liquid evaporation. Leading totals,
    // not independently integrated annual transfers or correction mass.
    cumulative_body_flows: Vec<[f64; 4]>,
}
impl Snapshot {
    fn capture(model: &Model, state: &State) -> Result<Self, String> {
        let regional = preparation::Snapshot::capture(model, state)?;
        let cp = &state.0;
        let layout = model
            .reference_pool
            .as_ref()
            .ok_or("Missing body layout.")?;
        let body_stocks = cp
            .reference_body_high_kilograms
            .as_ref()
            .unwrap()
            .iter()
            .zip(cp.reference_body_low_kilograms.as_ref().unwrap())
            .map(|(&high, &low)| [high, low])
            .collect();
        let cumulative_body_flows: Vec<[f64; 4]> = layout
            .members
            .iter()
            .map(|members| {
                std::array::from_fn(|k| {
                    total_mass(
                        &members
                            .iter()
                            .map(|&i| {
                                let f = cp.cumulative_surface_transfers[i];
                                match k {
                                    0 => f.rain,
                                    1 => f.melt,
                                    2 => cp.cumulative_runoff_transfers[i].terminal_delivery,
                                    _ => f.liquid_evaporation,
                                }
                            })
                            .collect::<Vec<_>>(),
                    )
                })
            })
            .collect();
        if cumulative_body_flows
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || *v < 0.)
        {
            return Err("Nonfinite cumulative body flow diagnostic.".into());
        }
        Ok(Self {
            regional,
            body_stocks,
            cumulative_body_flows,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyAssessment {
    pub reference_body_id: u32,
    pub reference_area_square_meters: f64,
    pub stock_high_kilograms: f64,
    pub stock_low_kilograms: f64,
    pub signed_stock_change_kilograms: f64,
    pub component_l1_change_kilograms: f64,
    /// Divides by the immutable reference area, not a derived wet surface.
    pub component_l1_change_millimeters: f64,
    pub annual_flow_kilograms: [f64; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnualAssessment {
    pub start_seconds: u64,
    pub end_seconds: u64,
    /// These six stocks exclude body-owned liquid; reference liquid/terminal are zero.
    pub regional_state_change: preparation::StateChange,
    pub regional_component_l1_change_by_stock_kilograms: [f64; 6],
    pub reference_body_component_l1_change_kilograms: f64,
    pub relative_total_inventory_component_l1_change: f64,
    pub maximum_body_component_l1_change_millimeters: f64,
    pub maximum_body_change_id: Option<u32>,
    pub flow_change_from_previous_year: Option<preparation::FlowChange>,
    pub annual_flow_kilograms: [f64; 5],
    pub annual_flow_global_millimeters: [f64; 5],
    pub reference_bodies: Vec<BodyAssessment>,
    pub closed_dry_terminal_water_kilograms: f64,
    pub meets_recorded_stationarity_criteria: Option<bool>,
    pub consecutive_matching_comparisons: u32,
    /// A recorded diagnostic candidate, never physical/eco/climate readiness.
    pub stationarity_candidate: bool,
    pub positive_atmospheric_cycling_observed: bool,
}

fn body_assessments(
    before: &Snapshot,
    after: &Snapshot,
    ids: &[u32],
    areas: &[f64],
) -> Result<Vec<BodyAssessment>, String> {
    ids.iter()
        .enumerate()
        .map(|(b, &id)| {
            let [old_high, old_low] = before.body_stocks[b];
            let [high, low] = after.body_stocks[b];
            let components = total_mass(&[(high - old_high).abs(), (low - old_low).abs()]);
            let flows: [f64; 4] = std::array::from_fn(|k| {
                after.cumulative_body_flows[b][k] - before.cumulative_body_flows[b][k]
            });
            // Difference corresponding components before adding them; summing
            // four large signed values can erase the very low tail we report.
            let signed = total_mass(&[high - old_high, low - old_low]);
            let mm = components / areas[b];
            if !components.is_finite()
                || !signed.is_finite()
                || !mm.is_finite()
                || flows.iter().any(|v| !v.is_finite() || *v < 0.)
            {
                return Err(
                    "Nonfinite body change or decreasing annual body flow diagnostic.".into(),
                );
            }
            Ok(BodyAssessment {
                reference_body_id: id,
                reference_area_square_meters: areas[b],
                stock_high_kilograms: high,
                stock_low_kilograms: low,
                signed_stock_change_kilograms: signed,
                component_l1_change_kilograms: components,
                component_l1_change_millimeters: mm,
                annual_flow_kilograms: flows,
            })
        })
        .collect()
}

/// O(N+B) previous boundary/flow state; no per-tick hook or physical-state mutation.
pub struct Monitor<'a> {
    model: &'a Model,
    criteria: Criteria,
    body_areas: Vec<f64>,
    before: Snapshot,
    previous_annual_flows: Option<Vec<[f64; 5]>>,
    consecutive: u32,
}
impl<'a> Monitor<'a> {
    pub fn new(model: &'a Model, state: &State, criteria: Criteria) -> Result<Self, String> {
        criteria.validate()?;
        if model.model_version() != REFERENCE_POOL_MODEL_VERSION || model.reference_pool.is_none() {
            return Err("Body preparation analysis requires seasonal-moisture-8.".into());
        }
        if !state.elapsed_seconds().is_multiple_of(YEAR_SECONDS) {
            return Err("Body preparation analysis must anchor at a whole-year boundary.".into());
        }
        let before = Snapshot::capture(model, state)?;
        let body_areas: Vec<_> = model
            .reference_pool
            .as_ref()
            .unwrap()
            .members
            .iter()
            .map(|members| total_mass(&members.iter().map(|&i| model.areas[i]).collect::<Vec<_>>()))
            .collect();
        if body_areas.iter().any(|a| !a.is_finite() || *a <= 0.) {
            return Err("Invalid reference-body diagnostic area.".into());
        }
        Ok(Self {
            model,
            criteria,
            body_areas,
            before,
            previous_annual_flows: None,
            consecutive: 0,
        })
    }
    pub fn last_observed_seconds(&self) -> u64 {
        self.before.regional.seconds
    }
    pub fn criteria(&self) -> Criteria {
        self.criteria
    }

    pub fn observe_year(&mut self, state: &State) -> Result<AnnualAssessment, String> {
        if state.elapsed_seconds() != self.before.regional.seconds + YEAR_SECONDS {
            return Err(
                "Body preparation observations require the next complete consecutive year.".into(),
            );
        }
        let after = Snapshot::capture(self.model, state)?;
        let annual = after.regional.annual_flows(&self.before.regional)?;
        let regional = preparation::state_change(
            &self.before.regional,
            &after.regional,
            &self.model.areas,
            self.model.initial_total,
        );
        let by_stock: [f64; 6] = std::array::from_fn(|k| {
            total_mass(
                &(0..self.model.areas.len())
                    .flat_map(|i| {
                        std::array::from_fn::<_, 2, _>(|c| {
                            (after.regional.stocks[i][k][c] - self.before.regional.stocks[i][k][c])
                                .abs()
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        });
        let bodies = body_assessments(
            &self.before,
            &after,
            &self.model.reference_pool.as_ref().unwrap().ids,
            &self.body_areas,
        )?;
        let body_l1 = total_mass(
            &bodies
                .iter()
                .map(|b| b.component_l1_change_kilograms)
                .collect::<Vec<_>>(),
        );
        let relative =
            total_mass(&[total_mass(&by_stock), body_l1]) / self.model.initial_total.max(1.);
        let mut maximum_body: f64 = 0.;
        let mut body_witness = None;
        for body in &bodies {
            if body.component_l1_change_millimeters > maximum_body {
                maximum_body = body.component_l1_change_millimeters;
                body_witness = Some(body.reference_body_id);
            }
        }
        let change = self.previous_annual_flows.as_ref().map(|previous| {
            preparation::flow_change(previous, &annual, &self.model.areas, self.criteria)
        });
        let matches = change.as_ref().map(|f| {
            meets_criteria(
                self.criteria,
                relative,
                regional.maximum_local_component_l1_change_millimeters,
                maximum_body,
                f.maximum_local_criterion_excess_millimeters,
            )
        });
        let consecutive = if matches == Some(true) {
            self.consecutive + 1
        } else {
            0
        };
        let totals: [f64; 5] =
            std::array::from_fn(|k| total_mass(&annual.iter().map(|f| f[k]).collect::<Vec<_>>()));
        let area = total_mass(&self.model.areas);
        let terminal = total_mass(
            &after
                .regional
                .stocks
                .iter()
                .flat_map(|s| s[4])
                .collect::<Vec<_>>(),
        );
        let result = AnnualAssessment {
            start_seconds: self.before.regional.seconds,
            end_seconds: after.regional.seconds,
            regional_state_change: regional,
            regional_component_l1_change_by_stock_kilograms: by_stock,
            reference_body_component_l1_change_kilograms: body_l1,
            relative_total_inventory_component_l1_change: relative,
            maximum_body_component_l1_change_millimeters: maximum_body,
            maximum_body_change_id: body_witness,
            flow_change_from_previous_year: change,
            annual_flow_kilograms: totals,
            annual_flow_global_millimeters: totals.map(|m| m / area),
            reference_bodies: bodies,
            closed_dry_terminal_water_kilograms: terminal,
            meets_recorded_stationarity_criteria: matches,
            consecutive_matching_comparisons: consecutive,
            stationarity_candidate: consecutive >= self.criteria.required_consecutive_comparisons,
            positive_atmospheric_cycling_observed: totals[0] > 0. && totals[1] > 0.,
        };
        if !finite(&result) {
            return Err("Nonfinite body preparation diagnostic.".into());
        }
        self.before = after;
        self.previous_annual_flows = Some(annual);
        self.consecutive = consecutive;
        Ok(result)
    }
}
fn meets_criteria(
    criteria: Criteria,
    relative: f64,
    regional_mm: f64,
    body_mm: f64,
    flow_excess: f64,
) -> bool {
    relative <= criteria.relative_inventory_component_change
        && regional_mm <= criteria.maximum_local_component_change_millimeters
        && body_mm <= criteria.maximum_local_component_change_millimeters
        && flow_excess == 0.
}
fn finite(r: &AnnualAssessment) -> bool {
    [
        r.regional_state_change
            .relative_inventory_component_l1_change,
        r.regional_state_change
            .maximum_local_component_l1_change_millimeters,
        r.reference_body_component_l1_change_kilograms,
        r.relative_total_inventory_component_l1_change,
        r.maximum_body_component_l1_change_millimeters,
        r.closed_dry_terminal_water_kilograms,
    ]
    .iter()
    .chain(&r.regional_component_l1_change_by_stock_kilograms)
    .chain(&r.annual_flow_kilograms)
    .chain(&r.annual_flow_global_millimeters)
    .all(|v| v.is_finite())
        && r.flow_change_from_previous_year.as_ref().is_none_or(|f| {
            f.maximum_local_change_millimeters.is_finite()
                && f.maximum_local_criterion_excess_millimeters.is_finite()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_owner_and_regional_flow_gate_is_required() {
        let criteria = Criteria::default();
        assert!(meets_criteria(criteria, 0.0005, 0., 0., 0.));
        assert!(!meets_criteria(criteria, 0.0005, 0., 2., 0.));
        assert!(!meets_criteria(criteria, 0.0005, 2., 0., 0.));
        assert!(!meets_criteria(criteria, 0.002, 0., 0., 0.));
        assert!(!meets_criteria(criteria, 0.0005, 0., 0., 0.1));
    }
    fn snapshot(body: Vec<[f64; 2]>, flows: Vec<[f64; 4]>) -> Snapshot {
        Snapshot {
            regional: preparation::Snapshot {
                seconds: 0,
                stocks: vec![],
                cumulative_flows: vec![],
            },
            body_stocks: body,
            cumulative_body_flows: flows,
        }
    }
    #[test]
    fn opposite_body_changes_and_signed_low_changes_do_not_cancel() {
        let a = snapshot(vec![[16., 0.], [16., 0.]], vec![[0.; 4]; 2]);
        let b = snapshot(vec![[17., 0.], [15., 0.]], vec![[1.; 4]; 2]);
        let reports = body_assessments(&a, &b, &[3, 8], &[2., 4.]).unwrap();
        assert_eq!(reports[0].signed_stock_change_kilograms, 1.);
        assert_eq!(reports[1].signed_stock_change_kilograms, -1.);
        assert_eq!(
            reports
                .iter()
                .map(|b| b.component_l1_change_kilograms)
                .sum::<f64>(),
            2.
        );
        assert_eq!(reports[0].component_l1_change_millimeters, 0.5);
        let a = snapshot(vec![[16., 0.]], vec![[0.; 4]]);
        let b = snapshot(vec![[16., 2_f64.powi(-51)]], vec![[0.; 4]]);
        assert_eq!(
            body_assessments(&a, &b, &[1], &[1.]).unwrap()[0].component_l1_change_kilograms,
            2_f64.powi(-51)
        );
    }
    #[test]
    fn representation_redistribution_is_visible_and_invalid_reductions_reject() {
        let a = snapshot(vec![[1., -2_f64.powi(-54)]], vec![[1.; 4]]);
        let b = snapshot(vec![[1_f64.next_down(), 2_f64.powi(-54)]], vec![[1.; 4]]);
        let r = body_assessments(&a, &b, &[1], &[1.]).unwrap();
        assert_eq!(r[0].signed_stock_change_kilograms, 0.);
        assert_eq!(r[0].component_l1_change_kilograms, 2_f64.powi(-52));
        let decreasing = snapshot(vec![[1., 0.]], vec![[0.; 4]]);
        assert!(body_assessments(&a, &decreasing, &[1], &[1.]).is_err());
        let overflow = snapshot(vec![[f64::MAX, 0.]], vec![[f64::INFINITY; 4]]);
        assert!(body_assessments(&a, &overflow, &[1], &[1.]).is_err());
    }
}
