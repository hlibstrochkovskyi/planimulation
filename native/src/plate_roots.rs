//! Experimental mesh-independent plate-root directions, not a generated-world model.
use crate::{Random, Surface, V, dot};

pub const PROPOSAL_VERSION: &str = "continuous-plate-roots-1";
const CANDIDATES_PER_ROOT: usize = 16;

fn uniform_direction(random: &mut Random) -> V {
    let y = random.next() * 2. - 1.;
    let phi = random.next() * std::f64::consts::TAU;
    let radial = (1. - y * y).sqrt();
    [radial * phi.cos(), y, radial * phi.sin()]
}

/// Root directions are determined before and independently of any mesh.
/// Weighted selection among a fixed number of continuous candidates favors
/// separation without enforcing an equal-area plate pattern.
pub fn directions(seed: &str, count: u32) -> Result<Vec<V>, String> {
    if seed.trim().is_empty() || seed.encode_utf16().count() > 128 || !(2..=32).contains(&count) {
        return Err("Invalid continuous plate-root seed or count.".into());
    }
    let mut random = Random::stream(seed, "tectonics.continuous-roots-1");
    let mut roots = vec![uniform_direction(&mut random)];
    while roots.len() < count as usize {
        let mut candidates = Vec::with_capacity(CANDIDATES_PER_ROOT);
        let mut weights = Vec::with_capacity(CANDIDATES_PER_ROOT);
        let mut total = 0.;
        for _ in 0..CANDIDATES_PER_ROOT {
            let candidate = uniform_direction(&mut random);
            let distance = roots
                .iter()
                .map(|root| (1. - dot(candidate, *root)).max(0.))
                .fold(f64::INFINITY, f64::min);
            let weight = distance * distance;
            total += weight;
            candidates.push(candidate);
            weights.push(weight);
        }
        if total == 0. {
            return Err("Continuous plate-root candidates collapsed.".into());
        }
        let mut target = random.next() * total;
        let mut selected = candidates.len() - 1;
        for (index, weight) in weights.iter().enumerate() {
            target -= weight;
            if target < 0. {
                selected = index;
                break;
            }
        }
        roots.push(candidates[selected]);
    }
    Ok(roots)
}

/// Snap each physical direction to its nearest still-unclaimed region.
/// This is a bounded approximation, not a global assignment optimum.
pub fn project(surface: &Surface, roots: &[V]) -> Result<Vec<u32>, String> {
    let n = surface.centers.len();
    if roots.is_empty()
        || roots.len() > n
        || roots.len() > 32
        || roots.iter().any(|root| {
            root.iter().any(|value| !value.is_finite()) || (dot(*root, *root) - 1.).abs() > 1e-12
        })
    {
        return Err("Invalid continuous plate-root projection.".into());
    }
    let mut used = vec![false; n];
    let mut cells = Vec::with_capacity(roots.len());
    for root in roots {
        let mut best = None;
        let mut best_score = f64::NEG_INFINITY;
        for (region, center) in surface.centers.iter().enumerate() {
            if used[region] {
                continue;
            }
            let score = dot(*root, *center);
            if score > best_score {
                best = Some(region);
                best_score = score;
            }
        }
        let region = best.ok_or("Continuous plate-root projection ran out of regions.")?;
        used[region] = true;
        cells.push(region as u32);
    }
    Ok(cells)
}
