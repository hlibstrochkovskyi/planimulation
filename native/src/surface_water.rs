//! Finite local snow, liquid, soil, and pending-runoff ownership.
//! Empirical temperature-index/bucket closure, not an energy or river solver.
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "surface-water-1";
const DAY: f64 = 86400.;

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub melt_kilograms_per_square_meter_degree_day: f64,
    pub soil_capacity_kilograms_per_square_meter: f64,
    pub soil_retained_fraction: f64,
    pub infiltration_response_seconds: f64,
    pub liquid_runoff_response_seconds: f64,
    pub soil_drainage_response_seconds: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            melt_kilograms_per_square_meter_degree_day: 3.,
            soil_capacity_kilograms_per_square_meter: 150.,
            soil_retained_fraction: 0.6,
            infiltration_response_seconds: 6. * 3600.,
            liquid_runoff_response_seconds: DAY,
            soil_drainage_response_seconds: 30. * DAY,
        }
    }
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        for (v, low, high) in [
            (self.melt_kilograms_per_square_meter_degree_day, 0., 20.),
            (self.soil_capacity_kilograms_per_square_meter, 1., 2000.),
            (self.soil_retained_fraction, 0., 1.),
            (self.infiltration_response_seconds, 3600., 30. * DAY),
            (self.liquid_runoff_response_seconds, 3600., 30. * DAY),
            (self.soil_drainage_response_seconds, DAY, 365. * DAY),
        ] {
            if !v.is_finite() || !(low..=high).contains(&v) {
                return Err("Invalid surface-water settings.".into());
            }
        }
        Ok(())
    }
}

