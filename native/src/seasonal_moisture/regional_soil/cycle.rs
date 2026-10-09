use super::*;
use ponded_soil::{transfer, transfer_recorded};

pub(super) fn move_water(
    source: &mut Mass,
    recipient: &mut Mass,
    request: f64,
    history: &mut Mass,
    resolution: &mut Option<ResolutionBudget>,
) -> Result<(f64, bool), String> {
    if !request.is_finite() || request < 0. {
        return Err("Invalid paired water-transfer request.".into());
    }
    let mut donor = source.stock(f64::MAX)?;
    let mut receiver = recipient.stock(f64::MAX)?;
    let before = resolution.map(|r| r.deferred_requests);
    let grant = if let Some(resolution) = resolution {
        transfer_recorded(
            &mut donor,
            &mut receiver,
            request,
            f64::MAX,
            history,
            resolution,
        )?
    } else {
        let grant = transfer(&mut donor, &mut receiver, request, f64::MAX)?;
        history.credit(grant)?;
        grant
    };
    *source = donor.into();
    *recipient = receiver.into();
    Ok((grant, resolution.map(|r| r.deferred_requests) != before))
}
fn deficit(capacity: f64, mass: Mass) -> f64 {
    moisture_transport::total_mass(&[capacity, -mass.high, -mass.low]).max(0.)
}
impl Model {
    pub(super) fn advance_candidate(
        &self,
        cp: &mut Checkpoint,
        seconds: u32,
    ) -> Result<[usize; 3], String> {
        let end = cp.elapsed_seconds + u64::from(seconds);
        let limit = self.forcing.maximum_coupled_step_seconds()?;
        if (cp.elapsed_seconds % limit + u64::from(seconds)).div_ceil(limit) > 4096 {
            return Err("Regional seasonal caller exceeds its coupled work bound.".into());
        }
        let mut counts = [0; 3];
        while cp.elapsed_seconds < end {
            let day = cp.elapsed_seconds / super::super::SECONDS_PER_DAY;
            let month = (day as usize % 365) * 12 / 365;
            let interval = (end - cp.elapsed_seconds)
                .min(86400 - cp.elapsed_seconds % 86400)
                .min(limit - cp.elapsed_seconds % limit);
            let half = interval as f64 * 0.5;
            for phase in 0..2 {
                if phase == 1 {
                    let advance = if cp.resolution.is_some() {
                        moisture_transport::Flow::advance_paired_retaining
                    } else {
                        moisture_transport::Flow::advance_paired
                    };
                    let advected = advance(
                        &self.forcing.flows[month],
                        &cp.vapor,
                        &cp.atmospheric_transfers,
                        interval as f64,
                        cp.settings.transport,
                    )?;
                    cp.vapor = advected.stocks;
                    cp.atmospheric_transfers = advected.directed_transfers;
                    counts[1] += advected.substeps;
                    if let Some(resolution) = &mut cp.resolution {
                        resolution.combine(advected.resolution)?;
                    }
                }
                self.vertical(cp, month, half)?;
                if cp.settings.routing_enabled {
                    self.route_drainage(cp, half)?;
                }
                counts[2] += self.surface(cp, half)?;
            }
            cp.elapsed_seconds += interval;
            counts[0] += 1;
        }
        Ok(counts)
    }

