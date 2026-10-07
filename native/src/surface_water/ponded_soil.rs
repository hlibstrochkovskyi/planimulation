//! Conservative vertical exchange for one initially terrestrial column.
//! Explicitly pinned operators; legacy seasonal models remain unchanged.
use super::CompensatedStock;
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "ponded-soil-exchange-1";
pub const RECORDED_MODEL_VERSION: &str = "ponded-soil-exchange-2";

/// Kilograms owned by one reservoir; the signed low component is not extra water.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mass {
    pub high: f64,
    pub low: f64,
}
impl Mass {
    pub(crate) fn stock(self, capacity: f64) -> Result<CompensatedStock, String> {
        CompensatedStock::new(self.high, self.low, capacity)
    }
    pub(crate) fn credit(&mut self, amount: f64) -> Result<(), String> {
        if !self.try_credit(amount)? {
            return Err("Cumulative transfer is below represented history precision.".into());
        }
        Ok(())
    }
    fn try_credit(&mut self, amount: f64) -> Result<bool, String> {
        if !amount.is_finite() || amount < 0. {
            return Err("Invalid cumulative transfer request.".into());
        }
        let before = self.stock(f64::MAX)?;
        let mut after = before;
        after.credit(amount)?;
        Mass::from(after).stock(f64::MAX)?;
        if (difference(after, before) - amount).abs() > 32. * f64::EPSILON * amount {
            return Ok(false);
        }
        *self = after.into();
        Ok(true)
    }
}

