//! Delayed, single-edge runoff transfers on a frozen drainage graph.
//! Linear storage response, not flood-wave hydraulics or lake spill routing.
use crate::{Surface, drainage::Drainage, moisture_transport::total_mass};
use serde::{Deserialize, Serialize};

pub const MODEL_VERSION: &str = "runoff-transport-1";

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub effective_speed_meters_per_second: f64,
    pub minimum_response_seconds: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            effective_speed_meters_per_second: 1.,
            minimum_response_seconds: 3600.,
        }
    }
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if !self.effective_speed_meters_per_second.is_finite()
            || !(0.1..=5.).contains(&self.effective_speed_meters_per_second)
            || !self.minimum_response_seconds.is_finite()
            || !(60. ..=86400.).contains(&self.minimum_response_seconds)
        {
            return Err("Invalid runoff-transport settings.".into());
        }
        Ok(())
    }
}

/// Integrated kilograms, not instantaneous discharge or new water production.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Transfers {
    pub sent: f64,
    pub received_transit: f64,
    pub terminal_delivery: f64,
    pub terminal_evaporation: f64,
}
impl Transfers {
    pub fn values(self) -> [f64; 4] {
        [
            self.sent,
            self.received_transit,
            self.terminal_delivery,
            self.terminal_evaporation,
        ]
    }
    pub fn from_values(v: [f64; 4]) -> Self {
        Self {
            sent: v[0],
            received_transit: v[1],
            terminal_delivery: v[2],
            terminal_evaporation: v[3],
        }
    }
    pub fn accumulate(&mut self, other: Self, roundoff: &mut [f64; 4]) {
        let mut sums = self.values();
        for ((sum, c), value) in sums.iter_mut().zip(roundoff).zip(other.values()) {
            let adjusted = value - *c;
            let next = *sum + adjusted;
            *c = (next - *sum) - adjusted;
            *sum = next;
        }
        *self = Self::from_values(sums);
    }
}

pub struct Network {
    receivers: Vec<u32>,
    response_seconds: Vec<f64>,
    minimum_response_seconds: f64,
}
pub(crate) struct Response {
    fractions: Vec<f64>,
}
#[derive(Debug)]
pub struct Step {
    pub transit_kilograms: Vec<f64>,
    pub sent_kilograms: Vec<f64>,
    pub received_transit_kilograms: Vec<f64>,
    pub terminal_delivery_kilograms: Vec<f64>,
    pub residual_kilograms: f64,
}

impl Network {
    /// Nonterminal edges have positive physical lengths; terminal self-edges
    /// use zero length and deliver to their own separate terminal stock.
    pub fn from_receivers(
        receivers: &[u32],
        edge_lengths_meters: &[f64],
        settings: Settings,
    ) -> Result<Self, String> {
        settings.validate()?;
        let n = receivers.len();
        if n == 0 || edge_lengths_meters.len() != n {
            return Err("Runoff graph and edge lengths do not match.".into());
        }
        let mut incoming = vec![0usize; n];
        let mut response_seconds = vec![0.; n];
        let mut minimum = f64::INFINITY;
        for (i, (&r, &length)) in receivers.iter().zip(edge_lengths_meters).enumerate() {
            if r as usize >= n
                || !length.is_finite()
                || (r as usize == i && length != 0.)
                || (r as usize != i && length <= 0.)
            {
                return Err("Invalid runoff receiver or physical edge length.".into());
            }
            if r as usize != i {
                incoming[r as usize] += 1;
                let tau = (length / settings.effective_speed_meters_per_second)
                    .max(settings.minimum_response_seconds);
                if !tau.is_finite() {
                    return Err("Runoff travel response overflow.".into());
                }
                response_seconds[i] = tau;
                minimum = minimum.min(tau);
            }
        }
        let mut order: Vec<_> = (0..n).filter(|&i| incoming[i] == 0).collect();
        let mut cursor = 0;
        while cursor < order.len() {
            let i = order[cursor];
            cursor += 1;
            let r = receivers[i] as usize;
            if r != i {
                incoming[r] -= 1;
                if incoming[r] == 0 {
                    order.push(r);
                }
            }
        }
        if order.len() != n {
            return Err("Runoff receiver graph contains a cycle.".into());
        }
        Ok(Self {
            receivers: receivers.to_vec(),
            response_seconds,
            minimum_response_seconds: minimum,
        })
    }

