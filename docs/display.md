# Natural planet display: `display-1`

Date: 2026-11-21. Scope: renderer and display presentation only (`src/renderer/`). Native simulation models, binary wire protocols, recipes, physical fields, fingerprints, picking, and checkpoints are unchanged.

## Model data vs. decorative display detail

Per [DESIGN.md](../DESIGN.md), rendering resolution may exceed model resolution, but decorative detail cannot create resources, traversable paths, or new simulation regions, and rendering changes must not alter simulation outcomes.

| Aspect | Authoritative model data | Decorative display detail (`display-1`) |
| --- | --- | --- |
| Region identity & picking | Model regions (`surface.centers`, `boundaryDirections`), exact region inspector values, native `water`/`bed` surface picking | None — picking raycasts against the model-region bed and water meshes; the inspector reports whole-region model values |
| Surface color | `climate-normals-1` temperatures/precipitation/biomes, `vegetation-2` yearly woody & herb cover, `terrain` elevation, `water` depths & body IDs, `seasonal-temperature-1` monthly normals | Calm Blue-Marble-style continuous blending, 28% dry-neighbor transition softening, subtle per-region and micro-relief tonal/rock/snow accents; selectable **Relief tint** restores the elevation-only hypsometric ramp |
| Coastline | Native wet/dry mask (`water.bodyIds` / active water frame) and regional depths | Anti-aliased zero contour of interpolated compressed height relative to local water level (`phi = sign(h) · asinh(|h| / 160)`), perturbed by deterministic domain-warped 3D noise strictly inside wet/dry transition zones |
| 3D relief & shading | Native regional bed elevation (`terrain.elevation`) and local least-squares slopes (`shading.ts`) | Bounded camera-local globe LOD subdivision (2×2 to 4×4 per fan, capped at 110,000 triangles), relief-scaled ridged+fBm fractal height and analytical micro-normals, soft ambient-filled hillshade, adaptive globe exaggeration (3.5×–6.5× by zoom) |
| Rivers | `drainage.receivers`, `rivers-1` annual/monthly discharge, flow classes (`Perennial` / `Seasonal`), spill segments | Deterministic cell-bounded meanders, acute Y-confluence curves, smoothed coastline mouth bisection, refined globe elevation draping, zoom-tiered hiding of short 1–2 hop coastal stubs at planet scale |

All procedural detail is deterministic and seeded from the world seed string via sine-free 3D hashing (`seedOffsetFromString`, `hash3`). No model random stream or `Math.random()` is used. The viewport caption (`Equirectangular · Natural color · display detail` / `Globe · Natural color · display detail`) and Display menu explicitly label decorative detail, and Display settings allow toggling **Surface style** (`Natural color` / `Relief tint`), **Decorative terrain & coast detail**, **Seasonal snow tint**, **Interpolate between regions**, **Relief shading**, and **River lines**.

## Color rules (`src/renderer/natural-color.ts`)

Natural surface colors blend continuously from model quantities using desaturated, satellite-inspired sRGB anchors:

1. **Bare substrate & deserts**: hot desert sand/ochre (`annualMean ≥ 18 °C`) and cold desert taupe (`annualMean < 18 °C`), transitioning to muted olive-brown tundra where `warmestMonth < 10 °C`.
2. **Herbaceous cover**: `vegetation-2` `herb` fraction blends warm savanna olive-ochre or temperate sage/grassland greens over the substrate, modulated by aridity ratio (`precipitation / (20 · mean + 140)`).
3. **Woody cover / forests**: `vegetation-2` `woody` fraction blends deep, dark satellite forest greens — boreal spruce-green, temperate woodland green, tropical seasonal olive-green, and dark tropical rainforest emerald. Before vegetation frames arrive, continuous woody and herb fractions fall back to the same Köppen aridity ramps from `climate-normals-1` (and to a calm latitude/elevation natural palette before climate normals finish).
4. **Bare rock**: steep local slopes and high elevations (`> 1,750 m` above sea level) expose warm or alpine slate/granite rock; in the fragment shader, steep micro-slopes in rugged regions reveal additional alpine scree.
5. **Snow and permanent ice caps**: regions with `warmestMonth < ~0 °C` smoothly transition to off-white polar ice sheets, plus high-altitude alpine snow accents where summers stay near freezing and precipitation is present. An optional **Seasonal snow tint** uses the active month's `seasonal-temperature-1` normal to dust sub-freezing regions (attenuated under closed forest canopy).
6. **Water and lakes**: ocean color follows a smooth logarithmic bathymetry ramp computed from the fractal coastline depth contour (`160 · sinh(-phi)`), transitioning from a muted coastal shelf blue to deep marine navy without a bright coastal halo. Inland water bodies (`bodyId ≠ mainOceanId`) use a distinct freshwater sapphire-teal ramp.

## Lighting, LOD, and coastline invariants (`src/renderer/refinement.ts`)

