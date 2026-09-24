# Explainable elevation: milestone B3

Historical B3 record. [B4 initial water](water.md) extended this unchanged elevation algorithm; the current build uses [C2b basin inspection](basin-inspection.md), model `basins-1`, and protocol 7. Measurements below refer to the B3 build.

Implemented model: `terrain-1`, binary protocol 4. B3 adds static elevation and a displaced globe to B1 plates and B2 crust. Water, erosion, geological age, subduction polarity, and geological time integration remain unimplemented.

## Datum and ownership

All native elevation fields are meters relative to the recipe's reference sphere. **Zero is not sea level.** Negative elevation does not establish an ocean, and positive elevation does not establish emerged land. Rust owns the physical fields. The flat map remains planar; the globe interpolates those fields for display.

Areas and neighbor distances remain those of the reference sphere, not slope-corrected topographic areas or distances. This is an explicit approximation for later transport design. Changing display exaggeration cannot change either the physical heights or these reference metrics.

The generated field is the sum of four inspectable contributions:

```text
elevation = crust_baseline + convergence + divergence + detail
```

No post-sum clamping or hidden correction changes this identity. These are initial-condition formulas, not accumulated mass-conserving deformation or erosion.

## Parameters

The recipe adds three required parameters; all previous fields remain required:

| Field | Default | Range | Meaning |
| --- | ---: | ---: | --- |
| `reliefScale` | 1 | 0–2 | Multiplier of both boundary contributions; not display exaggeration |
| `boundaryWidthKm` | 300 | 50–1000 | Physical e-folding distance for boundary effects |
| `detailAmplitudeMeters` | 300 | 0–1000 | Absolute upper bound on detail, not its measured peak |

`terrain-1` rejects earlier recipes, including `crust-1`, without silently upgrading files. Reuse an old seed by explicitly creating a new recipe and retaining the original. B1/B2 algorithms and random streams are unchanged. Adding this downstream subsystem does not reroll the plates or crust.

## Crust baseline

Let `T` be initial crust thickness in meters and `rho` its approximate density in kg/m³:

```text
baseline = -4500 + T * (1 - rho / 3300) - 7000 * (1 - 3000 / 3300)
```

This illustrative unloaded-column buoyancy contrast anchors the oceanic endmember at −4,500 m and the continental endmember near +166.67 m. The reference density 3,300 kg/m³ and offset are fixed model choices, not a solved mantle/water equilibrium or calibration to observed bathymetry. No thermal age or water-loading correction is included. Fixed Earth-scale endmembers and height coefficients are not validated planetary interiors at extreme recipe radii.

## Boundary response

Use each actual B1 half-boundary's continuous opening component `o` in m/year, not its dominant-motion category. Let `c` be mean continentality of its two incident regions:

```text
closing   = max(0, -o)
spreading = max(0,  o)

uplift_amplitude = 6000 * (0.25 + 0.75*c) * closing   / (0.04 + closing)
ridge_amplitude  = 2500 * (1 - c)        * spreading / (0.04 + spreading)
rift_amplitude   = 1500 * c              * spreading / (0.04 + spreading)
```

Amplitudes are meters. Saturation prevents unbounded heights at high speeds; the 0.04 m/year response scale is a versioned heuristic. These formulas do not convert a prescribed plate speed into elapsed geological time. Pure shear has no vertical term; oblique motion still contributes through its opening component.

