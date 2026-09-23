# Static plate kinematics: milestone B1

Status: implemented. This document records B1 with model recipe `tectonics-1`, binary protocol 2. [B2 initial crust](crust.md), [B3 elevation](terrain.md), and [B4 initial water](water.md) extend it; the current build uses `water-1` and protocol 5. Plate generation is unchanged. B1's measurements and recipe below are historical, not a description of geological time integration.

## What is implemented

- A complete, connected partition of the spherical region graph into seeded plates.
- Independent named random streams for plate roots, boundary-shape variation, and angular velocities.
- Rigid point velocities on a sphere, with physical units.
- Relative opening and signed shear at actual shared-boundary segments, rather than a random boundary label or a cell-wide approximation.
- Shared flat/globe layers for plate identity, dominant boundary motion, and speed. The region inspector shows the owning plate, its area, point speed, and local boundary components.
- Headless plate-area and boundary-count summaries, strict parameters, deterministic fingerprints, and tests.

Plates are **not** continents. A plate can later contain both continental and oceanic crust. No crust, subduction polarity, crust age, uplift, mountains, sea level, or plate advection is implemented in this increment. Diagnostic playback evolves only the existing scalar diffusion field and does not move plates.

## Recipe and fixed model choices

The existing schema has a new explicit model version and two required parameters:

```json
{
  "schemaVersion": 1,
  "modelVersion": "tectonics-1",
  "randomVersion": "fnv1a-utf8-mulberry32-1",
  "seed": "first-light",
  "subdivision": 5,
  "radiusMeters": 6371000,
  "plateCount": 12,
  "maxPlateSpeedCmPerYear": 8
}
```

Plate count is an integer from 2 to 32, no larger than the number of regions. At level 0 this limits it to 12. Maximum speed is finite and between 0 and 20 cm/year. Zero is supported and produces quiet boundaries without changing ownership. Incompatible parameter combinations are rejected, not silently clamped.

The speed parameter is an upper bound on the equatorial surface speed of any plate rotation, not a promised measured maximum. Individual plates draw an equatorial speed between 25% and 100% of the bound; speed at a particular region also depends on distance from the rotation axis. Absolute point speeds use the model's arbitrary reference frame. Relative boundary motion is unchanged by adding the same angular velocity to both plates.

Algorithm constants below are fixed by `tectonics-1`. Changing them requires a model-version review. Older `surface-1` and `surface-rust-1` recipes are rejected. To reuse visible parameters, create a new recipe and preserve the original file. There is no implicit migration or bitwise compatibility promise across model versions.

## Connected partition

1. Select the first root from mesh region IDs using `tectonics.seeds`.
2. For each later root, sample with weights proportional to the square of `min(1 − dot(candidate, existing_root))`. This favors separated roots without forcing equal-size plates. Existing roots have exactly zero weight.
3. Use `tectonics.partition-resistance` to sample six smooth sine modes in 3D. Their average, with amplitude 0.35 around one, creates strictly positive resistance in `[0.65, 1.35]`. These are geometric shape perturbations, not mantle material properties.
4. Run multi-source Dijkstra growth. Edge cost is unit-sphere chord length multiplied by mean endpoint resistance. Finalized regions expand their own plate to neighboring unclaimed regions. Every non-root has a same-plate predecessor, so each plate is connected by construction.
5. Heap ties are resolved by cost, plate ID, then region ID. Roots remain distinct and nonempty. No map coordinates or rendering randomness enter the partition.

Costs are dimensionless and independent of radius or speed. Changing radius preserves ownership. Changing the plate count preserves the prefix of selected roots and sampled rotations; final ownership can change globally. Changing speed preserves all ownership and boundary geometry. Geometry and diagnostic randomness retain their separate streams.

This is a static graph partition, not a fracture or mantle convection simulation. Its geometric and statistical biases must not be interpreted as geological evidence. Mesh resolution can affect boundary shape and plate areas; convergence of geological predictions has not been established.