/// All masses are kilograms of water equivalent, including snow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stocks {
    pub liquid: f64,
    pub snow: f64,
    pub soil: f64,
    /// Already removed from liquid/soil; not available to local evaporation.
    pub pending_runoff: f64,
}
impl Stocks {
    pub fn total(self) -> f64 {
        crate::moisture_transport::total_mass(&[
            self.liquid,
            self.snow,
            self.soil,
            self.pending_runoff,
        ])
    }
    pub fn validate(self, area: f64, is_land: bool, settings: Settings) -> Result<(), String> {
        if !area.is_finite()
            || area <= 0.
            || [self.liquid, self.snow, self.soil, self.pending_runoff]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.)
            || !self.total().is_finite()
            || !(area * settings.soil_capacity_kilograms_per_square_meter).is_finite()
            || self.soil > area * settings.soil_capacity_kilograms_per_square_meter
            || (!is_land && (self.soil != 0. || self.pending_runoff != 0.))
        {
            return Err("Invalid typed surface-water stock, area, or ownership.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transfers {
    pub rain: f64,
    pub snowfall: f64,
    pub melt: f64,
    pub liquid_evaporation: f64,
    pub soil_evaporation: f64,
    pub infiltration: f64,
    pub liquid_runoff: f64,
    pub soil_drainage: f64,
}
impl Transfers {
    pub fn from_values(v: [f64; 8]) -> Self {
        Self {
            rain: v[0],
            snowfall: v[1],
            melt: v[2],
            liquid_evaporation: v[3],
            soil_evaporation: v[4],
            infiltration: v[5],
            liquid_runoff: v[6],
            soil_drainage: v[7],
        }
    }
    pub fn values(self) -> [f64; 8] {
        [
            self.rain,
            self.snowfall,
            self.melt,
            self.liquid_evaporation,
            self.soil_evaporation,
            self.infiltration,
            self.liquid_runoff,
            self.soil_drainage,
        ]
    }
    pub fn accumulate(&mut self, other: Self) {
        self.rain += other.rain;
        self.snowfall += other.snowfall;
        self.melt += other.melt;
        self.liquid_evaporation += other.liquid_evaporation;
        self.soil_evaporation += other.soil_evaporation;
        self.infiltration += other.infiltration;
        self.liquid_runoff += other.liquid_runoff;
        self.soil_drainage += other.soil_drainage;
    }
    /// Kahan summation of long-lived ledgers; roundoff is checkpointed, not mass.
    pub fn accumulate_compensated(&mut self, other: Self, roundoff: &mut [f64; 8]) {
        let mut sums = self.values();
        for ((sum, correction), increment) in sums.iter_mut().zip(roundoff).zip(other.values()) {
            let adjusted = increment - *correction;
            let next = *sum + adjusted;
            *correction = (next - *sum) - adjusted;
            *sum = next;
        }
        *self = Self::from_values(sums);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub stocks: Stocks,
    pub transfers: Transfers,
    pub residual_kilograms: f64,
}

/// Interval-wide coefficients: avoid evaluating three exponentials per region.
pub(crate) struct Response {
    settings: Settings,
    days: f64,
    infiltration_fraction: f64,
    runoff_fraction: f64,
    drainage_fraction: f64,
}
impl Response {
    pub(crate) fn new(seconds: f64, settings: Settings) -> Result<Self, String> {
        if !seconds.is_finite() || seconds <= 0. || seconds > DAY {
            return Err("Invalid surface-water interval.".into());
        }
        Ok(Self {
            settings,
            days: seconds / DAY,
            infiltration_fraction: -(-seconds / settings.infiltration_response_seconds).exp_m1(),
            runoff_fraction: -(-seconds / settings.liquid_runoff_response_seconds).exp_m1(),
            drainage_fraction: -(-seconds / settings.soil_drainage_response_seconds).exp_m1(),
        })
    }
}

/// Prescribed precipitation adds mass; actual evaporation subtracts mass.
/// The caller owns the atmosphere and must apply both opposite transfers.
#[allow(clippy::too_many_arguments)]
pub fn advance(
    stocks: Stocks,
    area: f64,
    is_land: bool,
    temperature_celsius: f64,
    precipitation_kilograms: f64,
    potential_evaporation_kilograms: f64,
    seconds: f64,
    settings: Settings,
) -> Result<Step, String> {
    settings.validate()?;
    advance_prepared(
        stocks,
        area,
        is_land,
        temperature_celsius,
        precipitation_kilograms,
        potential_evaporation_kilograms,
        &Response::new(seconds, settings)?,
    )
}

pub(crate) fn advance_prepared(
    before: Stocks,
    area: f64,
    is_land: bool,
    temperature: f64,
    precipitation: f64,
    potential_evaporation: f64,
    response: &Response,
) -> Result<Step, String> {
    let settings = response.settings;
    before.validate(area, is_land, settings)?;
    if !temperature.is_finite()
        || !(-100. ..=50.).contains(&temperature)
        || [precipitation, potential_evaporation]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.)
    {
        return Err("Invalid surface-water forcing or interval.".into());
    }
    let mut stocks = before;
    let mut flux = Transfers::default();
    let warm = temperature > 0.;
    if warm {
        flux.rain = precipitation;
        stocks.liquid += precipitation;
    } else {
        flux.snowfall = precipitation;
        stocks.snow += precipitation;
    }
    let capacity = area * settings.soil_capacity_kilograms_per_square_meter;
    if warm {
        let potential_melt = area
            * settings.melt_kilograms_per_square_meter_degree_day
            * temperature
            * response.days;
        if !potential_melt.is_finite() {
            return Err("Surface-water melt forcing overflow.".into());
        }
        flux.melt = stocks.snow.min(potential_melt);
        stocks.snow -= flux.melt;
        stocks.liquid += flux.melt;
        flux.liquid_evaporation = potential_evaporation.min(stocks.liquid);
        stocks.liquid -= flux.liquid_evaporation;
        if is_land {
            // Dry soil limits access to the remaining atmospheric demand.
            flux.soil_evaporation = ((potential_evaporation - flux.liquid_evaporation)
                * (stocks.soil / capacity))
                .min(stocks.soil);
            stocks.soil -= flux.soil_evaporation;
            flux.infiltration = (stocks.liquid * response.infiltration_fraction)
                .min((capacity - stocks.soil).max(0.));
            stocks.liquid -= flux.infiltration;
            stocks.soil += flux.infiltration;
            flux.soil_drainage = (stocks.soil - capacity * settings.soil_retained_fraction).max(0.)
                * response.drainage_fraction;
            stocks.soil -= flux.soil_drainage;
        }
    }
    if is_land {
        flux.liquid_runoff = stocks.liquid * response.runoff_fraction;
        stocks.liquid -= flux.liquid_runoff;
        stocks.pending_runoff += flux.liquid_runoff + flux.soil_drainage;
    }
    stocks.validate(area, is_land, settings)?;
    let residual = (stocks.total() - before.total()) - precipitation
        + flux.liquid_evaporation
        + flux.soil_evaporation;
    let scale = before
        .total()
        .max(precipitation)
        .max(flux.liquid_evaporation + flux.soil_evaporation)
        .max(1.);
    if flux.values().iter().any(|v| !v.is_finite() || *v < 0.)
        || !residual.is_finite()
        || residual.abs() > 32. * f64::EPSILON * scale
    {
        return Err("Surface-water transfer exceeds its arithmetic tolerance.".into());
    }
    Ok(Step {
        stocks,
        transfers: flux,
        residual_kilograms: residual,
    })
}
