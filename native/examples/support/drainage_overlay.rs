//! Static bed-receiver links, NOT active rivers or hydraulic water-surface flow.
use planimulation_core::{Surface, drainage::Drainage};
use std::fmt::Write as _;

#[derive(Debug, PartialEq)]
pub struct Stats {
    pub potential_links: usize,
    pub closed_terminal_markers: usize,
}

pub fn append(
    svg: &mut String,
    s: &Surface,
    bed: &[f64],
    bodies: &[u32],
    minimum_area_square_meters: f64,
) -> Stats {
    let graph = Drainage::build(s, bed, bodies);
    let mut stats = Stats {
        potential_links: 0,
        closed_terminal_markers: 0,
    };
    let project = |i: usize| {
        let p = s.centers[i];
        (
            (p[2].atan2(p[0]) / std::f64::consts::TAU + 0.5) * 760.,
            (0.5 - p[1].clamp(-1., 1.).asin() / std::f64::consts::PI) * 380.,
        )
    };
    for (i, &body) in bodies.iter().enumerate() {
        let area = graph.contributing_area[i];
        if body != 0 || area < minimum_area_square_meters {
            continue;
        }
        let (x, y) = project(i);
        let receiver = graph.receivers[i] as usize;
        if receiver == i {
            stats.closed_terminal_markers += 1;
            write!(svg,"<circle cx=\"{x:.3}\" cy=\"{y:.3}\" r=\"1.8\" fill=\"none\" stroke=\"#fac96a\" stroke-width=\"0.7\"/>").unwrap();
            continue;
        }
        stats.potential_links += 1;
        let (mut rx, ry) = project(receiver);
        while rx - x > 380. {
            rx -= 760.;
        }
        while rx - x < -380. {
            rx += 760.;
        }
        let width = 0.35
            + 0.1
                * (area / minimum_area_square_meters.max(1.))
                    .ln()
                    .clamp(0., 4.);
        for shift in [-760., 0., 760.] {
            if (x + shift < 0. && rx + shift < 0.) || (x + shift > 760. && rx + shift > 760.) {
                continue;
            }
            write!(svg,"<path d=\"M {:.3},{y:.3} L {:.3},{ry:.3}\" fill=\"none\" stroke=\"#68c9f0\" stroke-width=\"{width:.3}\" opacity=\"0.8\"/>",x+shift,rx+shift).unwrap();
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s() -> Surface {
        Surface {
            centers: vec![[1., 0., 0.], [0., 0., 1.], [-1., 0., 0.]],
            faces: vec![],
            offsets: vec![0, 1, 3, 4],
            neighbors: vec![1, 0, 2, 1],
            distances: vec![1.; 4],
            areas: vec![1., 2., 4.],
            boundary_offsets: vec![],
            boundaries: vec![],
        }
    }
    #[test]
    fn overlay_filters_actual_accumulated_area_without_changing_receivers_or_bed() {
        let surface = s();
        let bed = [-1., 2., 4.];
        let bodies = [1, 0, 0];
        let mut svg = String::new();
        assert_eq!(
            append(&mut svg, &surface, &bed, &bodies, 5.),
            Stats {
                potential_links: 1,
                closed_terminal_markers: 0
            }
        );
        assert_eq!(svg.matches("<path").count(), 1);
        assert_eq!(bed, [-1., 2., 4.]);
        let mut svg = String::new();
        assert_eq!(
            append(&mut svg, &surface, &bed, &bodies, 100.),
            Stats {
                potential_links: 0,
                closed_terminal_markers: 0
            }
        );
        assert!(svg.is_empty());
    }
    #[test]
    fn closed_dry_terminal_is_marked_not_connected_to_a_fictional_target() {
        let mut svg = String::new();
        let surface = s();
        let bed = [4., 1., 5.];
        let bodies = [0; 3];
        assert_eq!(
            append(&mut svg, &surface, &bed, &bodies, 1.),
            Stats {
                potential_links: 2,
                closed_terminal_markers: 1
            }
        );
        assert_eq!(svg.matches("<circle").count(), 1);
    }
}