## Plate velocities and boundary geometry

Each plate draws a uniformly distributed rotation-axis direction and a bounded angular rate from `tectonics.motion`. Angular velocity is stored as an xyz vector in radians/year. With unit surface direction `p` and radius `R`:

```text
velocity(plate, p) = R * (omega[plate] × p)   # meters/year
```

The velocity is tangent to the sphere. Increasing radius while keeping the configured linear speed fixed decreases angular velocity proportionally; the resulting local linear speeds remain unchanged.

Two neighboring regions in different plates share a bent barycentric-dual boundary. It consists of two minor great-circle segments: each adjacent triangle's normalized centroid connects to the shared primal-edge midpoint. Store both segments separately. This preserves the actual geometry already used by the surface model, including its local normal changes.

For each segment:

1. Sample at the normalized midpoint of its endpoint directions.
2. Compute a unit tangent-plane normal to the segment, oriented from the lower region ID toward the higher region ID.
3. Evaluate both plates' velocities at that **same** location.
4. Project their difference onto the normal and the tangent `p × normal`.

```text
relative = velocity(B, p) − velocity(A, p)
opening = dot(relative, normal)          # positive: divergence
shear   = dot(relative, p × normal)      # signed, model-oriented
```

Both components are stored in meters/year and displayed in cm/year. No interpolation between incompatible local east/north frames is used. Neighbor order changes do not change the physical classification.

## Boundary categories and explanations

Classification is an explicit display/model convention, not an assertion that real boundaries have no oblique component:

| Condition | Code | Label |
| --- | ---: | --- |
| `hypot(opening, shear) ≤ 1e-8 m/year` | 0 | Quiet |
| Normal dominates (`abs(opening) ≥ abs(shear)`) and opening is negative | 1 | Convergent |
| Normal dominates and opening is positive | 2 | Divergent |
| Tangential component dominates | 3 | Transform-dominant |

The numerical quiet threshold and 45-degree dominance convention are versioned choices. They are not externally calibrated tectonic criteria. Future uplift should use the continuous components and crust properties, not treat this four-way label as complete physics. Nearby short segments can receive different labels because their true mesh-boundary normals differ. Do not smooth the underlying motion to make a prettier legend.

Each inspector row refers to one half-boundary segment. Two rows can therefore refer to the same neighboring plate while reporting different components. The current explanation identifies plate membership, the root region, and measured local drivers. It does not claim to explain a continent or mountain that has not yet been modeled.

Plate areas are sums of spherical region areas. The sum over plates equals the planet's surface area within numerical tolerance. Boundary counts in CLI summaries are **segment counts**, not boundary-length fractions.

## Protocol 2 and rendering

Framing, byte order, limits, and process ownership remain as documented in [Native/GPU foundation](native-foundation.md). Protocol 2 changes every response's protocol tag to `2`. A world header adds `boundarySegmentCount = B`. After the original surface arrays and diagnostic field, append:

| Array | Type | Length |
| --- | --- | ---: |
| Plate owner by region | Uint32 | N |
| Plate seed regions | Uint32 | P |
| Angular velocities, xyz | Float64 | 3P |
| Segment region pairs | Uint32 | 2B |
| Segment endpoints, xyz | Float64 | 6B |
| Opening and signed shear | Float64 | 2B |
| Segment classifications | Uint32 | B |

The additional body size is `4N + 28P + 76B` bytes. `B` must be even and cannot exceed the directed neighbor count. The adapter validates body length, owner/seed/type bounds, finite fields, cross-plate neighboring pairs, and exactly two segment records per shared primal edge. All arrays enter the initial world fingerprint. Diagnostic frames remain field-only and do not retransmit tectonics.

The display worker constructs a colored boundary-line overlay for each view. Flat boundary arcs are densified and clipped at the map seam, including polar endpoints; they never join across the map interior. Plate colors are categorical IDs, not an ordered scalar or land classification. Region-grid lines are optional and initially off to keep the plate structure legible. All display buffers are separate from the native model.

