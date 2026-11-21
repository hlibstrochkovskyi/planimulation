# Natural planet display: `display-1` and `display-2`

Date: 2026-11-21. Scope: renderer and display presentation only (`src/renderer/`). Native simulation models, binary wire protocols, recipes, physical fields, fingerprints, picking, and checkpoints are unchanged.

## Model data vs. decorative display detail

Per [DESIGN.md](../DESIGN.md), rendering resolution may exceed model resolution, but decorative detail cannot create resources, traversable paths, or new simulation regions, and rendering changes must not alter simulation outcomes.

| Aspect | Authoritative model data | Decorative display detail (`display-1` + `display-2`) |
| --- | --- | --- |
| Region identity & picking | Model regions (`surface.centers`, `boundaryDirections`), exact region inspector values, native `water`/`bed` surface picking | None — picking raycasts against the model-region bed and water meshes; the inspector reports whole-region model values |
| Surface color | `climate-normals-1` temperatures/precipitation/biomes, `vegetation-2` yearly woody & herb cover, `terrain` elevation, `water` depths & body IDs, `seasonal-temperature-1` monthly normals | Calm Blue-Marble-style continuous blending, 28% dry-neighbor transition softening, subtle per-region and close-up micro-relief rock/snow/littoral/riparian accents; selectable **Relief tint** restores the elevation-only hypsometric ramp |
| Coastline | Native wet/dry mask (`water.bodyIds` / active water frame) and regional depths | Anti-aliased zero contour of a symmetric, two-pass neighbor-relaxed signed coastline field (`±0.55` sign floors), perturbed by deterministic resolution-adaptive, octave-rotated, Nyquist band-limited 3D noise strictly inside wet/dry transition zones |
| 3D relief & shading | Native regional bed elevation (`terrain.elevation`), local least-squares slopes (`shading.ts`), `drainage.contributingArea`, and `rivers-1` discharge | Bounded camera-local globe LOD subdivision (2×2 to 4×4 per dual fan, capped at 120,000 triangles), resolution-normalized relief scaling, drainage-carved valleys (rivers sit in lowest detail), octave-rotated band-limited ridged+fBm detail, tilt-stable oblique survey lighting with sky-fill shadows and grazing horizon haze, adaptive globe exaggeration (3.5×–6.5× by zoom) |
| Rivers | `drainage.receivers`, `rivers-1` annual/monthly discharge, flow classes (`Perennial` / `Seasonal`), spill segments | Deterministic cell-bounded meanders, acute Y-confluence curves, smoothed coastline mouth bisection, valley-carved globe elevation draping, zoom-tiered hiding of short 1–2 hop coastal stubs at planet scale |

All procedural detail is deterministic and seeded from the world seed string via sine-free 3D hashing (`seedOffsetFromString`, `hash3`). No model random stream or `Math.random()` is used. The viewport caption (`Equirectangular · Natural color · display detail` / `Globe · Natural color · display detail`) and Display menu explicitly label decorative detail, and Display settings allow toggling **Surface style** (`Natural color` / `Relief tint`), **Decorative terrain & coast detail**, **Seasonal snow tint**, **Interpolate between regions**, **Relief shading**, and **River lines**.

## Color rules (`src/renderer/natural-color.ts`)

Natural surface colors blend continuously from model quantities using desaturated, satellite-inspired sRGB anchors:

1. **Bare substrate & deserts**: hot desert sand/ochre (`annualMean ≥ 18 °C`) and cold desert taupe (`annualMean < 18 °C`), transitioning to muted olive-brown tundra where `warmestMonth < 10 °C`.
2. **Herbaceous cover**: `vegetation-2` `herb` fraction blends warm savanna olive-ochre or temperate sage/grassland greens over the substrate, modulated by aridity ratio (`precipitation / (20 · mean + 140)`).
3. **Woody cover / forests**: `vegetation-2` `woody` fraction blends deep, desaturated satellite forest greens — boreal spruce-green, temperate woodland green, tropical seasonal olive-green, and tropical rainforest emerald — calibrated with enough luminance headroom that close-up forested slopes retain clear relief shading. Before vegetation frames arrive, continuous woody and herb fractions fall back to the same Köppen aridity ramps from `climate-normals-1` (and to a calm latitude/elevation natural palette before climate normals finish).
4. **Bare rock**: steep local slopes and high elevations (`> 1,750 m` above sea level) expose warm or alpine slate/granite rock; in the fragment shader, steep micro-slopes in rugged regions reveal additional alpine scree.
5. **Snow and permanent ice caps**: regions with `warmestMonth < ~0 °C` smoothly transition to off-white polar ice sheets, plus high-altitude alpine snow accents where summers stay near freezing and precipitation is present. An optional **Seasonal snow tint** uses the active month's `seasonal-temperature-1` normal to dust sub-freezing regions (attenuated under closed forest canopy).
6. **Water and lakes**: ocean color follows a smooth logarithmic bathymetry ramp calibrated up to `9,000 m` from the interpolated model depth smoothly tapered to zero along the perturbed coastline zero-contour, so `multiscale-geography-5` shelves, continental slopes, mid-ocean ridges, abyssal basins, and subduction trenches all read cleanly without a polygonal coastal halo. Inland water bodies (`bodyId ≠ mainOceanId`) use a distinct freshwater sapphire-teal ramp.