    pub fn from_surface(
        surface: &Surface,
        drainage: &Drainage,
        settings: Settings,
    ) -> Result<Self, String> {
        let n = surface.areas.len();
        if drainage.receivers.len() != n
            || surface.offsets.len() != n + 1
            || surface.neighbors.len() != surface.distances.len()
            || surface.offsets.first() != Some(&0)
            || surface.offsets.last().copied().map(|v| v as usize) != Some(surface.neighbors.len())
            || surface.offsets.windows(2).any(|p| p[0] > p[1])
        {
            return Err("Malformed physical runoff adjacency.".into());
        }
        // Reuse canonical-outlet validation, but do not collapse physical
        // arrival locations into a water body's minimum-ID diagnostic slot.
        drainage.route_runoff_units(&vec![0; n])?;
        let lengths = drainage
            .receivers
            .iter()
            .enumerate()
            .map(|(i, &r)| {
                if r as usize == i {
                    return Ok(0.);
                }
                let range = surface.offsets[i] as usize..surface.offsets[i + 1] as usize;
                range
                    .into_iter()
                    .find(|&k| surface.neighbors[k] == r)
                    .map(|k| surface.distances[k])
                    .ok_or_else(|| "Runoff receiver is not a physical neighbor.".into())
            })
            .collect::<Result<Vec<_>, String>>()?;
        Self::from_receivers(&drainage.receivers, &lengths, settings)
    }

    pub fn receivers(&self) -> &[u32] {
        &self.receivers
    }
    pub fn is_terminal(&self, i: usize) -> bool {
        self.receivers[i] as usize == i
    }
    pub fn response_seconds(&self) -> &[f64] {
        &self.response_seconds
    }
    /// Full coupled interval bound; routing executes for half of it at a time.
    pub fn maximum_coupled_step_seconds(&self) -> u64 {
        (self.minimum_response_seconds / 3.).floor().min(86400.) as u64
    }
    pub fn advance(&self, transit_kilograms: &[f64], seconds: f64) -> Result<Step, String> {
        self.advance_prepared(transit_kilograms, &self.prepare(seconds)?)
    }
    pub(crate) fn prepare(&self, seconds: f64) -> Result<Response, String> {
        if !seconds.is_finite()
            || seconds <= 0.
            || seconds > 86400.
            || seconds > self.minimum_response_seconds / 6.
        {
            return Err("Runoff interval exceeds its response-time bound.".into());
        }
        Ok(Response {
            fractions: self
                .response_seconds
                .iter()
                .enumerate()
                .map(|(i, &tau)| {
                    if self.is_terminal(i) {
                        1.
                    } else {
                        -(-seconds / tau).exp_m1()
                    }
                })
                .collect(),
        })
    }
    pub(crate) fn advance_prepared(
        &self,
        transit: &[f64],
        response: &Response,
    ) -> Result<Step, String> {
        let n = self.receivers.len();
        if transit.len() != n || transit.iter().any(|v| !v.is_finite() || *v < 0.) {
            return Err("Invalid runoff transit stock.".into());
        }
        let mut sent = vec![0.; n];
        let mut received = vec![0.; n];
        let mut delivered = vec![0.; n];
        let mut next = vec![0.; n];
        // Every departure reads the old stock, never this interval's arrivals.
        for i in 0..n {
            sent[i] = transit[i] * response.fractions[i];
            next[i] = transit[i] - sent[i];
            let r = self.receivers[i] as usize;
            if self.is_terminal(r) {
                delivered[r] += sent[i];
            } else {
                received[r] += sent[i];
            }
        }
        for i in 0..n {
            next[i] += received[i];
        }
        let initial = total_mass(transit);
        let final_total = total_mass(&[total_mass(&next), total_mass(&delivered)]);
        let residual = final_total - initial;
        if next
            .iter()
            .chain(&delivered)
            .any(|v| !v.is_finite() || *v < 0.)
            || !initial.is_finite()
            || !final_total.is_finite()
            || !residual.is_finite()
            || residual.abs() > 32. * f64::EPSILON * initial.max(1.)
        {
            return Err("Runoff transfer exceeds its arithmetic tolerance.".into());
        }
        Ok(Step {
            transit_kilograms: next,
            sent_kilograms: sent,
            received_transit_kilograms: received,
            terminal_delivery_kilograms: delivered,
            residual_kilograms: residual,
        })
    }
}
