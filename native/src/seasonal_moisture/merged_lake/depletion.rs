//! Full depletion of one owned layer, with physically keyed bounded reconciliation.
//! Equal-largest unresolved shares refuse rather than selecting a region ID.
use super::super::leaf_spill;
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock};

pub(super) fn sum(demand: &[f64]) -> Result<CompensatedStock, String> {
    if demand.iter().any(|v| !v.is_finite() || *v < 0.) {
        return Err("Invalid common-layer evaporation demand.".into());
    }
    let mut ordered = demand.to_vec();
    ordered.sort_by(f64::total_cmp);
    let mut stock = CompensatedStock::new(0., 0., f64::MAX)?;
    for amount in ordered {
        let before = stock;
        stock.credit(amount)?;
        if amount > 0. && (before.high, before.low) == (stock.high, stock.low) {
            return Err("Common-layer demand sum is below pair resolution.".into());
        }
        let residual = total_mass(&[stock.high - before.high, stock.low - before.low, -amount]);
        if !residual.is_finite()
            || residual.abs() > 32. * f64::EPSILON * before.high.max(amount).max(f64::MIN_POSITIVE)
        {
            return Err("Common-layer demand addition exceeds its local arithmetic bound.".into());
        }
    }
    Ok(stock)
}
pub(super) fn covers(demand: CompensatedStock, stock: CompensatedStock) -> bool {
    demand.high > stock.high || (demand.high == stock.high && demand.low >= stock.low)
}
fn debit(stock: &mut CompensatedStock, amount: f64) -> Result<(), String> {
    let before = *stock;
    if stock.withdraw(amount) != amount
        || (amount > 0. && (before.high, before.low) == (stock.high, stock.low))
    {
        return Err("Common-layer grant cannot make a represented full debit.".into());
    }
    CompensatedStock::new(stock.high, stock.low, f64::MAX)?;
    Ok(())
}
pub(super) fn full(donor: CompensatedStock, demand: &[f64]) -> Result<Vec<(usize, f64)>, String> {
    let total = sum(demand)?;
    if !covers(total, donor) {
        return Err("Common-layer depletion demand does not cover its complete pair.".into());
    }
    if donor.high == 0. {
        return Ok(Vec::new());
    }
    let factor = (donor.high + donor.low) / (total.high + total.low);
    let nominal: Vec<_> = demand.iter().map(|&r| (r * factor).min(r)).collect();
    let mut order: Vec<_> = (0..demand.len()).filter(|&i| demand[i] > 0.).collect();
    order.sort_by(|&a, &b| demand[a].total_cmp(&demand[b]));
    // First accept an exactly reconciling nominal allocation, including ties.
    let mut trial = donor;
    let mut trial_grants = Vec::new();
    let mut feasible = true;
    for &i in &order {
        if debit(&mut trial, nominal[i]).is_err() {
            feasible = false;
            break;
        }
        if nominal[i] > 0. {
            trial_grants.push((i, nominal[i]));
        }
    }
    if feasible && trial.high == 0. && trial.low == 0. {
        return Ok(trial_grants);
    }
    let &owner = order.last().ok_or("No positive common-layer demand.")?;
    if order.len() > 1 && demand[order[order.len() - 2]] == demand[owner] {
        return Err("Common-layer depletion has an unresolved equal-largest demand tie.".into());
    }
    let mut remaining = donor;
    let mut grants = Vec::new();
    for &i in &order[..order.len() - 1] {
        debit(&mut remaining, nominal[i])?;
        if nominal[i] > 0. {
            grants.push((i, nominal[i]));
        }
    }
    let mut recipient = CompensatedStock::new(0., 0., demand[owner])?;
    let corrected = leaf_spill::fill(&mut remaining, &mut recipient, demand[owner])?;
    if remaining.high != 0. || remaining.low != 0. {
        return Err("Common-layer remainder exceeds its physical recipient's demand.".into());
    }
    let deviation = total_mass(&[recipient.high, recipient.low, -nominal[owner]]);
    if !deviation.is_finite()
        || deviation.abs() > 32. * f64::EPSILON * nominal[owner].max(f64::MIN_POSITIVE)
    {
        return Err(
            "Common-layer reconciliation exceeds its recipient-local proportional bound.".into(),
        );
    }
    for amount in corrected {
        grants.push((owner, amount));
    }
    Ok(grants)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dyadic_ties_and_unique_largest_reconcile_with_full_pair_debits() {
        for (stock, requests) in [
            (3., vec![2., 2., 2.]),
            (7., vec![2., 4., 8.]),
            (1., vec![1., 2., 4.]),
        ] {
            let donor = CompensatedStock::new(stock, 0., f64::MAX).unwrap();
            let grants = full(donor, &requests).unwrap();
            let mut after = donor;
            let mut paid = vec![0.; requests.len()];
            for (i, amount) in grants {
                debit(&mut after, amount).unwrap();
                paid[i] += amount;
            }
            assert_eq!((after.high, after.low), (0., 0.));
            assert!(paid.iter().zip(&requests).all(|(p, r)| p <= r));
        }
        let donor = CompensatedStock::new(1., -2_f64.powi(-54), f64::MAX).unwrap();
        let grants = full(donor, &[2.]).unwrap();
        let mut after = donor;
        for (_, amount) in grants {
            debit(&mut after, amount).unwrap();
        }
        assert_eq!((after.high, after.low), (0., 0.));
    }
    #[test]
    fn unresolved_thirds_refuse_and_permutation_does_not_pick_ids() {
        let donor = CompensatedStock::new(1., 0., f64::MAX).unwrap();
        assert!(full(donor, &[1., 1., 1.]).unwrap_err().contains("tie"));
        let requests = [1., 2., 4.];
        let original = full(donor, &requests).unwrap();
        for order in [[2, 1, 0], [1, 0, 2], [0, 2, 1], [2, 0, 1], [1, 2, 0]] {
            let shuffled = order.map(|i| requests[i]);
            let result = full(donor, &shuffled)
                .unwrap()
                .into_iter()
                .map(|(i, g)| (order[i], g))
                .collect::<Vec<_>>();
            assert_eq!(original, result);
        }
    }
}