## Lighting, coastline, drainage valleys, and LOD (`src/renderer/refinement.ts`, `src/renderer/view-geometry.ts`)

- **Symmetric sawtooth-free coastlines**: `computeCoastField` runs two symmetric neighbor-relaxation passes over the signed wet/dry mask (`-1` wet, `+1` dry) and enforces symmetric `±0.55` sign floors. Because deep ocean depth and shallow shelves no longer bias the zero-contour gradient, narrow 1-region peninsulas stay smoothly connected without hex-grid sawtooth, and single-region islands on shallow `multiscale-geography-5` shelves place their zero-contour near mid-cell rather than pinning to straight outer polygon walls. Coastal noise frequencies scale with `meshFrequencyForRegions(regionCount)` (`sqrt(N) / 3.81`), rotate between octaves (`OCT_ROT`), and fade out before Nyquist (`pixelArc · freq`) so shorelines stay crisp at every resolution and zoom.
- **Coastline center-preservation invariant**: `strictMask` equals `-1` at wet centers and `+1` at dry centers, averaged at shared dual corners. Coastal noise is multiplied by `smoothstep(0.01, 0.28, 1 - |strictMask|) · (1 - smoothstep(0.68, 0.93, wCenter))`, which is C⁰-continuous across all dual edges, strictly `0` at every model region center (`wCenter = 1`), and strictly `0` inside every interior land or ocean region (`|strictMask| = 1`). Decorative detail therefore cannot flip any region center's wet/dry class or invent inland lakes or mid-ocean islands.
- **Resolution-normalized relief and drainage-carved valleys**: `computeReliefMetrics` normalizes physical neighbor slope to a `105 km` reference span (`effectiveReliefMeters`) so subdivision 7 (`56 km` cell spacing) keeps full foothill and alpine ruggedness while near-flat plains stay calm. `computeValleyField` computes a per-region `[0, 1]` drainage-valley factor from model contributing area and river discharge (carried in `tintField.a` outside travel-time mode); both 3D LOD heights (`sampleDetailHeightMeters`) and fragment-shader analytical micro-normals (`sampleDetailGrad`) sharpen ridges on interfluve divides (`valley = 0`), damp positive bumps, and carve concave valley floors along drainage channels (`valley → 1`) so rivers lie in the lowest local detail.
- **Octave-rotated, band-limited micro-relief**: `sampleDetailGrad` applies an orthonormal 3D rotation (`OCT_ROT`, with inverse-transpose gradient transform) between octaves to eliminate cubic lattice axis creases, decays slope energy cleanly per octave (`0.43 · 2.04 ≈ 0.88`), and band-limits every octave against `pixelArc · freq` so orbital views stay silky without sparkle while ×6–×8 close-ups resolve 1–5 km spurs and gullies.
- **Tilt-stable close-up lighting**: on the globe, survey light is constructed in the local surface tangent frame rotated by camera heading rather than tilted with camera pitch, lowering smoothly from `48°` elevation at orbit to an oblique `32°` NW sun at close range and synchronizing with `SpaceBackdrop` atmospheric scattering. Combined gradients use gentle compression (`g / (1 + 0.34 |g|)`), warm sunlit faces, cool sky-fill shadows, crevice ambient occlusion, and subtle grazing-angle aerial perspective haze on tilted close-ups.
- **Subdivision-7 compact geometry & LOD**: `buildRenderViewGeometry` uses allocation-free numeric corner hashing, collapses degree-2 edge midpoints so each hexagon/pentagon emits 6/5 dual-corner fans sharing 7/6 indexed vertices (`2×` fewer triangles, `2.5×` fewer corners, `5.1×` fewer vertices), skips Earcut triangulation for non-seam flat regions, skips zero-weight vertex texture fetches, clamps submerged base bed vertices to `waterLevelRadius` on surface layers for watertight base coastlines, lazily defers hidden boundary-line exaggeration updates, and partitions globe bed indices so `surface` layers skip underwater interior-ocean bed triangles via `setDrawRange`. `buildLodCache` / `buildLodPatch` shares the compact 6-fan topology and shifts a tight frustum-matched selection cone forward under tilt so ×6–×8 close-ups stay at `k = 4` within the `120,000`-triangle cap at both subdivision 6 and subdivision 7.
- **Rivers (`src/renderer/rivers.ts`)**: routed segments use 7-step Catmull-Rom splines with deterministic 2-harmonic lateral meanders bounded within `0.185×` chord length (plus ≤ `0.135×` interior node offset on non-mouth nodes), attenuated on steep slopes. River mouths bisect the exact resolution-adaptive `sampleCoastPhi = 0` contour in 8 steps, globe river vertices drape onto `baseHeight + sampleDetailHeightMeters(..., valleyFactor, meshFreq)`, and short 1–2 hop coastal stubs in the smallest discharge classes move into a detail tier that fades in when zoomed in (`flatCamera.zoom > 1.7` or `globe distance < 1.42`).

