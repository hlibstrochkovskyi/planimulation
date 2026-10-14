//! Diagnostic equirectangular display of real sampled beds; seam/pole polygon
//! projection is approximate and never changes authoritative classification.
use planimulation_core::World;
use std::fmt::Write as _;

fn color(height: f64, depth: f64) -> String {
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

pub fn render(before: &World, after: &World) -> String {
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1600\" height=\"560\" viewBox=\"0 0 1600 560\"><rect width=\"1600\" height=\"560\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style><text x=\"24\" y=\"30\" font-size=\"20\">Physical geography comparison — height/depth colors, not biomes</text><defs><clipPath id=\"clip\"><rect width=\"760\" height=\"380\"/></clipPath></defs>",
    );
    for (panel, (world, title)) in [
        (before, format!("Baseline: {}", before.recipe.model_version)),
        (after, format!("Candidate: {}", after.recipe.model_version)),
    ]
    .into_iter()
    .enumerate()
    {
        let x0 = 24 + panel * 792;
        write!(svg,"<text x=\"{x0}\" y=\"68\" font-size=\"14\">{title}</text><g transform=\"translate({x0},84)\" clip-path=\"url(#clip)\"><rect width=\"760\" height=\"380\" fill=\"#0c284c\"/>").unwrap();
        let surface = &world.surface;
        for (i, &height) in world.terrain.elevation.iter().enumerate() {
            let center = surface.centers[i];
            let longitude = center[2].atan2(center[0]);
            let fill = color(
                height - world.water.level_meters,
                world.water.depth_meters[i],
            );
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
        svg.push_str("</g>");
    }
    svg.push_str("<text x=\"24\" y=\"498\" font-size=\"13\">Fixed shared palette: dry height above each initial water level 0–4500 m; water depth 0–5000 m; extremes saturate.</text><text x=\"24\" y=\"521\" font-size=\"13\">Same requested 71% initial water coverage, not the same inferred water volume. Existing dry smoothing only; no new erosion.</text><text x=\"24\" y=\"544\" font-size=\"13\">Regional polygons are a seam/pole display approximation; no decorative terrain or biome classification is added.</text></svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::color;
    #[test]
    fn fixed_palette_endpoints_and_saturation_are_explicit() {
        assert_eq!(color(0., 0.), "#517749");
        assert_eq!(color(4500., 0.), "#e0d3b0");
        assert_eq!(color(9000., 0.), color(4500., 0.));
        assert_eq!(color(-1., 5000.), "#0c284c");
        assert_eq!(color(-1., 10000.), color(-1., 5000.));
    }
}
