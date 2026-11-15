//! Headless comparison adapter, not a World/IPC/checkpoint schema.
use planimulation_core::{
    drainage_landforms::{self, Construction},
    irregular_patch::{self, Geometry},
    terrain_patch_geometry::Settings,
};
use serde::Serialize;
use std::{fmt::Write as _, time::Instant};

pub const REPORT_VERSION: &str = "native-resampled-landform-patch-1";
pub const STRUCTURAL_REPORT_VERSION: &str = "native-structural-landform-patch-1";
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub report_version: &'static str,
    pub source_field_model_version: &'static str,
    pub geometry: Geometry,
    pub source_bed_meters: Vec<f64>,
    pub structural_actual_dry_receivers: Vec<u32>,
    pub construction: Construction,
    pub actual_dry_receivers: Vec<u32>,
    pub actual_dry_contributing_area_square_meters: Vec<f64>,
    pub closed_dry_fraction_of_active_area: f64,
    pub terminal_area_relative_residual: f64,
    pub mesh_eightfold_order: f64,
    pub actual_channel_eightfold_order_above_1km2: Option<f64>,
    pub geometry_seconds: f64,
    pub construction_and_routing_seconds: f64,
}

fn collection(g: &Geometry, receivers: &[u32]) -> Result<(Vec<f64>, f64, f64), String> {
    let n = receivers.len();
    let mut incoming = vec![0_usize; n];
    let mut c: Vec<_> = g
        .reference_areas_square_meters
        .iter()
        .zip(&g.fixed_external_boundary)
        .map(|(&a, &fixed)| if fixed { 0. } else { a })
        .collect();
    let total: f64 = c.iter().sum();
    let mut correction = vec![0.; n];
    for (i, &r) in receivers.iter().enumerate() {
        if r as usize != i {
            incoming[r as usize] += 1;
        }
    }
    let mut order: Vec<_> = (0..n).filter(|&i| incoming[i] == 0).collect();
    let mut cursor = 0;
    while cursor < order.len() {
        let i = order[cursor];
        cursor += 1;
        let r = receivers[i] as usize;
        if r == i {
            continue;
        }
        let amount = c[i] - correction[r];
        let next = c[r] + amount;
        correction[r] = (next - c[r]) - amount;
        c[r] = next;
        incoming[r] -= 1;
        if incoming[r] == 0 {
            order.push(r);
        }
    }
    if order.len() != n {
        return Err("Actual irregular graph contains a cycle.".into());
    }
    let terminal: f64 = (0..n)
        .filter(|&i| receivers[i] as usize == i)
        .map(|i| c[i])
        .sum();
    let closed = (0..n)
        .filter(|&i| receivers[i] as usize == i && !g.fixed_external_boundary[i])
        .map(|i| c[i])
        .fold(0., |a, b| a + b);
    let residual = (terminal - total).abs() / total;
    if !residual.is_finite() || residual > 1e-12 || c.iter().any(|v| !v.is_finite() || *v < 0.) {
        return Err("Irregular actual source areas do not reconcile.".into());
    }
    Ok((c, closed / total, residual))
}

fn orientation<'a>(links: impl Iterator<Item = (&'a [f64; 2], &'a [f64; 2])>) -> Option<f64> {
    let (mut cos, mut sin, mut total) = (0., 0., 0.);
    for (p, q) in links {
        let x = q[0] - p[0];
        let y = q[1] - p[1];
        let length = x.hypot(y);
        let angle = 8. * y.atan2(x);
        cos += length * angle.cos();
        sin += length * angle.sin();
        total += length;
    }
    (total > 0.).then(|| cos.hypot(sin) / total)
}

pub fn build(
    seed: &str,
    settings: Settings,
    source_bed: &[f64],
    construction_settings: drainage_landforms::Settings,
) -> Result<Review, String> {
    let started = Instant::now();
    let geometry = irregular_patch::build(seed, settings)?;
    let sampled = irregular_patch::sample_regular_source(
        settings,
        source_bed,
        &geometry.chart_points_meters,
    )?;
    let geometry_seconds = started.elapsed().as_secs_f64();
    from_geometry(
        geometry,
        sampled,
        construction_settings,
        geometry_seconds,
        irregular_patch::REGULAR_SOURCE_SAMPLING_VERSION,
    )
}

pub fn from_geometry(
    geometry: Geometry,
    sampled: Vec<f64>,
    construction_settings: drainage_landforms::Settings,
    geometry_seconds: f64,
    source_field_model_version: &'static str,
) -> Result<Review, String> {
    let started = Instant::now();
    let structural = drainage_landforms::dry_receivers(
        &sampled,
        &geometry.reference_areas_square_meters,
        &geometry.fixed_external_boundary,
        &geometry.edges,
    )?;
    let construction = drainage_landforms::construct(
        &sampled,
        &geometry.reference_areas_square_meters,
        &geometry.fixed_external_boundary,
        &geometry.edges,
        &structural,
        construction_settings,
    )?;
    let actual = drainage_landforms::dry_receivers(
        &construction.elevation_meters,
        &geometry.reference_areas_square_meters,
        &geometry.fixed_external_boundary,
        &geometry.edges,
    )?;
    let (areas, closed, residual) = collection(&geometry, &actual)?;
    let construction_and_routing_seconds = started.elapsed().as_secs_f64();
    let mesh_order = orientation(
        geometry
            .edges
            .iter()
            .filter(|e| {
                !(geometry.fixed_external_boundary[e.a as usize]
                    && geometry.fixed_external_boundary[e.b as usize])
            })
            .map(|e| {
                (
                    &geometry.chart_points_meters[e.a as usize],
                    &geometry.chart_points_meters[e.b as usize],
                )
            }),
    )
    .ok_or("No active mesh edges.")?;
    let channel_order = orientation(
        (0..actual.len())
            .filter(|&i| actual[i] as usize != i && areas[i] >= 1e6)
            .map(|i| {
                (
                    &geometry.chart_points_meters[i],
                    &geometry.chart_points_meters[actual[i] as usize],
                )
            }),
    );
    Ok(Review {
        report_version: "native-irregular-landform-review-1",
        source_field_model_version,
        geometry,
        source_bed_meters: sampled,
        structural_actual_dry_receivers: structural,
        construction,
        actual_dry_receivers: actual,
        actual_dry_contributing_area_square_meters: areas,
        closed_dry_fraction_of_active_area: closed,
        terminal_area_relative_residual: residual,
        mesh_eightfold_order: mesh_order,
        actual_channel_eightfold_order_above_1km2: channel_order,
        geometry_seconds,
        construction_and_routing_seconds,
    })
}

