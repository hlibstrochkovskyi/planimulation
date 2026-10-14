//! Display actual regional polygons without modifying spherical physical fields.
use planimulation_core::Surface;
use std::fmt::Write as _;

pub fn terrain_color(height: f64, depth: f64) -> String {
    let (a, b, t) = if depth > 0. {
        (
            [31., 117., 157.],
            [12., 40., 76.],
            (depth / 5000.).clamp(0., 1.),
        )
    } else {
        (
            [81., 119., 73.],
            [224., 211., 176.],
            (height / 4500.).clamp(0., 1.),
        )
    };
    format!(
        "#{:02x}{:02x}{:02x}",
        (a[0] + (b[0] - a[0]) * t).round() as u8,
        (a[1] + (b[1] - a[1]) * t).round() as u8,
        (a[2] + (b[2] - a[2]) * t).round() as u8
    )
}

/// Append to an existing 760x380 clipped panel. Seam/pole polygons are only a
/// display approximation; no adjacency or height is inferred from them.
pub fn append_field(svg: &mut String, surface: &Surface, fill: impl Fn(usize) -> String) {
    for i in 0..surface.centers.len() {
        let center = surface.centers[i];
        let longitude = center[2].atan2(center[0]);
        let fill = fill(i);
        let points: Vec<_> = surface.boundaries
            [surface.boundary_offsets[i] as usize..surface.boundary_offsets[i + 1] as usize]
            .iter()
            .map(|p| {
                let mut lon = p[2].atan2(p[0]);
                while lon - longitude > std::f64::consts::PI {
                    lon -= std::f64::consts::TAU;
                }
                while lon - longitude < -std::f64::consts::PI {
                    lon += std::f64::consts::TAU;
                }
                (
                    (lon / std::f64::consts::TAU + 0.5) * 760.,
                    (0.5 - p[1].clamp(-1., 1.).asin() / std::f64::consts::PI) * 380.,
                )
            })
            .collect();
        for shift in [-760., 0., 760.] {
            if points.iter().all(|(x, _)| x + shift < 0.)
                || points.iter().all(|(x, _)| x + shift > 760.)
            {
                continue;
            }
            write!(
                svg,
                "<polygon fill=\"{fill}\" stroke=\"{fill}\" stroke-width=\"0.12\" points=\""
            )
            .unwrap();
            for (x, y) in &points {
                write!(svg, "{:.3},{:.3} ", x + shift, y).unwrap();
            }
            svg.push_str("\"/>");
        }
    }
}