## Performance measurements

Measured with `node --import tsx scripts/look-review.ts` at 1600×1000 on `first-light` on AMD Ryzen 5 PRO 4650U (Radeon Vega Renoir integrated GPU, 12 threads, 14 GiB RAM):

### `multiscale-geography-4`, subdivision 6 (40,962 regions)

| Metric | Before (`9963809`, `display-1`) | After (`feature/display-2`) |
| --- | ---: | ---: |
| Generation & view build (`#generation-time`) | 8,537 ms | 1,709 ms |
| World-ready time (startup to ready) | 15,350 ms | 5,903 ms |
| Flat map base triangles | 491,544 | 245,902 (indexed) |
| Globe base triangles (bed + water) | 840,536 | 420,268 (indexed; ~259k drawn in surface) |
| Globe LOD triangles (×6 tilt / ×8 tilt) | 35,640 (`k=4`) / 15,948 (`k=4`) | 9,210 (`k=4`) / 5,022 (`k=4`) |
| Flat map RAF median / p95 | 33.3 ms / 33.4 ms (30.0 fps) | 16.7 ms / 16.7 ms (59.9 fps) |
| Globe overview RAF median / p95 | 23.4 ms / 46.8 ms (42.7 fps) | 16.7 ms / 16.7 ms (59.9 fps) |
| Globe ×8 + tilt RAF median / p95 | 39.1 ms / 67.9 ms (25.6 fps) | 16.7 ms / 16.7 ms (59.9 fps) |
| GPU / Renderer tab working set | 190.3 MB / 693.3 MB | 181.0 MB / 495.2 MB |

### `multiscale-geography-5`, subdivision 7 (163,842 regions)

| Metric | Before (`9963809`, `display-1`) | After (`feature/display-2`) |
| --- | ---: | ---: |
| Generation & view build (`#generation-time`) | 28,983 ms | 6,304 ms |
| World-ready time (dialog click to ready) | 50,709 ms | 22,500 ms |
| Flat map base triangles | 1,966,104 | 983,310 (indexed) |
| Globe base triangles (bed + water) | 3,362,096 | 1,681,048 (indexed; ~1.04M drawn in surface) |
| Globe LOD triangles (×6 tilt / ×8 tilt) | 49,900 (`k=2`) / 82,452 (`k=4`) | 57,246 (`k=4`) / 18,090 (`k=4`) |
| Flat map RAF median / p95 | 49.9 ms / 66.6 ms (20.0 fps) | 16.7 ms / 16.8 ms (59.9 fps) |
| Globe overview RAF median / p95 | 50.0 ms / 99.9 ms (20.0 fps) | 16.7 ms / 33.3 ms (59.9 fps) |
| Globe ×8 + tilt RAF median / p95 | 66.6 ms / 83.3 ms (15.0 fps) | 16.7 ms / 33.3 ms (59.9 fps) |
| GPU / Renderer tab working set | 135.7 MB / 1,765.1 MB | 183.0 MB / 1,184.0 MB |

## Limitations

- Natural colors represent annual/climatic cover states rather than dynamic weather, cloud cover, sea ice, or soil albedo.
- River meanders, alpine ridges, drainage-carved sub-region valleys, and fractal coastal inlets are purely decorative display geometry; they do not represent sub-region hydrology, channel hydraulics, or navigable harbors.
- Single-cell islands and steep coasts still respect the hard model center-preservation rule (decorative detail cannot erase a land center or move a shoreline across a neighbor's center); switching off **Interpolate between regions** or **Decorative terrain & coast detail** exposes the raw model polygons directly.
- On dynamic water layers where the active water mask differs from initial `world.water`, the globe bed falls back to drawing its full index range so changing shorelines remain visible.
- Seasonal snow tint uses the single selected climate-normal month (`#temperature-month`) rather than an animated seasonal snowpack.
