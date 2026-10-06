//! Conservative vertical exchange for one initially terrestrial column.
//! Standalone candidate: no seasonal model or checkpoint silently uses it.
use super::CompensatedStock;
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "ponded-soil-exchange-1";

/// Kilograms owned by one reservoir; the signed low component is not extra water.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mass {
    pub high: f64,
    pub low: f64,
}
impl From<CompensatedStock> for Mass {
    fn from(stock: CompensatedStock) -> Self {
        Self {
            high: stock.high,
            low: stock.low,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    /// All available surface liquid, regardless of its source or depth.
    pub liquid: Mass,
    pub soil: Mass,
    /// Finite recipient of evaporation, not an external loss.
    pub vapor: Mass,
    /// Owned drainage awaiting a separately specified receiving/routing policy.
    pub drainage: Mass,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub soil_capacity_kilograms_per_square_meter: f64,
    pub soil_retained_fraction: f64,
    pub infiltration_response_seconds: f64,
    pub soil_drainage_response_seconds: f64,
}
impl Default for Settings {
    fn default() -> Self {
        let old = super::Settings::default();
        Self {
            soil_capacity_kilograms_per_square_meter: old.soil_capacity_kilograms_per_square_meter,
            soil_retained_fraction: old.soil_retained_fraction,
            infiltration_response_seconds: old.infiltration_response_seconds,
            soil_drainage_response_seconds: old.soil_drainage_response_seconds,
        }
    }
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        super::Settings {
            soil_capacity_kilograms_per_square_meter: self.soil_capacity_kilograms_per_square_meter,
            soil_retained_fraction: self.soil_retained_fraction,
            infiltration_response_seconds: self.infiltration_response_seconds,
            soil_drainage_response_seconds: self.soil_drainage_response_seconds,
            ..Default::default()
        }
        .validate()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transfers {
    pub liquid_evaporation_kilograms: f64,
    pub soil_evaporation_kilograms: f64,
    pub infiltration_kilograms: f64,
    pub soil_drainage_kilograms: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step {
    pub state: State,
    pub transfers: Transfers,
    /// Liquid, soil, vapor, drainage identities, in that order.
    pub stock_residuals_kilograms: [f64; 4],
}

fn difference(after: CompensatedStock, before: CompensatedStock) -> f64 {
    crate::moisture_transport::total_mass(&[after.high - before.high, after.low, -before.low])
}

/// A single scalar grant debits and credits the normalized finite stocks.
/// Unrepresentable grants reject the whole value-owned candidate, not its ledger.
fn transfer(
    donor: &mut CompensatedStock,
    recipient: &mut CompensatedStock,
    request: f64,
    capacity: f64,
) -> Result<f64, String> {
    let before_donor = *donor;
    let before_recipient = *recipient;
    let grant = recipient.deposit(request.min(donor.available()), capacity);
    if donor.withdraw(grant) != grant {
        return Err("Ponded-soil donor cannot fund its recipient grant.".into());
    }
    CompensatedStock::new(donor.high, donor.low, f64::MAX)?;
    CompensatedStock::new(recipient.high, recipient.low, capacity)?;
    let tolerance = 32. * f64::EPSILON * grant;
    if (difference(*donor, before_donor) + grant).abs() > tolerance
        || (difference(*recipient, before_recipient) - grant).abs() > tolerance
    {
        return Err("Ponded-soil transfer is below the represented stock precision.".into());
    }
    Ok(grant)
}

/// One empirical warm-column step, with no binary wet/dry switch.
/// Evaporation uses a shared demand, followed by infiltration and soil drainage.
/// Temperature gating is inherited from the old closure, not soil thermodynamics.
pub fn advance(
    before: State,
    area_square_meters: f64,
    temperature_celsius: f64,
    evaporation_demand_kilograms: f64,
    seconds: f64,
    settings: Settings,
) -> Result<Step, String> {
    settings.validate()?;
    let capacity = area_square_meters * settings.soil_capacity_kilograms_per_square_meter;
    if !area_square_meters.is_finite()
        || area_square_meters <= 0.
        || !capacity.is_finite()
        || capacity <= 0.
        || !temperature_celsius.is_finite()
        || !(-100. ..=50.).contains(&temperature_celsius)
        || !evaporation_demand_kilograms.is_finite()
        || evaporation_demand_kilograms < 0.
        || !seconds.is_finite()
        || !(0. ..=86400.).contains(&seconds)
    {
        return Err("Invalid ponded-soil area, forcing, temperature, or interval.".into());
    }
    let decode = |mass: Mass, cap| CompensatedStock::new(mass.high, mass.low, cap);
    let initial = [
        decode(before.liquid, f64::MAX)?,
        decode(before.soil, capacity)?,
        decode(before.vapor, f64::MAX)?,
        decode(before.drainage, f64::MAX)?,
    ];
    let [mut liquid, mut soil, mut vapor, mut drainage] = initial;
    let mut transfers = Transfers::default();
    if seconds > 0. && temperature_celsius > 0. {
        transfers.liquid_evaporation_kilograms = transfer(
            &mut liquid,
            &mut vapor,
            evaporation_demand_kilograms,
            f64::MAX,
        )?;
        let soil_demand = (evaporation_demand_kilograms - transfers.liquid_evaporation_kilograms)
            * (soil.high / capacity);
        transfers.soil_evaporation_kilograms =
            transfer(&mut soil, &mut vapor, soil_demand, f64::MAX)?;

        // Integrate dB/dt = (C-B)/tau under a continuously funded liquid supply.
        // The finite surface donor caps this potential; no depth threshold applies.
        let room =
            crate::moisture_transport::total_mass(&[capacity, -soil.high, -soil.low]).max(0.);
        let potential = room * -(-seconds / settings.infiltration_response_seconds).exp_m1();
        transfers.infiltration_kilograms = transfer(&mut liquid, &mut soil, potential, capacity)?;

        let retained = capacity * settings.soil_retained_fraction;
        let excess =
            crate::moisture_transport::total_mass(&[soil.high, -retained, soil.low]).max(0.);
        let potential = excess * -(-seconds / settings.soil_drainage_response_seconds).exp_m1();
        transfers.soil_drainage_kilograms =
            transfer(&mut soil, &mut drainage, potential, f64::MAX)?;
    }
    let final_stocks = [liquid, soil, vapor, drainage];
    let t = transfers;
    let fluxes = [
        [
            -t.liquid_evaporation_kilograms,
            -t.infiltration_kilograms,
            0.,
        ],
        [
            -t.soil_evaporation_kilograms,
            t.infiltration_kilograms,
            -t.soil_drainage_kilograms,
        ],
        [
            t.liquid_evaporation_kilograms,
            t.soil_evaporation_kilograms,
            0.,
        ],
        [t.soil_drainage_kilograms, 0., 0.],
    ];
    let mut residuals = [0.; 4];
    for i in 0..4 {
        let terms = [
            final_stocks[i].high - initial[i].high,
            final_stocks[i].low,
            -initial[i].low,
            -fluxes[i][0],
            -fluxes[i][1],
            -fluxes[i][2],
        ];
        residuals[i] = crate::moisture_transport::total_mass(&terms);
        let scale = initial[i]
            .high
            .max(final_stocks[i].high)
            .max(fluxes[i].iter().map(|v| v.abs()).fold(0., f64::max));
        if !residuals[i].is_finite() || residuals[i].abs() > 32. * f64::EPSILON * scale {
            return Err(
                "Ponded-soil local stock identity exceeds its arithmetic tolerance.".into(),
            );
        }
    }
    Ok(Step {
        state: State {
            liquid: liquid.into(),
            soil: soil.into(),
            vapor: vapor.into(),
            drainage: drainage.into(),
        },
        transfers,
        stock_residuals_kilograms: residuals,
    })
}