    fn vertical(&self, cp: &mut Checkpoint, month: usize, seconds: f64) -> Result<(), String> {
        let pool = self.forcing.reference_pool.as_ref().unwrap();
        let settings = cp.settings;
        let evap_fraction = if settings.evaporation_enabled {
            -(-seconds / settings.evaporation_response_seconds).exp_m1()
        } else {
            0.
        };
        let mut demand = vec![0.; cp.liquid.len()];
        for (r, body_demand) in demand.iter_mut().enumerate() {
            let t = self.forcing.temperatures[month][r];
            let capacity = self.forcing.capacities[month][r];
            let extra = self
                .forcing
                .orographic_forcing
                .as_ref()
                .unwrap()
                .additional_rates_per_second[month][r];
            let precipitation = if settings.precipitation_enabled {
                moisture_transport::total_mass(&[cp.vapor[r].high, -capacity, cp.vapor[r].low])
                    .max(0.)
                    * -(-seconds * (1. / settings.precipitation_response_seconds + extra)).exp_m1()
            } else {
                0.
            };
            let recipient = if t <= 0. {
                &mut cp.snow[r]
            } else if let Some(b) = pool.by_region[r] {
                &mut cp.reference_bodies[b]
            } else {
                &mut cp.liquid[r]
            };
            let history = if t <= 0. {
                &mut cp.local_transfers[r].snowfall
            } else {
                &mut cp.local_transfers[r].rain
            };
            move_water(
                &mut cp.vapor[r],
                recipient,
                precipitation,
                history,
                &mut cp.resolution,
            )?;
            if t > 0. {
                let request = self.forcing.areas[r]
                    * settings.melt_kilograms_per_square_meter_degree_day
                    * t
                    * (seconds / 86400.);
                let recipient = if let Some(b) = pool.by_region[r] {
                    &mut cp.reference_bodies[b]
                } else {
                    &mut cp.liquid[r]
                };
                move_water(
                    &mut cp.snow[r],
                    recipient,
                    request,
                    &mut cp.local_transfers[r].melt,
                    &mut cp.resolution,
                )?;
            }
            let evaporation = deficit(capacity, cp.vapor[r]) * evap_fraction;
            if pool.by_region[r].is_some() {
                if t > 0. {
                    *body_demand = evaporation;
                }
            } else {
                let stocks = ponded_soil::State {
                    liquid: cp.liquid[r],
                    soil: cp.soil[r],
                    vapor: cp.vapor[r],
                    drainage: cp.drainage[r],
                };
                let f = &mut cp.local_transfers[r];
                let step = if let Some(resolution) = &mut cp.resolution {
                    let recorded = ponded_soil::advance_recorded(
                        ponded_soil::RecordedState {
                            stocks,
                            transfers: [
                                f.liquid_evaporation,
                                f.soil_evaporation,
                                f.infiltration,
                                f.soil_drainage,
                            ],
                        },
                        self.forcing.areas[r],
                        t,
                        evaporation,
                        seconds,
                        settings.soil,
                    )?;
                    [
                        f.liquid_evaporation,
                        f.soil_evaporation,
                        f.infiltration,
                        f.soil_drainage,
                    ] = recorded.transfers;
                    resolution.combine(recorded.resolution)?;
                    recorded.step
                } else {
                    let step = ponded_soil::advance(
                        stocks,
                        self.forcing.areas[r],
                        t,
                        evaporation,
                        seconds,
                        settings.soil,
                    )?;
                    let v = step.transfers;
                    f.liquid_evaporation
                        .credit(v.liquid_evaporation_kilograms)?;
                    f.soil_evaporation.credit(v.soil_evaporation_kilograms)?;
                    f.infiltration.credit(v.infiltration_kilograms)?;
                    f.soil_drainage.credit(v.soil_drainage_kilograms)?;
                    step
                };
                cp.liquid[r] = step.state.liquid;
                cp.soil[r] = step.state.soil;
                cp.vapor[r] = step.state.vapor;
                cp.drainage[r] = step.state.drainage;
            }
        }
        // Freeze all body demands before allocating, preserving a common shortage factor.
        for (body, members) in pool.members.iter().enumerate() {
            let requests: Vec<_> = members.iter().map(|&r| demand[r]).collect();
            let (_, grants, _) = super::super::reference_pool::allocate(
                cp.reference_bodies[body].stock(f64::MAX)?,
                &requests,
            )?;
            for (&r, grant) in members.iter().zip(grants) {
                let (actual, deferred) = move_water(
                    &mut cp.reference_bodies[body],
                    &mut cp.vapor[r],
                    grant,
                    &mut cp.local_transfers[r].liquid_evaporation,
                    &mut cp.resolution,
                )?;
                if actual != grant && !deferred {
                    return Err("Body evaporation changed a shared allocation grant.".into());
                }
            }
        }
        Ok(())
    }

    fn route_drainage(&self, cp: &mut Checkpoint, seconds: f64) -> Result<(), String> {
        let network = &self.forcing.routing;
        network.prepare(seconds)?;
        let old = cp.drainage.clone();
        for (r, mass) in old.iter().enumerate() {
            let request = mass.stock(f64::MAX)?.available()
                * if network.is_terminal(r) {
                    1.
                } else {
                    -(-seconds / network.response_seconds()[r]).exp_m1()
                };
            let target = network.receivers()[r] as usize;
            let mut donor = cp.drainage[r];
            let receiver = if network.is_terminal(target) {
                if let Some(b) = self.forcing.reference_pool.as_ref().unwrap().by_region[target] {
                    &mut cp.reference_bodies[b]
                } else {
                    &mut cp.liquid[target]
                }
            } else {
                &mut cp.drainage[target]
            };
            let (actual, deferred) = move_water(
                &mut donor,
                receiver,
                request,
                &mut cp.drainage_sent[r],
                &mut cp.resolution,
            )?;
            if actual != request && !deferred {
                return Err("Drainage request exceeds its frozen donor.".into());
            }
            cp.drainage[r] = donor;
        }
        Ok(())
    }

    fn surface(&self, cp: &mut Checkpoint, seconds: f64) -> Result<usize, String> {
        let layout = self
            .forcing
            .lake_exchange
            .as_ref()
            .unwrap()
            .regional
            .as_ref()
            .unwrap();
        surface::advance(
            layout,
            surface::Stocks {
                by_region: &self.forcing.reference_pool.as_ref().unwrap().by_region,
                liquid: &mut cp.liquid,
                bodies: &mut cp.reference_bodies,
                transfers: &mut cp.surface_transfers,
                resolution: &mut cp.resolution,
            },
            seconds,
        )
    }
}