Convergence supplies a simplified positive relief envelope even in oceanic crust. It does **not** select a subducting plate, produce an explicit trench/island arc, or model asymmetric overriding-plate uplift. Divergence blends ridge-like positive relief with rift-like negative relief. These qualitative distinctions are motivated by [USGS: Understanding plate motions](https://pubs.usgs.gov/gip/dynamic/understanding.html); our coefficients and propagation algorithm are independent approximations, not a calibrated geodynamic model.

### Physical-distance propagation

1. At each segment's normalized midpoint, compute the great-circle distance to each incident region center, using the recipe radius.
2. Seed that region with `amplitude * exp(-distance / width)` for each of the three positive amplitude channels; retain the largest seed if several segments reach it.
3. Propagate each channel over the spherical neighbor graph with a maximum-priority queue. Across an edge of reference length `d`, candidate strength is `current * exp(-d / width)`; retain only a larger candidate. Equal priorities have a stable region-ID ordering.
4. Store `convergence = reliefScale * strongest_uplift` and `divergence = reliefScale * (strongest_ridge - strongest_rift)`.

Equivalently, each channel is the maximum of source strengths attenuated along graph shortest paths. It is not a sum over mesh segments. Duplicating a source cannot amplify a mountain. Ridge and rift envelopes may have different winning sources; their difference is intentional, not cancellation of one local boundary's classification.

Graph paths and initial center offsets approximate distance to a sampled boundary. There are no map-seam or longitude special cases. Source midpoint sampling, mesh normals, and graph anisotropy still make output resolution-dependent. A narrow width on a coarse mesh can strongly attenuate a source before it reaches a center. No resolution-convergence claim is made. Effects can cross plate interiors and other boundaries; there is no flexural/elastic solver.

Increasing width cannot decrease an individual positive envelope, but the signed ridge-minus-rift result need not be monotone. Doubling radius and width together preserves all attenuation. Increasing only radius changes physical attenuation even though plate ownership and crust's angular pattern stay fixed.

## Bounded detail

The independent `terrain.detail` stream generates 24 spherical sine modes: eight each at angular frequencies 18, 36, and 72 with band weights 0.6, 0.3, and 0.1. Directions and phases are sampled independently; each mode has one eighth of its band's weight. The sum is multiplied by `detailAmplitudeMeters`.

This continuous unit-sphere field has no seam/pole branches and remains within the configured absolute bound. Physical wavelengths scale with radius. High frequencies are under-resolved on coarse meshes; this is bounded initial roughness, not erosion, ridged mountain geology, or visual-only noise. It enters the physical elevation and fingerprint. Setting its amplitude to zero removes it without changing crust, plates, or boundary effects.

## Native data and protocol

After protocol 3's crust data, protocol 4 appends five Float64 arrays of length `N`, in order: baseline, convergence, divergence, detail, elevation. Additional body size is `40N` bytes. All responses use protocol tag 4. All arrays enter the initial fingerprint. The adapter checks finite values, conservative contribution bounds, and reconstruction of elevation within 1e-8 m.

For supported parameters, a conservative overall elevation bound is −8,500 to +18,167 m. This is a safety bound, not a geological quality target. Diagnostic frames still transmit only the diagnostic scalar field; diagnostic steps do not evolve terrain. Actual min/max and physical-area-weighted mean are available headlessly and in the UI.

## Globe interpolation, picking, and exaggeration

The display worker assigns native heights to region centers. A shared dual-edge midpoint gets the mean height of its two incident regions; a shared face centroid gets the mean of its three incident regions. All repeated instances of a corner use one shared value. Triangles keep their region IDs for selection. Region and tectonic overlays use the same corner heights with a small radial display bias to avoid depth fighting.

Globe vertex radius, in normalized reference-sphere units, is:

```text
display_radius = 1 + applied_exaggeration * interpolated_height / planet_radius
```

The default requested exaggeration is 10×; options include 0× (reference sphere) and 1× (true radial scale). The applied factor is limited so no radial excursion exceeds 20% of planet radius. Requested and applied factors are explicitly distinguished when the safety limit applies. This cap is a display rule, not a physical-height clamp. It is especially relevant for experimental small planets.

Changing exaggeration rebuilds positions from immutable base directions, recomputes face normals and raycasting bounds, and leaves all native fields and the fingerprint unchanged. It never accumulates repeated displacement. Raycasting uses the actual displaced CPU geometry, not an undisplaced proxy sphere. Non-indexed facets expose the finite computational resolution; extra decorative topography is not generated. Flat geometry stays planar and unchanged at every exaggeration.

Elevation colors use the current world's min/max; compare numeric legends, not colors alone, across seeds. The convergence layer has a fixed 0–12,000 m scale. Interpolation and normals affect display only. Native regional values remain authoritative in the inspector, which reports the four contributions and their total.

## Validation

Directed native checks cover baseline endmembers, convergence/ridge/rift/zero-opening cases, saturation response, exact exponential decay on a three-node graph, competing sources without additive amplification, widening, zero relief/detail, half amplitude, stopped plate motion, radius/width scaling, diagnostic immutability, and seam/pole continuity. The elevation ensemble covers 20 seeds × levels 0, 2, 4, each generated twice. Tests retain the previous plate and crust ensembles.

Display tests check shared-corner agreement, reversible deformation, finite normals, actual ray hits on displaced regions, unchanged flat geometry/model arrays, area-weighted summaries, and safe exaggeration at small radii. Desktop tests exercise shared explanations, layer/view changes, recipe round trips, and 0×/10×/25× display changes without altering selection data or fingerprint.

### Recorded results: September 23, 2026

- All 17 Rust tests and 30 TypeScript tests passed, including the earlier surface, plate, and crust suites.
- TypeScript checking, Rust formatting, Clippy with warnings denied, and whitespace checks passed.
- Development and packaged Linux x64 desktop checks passed. The packaged run also exercised a 100 km-radius world with a requested 50× exaggeration, verified the explicit safety-limit notice, and preserved the physical fingerprint.
- Flat and relief-globe screenshots were reviewed. Packaging succeeded; the existing Vite advisory for a renderer chunk above 500 kB remains.
- Default `first-light`, level 5: fingerprint `76f41765`, 5,404,376 model-array bytes (5.15 MiB), minimum −4,640.3735 m, maximum +2,855.4746 m, reference-area-weighted mean −2,554.0866 m. These are regression observations, not targets or water depths.

On the Ryzen 5 PRO 4650U laptop with 14.84 GiB system RAM and hardware-enabled WebGL/compositing, the elevation layer at 10× applied globe exaggeration measured:

| Regions | Generation plus both view preparations | Flat RAF p95 | Globe RAF p95 |
| ---: | ---: | ---: | ---: |
| 10,242 | 970 ms | 16.7 ms | 16.7 ms |
| 40,962 | 2,755 ms | 16.7 ms | 16.7 ms |

Each sample covers 180 requested-animation-frame intervals during alternating zoom and native diagnostic diffusion. These are not GPU completion times or promises for future climate/civilization workloads. Exaggeration remains fixed during the timing sample; changing it rebuilds display positions and normals synchronously. The local raw report is `artifacts/native-desktop-benchmark.json`, replaced by each benchmark run. A separate default headless native-plus-adapter observation took about 142 ms, excluding display preparation.

## Next increment

Initial water fitting should consume this physical elevation and preserve an explicit volume. Connected water components, erosion, catchments, and climate are separate later increments. Do not interpret the existing zero datum or oceanic-crust label as a water model.
