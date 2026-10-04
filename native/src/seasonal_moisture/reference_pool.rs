//! Finite mobile liquid shared only within immutable reference connectivity.
//! This is a fast-availability approximation, not a current or lake-level solver.
use crate::{moisture_transport::total_mass, surface_water::CompensatedStock};
use std::collections::BTreeMap;

pub const MODEL_VERSION: &str = "reference-water-pool-1";

pub(super) struct Layout {
    pub ids: Vec<u32>,
    pub members: Vec<Vec<usize>>,
    pub by_region: Vec<Option<usize>>,
}
impl Layout {
    pub fn new(ids: &[u32]) -> Self {
        let mut groups = BTreeMap::<u32, Vec<usize>>::new();
        for (i, &id) in ids.iter().enumerate() {
            if id != 0 {
                groups.entry(id).or_default().push(i);
            }
        }
        let mut layout = Self {
            ids: Vec::new(),
            members: Vec::new(),
            by_region: vec![None; ids.len()],
        };
        for (id, members) in groups {
            for &i in &members {
                layout.by_region[i] = Some(layout.ids.len());
            }
            layout.ids.push(id);
            layout.members.push(members);
        }
        layout
    }
}

/// Each returned grant has exactly one actual donor debit. No last-recipient
/// remainder or priority grant is used. Unrepresentable tails remain owned.
pub(super) fn allocate(
    donor: CompensatedStock,
    demand: &[f64],
) -> Result<(CompensatedStock, Vec<f64>, f64), String> {
    CompensatedStock::new(donor.high, donor.low, f64::MAX)?;
    if demand.iter().any(|v| !v.is_finite() || *v < 0.) {
        return Err("Invalid reference-body evaporation demand.".into());
    }
    let total = total_mass(demand);
    if !total.is_finite() {
        return Err("Reference-body demand overflow.".into());
    }
    // A small arithmetic reserve protects a shared factor against sum/product
    // rounding at depletion. This water stays in the donor; it is not a sink.
    let safe = donor.available() * (1. - 8. * f64::EPSILON);
    let factor = if total == 0. || total <= safe {
        1.
    } else {
        safe / total
    };
    let mut after = donor;
    let mut grants = Vec::with_capacity(demand.len());
    for &request in demand {
        let wanted = request * factor;
        let actual = after.withdraw(wanted);
        if actual != wanted {
            return Err("Reference-body allocation would prioritize a recipient.".into());
        }
        grants.push(actual);
    }
    CompensatedStock::new(after.high, after.low, f64::MAX)?;
    let residual = total_mass(&[
        after.high,
        after.low,
        -donor.high,
        -donor.low,
        total_mass(&grants),
    ]);
    if !residual.is_finite() || residual.abs() > 32. * f64::EPSILON * donor.high.max(1.) {
        return Err("Reference-body allocation exceeds its arithmetic tolerance.".into());
    }
    Ok((after, grants, residual))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_dyadic_grants_and_signed_tails_have_one_owner() {
        let stock = CompensatedStock::new(32., 2_f64.powi(-50), f64::MAX).unwrap();
        let (after, grants, residual) = allocate(stock, &[1., 2., 4., 8.]).unwrap();
        assert_eq!(grants, [1., 2., 4., 8.]);
        assert_eq!(after.high, 17.);
        assert_eq!(after.low, stock.low);
        assert_eq!(residual, 0.);
        let stock = CompensatedStock::new(1., -2_f64.powi(-54), f64::MAX).unwrap();
        let (after, grants, _) = allocate(stock, &[1., 1., 1.]).unwrap();
        assert_eq!(grants[0], grants[1]);
        assert_eq!(grants[1], grants[2]);
        assert!(after.high > 0.);
        assert!(total_mass(&grants) <= stock.available());
    }
    #[test]
    fn common_fraction_is_not_an_index_priority_or_cross_body_transfer() {
        let stock = CompensatedStock::new(30., 0., f64::MAX).unwrap();
        let (_, a, _) = allocate(stock, &[10., 20., 40.]).unwrap();
        let (_, b, _) = allocate(stock, &[40., 10., 20.]).unwrap();
        assert_eq!(a, [b[1], b[2], b[0]]);
        assert_eq!(a[1], 2. * a[0]);
        assert_eq!(a[2], 4. * a[0]);
        let layout = Layout::new(&[9, 0, 2, 9, 2]);
        assert_eq!(layout.ids, [2, 9]);
        assert_eq!(layout.members, [vec![2, 4], vec![0, 3]]);
        assert_eq!(layout.by_region, [Some(1), None, Some(0), Some(1), Some(0)]);
        let (_, empty, _) =
            allocate(CompensatedStock::new(0., 0., f64::MAX).unwrap(), &[10.]).unwrap();
        assert_eq!(empty, [0.]);
    }
    #[test]
    fn large_and_uneven_request_sets_leave_the_reserve_owned() {
        for count in [1, 162, 40962, 100000] {
            let stock = CompensatedStock::new(1., 0., f64::MAX).unwrap();
            let demand: Vec<_> = (0..count).map(|i| 2_f64.powi((i % 80) - 40)).collect();
            let (after, grants, _) = allocate(stock, &demand).unwrap();
            assert!(after.high >= 0.);
            assert!(total_mass(&grants) <= 1.);
        }
    }
    #[test]
    fn invalid_overflow_and_subnormal_demands_are_explicit() {
        let stock = CompensatedStock::new(1., 0., f64::MAX).unwrap();
        for demand in [
            vec![-1.],
            vec![f64::NAN],
            vec![f64::INFINITY],
            vec![f64::MAX; 2],
        ] {
            assert!(allocate(stock, &demand).is_err());
        }
        let tiny = f64::from_bits(1);
        let (after, grants, _) =
            allocate(CompensatedStock::new(tiny, 0., f64::MAX).unwrap(), &[tiny]).unwrap();
        assert_eq!(grants, [tiny]);
        assert_eq!(after.high, 0.);
        assert_eq!(after.low, 0.);
    }
}