- **Soft physically-coherent lighting**: combined macro + analytical fractal gradients are softly compressed (`g / (1 + 0.48 |g|)`) so steep 3–5 km ridges sculpt cleanly without turning into black walls. Shaded mountain faces receive cool atmospheric sky-fill ambient light instead of dark clipping. On the globe, terrain relief normals transform into world space and share the exact active sun direction used by `SpaceBackdrop` atmospheric scattering (camera-relative survey light nearby, smoothly blending to the fixed world sun in distant day/night view).
- **Coastline center-preservation invariant**: `strictMask` equals `-1` at wet centers and `+1` at dry centers, averaged at shared dual corners. Coastal noise is multiplied by `smoothstep(0.02, 0.32, 1 - |strictMask|) · (1 - smoothstep(0.72, 0.94, wCenter))`, which is C⁰-continuous across all dual edges, strictly `0` at every model region center (`wCenter = 1`), and strictly `0` inside every interior land or ocean region (`|strictMask| = 1`). Decorative detail therefore cannot flip any region center's wet/dry class or invent inland lakes or mid-ocean islands.
- **Bounded camera-local globe LOD**: at overview (`distance ≥ 1.65`), the globe renders the base mesh with single-pass analytical per-pixel micro-normals (`0` LOD triangles). When zooming in (`distance < 1.65`), `geometry.worker.ts` asynchronously builds a subdivided patch around the camera focus (dry land and coastal ocean regions at `k = 2, 3, or 4`, i.e., 4–16 sub-triangles per fan), hard-capped at `MAX_LOD_TRIANGLES = 110,000` so subdivision 6 (40,962 regions) and subdivision 7 (163,842 regions) stay within budget. Outer patch edges bordering unrefined regions remain unsplit with locked base corner heights for watertight stitching; replaced coarse regions are culled in the vertex shader (`lodMaskField`) to avoid overdraw.
- **Rivers (`src/renderer/rivers.ts`)**: routed segments use 7-step Catmull-Rom splines with deterministic 2-harmonic lateral meanders bounded within `0.185×` chord length (plus ≤ `0.135×` interior node offset on non-mouth nodes), attenuated on steep slopes so alluvial lowland rivers meander while mountain canyons stay tight. River mouths bisect the exact `sampleCoastPhi = 0` contour in 8 steps, globe river vertices drape onto `baseHeight + sampleDetailHeightMeters`, and short 1–2 hop coastal stubs in the smallest discharge classes move into a detail tier that fades in when zoomed in (`flatCamera.zoom > 1.7` or `globe distance < 1.42`).

## Performance measurements

Measured with `node --import tsx scripts/look-review.ts` at 1600×1000 on `first-light` (`multiscale-geography-4`, subdivision 6 = 40,962 regions) on AMD Ryzen 5 PRO 4650U (Radeon Vega Renoir integrated GPU, 12 threads, 14 GiB RAM):

| Metric | Before (`4f2cd47`) | After (`feature/display-1`) |
| --- | ---: | ---: |
| Flat map base triangles | 491,544 | 491,544 |
| Globe base triangles (bed + water) | 840,536 | 840,536 (front-face culled) |
| Globe LOD triangles (overview / zoomed ×6) | 0 / 0 | 0 / 86,530 (`k = 4`, cap 110,000) |
| River line segments (total) | 10,606 | 18,545 (primary + zoom-gated detail) |
| Flat map RAF median / p95 | 16.7 ms / 33.3 ms (59.9 fps) | 16.7 ms / 33.4 ms (59.9 fps) |
| Globe overview RAF median / p95 | 16.7 ms / 16.7 ms (59.9 fps) | 16.7 ms / 35.4 ms (59.9 fps) |
| Globe ×6 + tilt RAF median / p95 | 33.3 ms / 50.0 ms (30.0 fps) | 33.3 ms / 50.0 ms (30.0 fps) |
| GPU process working set | 174.7 MB | 176.2 MB |
| Renderer tab working set | 536.5 MB | 852.7 MB |

Note: Renderer tab working set includes the persistent `geometry.worker.ts` LOD cache (~100–250 MB during subdivision-6 geometry construction before V8 GC compaction). On subdivision 7 (163,842 regions), the LOD triangle cap (`110,000`) bounds zoomed-in refined geometry to the same GPU budget; flat map and globe base meshes remain 4× larger until coarse spatial chunking is added if needed.

## Limitations

- Natural colors represent annual/climatic cover states rather than dynamic weather, cloud cover, sea ice, or soil albedo.
- River meanders, alpine ridges, and fractal coastal inlets are purely decorative display geometry; they do not represent sub-region hydrology, channel hydraulics, or navigable harbors.
- Extremely steep coasts on coarse meshes still approximate sub-region cliffs; switching off **Interpolate between regions** or **Decorative terrain & coast detail** exposes the raw model polygons directly.
- Seasonal snow tint uses the single selected climate-normal month (`#temperature-month`) rather than an animated seasonal snowpack.
