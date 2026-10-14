//! Pure actual-bed display, shared by generation review and read-only rendering.
#[path = "atlas_field.rs"]
mod atlas_field;
#[path = "drainage_overlay.rs"]
mod drainage_overlay;
#[path = "terrain_shading.rs"]
mod terrain_shading;
use planimulation_core::{Surface, water::Water};
use std::fmt::Write as _;
pub fn panel(
    svg: &mut String,
    s: &Surface,
    bed: &[f64],
    water: &Water,
    x: usize,
    y: usize,
    title: &str,
) -> Result<(), String> {
    let light = terrain_shading::lighting(s, bed, water.level_meters, 20.)?;
    write!(svg,"<text x=\"{x}\" y=\"{y}\" font-size=\"15\">{title}</text><g transform=\"translate({x},{})\" clip-path=\"url(#clip)\">",y+12).unwrap();
    atlas_field::append_field(svg, s, |i| {
        let base = atlas_field::terrain_color(bed[i] - water.level_meters, water.depth_meters[i]);
        if water.depth_meters[i] > 0. {
            return base;
        }
        let rgb = std::array::from_fn::<_, 3, _>(|j| {
            u8::from_str_radix(&base[1 + j * 2..3 + j * 2], 16).expect("Internal RGB palette.")
        });
        let lit = rgb.map(|v| (f64::from(v) * light[i]).round().clamp(0., 255.) as u8);
        format!("#{:02x}{:02x}{:02x}", lit[0], lit[1], lit[2])
    });
    drainage_overlay::append(svg, s, bed, &water.body_ids, 50_000_000_000.);
    svg.push_str("</g>");
    Ok(())
}
pub fn single(s: &Surface, bed: &[f64], water: &Water) -> Result<String, String> {
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"810\" height=\"515\"><rect width=\"810\" height=\"515\" fill=\"#10191f\"/><style>text{font-family:DejaVu Sans,sans-serif;fill:#e6edf3}</style><defs><clipPath id=\"clip\"><rect width=\"760\" height=\"380\"/></clipPath></defs>",
    );
    panel(
        &mut svg,
        s,
        bed,
        water,
        24,
        32,
        "Actual prepared bed and static drainage",
    )?;
    svg.push_str("<text x=\"24\" y=\"464\" font-size=\"12\">Actual slope hillshade; display exaggeration20x; NW45°. Colors are not biomes.</text><text x=\"24\" y=\"490\" font-size=\"12\">Blue: static receivers. Yellow: closed dry terminals. No physical state changed.</text></svg>");
    Ok(svg)
}
