//! Initial global filling, not drainage, precipitation, or communication between basins.
use crate::Surface;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "camelCase", deny_unknown_fields)]
pub enum WaterSettings {
    Coverage {
        fraction: f64,
    },
    Volume {
        #[serde(rename = "volumeCubicMeters")]
        volume_cubic_meters: f64,
    },
}
impl WaterSettings {
    pub fn validate(&self, surface_area: f64) -> Result<(), String> {
        let valid = match *self {
            Self::Coverage { fraction } => fraction.is_finite() && (0.0..=1.).contains(&fraction),
            Self::Volume {
                volume_cubic_meters: v,
            } => v.is_finite() && (0.0..=surface_area * 20_000.).contains(&v),
        };
        if valid {
            Ok(())
        } else {
            Err("Water coverage must be 0–1; volume must be nonnegative and at most a 20 km global equivalent layer.".into())
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Water {
    pub level_meters: f64,
    pub resolved_volume_cubic_meters: f64,
    pub depth_meters: Vec<f64>,
    /// Zero is dry; positive IDs follow ascending first-region traversal order.
    pub body_ids: Vec<u32>,
    pub main_ocean_id: u32,
}

impl Water {
    pub fn generate(
        s: &Surface,
        elevation: &[f64],
        settings: &WaterSettings,
    ) -> Result<Self, String> {
        if elevation.is_empty()
            || elevation.len() != s.areas.len()
            || elevation.iter().any(|h| !h.is_finite())
            || s.areas.iter().any(|a| !a.is_finite() || *a <= 0.)
        {
            return Err("Invalid initial water fitting fields.".into());
        }
        let total: f64 = s.areas.iter().sum();
        // Reference-sphere area and summed discrete areas differ by roundoff.
        settings.validate(total * (1. + 1e-12))?;
        let mut order: Vec<usize> = (0..elevation.len()).collect();
        order.sort_by(|&a, &b| elevation[a].total_cmp(&elevation[b]).then(a.cmp(&b)));
        let level = match *settings {
            WaterSettings::Coverage { fraction } => {
                let target = fraction * total;
                let mut area = 0.;
                let mut best_error = target;
                let mut level = elevation[order[0]];
                for (i, &id) in order.iter().enumerate() {
                    area += s.areas[id];
                    if i + 1 < order.len() && elevation[order[i + 1]] == elevation[id] {
                        continue;
                    }
                    // Equal-elevation plateaus cannot be split. Ties prefer less water.
                    let error = (area - target).abs();
                    if error < best_error {
                        best_error = error;
                        level = if i + 1 == order.len() {
                            elevation[id] + 1.
                        } else {
                            let next = elevation[order[i + 1]];
                            let midpoint = elevation[id] + (next - elevation[id]) * 0.5;
                            if midpoint > elevation[id] {
                                midpoint
                            } else {
                                next
                            }
                        };
                    }
                }
                level
            }
            WaterSettings::Volume {
                volume_cubic_meters,
            } => {
                let mut remaining = volume_cubic_meters;
                let mut active_area = 0.;
                let mut level = elevation[order[0]];
                for &id in &order {
                    let capacity = active_area * (elevation[id] - level);
                    if remaining <= capacity && active_area > 0. {
                        break;
                    }
                    remaining -= capacity;
                    level = elevation[id];
                    active_area += s.areas[id];
                }
                level + remaining / active_area
            }
        };
        let depth_meters: Vec<f64> = elevation.iter().map(|h| (level - h).max(0.)).collect();
        let volume: f64 = depth_meters.iter().zip(&s.areas).map(|(d, a)| d * a).sum();
        if !level.is_finite() || !volume.is_finite() {
            return Err("Initial water fitting overflowed.".into());
        }
        if let WaterSettings::Volume {
            volume_cubic_meters: requested,
        } = settings
            && (volume - requested).abs() > (requested * 1e-10).max(1e-9)
        {
            return Err(
                "Requested water volume cannot be resolved at this elevation precision.".into(),
            );
        }
        let mut body_ids = vec![0; elevation.len()];
        let mut count = 0;
        let mut main_ocean_id = 0;
        let mut largest_area = 0.;
        let mut queue = Vec::new();
        for start in 0..elevation.len() {
            if depth_meters[start] == 0. || body_ids[start] != 0 {
                continue;
            }
            count += 1;
            body_ids[start] = count;
            queue.clear();
            queue.push(start);
            let mut cursor = 0;
            let mut area = 0.;
            while cursor < queue.len() {
                let id = queue[cursor];
                cursor += 1;
                area += s.areas[id];
                for k in s.offsets[id]..s.offsets[id + 1] {
                    let next = s.neighbors[k as usize] as usize;
                    if depth_meters[next] > 0. && body_ids[next] == 0 {
                        body_ids[next] = count;
                        queue.push(next);
                    }
                }
            }
            if area > largest_area {
                largest_area = area;
                main_ocean_id = count;
            }
        }
        Ok(Self {
            level_meters: level,
            resolved_volume_cubic_meters: volume,
            depth_meters,
            body_ids,
            main_ocean_id,
        })
    }
}