## Validation

The suite contains eight Rust tests and 23 TypeScript tests at this increment. The plate ensemble covers 20 seeds × three resolutions (0, 2, 4) × three plate counts (2, 7, and the supported maximum at that resolution): 180 combinations. It verifies nonempty connected ownership, complete shared-boundary coverage, actual dual-edge endpoints, finite values, tangency, and speed bounds. This is a structural ensemble, not evidence of realistic plate-size distributions.

Controlled cases cover convergence, divergence, pure shear, quiet and oblique motion, common-frame invariance, and the quiet threshold. Parameter-response checks cover half speed, zero speed, doubled radius, increased plate count, and isolation from the diagnostic field. Diagnostic evolution leaves all tectonic data unchanged. Repeated native worlds compare full arrays.

Desktop checks exercise the new layers, exact selection of a native boundary region, opening/shear inspector rows, identical region data across views, zero-speed generation, custom plate count, recipe round trips, canceled generation, and continued diagnostic playback. Shader/renderer exceptions are checked. Screenshots are reviewed in addition to numerical tests.

Final validation on September 23, 2026 passed all eight Rust tests, 23 TypeScript tests, TypeScript checking, Rust formatting, and Clippy with warnings denied. Development desktop checks passed on September 20; the packaged Linux x64 application passed the same desktop checks on September 23. Packaging succeeded. Vite reports a renderer chunk above its 500 kB advisory threshold; this remains a bundle-size warning, not a failed check.

Default example on the validated Linux binary: `first-light`, level 5, 12 plates, speed cap 8 cm/year gives initial fingerprint `4b44c7fc`, 2,656 boundary segments (539 convergent, 620 divergent, 1,497 transform-dominant), and 4.45 MiB of model arrays. These counts are regression observations, not quality targets or expected geological proportions.

A native-plus-adapter timing sample measured approximately 132 ms at 10,242 regions and 435 ms at 40,962 regions. Four diagnostic steps plus field delivery had sample p95 approximately 2.4 ms and 6.6 ms respectively. These measurements exclude GPU rendering. The benchmark's TS reference computes only the surface/diagnostic field, **not tectonics**, so it is not a like-for-like language comparison.

The September 20 desktop sample used the Ryzen 5 PRO 4650U laptop with 14.84 GiB system memory, hardware-enabled WebGL/compositing, the plate layer, and the region grid off:

| Regions | Native generation plus preparation of both views | Flat RAF p95 | Globe RAF p95 |
| ---: | ---: | ---: | ---: |
| 10,242 | 998 ms | 16.8 ms | 16.7 ms |
| 40,962 | 1,975 ms | 16.7 ms | 16.7 ms |

Each view sample contains 180 requested-animation-frame intervals under alternating zoom and native diagnostic diffusion. These are not GPU completion timings or a guarantee of future simulation performance. Plates remain static during the measurement. The raw local report is `artifacts/native-desktop-benchmark.json`; rerunning `npm run benchmark:desktop` replaces it. Differences from the earlier foundation benchmark cannot be attributed to this feature alone.

## Next increment and scientific boundaries

Next add continuous continentality/crust properties independently of plate identity, then inspectable elevation contributions conditioned on boundary motion and crust, and finally explicit initial water fitting. Do not call convergence subduction until the relevant crust and polarity model exists. No conservation of crustal area/volume over geological time is claimed: this increment does not integrate that time.

The physical distinction between divergent, convergent, and transform motion follows the terminology in [USGS: Understanding plate motions](https://pubs.usgs.gov/gip/dynamic/understanding.html). [GPlates velocity calculations](https://www.gplates.org/docs/pygplates/sample-code/pygplates_calculate_velocities_by_plate_id) illustrate evaluating plate-associated velocities on a sphere. Our partition, initial-condition distributions, and category thresholds are separate simplified design choices, not GPlates reconstructions or a scientifically validated Earth model.
