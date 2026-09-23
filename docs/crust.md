# Initial crust: milestone B2

Historical B2 baseline: `crust-1`, binary protocol 3. [B3 elevation](terrain.md) and [B4 initial water](water.md) extend it; the current build uses `water-1` and protocol 5, leaving crust generation unchanged. The recipe, measurements, and limitations below describe B2 itself, which does not generate elevation, water, geological ages, subduction polarity, or geological history.

## Meaning and boundaries

Continentality is a dimensionless interpolation coordinate in `[0, 1]`, not a measured rock fraction. A region is continental-dominant when its value is strictly greater than 0.5. Crust distribution is independent of initial plate ownership: a plate can contain both crust endmembers, and a crust patch can cross plate boundaries.

Continental-dominant crust is **not emerged land**. Continental crust can eventually lie below water, and oceanic crust can support islands. No ocean fraction, coastlines, or water depth can be inferred without elevation and water. The thickness field is crust thickness, not lithosphere thickness or surface height. This model initializes a simplified material distribution; it does not explain the geological history that created it.

## Recipe

All existing recipe fields remain required. The model version changes from `tectonics-1` to `crust-1` and two required fields are added:

| Field | Default | Range | Meaning |
| --- | ---: | ---: | --- |
| `continentalFraction` | 0.38 | 0–1 | Requested spherical area fraction of continental-dominant regions |
| `continentalScale` | 1 | 0.5–2 | Dimensionless multiplier of the spatial wavelengths; larger means broader structure |

The fraction is displayed as a percentage in the UI. Actual area is measured and shown separately; computational regions cannot be partially selected to meet the target. Scale does not guarantee a specified continent count, an archipelago, or a supercontinent. Zero and one are supported experimental extremes, not ordinary Earth-like conditions.

Older recipes are rejected, not silently upgraded. Preserve the original recipe and explicitly create a new one to reuse an old seed. The initial fingerprint includes the requested parameters, fitted threshold, and all crust arrays. Exact reproduction is scoped to the same supported version/environment; cross-platform bitwise equivalence remains unverified.

## Coherent spherical potential

The native `crust.structure` random stream samples 24 sine modes in three bands. Their base frequencies are 3, 6, and 12, with total weights 0.70, 0.22, and 0.08. Each band contains eight modes with independent uniform spherical directions and phases. A mode's frequency is multiplied by a uniform factor in `[0.8, 1.2)` and divided by `continentalScale`; its weight is the band's weight divided by eight.

```text
potential(p) = sum(weight_j * sin(dot(wave_j, p) + phase_j))
```

Here `p` is a unit sphere direction, not latitude/longitude or physical meters. Values are bounded by `[-1, 1]`. There are no projection-seam or pole special cases. The same direction receives the same potential when mesh resolution changes. Increasing radius preserves the angular pattern, so physical patch sizes grow with radius.

This spectrum is a versioned initial-condition design, not a mantle solver or an observed distribution of continents. Low-resolution meshes can undersample smaller features. No resolution-convergence claim is made for patch counts or geological predictions.

## Area fitting and transition

Sort regions by decreasing potential, with region ID breaking sort ties. Accumulate physical spherical areas, considering only cuts between distinct potential values plus the empty/full sets. Select the cut with the smallest absolute area-fraction error; exact error ties retain the smaller selected area. Equal-value groups are never split into artificial speckles.

An interior threshold is the midpoint of the values on either side of the cut. Empty/full selections use a threshold above/below the sampled extrema by the transition width. The continuous field is:

```text
x = clamp((potential - threshold) / 0.12 + 0.5, 0, 1)
continentality = x² * (3 - 2x)
```

The width 0.12 is in potential units, **not a fixed distance in kilometers**. The transition occupies different physical widths depending on the local gradient. Choosing a larger requested fraction lowers the threshold and cannot remove an already selected continental region. Potential and plate data stay unchanged.

For distinct sampled values, the closest selectable area differs from the target by at most one region's area (a conservative test bound). Equal-value plateaus can produce a larger error; the UI reports actual coverage rather than claiming exact fitting. Changing resolution can change the fitted threshold and therefore continentality even at shared directions. Raw potential remains identical there.

## Initial material approximations

Native fields interpolate between versioned illustrative endmembers:

```text
thickness_meters = 7000 + 28000 * continentality
density_kg_per_cubic_meter = 3000 - 200 * continentality
```

These are coarse initial properties, not measured local geology, an isostatic equilibrium solution, or a mass-conserving mixing process. Radius scales surface geometry, not these fixed Earth-scale material endmembers; extreme radii are experimental, not validated planetary interiors. Regenerating with another crust fraction is a new initial condition, not creation of crust during simulation. Geological age, thermal subsidence, sediments, deformation, and separate upper/lower crust are deferred. Future relief must identify its own assumptions rather than relabel this thickness as elevation.