/// Deferred requests are diagnostics, never water stocks or completed transfers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolutionBudget {
    pub deferred_requests: u64,
    pub summed_deferred_request_kilograms: f64,
    pub maximum_deferred_request_kilograms: f64,
}
impl ResolutionBudget {
    pub fn validate(self) -> Result<(), String> {
        let sum = self.summed_deferred_request_kilograms;
        let max = self.maximum_deferred_request_kilograms;
        if !sum.is_finite()
            || !max.is_finite()
            || max < 0.
            || sum < max
            || (self.deferred_requests == 0 && (sum != 0. || max != 0.))
            || (self.deferred_requests > 0 && max == 0.)
        {
            return Err("Invalid numerical-resolution diagnostics.".into());
        }
        Ok(())
    }
    pub(crate) fn combine(&mut self, other: Self) -> Result<(), String> {
        self.validate()?;
        other.validate()?;
        let next = Self {
            deferred_requests: self
                .deferred_requests
                .checked_add(other.deferred_requests)
                .ok_or("Numerical-resolution count overflow.")?,
            summed_deferred_request_kilograms: self.summed_deferred_request_kilograms
                + other.summed_deferred_request_kilograms,
            maximum_deferred_request_kilograms: self
                .maximum_deferred_request_kilograms
                .max(other.maximum_deferred_request_kilograms),
        };
        next.validate()?;
        *self = next;
        Ok(())
    }
    fn defer(&mut self, request: f64) -> Result<(), String> {
        self.combine(Self {
            deferred_requests: 1,
            summed_deferred_request_kilograms: request,
            maximum_deferred_request_kilograms: request,
        })
    }
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
pub(crate) fn transfer(
    donor: &mut CompensatedStock,
    recipient: &mut CompensatedStock,
    request: f64,
    capacity: f64,
) -> Result<f64, String> {
    try_transfer(donor, recipient, request, capacity)?
        .ok_or_else(|| "Ponded-soil transfer is below the represented stock precision.".into())
}
fn try_transfer(
    donor: &mut CompensatedStock,
    recipient: &mut CompensatedStock,
    request: f64,
    capacity: f64,
) -> Result<Option<f64>, String> {
    if !request.is_finite() || request < 0. {
        return Err("Invalid ponded-soil transfer request.".into());
    }
    let before_donor = *donor;
    let before_recipient = *recipient;
    let mut next_donor = *donor;
    let mut next_recipient = *recipient;
    let grant = next_recipient.deposit(request.min(next_donor.available()), capacity);
    if next_donor.withdraw(grant) != grant {
        return Err("Ponded-soil donor cannot fund its recipient grant.".into());
    }
    CompensatedStock::new(next_donor.high, next_donor.low, f64::MAX)?;
    CompensatedStock::new(next_recipient.high, next_recipient.low, capacity)?;
    let tolerance = 32. * f64::EPSILON * grant;
    if (difference(next_donor, before_donor) + grant).abs() > tolerance
        || (difference(next_recipient, before_recipient) - grant).abs() > tolerance
    {
        return Ok(None);
    }
    *donor = next_donor;
    *recipient = next_recipient;
    Ok(Some(grant))
}

/// Commit both owners and the actual history together, or retain the entire grant.
pub(crate) fn transfer_recorded(
    donor: &mut CompensatedStock,
    recipient: &mut CompensatedStock,
    request: f64,
    capacity: f64,
    history: &mut Mass,
    resolution: &mut ResolutionBudget,
) -> Result<f64, String> {
    history.stock(f64::MAX)?;
    resolution.validate()?;
    let (mut d, mut r, mut h) = (*donor, *recipient, *history);
    let Some(grant) = try_transfer(&mut d, &mut r, request, capacity)? else {
        resolution.defer(request.min(donor.available()))?;
        return Ok(0.);
    };
    if !h.try_credit(grant)? {
        resolution.defer(grant)?;
        return Ok(0.);
    }
    (*donor, *recipient, *history) = (d, r, h);
    Ok(grant)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecordedState {
    pub stocks: State,
    /// Cumulative liquid evaporation, soil evaporation, infiltration, drainage.
    pub transfers: [Mass; 4],
}
pub struct RecordedStep {
    pub step: Step,
    pub transfers: [Mass; 4],
    pub resolution: ResolutionBudget,
}
pub fn advance_recorded(
    before: RecordedState,
    area: f64,
    temperature: f64,
    demand: f64,
    seconds: f64,
    settings: Settings,
) -> Result<RecordedStep, String> {
    for mass in before.transfers {
        mass.stock(f64::MAX)?;
    }
    let mut recording = (before.transfers, ResolutionBudget::default());
    let step = advance_inner(
        before.stocks,
        area,
        temperature,
        demand,
        seconds,
        settings,
        Some(&mut recording),
    )?;
    Ok(RecordedStep {
        step,
        transfers: recording.0,
        resolution: recording.1,
    })
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
    advance_inner(
        before,
        area_square_meters,
        temperature_celsius,
        evaporation_demand_kilograms,
        seconds,
        settings,
        None,
    )
}
fn advance_inner(
    before: State,
    area_square_meters: f64,
    temperature_celsius: f64,
    evaporation_demand_kilograms: f64,
    seconds: f64,
    settings: Settings,
    mut recording: Option<&mut ([Mass; 4], ResolutionBudget)>,
) -> Result<Step, String> {
    fn exchange(
        d: &mut CompensatedStock,
        r: &mut CompensatedStock,
        request: f64,
        capacity: f64,
        index: usize,
        recording: &mut Option<&mut ([Mass; 4], ResolutionBudget)>,
    ) -> Result<f64, String> {
        match recording {
            Some((histories, resolution)) => {
                transfer_recorded(d, r, request, capacity, &mut histories[index], resolution)
            }
            None => transfer(d, r, request, capacity),
        }
    }
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
        transfers.liquid_evaporation_kilograms = exchange(
            &mut liquid,
            &mut vapor,
            evaporation_demand_kilograms,
            f64::MAX,
            0,
            &mut recording,
        )?;
        let soil_demand = (evaporation_demand_kilograms - transfers.liquid_evaporation_kilograms)
            * (soil.high / capacity);
        transfers.soil_evaporation_kilograms = exchange(
            &mut soil,
            &mut vapor,
            soil_demand,
            f64::MAX,
            1,
            &mut recording,
        )?;

        // Integrate dB/dt = (C-B)/tau under a continuously funded liquid supply.
        // The finite surface donor caps this potential; no depth threshold applies.
        let room =
            crate::moisture_transport::total_mass(&[capacity, -soil.high, -soil.low]).max(0.);
        let potential = room * -(-seconds / settings.infiltration_response_seconds).exp_m1();
        transfers.infiltration_kilograms = exchange(
            &mut liquid,
            &mut soil,
            potential,
            capacity,
            2,
            &mut recording,
        )?;

        let retained = capacity * settings.soil_retained_fraction;
        let excess =
            crate::moisture_transport::total_mass(&[soil.high, -retained, soil.low]).max(0.);
        let potential = excess * -(-seconds / settings.soil_drainage_response_seconds).exp_m1();
        transfers.soil_drainage_kilograms = exchange(
            &mut soil,
            &mut drainage,
            potential,
            f64::MAX,
            3,
            &mut recording,
        )?;
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
