//! Finite local snow, liquid, soil, and pending-runoff ownership.
//! Empirical temperature-index/bucket closure, not an energy or river solver.
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "surface-water-1";
pub const COMPENSATED_MODEL_VERSION: &str = "surface-water-compensated-soil-1";
pub const PRECISE_SURFACE_MODEL_VERSION: &str = "surface-water-compensated-surface-1";
pub mod ponded_soil;
mod stock_precision;
pub(crate) use stock_precision::Stock as CompensatedStock;
pub(crate) use stock_precision::validate as validate_stock_precision;
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

#[derive(Clone, Copy, Debug)]
pub struct CompensatedStep {
    pub step: Step,
    /// A signed low component of the same soil stock, unlike a ledger correction.
    pub soil_low_kilograms: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct PreciseSurfaceStep {
    pub step: Step,
    pub soil_low_kilograms: f64,
    /// Signed component of the same snow water-equivalent mass.
    pub snow_low_kilograms: f64,
    pub liquid_low_kilograms: f64,
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
    Ok(advance_with_precision(
        before,
        area,
        is_land,
        temperature,
        precipitation,
        potential_evaporation,
        response,
        None,
        None,
        None,
    )?
    .step)
}

/// Headless candidate: same process order/laws, with a persistent soil low part.
#[allow(clippy::too_many_arguments)]
pub fn advance_compensated(
    before: Stocks,
    soil_low: f64,
    area: f64,
    is_land: bool,
    temperature: f64,
    precipitation: f64,
    potential_evaporation: f64,
    seconds: f64,
    settings: Settings,
) -> Result<CompensatedStep, String> {
    settings.validate()?;
    advance_compensated_prepared(
        before,
        soil_low,
        area,
        is_land,
        temperature,
        precipitation,
        potential_evaporation,
        &Response::new(seconds, settings)?,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn advance_compensated_prepared(
    before: Stocks,
    soil_low: f64,
    area: f64,
    is_land: bool,
    temperature: f64,
    precipitation: f64,
    potential_evaporation: f64,
    response: &Response,
) -> Result<CompensatedStep, String> {
    let result = advance_with_precision(
        before,
        area,
        is_land,
        temperature,
        precipitation,
        potential_evaporation,
        response,
        Some(soil_low),
        None,
        None,
    )?;
    Ok(CompensatedStep {
        step: result.step,
        soil_low_kilograms: result.soil_low_kilograms,
    })
}

/// Headless liquid/snow/soil candidate; no new transfer physics.
#[allow(clippy::too_many_arguments)]
pub fn advance_precise_surface(
    before: Stocks,
    soil_low: f64,
    snow_low: f64,
    liquid_low: f64,
    area: f64,
    is_land: bool,
    temperature: f64,
    precipitation: f64,
    potential_evaporation: f64,
    seconds: f64,
    settings: Settings,
) -> Result<PreciseSurfaceStep, String> {
    settings.validate()?;
    advance_precise_surface_prepared(
        before,
        soil_low,
        snow_low,
        liquid_low,
        area,
        is_land,
        temperature,
        precipitation,
        potential_evaporation,
        &Response::new(seconds, settings)?,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn advance_precise_surface_prepared(
    before: Stocks,
    soil_low: f64,
    snow_low: f64,
    liquid_low: f64,
    area: f64,
    is_land: bool,
    temperature: f64,
    precipitation: f64,
    potential_evaporation: f64,
    response: &Response,
) -> Result<PreciseSurfaceStep, String> {
    advance_with_precision(
        before,
        area,
        is_land,
        temperature,
        precipitation,
        potential_evaporation,
        response,
        Some(soil_low),
        Some(snow_low),
        Some(liquid_low),
    )
}

#[allow(clippy::too_many_arguments)]
fn advance_with_precision(
    before: Stocks,
    area: f64,
    is_land: bool,
    temperature: f64,
    precipitation: f64,
    potential_evaporation: f64,
    response: &Response,
    soil_low: Option<f64>,
    snow_low: Option<f64>,
    liquid_low: Option<f64>,
) -> Result<PreciseSurfaceStep, String> {
    let settings = response.settings;
    before.validate(area, is_land, settings)?;
    let mut precise = soil_low
        .map(|low| {
            stock_precision::Stock::new(
                before.soil,
                low,
                area * settings.soil_capacity_kilograms_per_square_meter,
            )
        })
        .transpose()?;
    let mut precise_snow = snow_low
        .map(|low| stock_precision::Stock::new(before.snow, low, f64::MAX))
        .transpose()?;
    let mut precise_liquid = liquid_low
        .map(|low| stock_precision::Stock::new(before.liquid, low, f64::MAX))
        .transpose()?;
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
        if let Some(liquid) = &mut precise_liquid {
            liquid.credit(precipitation)?;
            stocks.liquid = liquid.high;
        } else {
            stocks.liquid += precipitation;
        }
    } else {
        flux.snowfall = precipitation;
        if let Some(snow) = &mut precise_snow {
            snow.credit(precipitation)?;
            stocks.snow = snow.high;
        } else {
            stocks.snow += precipitation;
        }
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
        if let Some(snow) = &mut precise_snow {
            // Use the demand, not rounded high mass, to make tiny tails eligible.
            flux.melt = snow.withdraw(potential_melt);
            stocks.snow = snow.high;
        } else {
            stocks.snow -= flux.melt;
        }
        if let Some(liquid) = &mut precise_liquid {
            liquid.credit(flux.melt)?;
            stocks.liquid = liquid.high;
        } else {
            stocks.liquid += flux.melt;
        }
        flux.liquid_evaporation = potential_evaporation.min(stocks.liquid);
        if let Some(liquid) = &mut precise_liquid {
            flux.liquid_evaporation = liquid.withdraw(potential_evaporation);
            stocks.liquid = liquid.high;
        } else {
            stocks.liquid -= flux.liquid_evaporation;
        }
        if is_land {
            // Dry soil limits access to the remaining atmospheric demand.
            flux.soil_evaporation = ((potential_evaporation - flux.liquid_evaporation)
                * (stocks.soil / capacity))
                .min(stocks.soil);
            if let Some(soil) = &mut precise {
                flux.soil_evaporation = soil.withdraw(flux.soil_evaporation);
                stocks.soil = soil.high;
            } else {
                stocks.soil -= flux.soil_evaporation;
            }
            let nominal_infiltration = stocks.liquid * response.infiltration_fraction;
            let requested_infiltration = precise_liquid.map_or(nominal_infiltration, |liquid| {
                nominal_infiltration.min(liquid.available())
            });
            if let Some(soil) = &mut precise {
                flux.infiltration = soil.deposit(requested_infiltration, capacity);
                stocks.soil = soil.high;
            } else {
                flux.infiltration = requested_infiltration.min((capacity - stocks.soil).max(0.));
                stocks.soil += flux.infiltration;
            }
            if let Some(liquid) = &mut precise_liquid {
                let grant = liquid.withdraw(flux.infiltration);
                if grant != flux.infiltration {
                    return Err("Infiltration exceeds its represented liquid donor.".into());
                }
                stocks.liquid = liquid.high;
            } else {
                stocks.liquid -= flux.infiltration;
            }
            flux.soil_drainage = (stocks.soil - capacity * settings.soil_retained_fraction).max(0.)
                * response.drainage_fraction;
            if let Some(soil) = &mut precise {
                flux.soil_drainage = soil.withdraw(flux.soil_drainage);
                stocks.soil = soil.high;
            } else {
                stocks.soil -= flux.soil_drainage;
            }
        }
    }
    if is_land {
        flux.liquid_runoff = stocks.liquid * response.runoff_fraction;
        if let Some(liquid) = &mut precise_liquid {
            flux.liquid_runoff = liquid.withdraw(flux.liquid_runoff);
            stocks.liquid = liquid.high;
        } else {
            stocks.liquid -= flux.liquid_runoff;
        }
        stocks.pending_runoff += flux.liquid_runoff + flux.soil_drainage;
    }
    stocks.validate(area, is_land, settings)?;
    let low = precise.map_or(0., |soil| soil.low);
    if let Some(soil) = precise {
        stock_precision::Stock::new(soil.high, soil.low, capacity)?;
    }
    if !is_land && low != 0. {
        return Err("Reference water cannot own a soil low component.".into());
    }
    let legacy_residual = (stocks.total() - before.total()) - precipitation
        + flux.liquid_evaporation
        + flux.soil_evaporation;
    let soil_residual = soil_low.map_or(legacy_residual, |before_low| {
        legacy_residual + (low - before_low)
    });
    let snow_low_after = precise_snow.map_or(0., |snow| snow.low);
    if let Some(snow) = precise_snow {
        stock_precision::Stock::new(snow.high, snow.low, f64::MAX)?;
    }
    let snow_residual = snow_low.map_or(soil_residual, |before_low| {
        soil_residual + (snow_low_after - before_low)
    });
    let liquid_low_after = precise_liquid.map_or(0., |liquid| liquid.low);
    if let Some(liquid) = precise_liquid {
        stock_precision::Stock::new(liquid.high, liquid.low, f64::MAX)?;
    }
    let residual = liquid_low.map_or(snow_residual, |before_low| {
        snow_residual + (liquid_low_after - before_low)
    });
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
    Ok(PreciseSurfaceStep {
        step: Step {
            stocks,
            transfers: flux,
            residual_kilograms: residual,
        },
        soil_low_kilograms: low,
        snow_low_kilograms: snow_low_after,
        liquid_low_kilograms: liquid_low_after,
    })
}
