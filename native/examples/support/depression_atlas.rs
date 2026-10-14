//! Potential-depth map, never a replacement bed or a water-volume claim.
use super::geography_atlas::append_field;
use planimulation_core::{World, depression_analysis::Analysis};
use std::fmt::Write as _;

fn color(depth: f64, wet: bool) -> String {
    if wet {
        return "#0c284c".into();
    }
    if depth == 0. {
        return "#517749".into();
    }
    let t = (depth.ln_1p() / 500_f64.ln_1p()).clamp(0., 1.);
    let a = [150., 211., 225.];
    let b = [191., 68., 39.];
    format!(
        "#{:02x}{:02x}{:02x}",
        (a[0] + t * (b[0] - a[0])).round() as u8,
        (a[1] + t * (b[1] - a[1])).round() as u8,
        (a[2] + t * (b[2] - a[2])).round() as u8
    )
}

pub fn render(before: &World, after: &World, a: &Analysis, b: &Analysis) -> String {
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1600\" height=\"1040\"><rect width=\"1600\" height=\"1040\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style><text x=\"24\" y=\"30\" font-size=\"20\">Depression geometry — potential escape depth, NOT a filled bed or actual lake</text><defs><clipPath id=\"clip\"><rect width=\"760\" height=\"380\"/></clipPath></defs>",
    );
    for (row, ocean) in [false, true].into_iter().enumerate() {
        for (column, (world, analysis)) in [(before, a), (after, b)].into_iter().enumerate() {
            let x = 24 + column * 792;
            let y = 68 + row * 434;
            let target = if ocean {
                "main ocean"
            } else {
                "any initial water"
            };
            write!(svg,"<text x=\"{x}\" y=\"{y}\" font-size=\"14\">{} — escape to {target}</text><g transform=\"translate({x},{})\" clip-path=\"url(#clip)\">",world.recipe.model_version,y+16).unwrap();
            let escape = if ocean {
                &analysis.to_main_ocean
            } else {
                &analysis.to_initial_water
            };
            if let Some(escape) = escape {
                append_field(&mut svg, &world.surface, |i| {
                    color(
                        escape.escape_levels_meters[i]
                            - world.terrain.elevation[i].max(world.water.level_meters),
                        world.water.body_ids[i] != 0,
                    )
                });
            } else {
                svg.push_str(
                    "<text x=\"12\" y=\"30\">No explicit target: escape depth is undefined.</text>",
                );
            }
            svg.push_str("</g>");
        }
    }
    let mut x = 24;
    for (label, depth, wet) in [
        ("Initial water", 0., true),
        ("No barrier", 0., false),
        ("1 m", 1., false),
        ("10 m", 10., false),
        ("100 m", 100., false),
        ("500+ m", 500., false),
    ] {
        write!(svg,"<rect x=\"{x}\" y=\"946\" width=\"18\" height=\"18\" fill=\"{}\"/><text x=\"{}\" y=\"960\" font-size=\"13\">{label}</text>",color(depth,wet),x+26).unwrap();
        x += 170;
    }
    svg.push_str("<text x=\"24\" y=\"992\" font-size=\"13\">Common logarithmic depth palette, saturated at 500 m. Existing wet cells retain blue even when isolated from the main ocean.</text><text x=\"24\" y=\"1018\" font-size=\"13\">Closed spherical adjacency; explicit existing-water targets only. Heights, water stocks and static physical receivers remain unchanged.</text></svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::color;
    #[test]
    fn depth_palette_distinguishes_zero_water_and_saturated_positive_depth() {
        assert_eq!(color(0., false), "#517749");
        assert_eq!(color(100., true), "#0c284c");
        assert_eq!(color(500., false), "#bf4427");
        assert_eq!(color(1000., false), color(500., false));
        assert_ne!(color(0.1, false), color(0., false));
    }
}