pub fn atlas(r: &Review) -> String {
    let points = &r.geometry.chart_points_meters;
    let s = r.geometry.settings;
    let half = (f64::from(s.cells_x) - 1.) * s.spacing_meters / 2.;
    let scale = 480. / (2. * half);
    let point = |i: u32| {
        let p = points[i as usize];
        [(p[0] + half) * scale, (half - p[1]) * scale]
    };
    let mut out = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1550\" height=\"620\"><rect width=\"1550\" height=\"620\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style>",
    );
    for (panel, (title, bed, links)) in [
        (
            "Native initial source bed",
            r.source_bed_meters.as_slice(),
            false,
        ),
        (
            "Native irregular construction",
            r.construction.elevation_meters.as_slice(),
            false,
        ),
        (
            "Rebuilt actual dry collection",
            r.construction.elevation_meters.as_slice(),
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let origin = 24. + panel as f64 * 512.;
        let min = bed.iter().copied().fold(f64::INFINITY, f64::min);
        let max = bed.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        write!(out,"<text x=\"{origin}\" y=\"28\" font-size=\"18\">{title}</text><text x=\"{origin}\" y=\"49\" font-size=\"13\">Physical height {min:.1} to {max:.1} m</text><g transform=\"translate({origin},66)\" shape-rendering=\"crispEdges\">").unwrap();
        for &[ia, ib, ic] in &r.geometry.triangles {
            let [a, b, c] = [ia, ib, ic].map(|i| points[i as usize]);
            let dx = b[0] - a[0];
            let dy = b[1] - a[1];
            let ex = c[0] - a[0];
            let ey = c[1] - a[1];
            let det = dx * ey - dy * ex;
            let da = bed[ib as usize] - bed[ia as usize];
            let db = bed[ic as usize] - bed[ia as usize];
            let gx = (da * ey - db * dy) / det;
            let gy = (dx * db - ex * da) / det;
            let light =
                0.25 + 0.75 * ((gx - gy + 1.) / (3. * (gx * gx + gy * gy + 1.)).sqrt()).max(0.);
            let color = (255. * light).round() as u8;
            let [a, b, c] = [ia, ib, ic].map(point);
            write!(out,"<path d=\"M {:.4} {:.4} L {:.4} {:.4} L {:.4} {:.4} Z\" fill=\"#{color:02x}{color:02x}{color:02x}\"/>",a[0],a[1],b[0],b[1],c[0],c[1]).unwrap();
        }
        if links {
            out.push_str("<g stroke=\"#46b8d7\" stroke-width=\"0.65\" opacity=\"0.8\" shape-rendering=\"geometricPrecision\">");
            for i in 0..bed.len() {
                let j = r.actual_dry_receivers[i];
                if j as usize != i && r.actual_dry_contributing_area_square_meters[i] >= 1e6 {
                    let a = point(i as u32);
                    let b = point(j);
                    write!(
                        out,
                        "<path d=\"M {:.4} {:.4} L {:.4} {:.4}\"/>",
                        a[0], a[1], b[0], b[1]
                    )
                    .unwrap();
                }
            }
            out.push_str("</g>");
        }
        out.push_str("</g>");
    }
    write!(out,"<text x=\"24\" y=\"574\" font-size=\"14\">{} physical points / native Poisson mesh / approximate chart triangle normals, heights 1x / no height normalization</text>",points.len()).unwrap();
    let scope = if r.source_field_model_version == irregular_patch::REGULAR_SOURCE_SAMPLING_VERSION
    {
        "Source: resampled native preparation, not irregular erosion or a global World."
    } else {
        "Source: direct scaffold initial conditions, not an erosion-time solution or a global World."
    };
    write!(out,"<text x=\"24\" y=\"599\" font-size=\"13\">Cyan: actual dry area above 1 km2, not discharge. {scope}</text></svg>").unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn orientation_diagnostic_has_independent_axis_and_balanced_controls() {
        let origin = [0., 0.];
        let axes = [[1., 0.], [0., 1.], [-1., 0.], [0., -1.]];
        assert_eq!(orientation(axes.iter().map(|p| (&origin, p))), Some(1.));
        let points: Vec<_> = (0..16)
            .map(|i| {
                let a = i as f64 * std::f64::consts::TAU / 16.;
                [a.cos(), a.sin()]
            })
            .collect();
        assert!(orientation(points.iter().map(|p| (&origin, p))).unwrap() < 1e-14);
        assert_eq!(orientation(std::iter::empty()), None);
    }
}
