//! Explicitly selected paired-stock transport; the legacy operator is unchanged.
use super::{Flow, Settings};
use crate::surface_water::ponded_soil::{Mass, ResolutionBudget, transfer, transfer_recorded};

pub const MODEL_VERSION: &str = "moisture-transport-paired-1";
pub const RETAINING_MODEL_VERSION: &str = "moisture-transport-paired-2";
pub struct PairedStep {
    pub stocks: Vec<Mass>,
    /// Gross crossings in canonical physical-face/direction order, never inventory.
    pub directed_transfers: Vec<Mass>,
    pub substeps: usize,
    pub resolution: ResolutionBudget,
}
impl Flow {
    pub fn directed_contacts(&self) -> &[[usize; 2]] {
        &self.contacts
    }

    /// All requests read the old substep. One checked grant debits/credits both owners.
    /// Complete input/history remain untouched on any failure.
    pub fn advance_paired(
        &self,
        stocks: &[Mass],
        history: &[Mass],
        seconds: f64,
        settings: Settings,
    ) -> Result<PairedStep, String> {
        self.advance_paired_inner(stocks, history, seconds, settings, false)
    }
    pub fn advance_paired_retaining(
        &self,
        stocks: &[Mass],
        history: &[Mass],
        seconds: f64,
        settings: Settings,
    ) -> Result<PairedStep, String> {
        self.advance_paired_inner(stocks, history, seconds, settings, true)
    }
    fn advance_paired_inner(
        &self,
        stocks: &[Mass],
        history: &[Mass],
        seconds: f64,
        settings: Settings,
        retaining: bool,
    ) -> Result<PairedStep, String> {
        settings.validate()?;
        if stocks.len() != self.areas.len()
            || history.len() != self.contacts.len()
            || !seconds.is_finite()
            || seconds <= 0.
        {
            return Err("Invalid paired atmospheric shape or interval.".into());
        }
        let count = (seconds * self.max_outgoing_rate_per_second / settings.max_outgoing_fraction)
            .ceil()
            .max(1.);
        if !count.is_finite() || count > settings.max_substeps as f64 {
            return Err("Paired atmospheric transport exceeds its substep bound.".into());
        }
        let substeps = count as usize;
        let dt = seconds / count;
        let mut owned = stocks
            .iter()
            .map(|v| v.stock(f64::MAX))
            .collect::<Result<Vec<_>, _>>()?;
        let mut directed = history.to_vec();
        let mut resolution = ResolutionBudget::default();
        for h in &directed {
            h.stock(f64::MAX)?;
        }
        for _ in 0..substeps {
            let old = owned.clone();
            for edge in &self.transfers {
                // Leading concentration matches the declared upwind approximation;
                // the outgoing-fraction margin protects signed stock tails.
                let request = old[edge.source].high / self.areas[edge.source]
                    * edge.swept_area_square_meters_per_second
                    * dt;
                if !request.is_finite() || request < 0. {
                    return Err("Nonfinite paired atmospheric request.".into());
                }
                let mut donor = owned[edge.source];
                let mut recipient = owned[edge.recipient];
                let previous_deferrals = resolution.deferred_requests;
                let grant = if retaining {
                    transfer_recorded(
                        &mut donor,
                        &mut recipient,
                        request,
                        f64::MAX,
                        &mut directed[edge.directed_contact],
                        &mut resolution,
                    )?
                } else {
                    let grant = transfer(&mut donor, &mut recipient, request, f64::MAX)?;
                    directed[edge.directed_contact].credit(grant)?;
                    grant
                };
                if grant != request
                    && !(grant == 0. && resolution.deferred_requests > previous_deferrals)
                {
                    return Err("Paired atmospheric request violates its donor bound.".into());
                }
                owned[edge.source] = donor;
                owned[edge.recipient] = recipient;
            }
        }
        Ok(PairedStep {
            stocks: owned.into_iter().map(Into::into).collect(),
            directed_transfers: directed,
            substeps,
            resolution,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moisture_transport::Transfer;
    fn flow() -> Flow {
        let rate = 2_f64.powi(-70);
        Flow {
            areas: vec![1.; 3],
            contacts: vec![[0, 1], [1, 2]],
            transfers: vec![
                Transfer {
                    source: 0,
                    recipient: 1,
                    swept_area_square_meters_per_second: rate,
                    directed_contact: 0,
                },
                Transfer {
                    source: 1,
                    recipient: 2,
                    swept_area_square_meters_per_second: rate,
                    directed_contact: 1,
                },
            ],
            max_outgoing_rate_per_second: rate,
        }
    }
    #[test]
    fn retaining_transport_preserves_both_owners_and_history_for_an_unrepresentable_edge() {
        let mut flow = flow();
        flow.transfers.truncate(1);
        let stocks = [
            Mass {
                high: 1e-40,
                low: 0.,
            },
            Mass {
                high: 100.,
                low: 1e-15,
            },
            Mass::default(),
        ];
        let history = [Mass::default(); 2];
        assert!(
            flow.advance_paired(&stocks, &history, 1., Settings::default())
                .is_err()
        );
        let step = flow
            .advance_paired_retaining(&stocks, &history, 1., Settings::default())
            .unwrap();
        assert_eq!(step.stocks, stocks);
        assert_eq!(step.directed_transfers, history);
        assert_eq!(step.resolution.deferred_requests, 1);
        assert_eq!(
            step.resolution.maximum_deferred_request_kilograms,
            1e-40 * 2_f64.powi(-70)
        );
        let mut malformed = history;
        malformed[1] = Mass {
            high: 0.,
            low: 1e-40,
        };
        assert!(
            flow.advance_paired_retaining(&stocks, &malformed, 1., Settings::default())
                .is_err()
        );
    }
    #[test]
    fn tiny_gross_crossings_keep_each_owner_and_match_an_integer_oracle() {
        let initial = 1_i128 << 70;
        let mut stocks = vec![
            Mass {
                high: initial as f64,
                low: 0.
            };
            3
        ];
        let mut history = vec![Mass::default(); 2];
        for elapsed in 1..=1024_i128 {
            let result = flow()
                .advance_paired(&stocks, &history, 1., Default::default())
                .unwrap();
            stocks = result.stocks;
            history = result.directed_transfers;
            for (r, exact) in [initial - elapsed, initial, initial + elapsed]
                .into_iter()
                .enumerate()
            {
                assert_eq!(stocks[r].high as i128 + stocks[r].low as i128, exact);
            }
            assert_eq!(
                history,
                vec![
                    Mass {
                        high: elapsed as f64,
                        low: 0.
                    };
                    2
                ]
            );
        }
    }
    #[test]
    fn arrivals_cannot_depart_in_the_same_substep_and_bad_history_is_atomic() {
        let stocks = vec![
            Mass {
                high: 2_f64.powi(70),
                low: 0.,
            },
            Mass::default(),
            Mass::default(),
        ];
        let history = vec![Mass::default(); 2];
        let step = flow()
            .advance_paired(&stocks, &history, 1., Default::default())
            .unwrap();
        assert_eq!(step.stocks[1].high, 1.);
        assert_eq!(step.stocks[2].high, 0.);
        assert!(
            flow()
                .advance_paired(&stocks, &history[..1], 1., Default::default())
                .is_err()
        );
        let mut bad = history.clone();
        bad[0] = Mass { high: 0., low: 1. };
        assert!(
            flow()
                .advance_paired(&stocks, &bad, 1., Default::default())
                .is_err()
        );
        assert_eq!(history, vec![Mass::default(); 2]);
        assert_eq!(stocks[0].high, 2_f64.powi(70));
    }
}
