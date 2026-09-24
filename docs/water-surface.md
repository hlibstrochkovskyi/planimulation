# Water-surface display: milestone B5

Historical B5 record. [C2b basin inspection](basin-inspection.md) now adds native basin arrays with `basins-1` and protocol 7; the water-display algorithm below is unchanged.

This is a rendering increment on top of [B4 initial water](water.md), not a new water model. Recipes remain `water-1`, binary protocol 5, and the native fields and fingerprints are unchanged. The default layer is now **Land and water**. Depth, water-body, and geological layers remain analytical views of the bed.

## Geometry and ownership

The display worker copies each wet region's existing globe fan triangles into a separate water mesh. The angular vertices and region IDs are exactly those of the bed mesh. Dry regions contribute no water triangles. Water vertices all receive the resolved initial level from the native state:

```text
water_vertex_radius = 1 + applied_exaggeration * initial_level / reference_radius
```

No sphere-wide water shell is placed over dry regions. No decorative waves, displacement noise, new simulation regions, or modified native elevations are introduced. The flat atlas remains planar and uses the native wet mask for water coloring; its geometry is unchanged.

Arrays are preallocated from the wet triangle count and transferred from the display worker. The native protocol does not transmit any new display geometry. If `T` is the number of wet globe triangles, the additional worker result contains `92T` bytes: positions, region IDs, radial offsets, boundary lines, and line offsets. Renderer base-direction copies and GPU buffers add further memory; this is not the complete process footprint.

The renderer owns and releases the extra water geometry alongside the bed. The water shader shares field, selection, and view uniforms with the bed shader, but uses a distinct surface-color flag. There is no new native command or renderer filesystem capability.

## Shoreline approximation

The bed uses B3's shared-corner height interpolation; the water model uses one height and depth per whole region. These representations are deliberately not equated.

Water caps cover only native wet regions. Ordinary depth testing hides any cap portion behind a higher part of the interpolated bed. Within a wet region, the resulting visible shore can therefore follow the intersection of two display triangle surfaces. At a wet/dry regional boundary, the cap ends even if the dry region's interpolated corner happens to lie below the level. No water is drawn across a native dry region to smooth that boundary.

Consequences:

- A native wet region can contain a visibly exposed bed near a high interpolated corner.
- Visible coastlines and apparent connectivity can differ from the regional wet mask and connected-body labels, particularly on coarse meshes.
- Water triangle vertices have one radial level, but triangle interiors are planar chords, not exact spherical patches. Coarse meshes remain faceted.
- Pixel coverage and the displayed volume between triangles are not physical measurements. Native area, depth, volume, and body IDs in the inspector remain authoritative.

There is no explicit shoreline contour extraction, CPU polygon clipping, subregion hydrology, or vertical shore-wall mesh in this increment. The UI calls the shoreline a display approximation. More elaborate shore geometry must not silently alter the regional model.

## Layers and colors

**Land and water** shows the raised water mesh only on the globe. The flat view colors entire wet regions from the same native mask. Water color is a shallow-to-deep blue ramp using native region depth divided by the world's maximum depth. Land color is an elevation-above-initial-level ramp between muted green and stone; it does not indicate vegetation or biomes. Exposed bed inside a native wet region uses the low-land color. A tiny minimum color-ramp magnitude preserves a visible wet/dry distinction for extremely shallow water; it never changes depth or volume.

**Water depth**, **Water bodies**, and every other analytical layer hide the water mesh and expose the bed as before. Tectonic lines appear only in their own layers. The region-grid toggle adds lines on the water level in the surface view; bed lines are naturally occluded underwater. Water-grid endpoints use the existing small radial overlay bias, with no model effect.

## Exaggeration and picking

The same requested factor applies to bed and water. The 20%-of-radius display bound considers both the largest absolute bed height and, when water exists, the absolute water level. A high ocean on a small experimental planet must not escape the bound. The common factor remains the same when switching analytical layers, so layer changes do not change bed geometry. Dry worlds do not use an arbitrary empty-water level to constrain display relief.

Every change rebuilds positions from immutable directions and recomputes normals and raycasting bounds. At 0×, wet caps and bed triangles coincide. A small negative water polygon offset resolves depth-buffer fighting without raising physical water levels. Picking tests both visible meshes, chooses the nearest actual triangle, and prefers water for exactly coincident hits. Hidden analytical-layer water is excluded explicitly, because raycasting must not depend on drawing visibility alone.

Both meshes carry the same original region IDs. Clicking either exposed bed or water opens the same regional inspector, not a new surface entity. At a display shoreline, that inspector can correctly report a native wet region even when the clicked part is exposed interpolated bed.

## Validation

- Wet-only geometry at levels 0, 2, and 3; empty water mesh for dry worlds; exact preservation of flat geometry, bed geometry, and supplied model arrays.
- Fully flooded meshes share identical repeated corner positions. Positive and negative water levels remain uniform at vertices; displacement is reversible and respects the joint bound.
- A constructed mixed-height triangle verifies water selection below the display shore and bed selection above it; hidden water is ignored and zero-exaggeration ties prefer water.
- Rays through every center of a level-2 sphere verify water picking at poles, seam-adjacent regions, and both hemispheres.
- Desktop checks exercise water versus bed picking, unchanged selection data, analytical-layer switching, region-grid toggles, dry and fully flooded worlds, 0×/1×/10× water display, recipe round trips, cancellation, and native diagnostic playback.

### Recorded results: September 23, 2026

All 22 Rust tests and 37 TypeScript tests passed. TypeScript checking and whitespace checks passed. Development and packaged Linux x64 desktop suites passed, including water/bed picking and fully flooded 0×/1×/10× display. Flat and globe screenshots were reviewed. Packaging succeeded; the existing renderer-chunk advisory above 500 kB remains.

The default level-5 `first-light` fingerprint remains `5408377f`; model arrays remain 5,527,300 bytes. The native model, protocol, and recipe schema did not change.

On the Ryzen 5 PRO 4650U laptop with 14.84 GiB RAM and hardware-enabled WebGL/compositing, the Land and water layer at 10× applied exaggeration measured:

| Regions | Generation plus both view preparations | Flat RAF p95 | Globe RAF p95 |
| ---: | ---: | ---: | ---: |
| 10,242 | 1,126 ms | 16.7 ms | 16.7 ms |
| 40,962 | 4,297 ms | 16.8 ms | 16.7 ms |

No simultaneous project builds or tests ran during the samples. Each sample covers 180 requested-animation-frame intervals under alternating zoom and native diagnostic diffusion. This is not a GPU timer or a future climate-performance guarantee. Generation timing includes desktop interaction and both view preparations. The increased geometry carries a real preparation/memory cost; timings across B4/B5 are single observations, not a controlled speed comparison. The local raw report is `artifacts/native-desktop-benchmark.json`, replaced by subsequent runs.

## Next step

Milestone C starts with drainage structure and constructed depression/spill experiments, not climate-derived rivers. Resolved-state/image export also remains outstanding in milestone B. This display change neither advances those systems nor changes water stocks.