The broad thickness contrast is consistent with [USGS: Global crustal structure](https://www.usgs.gov/publications/crust-and-lithospheric-structure-global-crustal-structure), which also describes substantial variation in real crust. [USGS: Continental crust, a global view](https://www.usgs.gov/publications/seismic-velocity-structure-and-composition-continental-crust-a-global-view) reports an estimated mean density of 2,830 kg/m³. These sources motivate the distinction; they do not calibrate our spectrum, threshold, interpolation, or specific endmembers.

## Ownership, wire format, and viewing

Rust owns generation and stored properties. After protocol 2's plate arrays, protocol 3 appends Float64 little-endian data in this order:

| Field | Count | Unit |
| --- | ---: | --- |
| Fitted threshold | 1 | Dimensionless potential |
| Potential | N | Dimensionless |
| Continentality | N | Dimensionless |
| Thickness | N | m |
| Density | N | kg/m³ |

The body grows by `8 + 32N` bytes. Every response uses protocol tag 3. The adapter checks lengths, finite values, and supported ranges. Diagnostic frames are unchanged field-only messages; diagnostic playback does not update crust.

Flat and globe layers read identical native fields. Both remain geometrically smooth because elevation is not implemented. Continentality and thickness have identical normalized patterns in this increment; thickness is a unit-bearing derived property, not additional independent detail. The inspector exposes density, potential, threshold, and the interpolation assumptions. Rendering does not invent crust or consume random streams.

Read-only diagnostics report continental-dominant area, area-weighted mean continentality, connected patch count, and largest patch area using the spherical neighbor graph, including seam-crossing connections. These are crust patches, not landmasses. Mean continentality and dominant-area fraction are different quantities.

## Validation

Native tests cover weighted fitting on constructed unequal-area regions, equal-value plateaus, empty/full extremes, monotone parameter response, finite bounds, exact regeneration, material formulas, seam/pole continuity, raw-field refinement, and independence from plates, radius doubling, and diagnostic time. The crust ensemble covers 20 seeds × three resolutions (0, 2, 4) × three scales (0.5, 1, 2) × five fractions (0, 0.1, 0.38, 0.7, 1): 900 combinations, each generated twice. This verifies structural behavior, not scientific realism.

Bridge tests verify typed arrays, malformed crust rejection, numerical units, area and connectivity summaries, and preservation of tectonics when crust parameters change. Constructed summary tests distinguish disconnected patches from a joining graph path and check the strict 0.5 threshold. The finest resolution with 32 plates fits the existing 32 MiB transport limit. Desktop checks cover both views, shared inspector values, new layer legends, new parameter controls, zero continental fraction, and recipe round trips.

### Recorded results: September 23, 2026

- All 12 Rust tests and 26 TypeScript tests passed, including the existing surface and plate suites.
- TypeScript checking, Rust formatting, Clippy with warnings denied, and whitespace checks passed.
- Development and packaged Linux x64 desktop checks passed. Flat/globe screenshots were reviewed.
- Packaging succeeded. The existing Vite advisory for a renderer chunk above 500 kB remains; it is not a failed check.
- Default `first-light`, level 5: fingerprint `2a7ea88c`, 4,994,696 bytes of model arrays (4.76 MiB), continental-dominant area fraction 0.3800175971, mean continentality 0.3811437023, one connected continental patch, fitted threshold 0.03934107377. These are regression observations, not quality targets.

On the Ryzen 5 PRO 4650U laptop (14.84 GiB system RAM), a desktop timing sample with the continentality layer and hardware-enabled WebGL/compositing measured:

| Regions | Generation plus both view preparations | Flat RAF p95 | Globe RAF p95 |
| ---: | ---: | ---: | ---: |
| 10,242 | 1,046 ms | 16.7 ms | 16.7 ms |
| 40,962 | 2,450 ms | 16.7 ms | 16.7 ms |

Each sample spans 180 requested-animation-frame intervals during alternating zoom and native diagnostic diffusion. This is not a GPU timer or a guarantee of future simulation throughput. Crust and plates stay static; only the diagnostic field evolves. Timing varies with workload and machine conditions. The local raw report is `artifacts/native-desktop-benchmark.json`, replaced on each benchmark run. A separate default native-plus-adapter headless sample took about 158 ms, excluding view preparation and rendering.

## Next increment

Add inspectable elevation contributions: a crust-conditioned baseline, effects of relative plate motion, and bounded local detail. Then fit initial water with explicit volume accounting. Crust age and subduction polarity need their own models before being presented as computed facts. Preserve B1's continuous opening/shear components instead of reducing geology to boundary colors.
