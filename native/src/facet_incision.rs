//! Frozen-characteristic linear incision with an explicit receiver-height
//! constraint. The limiter changes the numerical process, not a stock ledger.
use crate::{landscape, terrain_patch_facets::Descent, terrain_patch_geometry};
use serde::Serialize;

pub const MODEL_VERSION: &str = "frozen-facet-incision-1";
pub const POWER_MODEL_VERSION: &str = "frozen-facet-power-incision-1";
pub const SPECIFIC_MODEL_VERSION: &str = "frozen-facet-specific-incision-1";
/// E = K_s a^m S; a is specific area (m), K_s is m^(1-m)/year.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpecificSettings {
    pub erodibility_coefficient: f64,
    pub specific_area_exponent: f64,
    pub step_years: f64,
}
impl SpecificSettings {
    pub(crate) fn valid_law(self) -> bool {
        self.erodibility_coefficient.is_finite()
            && self.erodibility_coefficient >= 0.
            && self.specific_area_exponent.is_finite()
            && (0. ..=1.).contains(&self.specific_area_exponent)
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub erodibility_per_year: f64,
    pub step_years: f64,
}
/// E = K A^m S, with slope exponent fixed at one. For area measured in m²,
/// K has units meters^(1-2m)/year, NOT necessarily year^-1.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerSettings {
    pub erodibility_coefficient: f64,
    pub area_exponent: f64,
    pub step_years: f64,
}
impl PowerSettings {
    pub(crate) fn valid_law(self) -> bool {
        self.erodibility_coefficient.is_finite()
            && self.erodibility_coefficient >= 0.
            && self.area_exponent.is_finite()
            && (0. ..=1.).contains(&self.area_exponent)
    }
}
struct Law {
    coefficient: f64,
    exponent: f64,
    step_years: f64,
    model: &'static str,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Constraint {
    pub limited_nodes: usize,
    /// Hypothetical extra incision prevented by the explicit receiver bound.
    /// Never credit this amount to a detached-material inventory.
    pub prevented_incision_meters: Vec<f64>,
    pub prevented_reference_prism_cubic_meters: f64,
    pub maximum_prevented_incision_meters: f64,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultRecord {
    pub model_version: &'static str,
    pub incision: landscape::Incision,
    pub receiver_constraint: Constraint,
}
#[derive(Default)]
struct Sum {
    value: f64,
    correction: f64,
}
impl Sum {
    fn add(&mut self, v: f64) -> Result<(), String> {
        let adjusted = v - self.correction;
        let next = self.value + adjusted;
        self.correction = (next - self.value) - adjusted;
        self.value = next;
        if !v.is_finite() || v < 0. || !next.is_finite() || !self.correction.is_finite() {
            return Err("Facet material accumulation overflowed.".into());
        }
        Ok(())
    }
}

/// Geometry, derivatives and contributing area must come from the same physical
/// routing problem. Numerical shape/DAG validation is not recipe authentication.
pub fn incise(
    h: &[f64],
    reference_areas: &[f64],
    contributing_areas: &[f64],
    descents: &[Descent],
    settings: Settings,
) -> Result<ResultRecord, String> {
    incise_impl(
        h,
        reference_areas,
        contributing_areas,
        descents,
        None,
        Law {
            coefficient: settings.erodibility_per_year,
            exponent: 0.5,
            step_years: settings.step_years,
            model: MODEL_VERSION,
        },
    )
}

/// Separately pinned exponent-aware candidate. The m=.5 path deliberately uses
/// sqrt rather than powf, preserving the old primitive's arithmetic exactly.
pub fn incise_power(
    h: &[f64],
    reference_areas: &[f64],
    contributing_areas: &[f64],
    descents: &[Descent],
    settings: PowerSettings,
) -> Result<ResultRecord, String> {
    if !settings.valid_law() {
        return Err("Invalid power-incision law.".into());
    }
    incise_impl(
        h,
        reference_areas,
        contributing_areas,
        descents,
        None,
        Law {
            coefficient: settings.erodibility_coefficient,
            exponent: settings.area_exponent,
            step_years: settings.step_years,
            model: POWER_MODEL_VERSION,
        },
    )
}
/// Collected physical area remains in the result. Specific pressure is a
/// separate field and is undefined at terminals; never relabel length as m².
pub fn incise_specific(
    h: &[f64],
    reference_areas: &[f64],
    contributing_areas: &[f64],
    specific_areas: &[Option<f64>],
    descents: &[Descent],
    settings: SpecificSettings,
) -> Result<ResultRecord, String> {
    if !settings.valid_law()
        || specific_areas.len() != h.len()
        || specific_areas.iter().zip(descents).any(|(a, d)| match a {
            Some(a) => !a.is_finite() || *a <= 0. || d.branches().next().is_none(),
            None => d.branches().next().is_some(),
        })
    {
        return Err("Invalid specific-incision pressure or law.".into());
    }
    incise_impl(
        h,
        reference_areas,
        contributing_areas,
        descents,
        Some(specific_areas),
        Law {
            coefficient: settings.erodibility_coefficient,
            exponent: settings.specific_area_exponent,
            step_years: settings.step_years,
            model: SPECIFIC_MODEL_VERSION,
        },
    )
}
fn incise_impl(
    h: &[f64],
    reference_areas: &[f64],
    contributing_areas: &[f64],
    descents: &[Descent],
    specific_areas: Option<&[Option<f64>]>,
    law: Law,
) -> Result<ResultRecord, String> {
    let n = h.len();
    if !(1..=terrain_patch_geometry::MAX_CELLS).contains(&n)
        || reference_areas.len() != n
        || contributing_areas.len() != n
        || descents.len() != n
        || h.iter().any(|v| !v.is_finite())
        || reference_areas.iter().any(|v| !v.is_finite() || *v <= 0.)
        || contributing_areas.iter().any(|v| !v.is_finite() || *v < 0.)
        || !law.coefficient.is_finite()
        || law.coefficient < 0.
        || !law.step_years.is_finite()
        || law.step_years <= 0.
    {
        return Err("Invalid facet-incision fields or settings.".into());
    }
    let coefficient = law.coefficient * law.step_years;
    if !coefficient.is_finite() || law.coefficient > 0. && coefficient == 0. {
        return Err("Unrepresentable facet-incision time coefficient.".into());
    }
    let mut incoming = vec![0usize; n];
    for (i, d) in descents.iter().enumerate() {
        if !d.slope.is_finite()
            || d.slope < 0.
            || d.unit_tangent_direction.iter().any(|v| !v.is_finite())
            || d.receivers.iter().any(|&j| j as usize >= n)
            || d.area_weights
                .iter()
                .any(|w| !w.is_finite() || *w < 0. || *w > 1.)
            || d.derivative_coefficients_per_meter
                .iter()
                .any(|a| !a.is_finite() || *a < 0.)
        {
            return Err("Invalid facet-incision descent metadata.".into());
        }
        let mut weights = 0.;
        let mut alpha = 0.;
        let mut count = 0;
        for k in 0..2 {
            let j = d.receivers[k] as usize;
            let a = d.derivative_coefficients_per_meter[k];
            let w = d.area_weights[k];
            if w == 0. {
                if a != 0. {
                    return Err("Inactive facet receiver has a nonzero derivative.".into());
                }
                continue;
            }
            if a <= 0. || j == i || h[j] > h[i] {
                return Err(
                    "Facet receivers must be distinct and no higher than their donors.".into(),
                );
            }
            weights += w;
            alpha += a;
            count += 1;
            incoming[j] += 1;
        }
        if count > 0
            && ((weights - 1.).abs() > 1e-12
                || !alpha.is_finite()
                || alpha <= 0.
                || contributing_areas[i] <= 0.)
            || count == 2 && d.receivers[0] == d.receivers[1]
        {
            return Err("Invalid facet-incision partition or derivative sum.".into());
        }
    }
    let mut order: Vec<_> = (0..n).filter(|&i| incoming[i] == 0).collect();
    let mut cursor = 0;
    while cursor < order.len() {
        let i = order[cursor];
        cursor += 1;
        for (j, _, _) in descents[i].branches() {
            incoming[j] -= 1;
            if incoming[j] == 0 {
                order.push(j);
            }
        }
    }
    if order.len() != n {
        return Err("Facet-incision graph must be acyclic.".into());
    }
    let mut result = landscape::Incision {
        elevation_meters: h.to_vec(),
        removed_thickness_meters: vec![0.; n],
        contributing_area_square_meters: contributing_areas.to_vec(),
        removed_material_cubic_meters: 0.,
        changed_nodes: 0,
        unresolved_positive_incision_nodes: 0,
    };
    let mut constraint = Constraint {
        limited_nodes: 0,
        prevented_incision_meters: vec![0.; n],
        prevented_reference_prism_cubic_meters: 0.,
        maximum_prevented_incision_meters: 0.,
    };
    let mut detached = Sum::default();
    let mut prevented = Sum::default();
    for &i in order.iter().rev() {
        if coefficient == 0. || descents[i].branches().next().is_none() {
            continue;
        }
        let d = &descents[i];
        let alpha_sum: f64 = d.branches().map(|(_, _, a)| a).sum();
        let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
        for (j, _, _) in d.branches() {
            low = low.min(result.elevation_meters[j]);
            high = high.max(result.elevation_meters[j]);
        }
        // Stable two-point interpolation, including the single-edge case.
        let high_fraction: f64 = d
            .branches()
            .filter(|(j, _, _)| result.elevation_meters[*j] == high)
            .map(|(_, _, a)| a / alpha_sum)
            .sum();
        let base = low + (high - low) * high_fraction;
        if !base.is_finite() || !high.is_finite() || base < low || base > high || high > h[i] {
            return Err("Invalid facet-incision receiver interval.".into());
        }
        if h[i] == base {
            continue;
        }
        let difference = h[i] - base;
        let pressure = match specific_areas {
            Some(a) => a[i].ok_or("Missing nonterminal specific-area pressure.")?,
            None => contributing_areas[i],
        };
        let area_factor = if law.exponent == 0.5 {
            pressure.sqrt()
        } else {
            pressure.powf(law.exponent)
        };
        let gamma = coefficient * area_factor * alpha_sum;
        if !difference.is_finite() || difference <= 0. || !gamma.is_finite() || gamma <= 0. {
            return Err("Unrepresentable positive facet-incision coefficient or relief.".into());
        }
        let fraction = if gamma <= 1. {
            gamma / (1. + gamma)
        } else {
            1. / (1. + 1. / gamma)
        };
        let requested = difference * fraction;
        if !requested.is_finite() || requested <= 0. {
            return Err("Unrepresentable requested facet incision.".into());
        }
        let unconstrained = if gamma <= 1. {
            h[i] - requested
        } else {
            base + difference / (1. + gamma)
        };
        if !unconstrained.is_finite() || unconstrained < base || unconstrained > h[i] {
            return Err("Facet incision left its old-bed/weighted-base interval.".into());
        }
        // Declared process constraint, NOT a post-hoc mass repair. Only the
        // accepted constrained loss is debited from the actual bed below.
        let updated = unconstrained.max(high);
        let withheld = updated - unconstrained;
        let actual = h[i] - updated;
        if withheld > 0. {
            constraint.limited_nodes += 1;
            constraint.prevented_incision_meters[i] = withheld;
            constraint.maximum_prevented_incision_meters =
                constraint.maximum_prevented_incision_meters.max(withheld);
        } else if actual == 0. {
            result.unresolved_positive_incision_nodes += 1;
        }
        let volume = actual * reference_areas[i];
        let hypothetical = withheld * reference_areas[i];
        if actual > 0. && volume == 0. || withheld > 0. && hypothetical == 0. {
            return Err("Unrepresentable facet-incision prism.".into());
        }
        detached.add(volume)?;
        prevented.add(hypothetical)?;
        result.elevation_meters[i] = updated;
        result.removed_thickness_meters[i] = actual;
        result.changed_nodes += usize::from(actual > 0.);
    }
    result.removed_material_cubic_meters = detached.value;
    constraint.prevented_reference_prism_cubic_meters = prevented.value;
    Ok(ResultRecord {
        model_version: law.model,
        incision: result,
        receiver_constraint: constraint,
    })
}
