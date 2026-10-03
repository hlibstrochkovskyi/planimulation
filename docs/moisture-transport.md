# Conservative prescribed moisture transport

`moisture-transport-1` advances a prescribed column-water stock on the native sphere with a frozen tangent wind field. This is a reusable Rust transport kernel and a bounded headless seasonal report. It is the first time-varying atmospheric-water calculation, but it is not a coupled climate model or desktop playback. No generated surface water is consumed or modified.

## Model and units

Each region stores `M_i` kilograms of column water over its reference spherical area `A_i` in m². The transported concentration is `q_i = M_i / A_i` in kg/m². The modeled equation is `∂q/∂t + div(q v) = 0`. There is no atmospheric depth, air-density field, or claim of atmospheric mass or momentum conservation. A prescribed convergent velocity can concentrate column water; a constant concentration is a fixed point only for a divergence-free velocity.

The method uses piecewise-constant upwind donor concentrations and shared boundary fluxes. [LeVeque's finite-volume lecture](https://faculty.washington.edu/rjl/classes/am574w2011/slides/am574lecture7nup3.pdf) describes this first-order reconstruction and the associated numerical diffusion. We implement the method locally; no external solver code or dependency is imported. Mesh geometry, quadrature, bounded substepping, and error tolerances below are our implementation choices.

## Shared spherical boundaries

The existing computational regions are **barycentric duals**, not spherical Voronoi polygons. For each primal mesh edge, its two incident triangles supply two dual boundary segments: from each normalized triangle centroid to the normalized primal-edge midpoint. Geometry is reconstructed from native faces and checked against each existing region area and the recorded radius. Every undirected boundary appears once in sorted region-ID order.

For a segment with unit-sphere endpoints `a,b`:

```text
α = atan2(|a × b|, a · b)
p = normalize(a + b)
n = ±normalize(a × b), directed out of the lower-ID region
arc length = R α
quadrature weight = 2 R sin(α/2)
g = v(p) · n · quadrature weight             [m²/s]
```

The segment conormal is tangent to the sphere. The curvature-aware quadrature integrates rigid-rotation velocities `v(x) = R ω × x` exactly: the integral of `x` over that arc is `2 R sin(α/2) p`. For arbitrary prescribed smooth velocities it is an approximation, not an exact general flux integral. This gives the closed-cell rigid-rotation test a uniform-concentration fixed point without a divergence correction. Oppositely directed flows on the two segments retain their own donors instead of being canceled into one net rate before upwinding.

Longitude never enters connectivity or boundary construction. Transport crosses the flat-map seam and approaches the poles through the same geometry.

## Update and numerical contract

For each segment, choose its donor by the sign of `g`, and calculate `ΔM = |g| q_donor Δt`. All fluxes read the old stock snapshot. The same amount enters the recipient's incoming accumulator and the donor's outgoing accumulator. Then update `M_new = (M_old − outgoing) + incoming` in separate stock storage.

Let `r_i = Σ outgoing |g| / A_i`. Substeps are chosen before processing an interval:

```text
N = max(1, ceil(interval_seconds × max(r_i) / max_outgoing_fraction))
Δt = interval_seconds / N
```

The default maximum outgoing fraction is 0.8, leaving a margin below complete donor depletion. Supported fractions are greater than zero and at most 0.9. The default substep limit is 4,096 per call; a caller can explicitly choose 1–1,000,000. Exceeding the declared work limit rejects the interval. The calculation never clamps negative stocks, rescales the final distribution, or adds a residual to a favored region. Invalid geometry, non-tangent/non-finite wind, invalid stock, overflow, and failed mass checks return an error without changing the caller's inputs.

Budget totals use compensated summation. An interval must satisfy `|final − initial| ≤ 1e−12 × max(initial, 1 kg)`. This is a floating-point tolerance, not exact integer water accounting. The report also applies that bound to its entire run. Gross transported mass counts every crossing, so it can exceed the total stock during repeated transport; it is not an external input or a net transfer.

## Seasonal report and reproducibility

```sh
cargo run --release --locked --manifest-path native/Cargo.toml --example moisture_transport_report -- docs/scenarios/seasonal-temperature.json
cargo run --release --locked --manifest-path native/Cargo.toml --example moisture_transport_report -- docs/scenarios/seasonal-temperature.json 730
cargo run --release --locked --manifest-path native/Cargo.toml --example moisture_transport_report -- --ensemble
cargo test --locked --manifest-path native/Cargo.toml --test moisture_transport --example moisture_transport_report
```

The report starts at northern spring equinox and accepts 1–3,650 model days. A model day is 86,400 seconds, a year is 365 days, and the existing twelve numbered month bins are preserved. Wind is held at its monthly mean throughout a day and sampled directly at the boundary-segment midpoints from the same daily belt formula as [`seasonal-wind-1`](seasonal-wind.md). It is not interpolated from map colors or local east/north components at different centers.

The artificial starting column is 25 kg/m² over initially wet regions and 5 kg/m² over initially dry regions. These values are declared test conditions, not inferred atmospheric moisture or evaporation. They are **not deducted from surface water**. Source and sink terms are zero throughout the run. The report records the full recipe, model versions, wind and transport settings, calendar, final stocks, cumulative budget, column extrema, and net movement onto initially dry regions. It repeats the run with a 0.4 outgoing-fraction limit and reports the relative L1 difference in final stocks.

The ensemble contains all combinations of three fixed seeds, subdivisions 2–4, and radii 1,000 and 6,371 km: eighteen one-year runs and their refined-step counterparts. Failed cases retain their recipes and reasons. Generated terrain and wet masks change with resolution, so ensemble differences do not establish spatial convergence. No random stream is consumed by transport. Stock JSON round-trip continuation is tested under the same prescribed flow; there is no complete dynamic-climate checkpoint schema or cross-version replay promise.

Measured October 3, 2026 on the supported Linux environment: all eighteen baseline runs and their refined counterparts completed. The largest baseline relative cumulative mass residual was `5.2e−16`. Final-stock relative L1 differences after reducing the outgoing-fraction limit ranged from `9.1e−5` to `7.2e−4` (about 0.009–0.072%). Maximum final columns ranged from 60.1 to 218.7 kg/m² despite an initial maximum of 25 kg/m². This demonstrates convergence-driven concentration in the deliberately source-free run and the need for explicit phase exchange before interpreting it as a climate. It is not a calibrated atmospheric-water range.

## Validation and remaining coupling

Tests check dual boundary counts and area agreement at subdivisions 0–6, zero wind, empty stocks, arbitrary-axis rigid rotation, single-donor old-snapshot transfers, mass conservation, nonnegative stocks, seam-crossing of an analytically rotated smooth blob, decreasing errors under spatial refinement, convergence under smaller transport steps, deterministic replay, stock round trips, and atomic rejection. The headless report test checks repeatability and net delivery to dry regions under the seasonal wind. These establish bounded numerical behavior; they do not validate an Earth-like moisture climatology.

The separate [finite seasonal-moisture model](seasonal-moisture.md) adds six owned stocks and [delayed runoff with evaporating terminal stores](runoff-transport.md), regional atmospheric/surface/graph ledgers, and a complete bounded-model checkpoint. This adds a finite runoff-return path without changing the source-free report above. An opt-in [desktop mode](seasonal-water-desktop.md) now advances and inspects that finite model. Temperature, effective-column exchange, [surface laws](surface-water.md), and routing remain uncalibrated. Terrain lifting, rain shadows, groundwater, basin lake levels/spills, changing water surfaces, general world checkpoints, and desktop seasonal save/load remain unimplemented. Milestone D is incomplete.
